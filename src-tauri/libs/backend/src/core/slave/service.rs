//! 从站服务实现

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};

use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    core::{
        slave::{
            InitializationProfile, SimulationProfile,
            connection::listener_lifecycle::SlaveListenerRuntime,
            data::point_store::SlavePointStore,
            protocol::asdu_dispatch::RequestContext,
            reporting::{
                broadcast_runtime::SlaveBroadcastRuntime, soe_support::SoeEvent, spontaneous::SpontaneousCoalescer,
            },
            simulation::{engine::SimulationRuntime, runtime::SlaveSimulationTaskRuntime},
        },
        types::{
            BackendSoeEvent, ConnectionInfo, LinkParams, SlaveMessageRecord, SlaveStationPolicy,
            SlaveStationPolicyOverride, StationConfig, StationStatus,
        },
    },
    db::DatabaseService,
    network::{Iec104Protocol, TcpServer},
    utils::pcap::CapturePacket,
};

mod lifecycle;
mod points;
mod policy;
mod request_support;
mod telemetry;

pub(crate) use request_support::{
    build_setpoint_command_payload, decode_slave_command_events, enqueue_response_from_ctx,
    first_ioa_from_decoded_asdu, send_response_from_ctx,
};

/// 传输原因：突发/自发（用于数据点主动上报）
const COT_SPONTANEOUS: u8 = 3;
/// IEC104 链路最大重传尝试次数
const IEC104_LINK_MAX_RETRANSMIT_ATTEMPTS: u32 = 3;

/// 帧计数错误恢复策略
///
/// 当检测到帧计数错误（V(R) 与 N(S) 不匹配）时的恢复策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameCountRecoveryStrategy {
    /// TCP 重连策略：断开连接并等待主站重连
    TcpReconnect,
    /// STOPDT/STARTDT 策略：发送 STOPDT 和 STARTDT 重置帧计数
    StopDtStartDt,
}

impl FrameCountRecoveryStrategy {
    /// 从环境变量读取恢复策略
    ///
    /// 环境变量 `IEC104_FRAME_COUNT_RECOVERY` 可选值：
    /// - `"stopdt"` / `"stopdt_startdt"` / `"stopdtstartdt"` → StopDtStartDt
    /// - 其他或未设置 → TcpReconnect（默认）
    fn from_env() -> Self {
        let raw = std::env::var("IEC104_FRAME_COUNT_RECOVERY").unwrap_or_default().trim().to_ascii_lowercase();
        match raw.as_str() {
            "stopdt" | "stopdt_startdt" | "stopdtstartdt" => Self::StopDtStartDt,
            _ => Self::TcpReconnect,
        }
    }
}

/// 从站命令不匹配策略
///
/// 定义当接收到的控制命令与数据点类型不匹配时的处理策略。
///
/// # 策略说明
///
/// - **Strict**：严格模式，拒绝类型不匹配的命令
/// - **Compatible**：兼容模式，尝试类型转换
/// - **Debug**：调试模式，记录详细日志但不拒绝
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlaveCommandMismatchPolicy {
    /// 严格模式：拒绝类型不匹配的命令
    Strict,
    /// 兼容模式：尝试类型转换
    Compatible,
    /// 调试模式：记录日志但不拒绝
    Debug,
}

impl Default for SlaveCommandMismatchPolicy {
    fn default() -> Self {
        Self::Strict
    }
}

/// 从站服务
///
/// IEC104 从站的核心服务实现，管理从站的完整生命周期。
struct TransportRuntime {
    status: Arc<RwLock<StationStatus>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    server: Arc<RwLock<Option<Arc<TcpServer>>>>,
    protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    link_params: Arc<RwLock<LinkParams>>,
    server_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    station_time_offset_ms: Arc<RwLock<i64>>,
}

struct PointRuntimeStore {
    data_points: Arc<SlavePointStore>,
    initialization_profile: Arc<RwLock<InitializationProfile>>,
}

struct TelemetryBuffers {
    message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    message_tracking_enabled: Arc<AtomicBool>,
    message_tracking_guard: Arc<Mutex<()>>,
    next_message_id: Arc<AtomicU64>,
    spontaneous_coalescer: Arc<Mutex<SpontaneousCoalescer>>,
    soe_events: Arc<RwLock<VecDeque<SoeEvent>>>,
    soe_records: Arc<RwLock<VecDeque<BackendSoeEvent>>>,
    next_soe_id: Arc<AtomicU64>,
}

struct PolicyStore {
    station_policy_defaults: Arc<RwLock<SlaveStationPolicy>>,
    station_policy_overrides: Arc<RwLock<HashMap<u16, SlaveStationPolicyOverride>>>,
    station_policy_connection_override_enabled: Arc<RwLock<bool>>,
    command_mismatch_policy: Arc<RwLock<SlaveCommandMismatchPolicy>>,
}

struct SimulationState {
    simulation_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    simulation_profile: Arc<RwLock<SimulationProfile>>,
    simulation_seed: Arc<Mutex<u64>>,
    simulation_runtime: Arc<Mutex<SimulationRuntime>>,
}

struct ServiceContext {
    db_service: Arc<DatabaseService>,
    runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
}

/// IEC104 从站的核心服务实现，管理从站的完整生命周期。
pub struct SlaveService {
    config: StationConfig,
    transport: TransportRuntime,
    points: PointRuntimeStore,
    telemetry: TelemetryBuffers,
    policy: PolicyStore,
    simulation: SimulationState,
    context: ServiceContext,
}

impl SlaveService {
    /// 创建监听运行时
    ///
    /// 构建用于监听任务的运行时上下文，包含所有必要的共享状态。
    fn listener_runtime(&self) -> SlaveListenerRuntime {
        SlaveListenerRuntime {
            status: Arc::clone(&self.transport.status),
            connections: Arc::clone(&self.transport.connections),
            peer_local_addrs: Arc::clone(&self.transport.peer_local_addrs),
            message_records: Arc::clone(&self.telemetry.message_records),
            capture_packets: Arc::clone(&self.telemetry.capture_packets),
            message_tracking_enabled: Arc::clone(&self.telemetry.message_tracking_enabled),
            message_tracking_guard: Arc::clone(&self.telemetry.message_tracking_guard),
            next_message_id: Arc::clone(&self.telemetry.next_message_id),
            server: Arc::clone(&self.transport.server),
            protocols: Arc::clone(&self.transport.protocols),
            link_params: Arc::clone(&self.transport.link_params),
            server_task: Arc::clone(&self.transport.server_task),
            spontaneous_coalescer: Arc::clone(&self.telemetry.spontaneous_coalescer),
            soe_events: Arc::clone(&self.telemetry.soe_events),
            station_time_offset_ms: Arc::clone(&self.transport.station_time_offset_ms),
            runtime_event_handle: Arc::clone(&self.context.runtime_event_handle),
        }
    }

    /// 创建模拟任务运行时
    ///
    /// 构建用于数据模拟任务的运行时上下文，包含模拟引擎所需的共享状态。
    ///
    /// # 返回
    ///
    /// 返回 SlaveSimulationTaskRuntime 实例，包含以下字段：
    /// - `station_id`：从站 ID
    /// - `default_common_address`：默认公共地址
    /// - `data_points`：数据点缓存（用于更新模拟值）
    /// - `simulation_task`：模拟任务句柄
    /// - `simulation_profile`：模拟配置
    /// - `simulation_seed`：随机种子
    /// - `simulation_runtime`：模拟运行时状态
    /// - `runtime_event_handle`：Tauri 应用句柄
    fn simulation_task_runtime(&self) -> SlaveSimulationTaskRuntime {
        SlaveSimulationTaskRuntime {
            station_id: self.config.id.clone(),
            default_common_address: self.config.common_address,
            data_points: Arc::clone(&self.points.data_points),
            simulation_task: Arc::clone(&self.simulation.simulation_task),
            status: Arc::clone(&self.transport.status),
            simulation_profile: Arc::clone(&self.simulation.simulation_profile),
            simulation_seed: Arc::clone(&self.simulation.simulation_seed),
            simulation_runtime: Arc::clone(&self.simulation.simulation_runtime),
            runtime_event_handle: Arc::clone(&self.context.runtime_event_handle),
        }
    }

    /// 创建广播运行时
    ///
    /// 构建用于数据点广播的运行时上下文，包含广播所需的所有共享状态。
    /// 广播运行时用于将数据点变化主动上报给所有已连接的主站。
    ///
    /// # 返回
    ///
    /// 返回 SlaveBroadcastRuntime 实例，包含以下字段：
    /// - `station_id`：从站 ID
    /// - `default_common_address`：默认公共地址
    /// - `server`：TCP 服务器实例（用于发送数据）
    /// - `protocols`：协议实例映射表
    /// - `connections`：连接信息映射表
    /// - `peer_local_addrs`：对端到本地地址映射
    /// - `message_records`：消息记录队列
    /// - `next_message_id`：下一个消息 ID
    /// - `capture_packets`：PCAP 抓包队列
    /// - `link_params`：链路参数
    /// - `station_policy_defaults`：从站策略默认值
    /// - `station_policy_overrides`：从站策略覆盖
    /// - `station_policy_connection_override_enabled`：是否启用连接级策略覆盖
    /// - `spontaneous_coalescer`：突发上报合并器
    /// - `soe_events`：SOE 事件队列
    /// - `soe_records`：SOE 事件记录
    /// - `station_time_offset_ms`：从站时间偏移
    /// - `runtime_event_handle`：Tauri 应用句柄
    /// - `next_soe_id`：下一个 SOE 事件 ID
    fn broadcast_runtime(&self) -> SlaveBroadcastRuntime {
        SlaveBroadcastRuntime {
            station_id: self.config.id.clone(),
            default_common_address: self.config.common_address,
            server: Arc::clone(&self.transport.server),
            protocols: Arc::clone(&self.transport.protocols),
            connections: Arc::clone(&self.transport.connections),
            peer_local_addrs: Arc::clone(&self.transport.peer_local_addrs),
            message_records: Arc::clone(&self.telemetry.message_records),
            next_message_id: Arc::clone(&self.telemetry.next_message_id),
            capture_packets: Arc::clone(&self.telemetry.capture_packets),
            link_params: Arc::clone(&self.transport.link_params),
            station_policy_defaults: Arc::clone(&self.policy.station_policy_defaults),
            station_policy_overrides: Arc::clone(&self.policy.station_policy_overrides),
            station_policy_connection_override_enabled: Arc::clone(
                &self.policy.station_policy_connection_override_enabled,
            ),
            spontaneous_coalescer: Arc::clone(&self.telemetry.spontaneous_coalescer),
            soe_events: Arc::clone(&self.telemetry.soe_events),
            soe_records: Arc::clone(&self.telemetry.soe_records),
            station_time_offset_ms: Arc::clone(&self.transport.station_time_offset_ms),
            runtime_event_handle: Arc::clone(&self.context.runtime_event_handle),
            next_soe_id: Arc::clone(&self.telemetry.next_soe_id),
        }
    }

    /// 创建从站服务实例
    ///
    /// 根据提供的配置创建一个新的从站服务实例。
    /// 所有共享状态都使用 Arc 包装，支持多线程访问。
    ///
    /// # 参数
    ///
    /// * `config` - 从站配置（ID、名称、地址、公共地址等）
    ///
    /// # 返回
    ///
    /// 返回初始化完成的 SlaveService 实例，初始状态为 Stopped。
    ///
    /// # 初始状态
    ///
    /// - 状态：Stopped
    /// - 连接列表：空
    /// - 数据点列表：空
    /// - 消息跟踪：禁用
    /// - 模拟任务：未启动
    /// - 链路参数：默认值
    pub fn new(
        config: StationConfig,
        db_service: Arc<DatabaseService>,
        runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    ) -> Self {
        let simulation_profile = SimulationProfile::default();
        Self {
            config,
            transport: TransportRuntime {
                status: Arc::new(RwLock::new(StationStatus::Stopped)),
                connections: Arc::new(RwLock::new(HashMap::new())),
                peer_local_addrs: Arc::new(RwLock::new(HashMap::new())),
                server: Arc::new(RwLock::new(None)),
                protocols: Arc::new(RwLock::new(HashMap::new())),
                link_params: Arc::new(RwLock::new(LinkParams::default())),
                server_task: Arc::new(Mutex::new(None)),
                station_time_offset_ms: Arc::new(RwLock::new(0)),
            },
            points: PointRuntimeStore {
                data_points: Arc::new(SlavePointStore::new()),
                initialization_profile: Arc::new(RwLock::new(InitializationProfile::default())),
            },
            telemetry: TelemetryBuffers {
                message_records: Arc::new(RwLock::new(VecDeque::new())),
                capture_packets: Arc::new(RwLock::new(VecDeque::new())),
                message_tracking_enabled: Arc::new(AtomicBool::new(false)),
                message_tracking_guard: Arc::new(Mutex::new(())),
                next_message_id: Arc::new(AtomicU64::new(0)),
                spontaneous_coalescer: Arc::new(Mutex::new(SpontaneousCoalescer::default())),
                soe_events: Arc::new(RwLock::new(VecDeque::new())),
                soe_records: Arc::new(RwLock::new(VecDeque::new())),
                next_soe_id: Arc::new(AtomicU64::new(0)),
            },
            policy: PolicyStore {
                station_policy_defaults: Arc::new(RwLock::new(SlaveStationPolicy::default())),
                station_policy_overrides: Arc::new(RwLock::new(HashMap::new())),
                station_policy_connection_override_enabled: Arc::new(RwLock::new(false)),
                command_mismatch_policy: Arc::new(RwLock::new(SlaveCommandMismatchPolicy::default())),
            },
            simulation: SimulationState {
                simulation_task: Arc::new(Mutex::new(None)),
                simulation_profile: Arc::new(RwLock::new(simulation_profile.clone())),
                simulation_seed: Arc::new(Mutex::new(simulation_profile.seed)),
                simulation_runtime: Arc::new(Mutex::new(SimulationRuntime::default())),
            },
            context: ServiceContext { db_service, runtime_event_handle },
        }
    }
}
