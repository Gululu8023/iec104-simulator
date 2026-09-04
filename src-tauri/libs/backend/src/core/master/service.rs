//! 主站服务实现

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use log::{info, warn};
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    core::{
        iec104_registry::iec104_type_name,
        master::{
            CommandDispatchReceipt, Iec104Command, QualifierMode,
            command::{
                dispatch::{MasterCommandDispatchRuntime, dispatch_command_via_runtime},
                store::PendingCommand,
            },
            connection::{
                lifecycle::{MasterLifecycleRuntime, disconnect_connection},
                transport_runtime::{MasterConnectionTransportRuntime, connect_to_slave_via_transport},
            },
            data::{
                ingest::MasterIngestRuntime,
                point_scope::{MasterPointScope, data_point_key},
                point_store::MasterPointStore,
            },
            file_transfer::{
                MasterFileTransferRuntime, cancel_master_file_transfer, get_master_file_transfer_session,
                start_master_file_transfer, store::MasterFileTransferSessionRuntime,
            },
            message_store::{
                append_master_message, register_master_message_tracking, reset_master_message_tracking_state,
            },
        },
        shared::{capture_export::export_capture_packets, runtime_hint::emit_master_runtime_hint},
        types::{
            BackendSoeEvent, ConnectionInfo, DataPoint, DataTransferState, LinkParams, MasterFileTransferMode,
            MasterFileTransferSession, MasterMessageRecord, MessageDirection, PointDef, StationConfig, StationStatus,
            TransportState,
        },
    },
    db::DatabaseService,
    errors::{AppError, AppResult, Iec104ExecutionError, NetworkError},
    network::{Iec104Protocol, TcpClient},
    utils::{
        bytes_to_hex,
        pcap::{CaptureFormat, CapturePacket},
    },
};

/// 主站文件传输请求
///
/// 封装了 IEC104 文件传输所需的所有参数，支持多种传输模式：
/// - 目录查询（Directory Query）
/// - 文件读取（File Read）
/// - 文件写入（File Write）
/// - 透明文件传输（Transparent File）
#[derive(Debug, Clone)]
pub struct MasterFileTransferRequest {
    /// 连接标识符（运行时 UUID）
    pub connection_id: String,
    /// 从站 ID（用于数据库关联）
    pub slave_id: i64,
    /// 公共地址（ASDU 公共地址）
    pub common_address: u16,
    /// 文件传输模式
    pub mode: MasterFileTransferMode,
    /// 信息对象地址（IOA，文件标识）
    pub ioa: u32,
    /// 文件名称或编号（Name of File）
    pub nof: Option<u16>,
    /// 本地文件路径（用于文件读写）
    pub local_path: Option<String>,
    /// 查询范围起始时间（CP56Time2a 格式，7 字节）
    pub query_range_start_time: [u8; 7],
    /// 查询范围结束时间（CP56Time2a 格式，7 字节）
    pub query_range_end_time: [u8; 7],
    /// 单个 ASDU 最大字节数（用于分段传输）
    pub max_asdu_bytes: u16,
}

/// 主站服务
///
/// IEC 60870-5-104 协议主站的核心服务实现,负责管理与多个从站的连接、
/// 数据采集、命令下发、文件传输等功能。
pub struct MasterService {
    /// 站点配置（ID、名称、公共地址等）
    config: StationConfig,

    /// 当前状态（Stopped/Starting/Running/Stopping）
    status: Arc<RwLock<StationStatus>>,

    /// 连接列表（key: connection_id, value: 连接信息）
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,

    /// 正在建立的连接目标，用于原子化重复连接准入。
    connecting_targets: Arc<Mutex<Vec<(std::net::SocketAddr, Option<i64>)>>>,

    /// 数据点缓存
    data_points: Arc<MasterPointStore>,

    /// 持久化点表派生的禁用点/禁用从站准入索引
    point_admission: Arc<RwLock<crate::core::master::data::point_scope::MasterPointAdmission>>,

    /// 通信报文缓存（环形队列,用于前端实时展示）
    message_records: Arc<RwLock<VecDeque<MasterMessageRecord>>>,

    /// SOE 事件缓存（环形队列,独立于通信报文,仅记录带时标的变化事件）
    soe_records: Arc<RwLock<VecDeque<BackendSoeEvent>>>,

    /// 捕获包缓存（用于导出 PCAP/PCAPNG 格式）
    capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,

    /// 通信报文追踪开关（原子布尔值,控制是否记录报文）
    message_tracking_enabled: Arc<AtomicBool>,

    /// 通信报文追踪重置串行锁（防止并发重置导致状态不一致）
    message_tracking_guard: Arc<Mutex<()>>,

    /// 待完成命令队列（用于跟踪命令的激活确认/否定响应）
    pending_commands: Arc<RwLock<VecDeque<PendingCommand>>>,

    /// 报文 ID 生成器（原子递增,用于唯一标识每条报文）
    next_message_id: Arc<AtomicU64>,

    /// SOE 事件 ID 生成器（原子递增,用于唯一标识每个 SOE 事件）
    next_soe_id: Arc<AtomicU64>,

    /// TCP 客户端映射（key: connection_id, value: TCP 客户端实例）
    clients: Arc<RwLock<HashMap<String, Arc<TcpClient>>>>,

    /// IEC104 协议处理器映射（key: connection_id, value: 协议编解码器）
    protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,

    /// 连接事件监控任务（每个连接一个后台任务,负责接收和处理数据）
    connection_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,

    /// 运行时事件发布器
    runtime_event_handle: Arc<crate::services::RuntimeEventSink>,

    /// 数据库服务（用于持久化 I-frame 数据点到 SQLite）
    db_service: Arc<DatabaseService>,

    /// 运行时连接 UUID → profile_id 映射（用于关联配置文件）
    connection_profile_map: Arc<RwLock<HashMap<String, i64>>>,

    /// 文件传输会话（key: connection_id, value: 会话运行时,每连接最多一个）
    file_transfer_sessions: Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
}

impl MasterService {
    /// 构造生命周期运行时
    ///
    /// 生命周期运行时负责管理连接的创建、断开和监控任务。
    /// 包含连接状态、客户端实例、协议处理器等核心资源的引用。
    pub(crate) fn lifecycle_runtime(&self) -> MasterLifecycleRuntime {
        MasterLifecycleRuntime {
            status: Arc::clone(&self.status),
            connections: Arc::clone(&self.connections),
            connecting_targets: Arc::clone(&self.connecting_targets),
            message_records: Arc::clone(&self.message_records),
            pending_commands: Arc::clone(&self.pending_commands),
            next_message_id: Arc::clone(&self.next_message_id),
            clients: Arc::clone(&self.clients),
            protocols: Arc::clone(&self.protocols),
            connection_tasks: Arc::clone(&self.connection_tasks),
            runtime_event_handle: Arc::clone(&self.runtime_event_handle),
            connection_profile_map: Arc::clone(&self.connection_profile_map),
            file_transfer_sessions: Arc::clone(&self.file_transfer_sessions),
        }
    }

    /// 构造数据摄取运行时
    fn ingest_runtime(&self) -> MasterIngestRuntime {
        MasterIngestRuntime {
            data_points: Arc::clone(&self.data_points),
            point_admission: Arc::clone(&self.point_admission),
            message_records: Arc::clone(&self.message_records),
            soe_records: Arc::clone(&self.soe_records),
            pending_commands: Arc::clone(&self.pending_commands),
            next_message_id: Arc::clone(&self.next_message_id),
            next_soe_id: Arc::clone(&self.next_soe_id),
            runtime_event_handle: Arc::clone(&self.runtime_event_handle),
            file_transfer_sessions: Arc::clone(&self.file_transfer_sessions),
            db_service: Arc::clone(&self.db_service),
        }
    }

    /// 构造传输运行时
    pub(crate) fn transport_runtime(&self) -> MasterConnectionTransportRuntime {
        MasterConnectionTransportRuntime {
            connections: Arc::clone(&self.connections),
            capture_packets: Arc::clone(&self.capture_packets),
            ingest_runtime: self.ingest_runtime(),
        }
    }

    /// 构造命令分发运行时
    pub(crate) fn command_dispatch_runtime(&self) -> MasterCommandDispatchRuntime {
        MasterCommandDispatchRuntime {
            station_id: self.config.id.clone(),
            connections: Arc::clone(&self.connections),
            data_points: Arc::clone(&self.data_points),
            point_admission: Arc::clone(&self.point_admission),
            message_records: Arc::clone(&self.message_records),
            pending_commands: Arc::clone(&self.pending_commands),
            next_message_id: Arc::clone(&self.next_message_id),
            clients: Arc::clone(&self.clients),
            protocols: Arc::clone(&self.protocols),
            runtime_event_handle: Arc::clone(&self.runtime_event_handle),
            connection_profile_map: Arc::clone(&self.connection_profile_map),
        }
    }

    /// 获取站点 ID
    pub(crate) fn station_id(&self) -> String {
        self.config.id.clone()
    }

    /// 获取站点名称
    pub(crate) fn station_name(&self) -> String {
        self.config.name.clone()
    }

    /// 构造文件传输运行时
    fn file_transfer_runtime(&self) -> MasterFileTransferRuntime {
        MasterFileTransferRuntime {
            station_id: self.config.id.clone(),
            sessions: Arc::clone(&self.file_transfer_sessions),
            clients: Arc::clone(&self.clients),
            protocols: Arc::clone(&self.protocols),
            connections: Arc::clone(&self.connections),
            message_records: Arc::clone(&self.message_records),
            next_message_id: Arc::clone(&self.next_message_id),
            runtime_event_handle: Arc::clone(&self.runtime_event_handle),
        }
    }

    /// 创建新的主站服务
    ///
    /// 初始化所有内部状态和缓存,注册报文追踪机制。
    /// 创建后的服务处于 Stopped 状态,需要调用 `start()` 启动。
    ///
    /// # 参数
    ///
    /// * `config` - 站点配置,包含 ID、名称、公共地址等信息
    ///
    /// # 返回
    ///
    /// 返回新创建的主站服务实例
    pub fn new(
        config: StationConfig,
        db_service: Arc<DatabaseService>,
        runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    ) -> Self {
        let message_records = Arc::new(RwLock::new(VecDeque::new()));
        let capture_packets = Arc::new(RwLock::new(VecDeque::new()));
        let message_tracking_enabled = Arc::new(AtomicBool::new(false));
        let message_tracking_guard = Arc::new(Mutex::new(()));
        register_master_message_tracking(
            &message_records,
            &capture_packets,
            &message_tracking_enabled,
            &message_tracking_guard,
        );

        Self {
            config,
            status: Arc::new(RwLock::new(StationStatus::Stopped)),
            connections: Arc::new(RwLock::new(HashMap::new())),
            connecting_targets: Arc::new(Mutex::new(Vec::new())),
            data_points: Arc::new(MasterPointStore::new()),
            point_admission: Arc::new(RwLock::new(Default::default())),
            message_records,
            soe_records: Arc::new(RwLock::new(VecDeque::new())),
            capture_packets,
            message_tracking_enabled,
            message_tracking_guard,
            pending_commands: Arc::new(RwLock::new(VecDeque::new())),
            next_message_id: Arc::new(AtomicU64::new(0)),
            next_soe_id: Arc::new(AtomicU64::new(0)),
            clients: Arc::new(RwLock::new(HashMap::new())),
            protocols: Arc::new(RwLock::new(HashMap::new())),
            connection_tasks: Arc::new(Mutex::new(HashMap::new())),
            runtime_event_handle,
            db_service,
            connection_profile_map: Arc::new(RwLock::new(HashMap::new())),
            file_transfer_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置通信报文追踪开关
    pub async fn set_message_tracking_enabled(&self, enabled: bool) {
        reset_master_message_tracking_state(
            &self.message_tracking_guard,
            &self.message_tracking_enabled,
            enabled,
            &self.message_records,
            &self.capture_packets,
            &self.next_message_id,
        )
        .await;
    }

    /// 启动主站
    pub async fn start(&self) -> AppResult<()> {
        {
            let mut status = self.status.write().await;
            if *status != StationStatus::Stopped {
                warn!("主站 {} 已经在运行或正在启动", self.config.name);
                return Ok(());
            }

            info!("启动主站: {}", self.config.name);
            *status = StationStatus::Starting;
        }

        // 启动时重置易失态缓存，避免残留状态干扰命令跟踪。
        self.pending_commands.write().await.clear();
        self.file_transfer_sessions.write().await.clear();
        self.message_records.write().await.clear();
        self.capture_packets.write().await.clear();
        self.next_message_id.store(0, Ordering::Relaxed);

        *self.status.write().await = StationStatus::Running;
        info!("主站 {} 启动成功", self.config.name);
        emit_master_runtime_hint(&self.runtime_event_handle, &self.config.id, None, "station-started").await;
        Ok(())
    }

    /// 停止主站
    pub async fn stop(&self) -> AppResult<()> {
        {
            let mut status = self.status.write().await;
            if *status == StationStatus::Stopped {
                warn!("主站 {} 已经停止", self.config.name);
                return Ok(());
            }

            info!("停止主站: {}", self.config.name);
            *status = StationStatus::Stopping;
        }

        // 断开所有连接
        let connection_ids: Vec<String> = self.clients.read().await.keys().cloned().collect();
        for connection_id in connection_ids {
            if let Err(e) = self.disconnect(&connection_id).await {
                warn!("断开连接 {} 失败: {}", connection_id, e);
            }
        }
        self.file_transfer_sessions.write().await.clear();

        *self.status.write().await = StationStatus::Stopped;
        info!("主站 {} 停止成功", self.config.name);
        emit_master_runtime_hint(&self.runtime_event_handle, &self.config.id, None, "station-stopped").await;
        Ok(())
    }

    /// 获取当前状态
    pub async fn get_status(&self) -> StationStatus {
        *self.status.read().await
    }

    /// 获取站点配置
    pub fn get_config(&self) -> &StationConfig {
        &self.config
    }

    /// 获取主站公共地址
    pub fn common_address(&self) -> u16 {
        self.config.common_address
    }

    /// 绑定前端事件推送句柄
    /// 获取所有连接
    pub async fn get_connections(&self) -> Vec<ConnectionInfo> {
        self.connections.read().await.values().cloned().collect()
    }

    /// 获取所有数据点
    pub(crate) async fn sync_data_points(
        &self,
        cursor: Option<crate::core::types::PointSyncCursor>,
    ) -> crate::core::shared::versioned_point_store::PointStoreSync<
        crate::core::master::data::point_scope::MasterPointKey,
        DataPoint,
    > {
        self.data_points.sync_since(cursor).await
    }

    pub(crate) async fn point_cursor(&self) -> crate::core::types::PointSyncCursor {
        self.data_points.cursor().await
    }

    /// 将持久化的点表定义预加载到内存
    pub async fn preload_point_defs(
        &self,
        slave_id: i64,
        link_profile_id: Option<i64>,
        common_address: Option<u16>,
        defs: &[PointDef],
        scope_enabled: bool,
    ) {
        let now = chrono::Utc::now();
        let scope = MasterPointScope::ConfiguredSlave(slave_id);
        let common_address_value = common_address.unwrap_or(0);
        let mut admission = self.point_admission.write().await;
        let mut dp = self.data_points.transaction().await;
        let previous_keys = admission.replace_scope(&scope, common_address_value, defs, scope_enabled);
        for key in previous_keys {
            dp.remove(&key);
        }
        dp.remove_where(|key, point| {
            key.scope == scope && admission.is_disabled(&scope, point.common_address.unwrap_or(0), point.address)
        });
        if !scope_enabled {
            return;
        }
        for def in defs {
            if !def.is_enabled {
                continue;
            }
            let key = data_point_key(&scope, common_address_value, def.address);
            dp.entry(key).or_insert_with(|| {
                let value = def
                    .default_value
                    .as_ref()
                    .and_then(|v| match v {
                        serde_json::Value::Number(n) => n.as_f64(),
                        serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                        serde_json::Value::String(s) => s.parse::<f64>().ok(),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                DataPoint {
                    connection_id: String::new(),
                    link_profile_id,
                    slave_id: Some(slave_id),
                    common_address,
                    address: def.address,
                    name: def.name.clone(),
                    description: def.description.clone(),
                    control_ioa: def.control_ioa,
                    gi_group: def.gi_group,
                    counter_group: def.counter_group,
                    type_id: def.type_id,
                    data_type: iec104_type_name(def.type_id).to_string(),
                    value,
                    quality: 0,
                    quality_common: Some(crate::core::types::DataPointQualityCommon::default()),
                    quality_detail: Some(crate::core::types::DataPointQualityDetail::None),
                    business_usable: true,
                    timestamp: now,
                    latest_event_timestamp: None,
                    timestamp_detail: None,
                    report_count: None,
                    latest_cause: None,
                    point_source: Some(def.source),
                    control_status_snapshot: None,
                }
            });
        }
    }

    pub async fn clear_point_scope(&self, slave_id: i64) {
        let scope = MasterPointScope::ConfiguredSlave(slave_id);
        let mut admission = self.point_admission.write().await;
        let mut points = self.data_points.transaction().await;
        for key in admission.clear_scope(&scope) {
            points.remove(&key);
        }
    }

    /// 获取主站通信报文（支持按ID增量拉取）
    pub async fn get_messages(&self, since_id: Option<u64>, limit: usize) -> Vec<MasterMessageRecord> {
        let bounded_limit = limit.clamp(1, 1000);
        let messages = self.message_records.read().await;
        messages
            .iter()
            .filter(|item| since_id.map(|id| item.id > id).unwrap_or(true))
            .take(bounded_limit)
            .cloned()
            .collect()
    }

    /// 获取 SOE 事件（支持按ID增量拉取）
    pub async fn get_soe_events(&self, since_id: Option<u64>, limit: usize) -> Vec<BackendSoeEvent> {
        let bounded_limit = limit.clamp(1, 1000);
        let records = self.soe_records.read().await;
        records
            .iter()
            .filter(|item| since_id.map(|id| item.id > id).unwrap_or(true))
            .take(bounded_limit)
            .cloned()
            .collect()
    }

    /// 清空 SOE 事件缓存
    pub async fn clear_soe_events(&self) {
        self.soe_records.write().await.clear();
    }

    /// 导出捕获包
    pub async fn export_capture(&self, connection_id: Option<&str>, format: CaptureFormat) -> AppResult<Vec<u8>> {
        Ok(export_capture_packets(&self.capture_packets, connection_id, format).await)
    }

    /// 连接到从站
    pub async fn connect_to_slave(
        &self,
        slave_addr: std::net::SocketAddr,
        profile_id: Option<i64>,
        link_params: Option<LinkParams>,
        auto_start_data_transfer: bool,
        auto_gi: bool,
        auto_reconnect: bool,
        retry_count: u32,
        retry_interval_seconds: u32,
    ) -> AppResult<String> {
        connect_to_slave_via_transport(
            self,
            slave_addr,
            profile_id,
            link_params,
            auto_start_data_transfer,
            auto_gi,
            auto_reconnect,
            retry_count,
            retry_interval_seconds,
        )
        .await
    }

    /// 断开连接
    pub async fn disconnect(&self, connection_id: &str) -> AppResult<()> {
        disconnect_connection(&self.lifecycle_runtime(), &self.config.id, &self.config.name, connection_id).await
    }

    /// 启动文件传输
    pub async fn start_file_transfer(
        &self,
        request: MasterFileTransferRequest,
    ) -> AppResult<MasterFileTransferSession> {
        start_master_file_transfer(&self.file_transfer_runtime(), request).await
    }

    /// 获取文件传输会话
    pub async fn get_file_transfer_session(&self, connection_id: &str) -> Option<MasterFileTransferSession> {
        get_master_file_transfer_session(&self.file_transfer_runtime(), connection_id).await
    }

    /// 取消文件传输
    pub async fn cancel_file_transfer(&self, connection_id: &str) -> AppResult<MasterFileTransferSession> {
        cancel_master_file_transfer(&self.file_transfer_runtime(), connection_id).await
    }

    /// 发送测试帧
    pub async fn send_test_frame(&self, connection_id: &str) -> AppResult<()> {
        info!("主站 {} 发送 TESTFR_ACT 到连接: {}", self.config.name, connection_id);

        let client = self
            .clients
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or(AppError::Network(NetworkError::NotConnected))?;
        let protocol = self
            .protocols
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or(AppError::Network(NetworkError::NotConnected))?;

        let frame = protocol.lock().await.generate_test_act()?;
        client.send(&frame).await?;

        if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
            conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
            conn.tx_bytes = conn.tx_bytes.saturating_add(frame.len() as u64);
        }

        append_master_message(
            &self.message_records,
            &self.next_message_id,
            MessageDirection::Sent,
            connection_id,
            "发送 TESTFR_ACT".to_string(),
            Some(bytes_to_hex(&frame)),
            None,
            None,
            None,
        )
        .await;
        emit_master_runtime_hint(&self.runtime_event_handle, &self.config.id, Some(connection_id), "test-frame-sent")
            .await;
        Ok(())
    }

    /// 手动启动数据传输
    pub async fn start_data_transfer(&self, connection_id: &str) -> AppResult<()> {
        info!("主站 {} 手动启动数据传输链路: {}", self.config.name, connection_id);

        let client = self
            .clients
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or(AppError::Network(NetworkError::NotConnected))?;
        let protocol = self
            .protocols
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or(AppError::Network(NetworkError::NotConnected))?;

        {
            let mut guard = self.connections.write().await;
            let Some(conn) = guard.get_mut(connection_id) else {
                return Err(AppError::Network(NetworkError::NotConnected));
            };
            if conn.transport_state != TransportState::Connected {
                return Err(AppError::Network(NetworkError::NotConnected));
            }
            if matches!(conn.data_transfer_state, DataTransferState::Started | DataTransferState::Starting) {
                return Ok(());
            }
        }

        let frame = {
            let mut guard = protocol.lock().await;
            guard.begin_data_transfer()?
        };

        if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
            conn.data_transfer_state = DataTransferState::Starting;
        }

        if let Err(err) = client.send(&frame).await {
            if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
                conn.data_transfer_state = DataTransferState::Error;
            }
            return Err(err.into());
        }

        if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
            conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
            conn.tx_bytes = conn.tx_bytes.saturating_add(frame.len() as u64);
        }

        append_master_message(
            &self.message_records,
            &self.next_message_id,
            MessageDirection::Sent,
            connection_id,
            "发送 STARTDT_ACT (manual)".to_string(),
            Some(bytes_to_hex(&frame)),
            None,
            None,
            None,
        )
        .await;
        emit_master_runtime_hint(&self.runtime_event_handle, &self.config.id, Some(connection_id), "manual-startdt")
            .await;

        Ok(())
    }

    /// 手动停止数据传输
    pub async fn stop_data_transfer(&self, connection_id: &str) -> AppResult<()> {
        info!("主站 {} 手动停止数据传输链路: {}", self.config.name, connection_id);

        let client = self
            .clients
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or(AppError::Network(NetworkError::NotConnected))?;
        let protocol = self
            .protocols
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or(AppError::Network(NetworkError::NotConnected))?;

        {
            let mut guard = self.connections.write().await;
            let Some(conn) = guard.get_mut(connection_id) else {
                return Err(AppError::Network(NetworkError::NotConnected));
            };
            if conn.transport_state != TransportState::Connected {
                return Err(AppError::Network(NetworkError::NotConnected));
            }
            if matches!(conn.data_transfer_state, DataTransferState::Stopped | DataTransferState::Stopping) {
                return Ok(());
            }
        }

        let frame = {
            let mut guard = protocol.lock().await;
            guard.begin_stop_data_transfer()?
        };

        if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
            conn.data_transfer_state = DataTransferState::Stopping;
        }

        if let Err(err) = client.send(&frame).await {
            if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
                conn.data_transfer_state = DataTransferState::Error;
            }
            return Err(err.into());
        }

        if let Some(conn) = self.connections.write().await.get_mut(connection_id) {
            conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
            conn.tx_bytes = conn.tx_bytes.saturating_add(frame.len() as u64);
        }

        append_master_message(
            &self.message_records,
            &self.next_message_id,
            MessageDirection::Sent,
            connection_id,
            "发送 STOPDT_ACT (manual)".to_string(),
            Some(bytes_to_hex(&frame)),
            None,
            None,
            None,
        )
        .await;
        emit_master_runtime_hint(&self.runtime_event_handle, &self.config.id, Some(connection_id), "manual-stopdt")
            .await;

        Ok(())
    }

    /// 统一命令分发入口
    pub async fn dispatch_command(
        &self,
        connection_id: &str,
        slave_id: i64,
        common_address: u16,
        command: Iec104Command,
    ) -> Result<CommandDispatchReceipt, Iec104ExecutionError> {
        self.dispatch_command_with_qualifier_mode(
            connection_id,
            slave_id,
            common_address,
            command,
            QualifierMode::Standard,
        )
        .await
    }

    pub async fn dispatch_command_with_qualifier_mode(
        &self,
        connection_id: &str,
        slave_id: i64,
        common_address: u16,
        command: Iec104Command,
        qualifier_mode: QualifierMode,
    ) -> Result<CommandDispatchReceipt, Iec104ExecutionError> {
        dispatch_command_via_runtime(
            &self.command_dispatch_runtime(),
            connection_id,
            slave_id,
            common_address,
            command,
            qualifier_mode,
        )
        .await
    }
}
