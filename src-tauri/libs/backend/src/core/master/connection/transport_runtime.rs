//! 传输运行时管理
//!
//! 本模块负责主站连接的 TCP 数据传输和协议帧处理,是连接运行时的核心组件。
//!
//! # 核心功能
//!
//! - **TCP 数据收发**：处理 TCP 连接的数据接收和发送
//! - **协议帧解析**：解析 IEC104 协议帧（I-frame、U-frame、S-frame）
//! - **数据摄取**：将接收到的 ASDU 数据传递给数据摄取运行时处理
//! - **超时检测**：定期检测命令超时和链路超时（t1/t2/t3）
//! - **自动总召**：连接建立后可选自动发送总召唤命令
//! - **帧计数恢复**：处理帧序号不匹配的恢复策略
//! - **报文捕获**：记录所有收发的报文到捕获缓存（用于 PCAP 导出）
//!
//! # 后台监控任务
//!
//! 每个连接都有一个后台监控任务（`connection_monitor_task`）,负责：
//! - 接收 TCP 数据并解析协议帧
//! - 定期检测命令超时（每秒一次）
//! - 定期检测链路超时（t1/t2/t3）
//! - 处理连接断开和自动重连
//!
//! # 帧计数恢复策略
//!
//! 当检测到帧序号不匹配时,支持两种恢复策略：
//! - **TcpReconnect**（默认）：断开并重新建立 TCP 连接
//! - **StopDtStartDt**：发送 STOPDT 然后 STARTDT 重置序号
//!
//! 可通过环境变量 `IEC104_FRAME_COUNT_RECOVERY` 配置。

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use chrono::Utc;
use log::{debug, error, info, warn};
use tokio::{
    sync::{Mutex, RwLock, oneshot},
    task::JoinHandle,
    time::{Duration, Instant, MissedTickBehavior, interval},
};

use super::{
    super::{
        command::{store::remove_pending_commands_for_connection, timeout::expire_pending_commands},
        commands::{Iec104Command, InterrogationQualifier, MasterCommands},
        data::ingest::{MasterIngestRuntime, handle_received_iframe},
        message_store::{append_capture_packet, append_master_message},
        service::MasterService,
    },
    lifecycle::{
        MasterLifecycleRuntime, PreparedMasterConnection, cleanup_connection_task, prepare_connection,
        register_monitor_task, release_connection_target, reserve_connection_target,
    },
};
use crate::{
    core::{
        protocol_adapter::encode_asdu,
        shared::runtime_hint::{emit_master_runtime_hint, emit_master_runtime_hint_with_attempts},
        types::{ConnectionInfo, DataTransferState, LinkParams, MessageDirection, TransportState},
    },
    errors::{AppResult, Iec104ErrorCode},
    network::{Iec104Protocol, NetworkEvent, ProtocolMessage, TcpClient, UFrameType},
    utils::{HEX_PREVIEW_MAX_BYTES, bytes_to_hex, bytes_to_hex_preview, logger::iec104_log_kv, pcap::CapturePacket},
};

/// 待完成命令超时阈值（秒）
///
/// 命令发送后如果在此时间内未收到响应,则视为超时。
const PENDING_COMMAND_TIMEOUT_SECONDS: i64 = 15;

/// IEC104 链路最大重传尝试次数
///
/// 当发送失败时,最多重试此次数后放弃。
const IEC104_LINK_MAX_RETRANSMIT_ATTEMPTS: u32 = 3;

/// 帧计数恢复策略
///
/// 当检测到帧序号不匹配时的恢复策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameCountRecoveryStrategy {
    /// TCP 重连策略：断开并重新建立 TCP 连接
    TcpReconnect,
    /// STOPDT/STARTDT 策略：发送 STOPDT 然后 STARTDT 重置序号
    StopDtStartDt,
}

impl FrameCountRecoveryStrategy {
    /// 从环境变量读取恢复策略
    ///
    /// 环境变量 `IEC104_FRAME_COUNT_RECOVERY` 可选值：
    /// - `"stopdt"` / `"stopdt_startdt"` / `"stopdtstartdt"` → StopDtStartDt
    /// - 其他或未设置 → TcpReconnect（默认）
    pub(crate) fn from_env() -> Self {
        let raw = std::env::var("IEC104_FRAME_COUNT_RECOVERY").unwrap_or_default().trim().to_ascii_lowercase();
        match raw.as_str() {
            "stopdt" | "stopdt_startdt" | "stopdtstartdt" => Self::StopDtStartDt,
            _ => Self::TcpReconnect,
        }
    }
}

/// 主站连接传输运行时
///
/// 封装了传输层所需的共享状态和资源引用。
#[derive(Clone)]
pub(crate) struct MasterConnectionTransportRuntime {
    /// 连接列表（key: connection_id, value: 连接信息）
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    /// 捕获包缓存（用于导出 PCAP/PCAPNG 格式）
    pub(crate) capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    /// 数据摄取运行时（用于处理接收到的 ASDU 数据）
    pub(crate) ingest_runtime: MasterIngestRuntime,
}

/// 主站连接传输任务参数
///
/// 封装了后台监控任务所需的所有参数。
pub(crate) struct MasterConnectionTransportTaskArgs {
    /// 生命周期运行时
    pub(crate) lifecycle: MasterLifecycleRuntime,
    /// 连接 ID
    pub(crate) connection_id: String,
    /// 主站 ID
    pub(crate) station_id: String,
    /// 公共地址
    pub(crate) common_address: u16,
    /// 配置文件 ID（可选）
    pub(crate) profile_id: Option<i64>,
    /// 链路参数（t0/t1/t2/t3/k/w 等）
    pub(crate) link_params: LinkParams,
    /// 是否自动启动数据传输
    pub(crate) auto_start_data_transfer: bool,
    /// 是否自动发送总召唤
    pub(crate) auto_gi: bool,
    /// 是否启用自动重连
    pub(crate) auto_reconnect: bool,
    /// TCP 客户端实例
    pub(crate) client: Arc<TcpClient>,
    /// IEC104 协议处理器
    pub(crate) protocol: Arc<Mutex<Iec104Protocol>>,
    /// 帧计数恢复策略
    pub(crate) frame_count_recovery: FrameCountRecoveryStrategy,
}

async fn recover_protocol_connection(
    lifecycle: &MasterLifecycleRuntime,
    connection_id: &str,
    client: &TcpClient,
    protocol: &Mutex<Iec104Protocol>,
    link_params: LinkParams,
    auto_start_data_transfer: bool,
    auto_reconnect: bool,
) -> bool {
    if !auto_reconnect || !client.reconnect_enabled() {
        let _ = client.disconnect().await;
        return false;
    }

    remove_pending_commands_for_connection(&lifecycle.pending_commands, connection_id).await;
    {
        let mut protocol = protocol.lock().await;
        *protocol = Iec104Protocol::new();
        protocol.set_max_unconfirmed(link_params.k_value);
        protocol.set_max_asdu_len(link_params.max_asdu_bytes as usize);
    }
    if let Some(connection) = lifecycle.connections.write().await.get_mut(connection_id) {
        connection.transport_state = TransportState::Connecting;
        connection.data_transfer_state =
            if auto_start_data_transfer { DataTransferState::Starting } else { DataTransferState::Stopped };
        connection.reconnecting = true;
        connection.reconnect_attempts = 0;
    }
    let _ = client.request_reconnect().await;
    true
}

/// 通过传输运行时连接到从站
///
/// 这是主站连接到从站的主入口函数,负责准备连接、启动后台监控任务。
///
/// # 参数
///
/// * `service` - 主站服务实例
/// * `slave_addr` - 从站地址（IP:Port）
/// * `profile_id` - 可选的配置文件 ID
/// * `link_params` - 可选的链路参数
/// * `auto_start_data_transfer` - 是否自动发送 STARTDT
/// * `auto_gi` - 是否自动发送总召唤
/// * `auto_reconnect` - 是否启用自动重连
/// * `retry_count` - 重连重试次数
/// * `retry_interval_seconds` - 重连间隔（秒）
///
/// # 返回
///
/// - `Ok(String)` - 返回连接 ID
/// - `Err(AppError)` - 连接失败
///
/// # 执行流程
///
/// 1. 准备连接（建立 TCP 连接,初始化协议）
/// 2. 构建传输任务参数
/// 3. 启动后台监控任务（`connection_monitor_task`）
/// 4. 注册任务句柄到生命周期运行时
/// 5. 返回连接 ID
pub(crate) async fn connect_to_slave_via_transport(
    service: &MasterService,
    slave_addr: std::net::SocketAddr,
    profile_id: Option<i64>,
    link_params: Option<LinkParams>,
    auto_start_data_transfer: bool,
    auto_gi: bool,
    auto_reconnect: bool,
    retry_count: u32,
    retry_interval_seconds: u32,
) -> AppResult<String> {
    let lifecycle = service.lifecycle_runtime();
    let station_id = service.station_id();
    let station_name = service.station_name();
    reserve_connection_target(&lifecycle, slave_addr, profile_id).await?;
    let prepared = prepare_connection(
        &lifecycle,
        &station_id,
        &station_name,
        slave_addr,
        profile_id,
        link_params,
        auto_start_data_transfer,
        auto_reconnect,
        retry_count,
        retry_interval_seconds,
    )
    .await;
    release_connection_target(&lifecycle, slave_addr, profile_id).await;
    let PreparedMasterConnection { connection_id, client, protocol, link_params } = prepared?;

    let transport_runtime = service.transport_runtime();
    let (task_handle, start_gate) =
        spawn_connection_transport_runtime(transport_runtime, MasterConnectionTransportTaskArgs {
            lifecycle: lifecycle.clone(),
            connection_id: connection_id.clone(),
            station_id,
            common_address: service.common_address(),
            profile_id,
            link_params,
            auto_start_data_transfer,
            auto_gi,
            auto_reconnect,
            client,
            protocol,
            frame_count_recovery: FrameCountRecoveryStrategy::from_env(),
        })
        .await;

    register_monitor_task(&lifecycle, connection_id.clone(), task_handle).await;
    let _ = start_gate.send(());

    Ok(connection_id)
}

pub(crate) async fn spawn_connection_transport_runtime(
    runtime: MasterConnectionTransportRuntime,
    args: MasterConnectionTransportTaskArgs,
) -> (JoinHandle<()>, oneshot::Sender<()>) {
    let MasterConnectionTransportRuntime { connections, capture_packets, ingest_runtime } = runtime;
    let MasterConnectionTransportTaskArgs {
        lifecycle,
        connection_id,
        station_id,
        common_address,
        profile_id,
        link_params,
        auto_start_data_transfer,
        auto_gi,
        auto_reconnect,
        client,
        protocol,
        frame_count_recovery,
    } = args;

    let mut event_receiver =
        client.get_event_receiver().await.expect("master connection event receiver must be taken exactly once");
    let data_points = Arc::clone(&ingest_runtime.data_points);
    let point_admission = Arc::clone(&ingest_runtime.point_admission);
    let message_records = Arc::clone(&ingest_runtime.message_records);
    let pending_commands = Arc::clone(&ingest_runtime.pending_commands);
    let next_message_id = Arc::clone(&ingest_runtime.next_message_id);
    let runtime_event_handle = Arc::clone(&ingest_runtime.runtime_event_handle);
    let client_for_task = client.clone();
    let protocol_for_task = protocol.clone();
    let task_lifecycle = lifecycle.clone();
    let connection_id_for_task = connection_id.clone();
    let station_id_for_task = station_id.clone();
    let common_address_for_task = common_address;
    let link_params_for_task = link_params;
    let profile_id_for_task = profile_id;
    let db_service_for_task = ingest_runtime.db_service.clone();
    let (start_gate, start_receiver) = oneshot::channel();
    let task_handle = tokio::spawn(async move {
        if start_receiver.await.is_err() {
            return;
        }
        let mut handshake_started_at = Instant::now();
        let mut last_activity = Instant::now();
        let mut pending_test: Option<Instant> = None;
        let mut last_data_transfer_state =
            if auto_start_data_transfer { DataTransferState::Starting } else { DataTransferState::Stopped };
        let mut unacked_since: Option<Instant> = None;
        let mut retransmit_attempts: u32 = 0;
        let mut last_peer_ack_sequence: u16 = 0;
        let mut pending_ack_count: u16 = 0;
        let mut pending_ack_since: Option<Instant> = None;
        let mut frame_count_recovery_attempted = false;
        let mut local_socket: Option<std::net::SocketAddr> = None;
        let mut last_rx_hint_at = Instant::now();
        let mut connected_event_seen = false;
        let mut last_send_activity = client_for_task.send_activity();
        let task_ingest_runtime = ingest_runtime.clone();

        let t0_seconds = link_params_for_task.t0_seconds.max(1);
        let t1_seconds = link_params_for_task.t1_seconds.max(1);
        let t2_seconds = link_params_for_task.t2_seconds.max(1);
        let t3_seconds = link_params_for_task.t3_seconds.max(1);
        let w_value = link_params_for_task.w_value.max(1);

        let mut keepalive = interval(Duration::from_secs(1));
        keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);

        'monitor: loop {
            tokio::select! {
                    maybe_event = event_receiver.recv() => {
                        let Some(event) = maybe_event else {
                            break;
                        };

                        match event {
                    NetworkEvent::Connected { peer_addr, local_addr } => {
                        last_activity = Instant::now();
                        debug!(
                            "[master] connected {} peer_addr={}",
                            iec104_log_kv(
                                &station_id_for_task,
                                Some(&connection_id_for_task),
                                None,
                                None,
                                None,
                                None,
                                None,
                            ),
                            peer_addr
                        );
                        local_socket = local_addr.parse::<std::net::SocketAddr>().ok();
                        let is_reconnected = connected_event_seen;
                        connected_event_seen = true;
                        let restart_frame = if is_reconnected {
                            let mut protocol = protocol_for_task.lock().await;
                            *protocol = Iec104Protocol::new();
                            protocol.set_max_unconfirmed(link_params_for_task.k_value);
                            protocol.set_max_asdu_len(link_params_for_task.max_asdu_bytes as usize);
                            protocol.set_state(crate::network::ProtocolState::Connected);
                            if auto_start_data_transfer {
                                protocol.begin_data_transfer().ok()
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            conn.transport_state = TransportState::Connected;
                            conn.data_transfer_state = if is_reconnected && auto_start_data_transfer {
                                DataTransferState::Starting
                            } else if is_reconnected {
                                DataTransferState::Stopped
                            } else {
                                conn.data_transfer_state
                            };
                            conn.reconnecting = false;
                            conn.reconnect_attempts = 0;
                            if is_reconnected {
                                conn.connected_at = Some(Utc::now());
                            }
                        }
                        if is_reconnected {
                            pending_test = None;
                            pending_ack_count = 0;
                            pending_ack_since = None;
                            unacked_since = None;
                            retransmit_attempts = 0;
                            last_peer_ack_sequence = 0;
                            frame_count_recovery_attempted = false;
                            handshake_started_at = Instant::now();
                            if let Some(frame) = restart_frame.as_ref() {
                                let frame_len = frame.len() as u64;
                                if client_for_task.send(frame).await.is_ok() {
                                    if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                        conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
                                        conn.tx_bytes = conn.tx_bytes.saturating_add(frame_len);
                                    }
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Sent,
                                        &connection_id_for_task,
                                        format!("TCP重连已恢复，重新发送 STARTDT_ACT 至 {}", peer_addr),
                                        Some(bytes_to_hex(frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                } else if recover_protocol_connection(
                                    &task_lifecycle,
                                    &connection_id_for_task,
                                    &client_for_task,
                                    &protocol_for_task,
                                    link_params_for_task,
                                    auto_start_data_transfer,
                                    auto_reconnect,
                                )
                                .await
                                {
                                    continue 'monitor;
                                } else {
                                    break 'monitor;
                                }
                            } else {
                                append_master_message(
                                    &message_records,
                                    &next_message_id,
                                    MessageDirection::Received,
                                    &connection_id_for_task,
                                    format!("TCP重连已恢复，等待手动 STARTDT: {}", peer_addr),
                                    None,
                                    None,
                                    None,
                                    None,
                                )
                                .await;
                            }
                        }
                        emit_master_runtime_hint(
                            &runtime_event_handle,
                            &station_id_for_task,
                            Some(&connection_id_for_task),
                            "connected",
                        )
                        .await;
                    }
                    NetworkEvent::Disconnected { peer_addr, reason } => {
                        info!(
                            "[master] disconnected station_id={} connection_id={} peer_addr={} reason={}",
                            station_id_for_task,
                            connection_id_for_task,
                            peer_addr,
                            reason
                        );
                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            if auto_reconnect && client_for_task.reconnect_enabled() {
                                conn.transport_state = TransportState::Connecting;
                                conn.data_transfer_state = if auto_start_data_transfer {
                                    DataTransferState::Starting
                                } else {
                                    DataTransferState::Stopped
                                };
                                conn.reconnecting = true;
                                conn.reconnect_attempts = 0;
                            } else {
                                conn.transport_state = TransportState::Disconnected;
                                conn.data_transfer_state = DataTransferState::Stopped;
                                conn.reconnecting = false;
                                conn.reconnect_attempts = 0;
                            }
                        }
                        protocol_for_task
                            .lock()
                            .await
                            .set_state(crate::network::ProtocolState::Disconnected);
                        pending_test = None;
                        pending_ack_count = 0;
                        pending_ack_since = None;
                        unacked_since = None;
                        retransmit_attempts = 0;
                        last_peer_ack_sequence = 0;
                        frame_count_recovery_attempted = false;
                        local_socket = None;
                        append_master_message(
                            &message_records,
                            &next_message_id,
                            MessageDirection::Error,
                            &connection_id_for_task,
                            format!("连接断开: {} ({})", peer_addr, reason),
                            None,
                            None,
                            None,
                            None,
                        )
                        .await;
                        emit_master_runtime_hint(
                            &runtime_event_handle,
                            &station_id_for_task,
                            Some(&connection_id_for_task),
                            "disconnected",
                        )
                        .await;
                        remove_pending_commands_for_connection(&pending_commands, &connection_id_for_task).await;
                        if !auto_reconnect || !client_for_task.reconnect_enabled() {
                            break;
                        }
                        continue;
                    }
                    NetworkEvent::ReconnectAttemptFailed {
                        addr,
                        attempt,
                        max_attempts,
                        error,
                    } => {
                        last_activity = Instant::now();
                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            conn.transport_state = TransportState::Connecting;
                            conn.data_transfer_state = if auto_start_data_transfer {
                                DataTransferState::Starting
                            } else {
                                DataTransferState::Stopped
                            };
                            conn.reconnecting = true;
                            conn.reconnect_attempts = attempt;
                            conn.max_reconnect_attempts = max_attempts;
                        }
                        append_master_message(
                            &message_records,
                            &next_message_id,
                            MessageDirection::Error,
                            &connection_id_for_task,
                            format!("自动重连失败 ({}/{}): {} [{}]", attempt, max_attempts, error, addr),
                            None,
                            None,
                            None,
                            None,
                        )
                        .await;
                        emit_master_runtime_hint_with_attempts(
                            &runtime_event_handle,
                            &station_id_for_task,
                            Some(&connection_id_for_task),
                            "reconnect-attempt-failed",
                            Some(attempt),
                            Some(max_attempts),
                        )
                        .await;
                    }
                    NetworkEvent::ReconnectExhausted {
                        addr,
                        attempts,
                        max_attempts,
                        last_error,
                    } => {
                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            conn.transport_state = TransportState::Error;
                            conn.data_transfer_state = DataTransferState::Error;
                            conn.reconnecting = false;
                            conn.reconnect_attempts = attempts;
                            conn.max_reconnect_attempts = max_attempts;
                        }
                        append_master_message(
                            &message_records,
                            &next_message_id,
                            MessageDirection::Error,
                            &connection_id_for_task,
                            format!(
                                "自动重连已终止 ({}/{}): {} [{}]",
                                attempts, max_attempts, last_error, addr
                            ),
                            None,
                            None,
                            None,
                            None,
                        )
                        .await;
                        emit_master_runtime_hint_with_attempts(
                            &runtime_event_handle,
                            &station_id_for_task,
                            Some(&connection_id_for_task),
                            "reconnect-exhausted",
                            Some(attempts),
                            Some(max_attempts),
                        )
                        .await;
                        break 'monitor;
                    }
                    NetworkEvent::Error { peer_addr, error } => {
                        last_activity = Instant::now();
                        error!(
                            "[master] network_error {} peer_addr={:?} error={}",
                            iec104_log_kv(
                                &station_id_for_task,
                                Some(&connection_id_for_task),
                                None,
                                None,
                                None,
                                Some("NETWORK_ERROR"),
                                None,
                            ),
                            peer_addr,
                            error
                        );
                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            conn.transport_state = TransportState::Error;
                            conn.data_transfer_state = DataTransferState::Error;
                        }
                        append_master_message(
                            &message_records,
                            &next_message_id,
                            MessageDirection::Error,
                            &connection_id_for_task,
                            format!("网络错误 peer={:?}: {}", peer_addr, error),
                            None,
                            None,
                            None,
                            None,
                        )
                        .await;
                        emit_master_runtime_hint(
                            &runtime_event_handle,
                            &station_id_for_task,
                            Some(&connection_id_for_task),
                            "network-error",
                        )
                        .await;
                    }
                    NetworkEvent::DataReceived { peer_addr, data } => {
                        last_activity = Instant::now();
                        pending_test = None;
                        expire_pending_commands(
                            &pending_commands,
                            &data_points,
                            &point_admission,
                            &message_records,
                            &next_message_id,
                            &connection_id_for_task,
                            PENDING_COMMAND_TIMEOUT_SECONDS,
                        )
                        .await;

                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            conn.rx_bytes = conn.rx_bytes.saturating_add(data.len() as u64);
                        }


                        if let (Some(local), Ok(remote)) = (local_socket, peer_addr.parse::<std::net::SocketAddr>()) {
                            append_capture_packet(
                                &capture_packets,
                                CapturePacket {
                                    timestamp: Utc::now(),
                                    connection_id: connection_id_for_task.clone(),
                                    src: remote,
                                    dst: local,
                                    payload: data.clone(),
                                },
                            )
                            .await;
                        }

                        let messages = {
                            let mut protocol = protocol_for_task.lock().await;
                            match protocol.handle_received_data(&data) {
                                Ok(msgs) => msgs,
                                Err(e) => {
                                    warn!(
                                        "[master] protocol_parse_error {} peer_addr={} error={}",
                                        iec104_log_kv(
                                            &station_id_for_task,
                                            Some(&connection_id_for_task),
                                            None,
                                            None,
                                            None,
                                            Some("PROTOCOL_PARSE_FAILED"),
                                            None,
                                        ),
                                        peer_addr,
                                        e
                                    );
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Error,
                                        &connection_id_for_task,
                                        if data.len() > HEX_PREVIEW_MAX_BYTES {
                                            format!("协议解析失败(原始hex预览{}B): {}", HEX_PREVIEW_MAX_BYTES, e)
                                        } else {
                                            format!("协议解析失败: {}", e)
                                        },
                                        Some(bytes_to_hex_preview(&data)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    emit_master_runtime_hint(
                                        &runtime_event_handle,
                                        &station_id_for_task,
                                        Some(&connection_id_for_task),
                                        "protocol-parse-error",
                                    )
                                    .await;
                                    continue;
                                }
                            }
                        };

                        if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                            conn.rx_apdu_count =
                                conn.rx_apdu_count.saturating_add(messages.len() as u64);
                        }

                        let (peer_ack_sequence, unconfirmed_count) = {
                            let protocol = protocol_for_task.lock().await;
                            (protocol.ack_sequence(), protocol.unconfirmed_count())
                        };
                        if peer_ack_sequence != last_peer_ack_sequence {
                            last_peer_ack_sequence = peer_ack_sequence;
                            retransmit_attempts = 0;
                            unacked_since = if unconfirmed_count > 0 {
                                Some(Instant::now())
                            } else {
                                None
                            };
                        } else if unconfirmed_count == 0 {
                            retransmit_attempts = 0;
                            unacked_since = None;
                        }

                        let mut iframe_count_in_batch: u16 = 0;
                        let mut frame_count_error: Option<(u16, u16)> = None;

                        for message in messages {
                            match message {
                                ProtocolMessage::FrameCountError {
                                    expected_receive_sequence,
                                    got_send_sequence,
                                } => {
                                    frame_count_error = Some((expected_receive_sequence, got_send_sequence));
                                    break;
                                }
                                ProtocolMessage::AckRequired {
                                    expected_receive_sequence,
                                    got_send_sequence,
                                } => {
                                    debug!(
                                        "[master] duplicate iframe dropped: expected_vr={} got_ns={}",
                                        expected_receive_sequence,
                                        got_send_sequence
                                    );

                                    let ack = protocol_for_task.lock().await.generate_ack();
                                    if let Ok(ack) = ack {
                                        let ack_len = ack.len() as u64;
                                        if client_for_task.send(&ack).await.is_ok() {
                                            if let Some(conn) =
                                                connections.write().await.get_mut(&connection_id_for_task)
                                            {
                                                conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
                                                conn.tx_bytes = conn.tx_bytes.saturating_add(ack_len);
                                            }
                                            append_master_message(
                                                &message_records,
                                                &next_message_id,
                                                MessageDirection::Sent,
                                                &connection_id_for_task,
                                                format!(
                                                    "发送 S 帧确认(duplicate) V(R)={} (got N(S)={})",
                                                    expected_receive_sequence, got_send_sequence
                                                ),
                                                Some(bytes_to_hex(&ack)),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;

                                            pending_ack_count = 0;
                                            pending_ack_since = None;
                                        }
                                    }
                                }
                                ProtocolMessage::UFrame { u_type: UFrameType::StartDtCon, raw_frame } => {
                                    if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                        conn.transport_state = TransportState::Connected;
                                        conn.data_transfer_state = DataTransferState::Started;
                                    }
                                    pending_test = None;
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        "收到 STARTDT_CON".to_string(),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;

                                    // 自动总召：STARTDT_CON 后按从站逐个 GI（一条链路多 COA）
                                    if auto_gi {
                                        let mut gi_common_addresses = Vec::<u16>::new();
                                        if let Some(profile_id) = profile_id_for_task {
                                            let db = db_service_for_task.as_ref();
                                            if let Ok(slaves) =
                                                db.master_slaves.get_by_connection(profile_id)
                                            {
                                                for slave in slaves {
                                                    if slave.enabled {
                                                        gi_common_addresses
                                                            .push(slave.common_address);
                                                    }
                                                }
                                            }
                                        }
                                        if gi_common_addresses.is_empty() {
                                            gi_common_addresses.push(common_address_for_task);
                                        }
                                        gi_common_addresses.sort_unstable();
                                        gi_common_addresses.dedup();

                                        for common_address in gi_common_addresses {
                                            let gi_cmd = Iec104Command::GeneralInterrogation {
                                                qoi: InterrogationQualifier::Station,
                                            };
                                            match MasterCommands::build_asdu(common_address, &gi_cmd) {
                                                Ok(asdu) => {
                                                    let asdu_bytes = encode_asdu(&asdu);
                                                    let gi_frame = protocol_for_task.lock().await.send_asdu(&asdu_bytes);
                                                    match gi_frame {
                                                        Ok(frame) => {
                                                            let frame_len = frame.len() as u64;
                                                            if client_for_task.send(&frame).await.is_ok() {
                                                                if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                                                    conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
                                                                    conn.tx_bytes = conn.tx_bytes.saturating_add(frame_len);
                                                                }
                                                                append_master_message(
                                                                    &message_records,
                                                                    &next_message_id,
                                                                    MessageDirection::Sent,
                                                                    &connection_id_for_task,
                                                                    format!("自动总召: 发送 C_IC_NA_1 (CA={})", asdu.common_address),
                                                                    Some(bytes_to_hex(&frame)),
                                                                    Some(asdu.type_id),
                                                                    Some(asdu.cause as u16),
                                                                    Some(asdu.common_address),
                                                                )
                                                                .await;
                                                                info!(
                                                                    "[master] auto-gi sent {} ca={}",
                                                                    iec104_log_kv(
                                                                        &station_id_for_task,
                                                                        Some(&connection_id_for_task),
                                                                        Some(asdu.type_id),
                                                                        Some(asdu.cause as u16),
                                                                        None,
                                                                        None,
                                                                        None,
                                                                    ),
                                                                    asdu.common_address
                                                                );
                                                            }
                                                        }
                                                        Err(err) => {
                                                            warn!("[master] auto-gi frame build failed: {}", err);
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    warn!("[master] auto-gi asdu build failed: {}", err.message);
                                                }
                                            }
                                        }
                                    }
                                }
                                ProtocolMessage::UFrame { u_type: UFrameType::StartDtAct, raw_frame } => {
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        "收到 STARTDT_ACT".to_string(),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    let frame = protocol_for_task.lock().await.generate_startdt_con();
                                    if let Ok(frame) = frame {
                                        let sent_len = frame.len() as u64;
                                        if client_for_task.send(&frame).await.is_ok() {
                                            if let Some(conn) = connections
                                                .write()
                                                .await
                                                .get_mut(&connection_id_for_task)
                                            {
                                                conn.tx_apdu_count =
                                                    conn.tx_apdu_count.saturating_add(1);
                                                conn.tx_bytes = conn.tx_bytes.saturating_add(sent_len);
                                            }
                                            append_master_message(
                                                &message_records,
                                                &next_message_id,
                                                MessageDirection::Sent,
                                                &connection_id_for_task,
                                                "发送 STARTDT_CON".to_string(),
                                                Some(bytes_to_hex(&frame)),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                        }
                                    }
                                    if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                        conn.transport_state = TransportState::Connected;
                                        conn.data_transfer_state = DataTransferState::Started;
                                    }
                                }
                                ProtocolMessage::UFrame { u_type: UFrameType::StopDtAct, raw_frame } => {
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        "收到 STOPDT_ACT".to_string(),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    let frame = protocol_for_task.lock().await.generate_stopdt_con();
                                    if let Ok(frame) = frame {
                                        let sent_len = frame.len() as u64;
                                        if client_for_task.send(&frame).await.is_ok() {
                                            if let Some(conn) = connections
                                                .write()
                                                .await
                                                .get_mut(&connection_id_for_task)
                                            {
                                                conn.tx_apdu_count =
                                                    conn.tx_apdu_count.saturating_add(1);
                                                conn.tx_bytes = conn.tx_bytes.saturating_add(sent_len);
                                            }
                                            append_master_message(
                                                &message_records,
                                                &next_message_id,
                                                MessageDirection::Sent,
                                                &connection_id_for_task,
                                                "发送 STOPDT_CON".to_string(),
                                                Some(bytes_to_hex(&frame)),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                        }
                                    }
                                    if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                        conn.data_transfer_state = DataTransferState::Stopped;
                                    }
                                }
                                ProtocolMessage::UFrame { u_type: UFrameType::TestFrAct, raw_frame } => {
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        "收到 TESTFR_ACT".to_string(),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    let frame = protocol_for_task.lock().await.generate_test_response();
                                    if let Ok(frame) = frame {
                                        let sent_len = frame.len() as u64;
                                        if client_for_task.send(&frame).await.is_ok() {
                                            if let Some(conn) = connections
                                                .write()
                                                .await
                                                .get_mut(&connection_id_for_task)
                                            {
                                                conn.tx_apdu_count =
                                                    conn.tx_apdu_count.saturating_add(1);
                                                conn.tx_bytes = conn.tx_bytes.saturating_add(sent_len);
                                            }
                                            append_master_message(
                                                &message_records,
                                                &next_message_id,
                                                MessageDirection::Sent,
                                                &connection_id_for_task,
                                                "发送 TESTFR_CON".to_string(),
                                                Some(bytes_to_hex(&frame)),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                        }
                                    }
                                }
                                ProtocolMessage::UFrame { u_type: UFrameType::TestFrCon, raw_frame } => {
                                    pending_test = None;
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        "收到 TESTFR_CON".to_string(),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    emit_master_runtime_hint(
                                        &runtime_event_handle,
                                        &station_id_for_task,
                                        Some(&connection_id_for_task),
                                        "test-frame-confirmed",
                                    )
                                    .await;
                                }
                                ProtocolMessage::UFrame { u_type: UFrameType::StopDtCon, raw_frame } => {
                                    if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                        conn.data_transfer_state = DataTransferState::Stopped;
                                    }
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        "收到 STOPDT_CON".to_string(),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                }
                                ProtocolMessage::IgnoredFrame { frame_type, reason } => {
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Error,
                                        &connection_id_for_task,
                                        format!("忽略 {} 帧: {}", frame_type, reason),
                                        None,
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                }
                                ProtocolMessage::IFrame { decoded_asdu, raw_frame, .. } => {
                                    iframe_count_in_batch = iframe_count_in_batch.saturating_add(1);
                                    handle_received_iframe(
                                        &task_ingest_runtime,
                                        &station_id_for_task,
                                        &connection_id_for_task,
                                        profile_id_for_task,
                                        &decoded_asdu,
                                        &raw_frame,
                                    )
                                    .await;
                                }
                                ProtocolMessage::IFrameDecodeError {
                                    send_sequence,
                                    receive_sequence,
                                    error,
                                    raw_frame,
                                } => {
                                    iframe_count_in_batch = iframe_count_in_batch.saturating_add(1);
                                    warn!(
                                        "[master] ASDU decode failed after accepting I-frame station_id={} connection_id={} ns={} nr={} error={}",
                                        station_id_for_task,
                                        connection_id_for_task,
                                        send_sequence,
                                        receive_sequence,
                                        error
                                    );
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Error,
                                        &connection_id_for_task,
                                        format!("I 帧已接收，但 ASDU 解码失败: {}", error),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                }
                                ProtocolMessage::SFrame { receive_sequence, raw_frame } => {
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Received,
                                        &connection_id_for_task,
                                        format!("收到 S 帧确认 seq={}", receive_sequence),
                                        Some(bytes_to_hex(&raw_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                }
                            }
                        }

                        // 节流式通知前端刷新（250ms 最小间隔，对标从站 rx-data hint）
                        if iframe_count_in_batch > 0 && last_rx_hint_at.elapsed() >= Duration::from_millis(250) {
                            last_rx_hint_at = Instant::now();
                            emit_master_runtime_hint(
                                &runtime_event_handle,
                                &station_id_for_task,
                                Some(&connection_id_for_task),
                                "rx-data",
                            )
                            .await;
                        }

                        if let Some((expected_receive_sequence, got_send_sequence)) = frame_count_error {
                            let error_text = format!(
                                "Frame count error: expected V(R)={}, got N(S)={}",
                                expected_receive_sequence, got_send_sequence
                            );
                            warn!(
                                "[master] {} station_id={} connection_id={}",
                                error_text, station_id_for_task, connection_id_for_task
                            );
                            append_master_message(
                                &message_records,
                                &next_message_id,
                                MessageDirection::Error,
                                &connection_id_for_task,
                                error_text,
                                None,
                                None,
                                None,
                                None,
                            )
                            .await;
                            emit_master_runtime_hint(
                                &runtime_event_handle,
                                &station_id_for_task,
                                Some(&connection_id_for_task),
                                "frame-count-error",
                            )
                            .await;
                            if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                conn.data_transfer_state = DataTransferState::Error;
                            }

                            match frame_count_recovery {
                                FrameCountRecoveryStrategy::StopDtStartDt
                                    if !frame_count_recovery_attempted =>
                                {
                                    frame_count_recovery_attempted = true;
                                    info!(
                                        "[master] frame count recovery: STOPDT/STARTDT station_id={} connection_id={}",
                                        station_id_for_task, connection_id_for_task
                                    );

                                    let stop_frame = {
                                        let mut protocol = protocol_for_task.lock().await;
                                        protocol.begin_stop_data_transfer()
                                    };
                                    let Ok(stop_frame) = stop_frame else {
                                        if recover_protocol_connection(
                                            &task_lifecycle,
                                            &connection_id_for_task,
                                            &client_for_task,
                                            &protocol_for_task,
                                            link_params_for_task,
                                            auto_start_data_transfer,
                                            auto_reconnect,
                                        )
                                        .await
                                        {
                                            continue 'monitor;
                                        }
                                        break 'monitor;
                                    };
                                    let sent_len = stop_frame.len() as u64;
                                    if client_for_task.send(&stop_frame).await.is_err() {
                                        if recover_protocol_connection(
                                            &task_lifecycle,
                                            &connection_id_for_task,
                                            &client_for_task,
                                            &protocol_for_task,
                                            link_params_for_task,
                                            auto_start_data_transfer,
                                            auto_reconnect,
                                        )
                                        .await
                                        {
                                            continue 'monitor;
                                        }
                                        break 'monitor;
                                    }
                                    if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                        conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
                                        conn.tx_bytes = conn.tx_bytes.saturating_add(sent_len);
                                    }
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Sent,
                                        &connection_id_for_task,
                                        "发送 STOPDT_ACT (frame-count-recovery)".to_string(),
                                        Some(bytes_to_hex(&stop_frame)),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;

                                    let start_frame = {
                                        let mut protocol = protocol_for_task.lock().await;
                                        protocol.begin_data_transfer()
                                    };
                                    let Ok(start_frame) = start_frame else {
                                        if recover_protocol_connection(
                                            &task_lifecycle,
                                            &connection_id_for_task,
                                            &client_for_task,
                                            &protocol_for_task,
                                            link_params_for_task,
                                            auto_start_data_transfer,
                                            auto_reconnect,
                                        )
                                        .await
                                        {
                                            pending_test = None;
                                            pending_ack_count = 0;
                                            pending_ack_since = None;
                                            unacked_since = None;
                                            retransmit_attempts = 0;
                                            continue 'monitor;
                                        }
                                        break 'monitor;
                                    };

                                    handshake_started_at = Instant::now();
                                    pending_test = None;
                                    unacked_since = None;
                                    retransmit_attempts = 0;
                                    last_peer_ack_sequence = 0;
                                    pending_ack_count = 0;
                                    pending_ack_since = None;

                                    if let Some(conn) =
                                        connections.write().await.get_mut(&connection_id_for_task)
                                    {
                                        conn.data_transfer_state = DataTransferState::Starting;
                                    }

                                    let sent_len = start_frame.len() as u64;
                                    if client_for_task.send(&start_frame).await.is_ok() {
                                        if let Some(conn) =
                                            connections.write().await.get_mut(&connection_id_for_task)
                                        {
                                            conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
                                            conn.tx_bytes = conn.tx_bytes.saturating_add(sent_len);
                                        }
                                        append_master_message(
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Sent,
                                            &connection_id_for_task,
                                            "发送 STARTDT_ACT (frame-count-recovery)".to_string(),
                                            Some(bytes_to_hex(&start_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                    } else if recover_protocol_connection(
                                        &task_lifecycle,
                                        &connection_id_for_task,
                                        &client_for_task,
                                        &protocol_for_task,
                                        link_params_for_task,
                                        auto_start_data_transfer,
                                        auto_reconnect,
                                    )
                                    .await
                                    {
                                        continue 'monitor;
                                    } else {
                                        break 'monitor;
                                    }

                                    emit_master_runtime_hint(
                                        &runtime_event_handle,
                                        &station_id_for_task,
                                        Some(&connection_id_for_task),
                                        "frame-count-recovery-startdt",
                                    )
                                    .await;

                                    continue;
                                }
                                _ => {}
                            }

                            info!(
                                "[master] frame count recovery: tcp reconnect station_id={} connection_id={}",
                                station_id_for_task, connection_id_for_task
                            );
                            if recover_protocol_connection(
                                &task_lifecycle,
                                &connection_id_for_task,
                                &client_for_task,
                                &protocol_for_task,
                                link_params_for_task,
                                auto_start_data_transfer,
                                auto_reconnect,
                            )
                            .await
                            {
                                pending_test = None;
                                pending_ack_count = 0;
                                pending_ack_since = None;
                                unacked_since = None;
                                retransmit_attempts = 0;
                                continue 'monitor;
                            }
                            break 'monitor;
                        }

                        // S 帧确认（w/t2 策略）
                        if iframe_count_in_batch > 0 {
                            pending_ack_count =
                                pending_ack_count.saturating_add(iframe_count_in_batch);
                            if pending_ack_since.is_none() {
                                pending_ack_since = Some(Instant::now());
                            }

                            if pending_ack_count >= w_value {
                                let ack = protocol_for_task.lock().await.generate_ack();
                                if let Ok(ack) = ack {
                                    let ack_len = ack.len() as u64;
                                    if client_for_task.send(&ack).await.is_ok() {
                                        if let Some(conn) =
                                            connections.write().await.get_mut(&connection_id_for_task)
                                        {
                                            conn.tx_apdu_count =
                                                conn.tx_apdu_count.saturating_add(1);
                                            conn.tx_bytes = conn.tx_bytes.saturating_add(ack_len);
                                        }
                                        append_master_message(
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Sent,
                                            &connection_id_for_task,
                                            "发送 S 帧确认".to_string(),
                                            Some(bytes_to_hex(&ack)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        pending_ack_count = 0;
                                        pending_ack_since = None;
                                    } else {
                                        if let Some(conn) =
                                            connections.write().await.get_mut(&connection_id_for_task)
                                        {
                                            conn.data_transfer_state = DataTransferState::Error;
                                        }
                                        append_master_message(
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Error,
                                            &connection_id_for_task,
                                            "发送 S 帧确认失败".to_string(),
                                            None,
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        emit_master_runtime_hint(
                                            &runtime_event_handle,
                                            &station_id_for_task,
                                            Some(&connection_id_for_task),
                                            "ack-send-failed",
                                        )
                                        .await;
                                        if recover_protocol_connection(
                                            &task_lifecycle,
                                            &connection_id_for_task,
                                            &client_for_task,
                                            &protocol_for_task,
                                            link_params_for_task,
                                            auto_start_data_transfer,
                                            auto_reconnect,
                                        )
                                        .await
                                        {
                                            pending_test = None;
                                            pending_ack_count = 0;
                                            pending_ack_since = None;
                                            unacked_since = None;
                                            retransmit_attempts = 0;
                                            continue 'monitor;
                                        }
                                        break 'monitor;
                                    }
                                }
                            }
                        }

                        emit_master_runtime_hint(
                            &runtime_event_handle,
                            &station_id_for_task,
                            Some(&connection_id_for_task),
                            "rx-data",
                        )
                        .await;
                    }
                    _ => {}
                        }
                    }
                    _ = keepalive.tick() => {
                        let send_activity = client_for_task.send_activity();
                        if send_activity != last_send_activity {
                            last_send_activity = send_activity;
                            last_activity = Instant::now();
                            let has_unconfirmed = protocol_for_task.lock().await.unconfirmed_count() > 0;
                            if has_unconfirmed && unacked_since.is_none() {
                                unacked_since = Some(Instant::now());
                            }
                        }
                        if let Some(local) = local_socket {
                            for (peer_addr, payload) in client_for_task.take_sent_frames().await {
                                if let Ok(remote) = peer_addr.parse::<std::net::SocketAddr>() {
                                    append_capture_packet(
                                        &capture_packets,
                                        CapturePacket {
                                            timestamp: Utc::now(),
                                            connection_id: connection_id_for_task.clone(),
                                            src: local,
                                            dst: remote,
                                            payload,
                                        },
                                    )
                                    .await;
                                }
                            }
                        }
                        expire_pending_commands(
                            &pending_commands,
                            &data_points,
                            &point_admission,
                            &message_records,
                            &next_message_id,
                            &connection_id_for_task,
                            PENDING_COMMAND_TIMEOUT_SECONDS,
                        )
                        .await;

                        let conn_states = connections
                            .read()
                            .await
                            .get(&connection_id_for_task)
                            .map(|c| (c.transport_state, c.data_transfer_state));

                        let Some((transport_state, data_transfer_state)) = conn_states else {
                            break;
                        };

                        if data_transfer_state == DataTransferState::Starting
                            && last_data_transfer_state != DataTransferState::Starting
                        {
                            handshake_started_at = Instant::now();
                        }
                        if data_transfer_state != DataTransferState::Started {
                            pending_test = None;
                        }
                        last_data_transfer_state = data_transfer_state;

                        if data_transfer_state == DataTransferState::Starting
                            && handshake_started_at.elapsed() >= Duration::from_secs(t0_seconds)
                        {
                            if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                conn.data_transfer_state = DataTransferState::Timeout;
                            }
                            append_master_message(
                                &message_records,
                                &next_message_id,
                                MessageDirection::Error,
                                &connection_id_for_task,
                                format!("握手超时: 等待 STARTDT_CON 超过 {}s", t0_seconds),
                                None,
                                None,
                                None,
                                None,
                            )
                            .await;
                            emit_master_runtime_hint(
                                &runtime_event_handle,
                                &station_id_for_task,
                                Some(&connection_id_for_task),
                                "handshake-timeout",
                            )
                            .await;
                            if recover_protocol_connection(
                                &task_lifecycle,
                                &connection_id_for_task,
                                &client_for_task,
                                &protocol_for_task,
                                link_params_for_task,
                                auto_start_data_transfer,
                                auto_reconnect,
                            )
                            .await
                            {
                                pending_test = None;
                                pending_ack_count = 0;
                                pending_ack_since = None;
                                unacked_since = None;
                                retransmit_attempts = 0;
                                continue 'monitor;
                            }
                            break 'monitor;
                        }

                        if data_transfer_state != DataTransferState::Started
                            || transport_state != TransportState::Connected
                        {
                            continue;
                        }

                        if let Some(sent_at) = pending_test {
                            if sent_at.elapsed() >= Duration::from_secs(t1_seconds) {
                                if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                    conn.data_transfer_state = DataTransferState::Timeout;
                                }
                                append_master_message(
                                    &message_records,
                                    &next_message_id,
                                    MessageDirection::Error,
                                    &connection_id_for_task,
                                    format!("链路超时: 等待 TESTFR_CON 超过 {}s", t1_seconds),
                                    None,
                                    None,
                                    None,
                                    None,
                                )
                                .await;
                                emit_master_runtime_hint(
                                    &runtime_event_handle,
                                    &station_id_for_task,
                                    Some(&connection_id_for_task),
                                    "keepalive-timeout",
                                )
                                .await;
                                if recover_protocol_connection(
                                    &task_lifecycle,
                                    &connection_id_for_task,
                                    &client_for_task,
                                    &protocol_for_task,
                                    link_params_for_task,
                                    auto_start_data_transfer,
                                    auto_reconnect,
                                )
                                .await
                                {
                                    pending_test = None;
                                    pending_ack_count = 0;
                                    pending_ack_since = None;
                                    unacked_since = None;
                                    retransmit_attempts = 0;
                                    continue 'monitor;
                                }
                                break 'monitor;
                            }
                            continue;
                        }

                        // t2: 延迟确认（在无后续 I 帧时，也要及时发送 S 帧确认）
                        if pending_ack_count > 0 {
                            if let Some(since) = pending_ack_since {
                                if since.elapsed() >= Duration::from_secs(t2_seconds) {
                                    let ack = protocol_for_task.lock().await.generate_ack();
                                    if let Ok(ack) = ack {
                                        let ack_len = ack.len() as u64;
                                        if client_for_task.send(&ack).await.is_ok() {
                                            last_activity = Instant::now();
                                            if let Some(conn) = connections
                                                .write()
                                                .await
                                                .get_mut(&connection_id_for_task)
                                            {
                                                conn.tx_apdu_count =
                                                    conn.tx_apdu_count.saturating_add(1);
                                                conn.tx_bytes = conn.tx_bytes.saturating_add(ack_len);
                                            }
                                            append_master_message(
                                                &message_records,
                                                &next_message_id,
                                                MessageDirection::Sent,
                                                &connection_id_for_task,
                                                format!("发送 S 帧确认 (t2 {}s)", t2_seconds),
                                                Some(bytes_to_hex(&ack)),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                            pending_ack_count = 0;
                                            pending_ack_since = None;
                                        } else {
                                            if let Some(conn) = connections
                                                .write()
                                                .await
                                                .get_mut(&connection_id_for_task)
                                            {
                                                conn.data_transfer_state = DataTransferState::Error;
                                            }
                                            append_master_message(
                                                &message_records,
                                                &next_message_id,
                                                MessageDirection::Error,
                                                &connection_id_for_task,
                                                "发送 S 帧确认失败".to_string(),
                                                None,
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                            emit_master_runtime_hint(
                                                &runtime_event_handle,
                                                &station_id_for_task,
                                                Some(&connection_id_for_task),
                                                "ack-send-failed",
                                            )
                                            .await;
                                            if recover_protocol_connection(
                                                &task_lifecycle,
                                                &connection_id_for_task,
                                                &client_for_task,
                                                &protocol_for_task,
                                                link_params_for_task,
                                                auto_start_data_transfer,
                                                auto_reconnect,
                                            )
                                            .await
                                            {
                                                pending_test = None;
                                                pending_ack_count = 0;
                                                pending_ack_since = None;
                                                unacked_since = None;
                                                retransmit_attempts = 0;
                                                continue 'monitor;
                                            }
                                            break 'monitor;
                            }
                        }
                    }
                }
            }

                        // t1: 未确认 I 帧超时重发（超过次数后断链）
                        let (unconfirmed_count, unconfirmed_frames) = {
                            let protocol = protocol_for_task.lock().await;
                            (protocol.unconfirmed_count(), protocol.unconfirmed_frames())
                        };

                        if unconfirmed_count == 0 {
                            retransmit_attempts = 0;
                            unacked_since = None;
                        } else {
                            if unacked_since.is_none() {
                                unacked_since = Some(Instant::now());
                            }

                            if unacked_since
                                .map(|at| at.elapsed() >= Duration::from_secs(t1_seconds))
                                .unwrap_or(false)
                            {
                                if retransmit_attempts >= IEC104_LINK_MAX_RETRANSMIT_ATTEMPTS {
                                    if let Some(conn) = connections
                                        .write()
                                        .await
                                        .get_mut(&connection_id_for_task)
                                    {
                                        conn.data_transfer_state = DataTransferState::Timeout;
                                    }
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Error,
                                        &connection_id_for_task,
                                        format!(
                                            "链路超时: I 帧未确认超过 {}s (重发次数 {})",
                                            t1_seconds, retransmit_attempts
                                        ),
                                        None,
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    emit_master_runtime_hint(
                                        &runtime_event_handle,
                                        &station_id_for_task,
                                        Some(&connection_id_for_task),
                                        "t1-timeout",
                                    )
                                    .await;
                                    if recover_protocol_connection(
                                        &task_lifecycle,
                                        &connection_id_for_task,
                                        &client_for_task,
                                        &protocol_for_task,
                                        link_params_for_task,
                                        auto_start_data_transfer,
                                        auto_reconnect,
                                    )
                                    .await
                                    {
                                        pending_test = None;
                                        pending_ack_count = 0;
                                        pending_ack_since = None;
                                        unacked_since = None;
                                        retransmit_attempts = 0;
                                        continue 'monitor;
                                    }
                                    break 'monitor;
                                }

                                retransmit_attempts = retransmit_attempts.saturating_add(1);
                                unacked_since = Some(Instant::now());

                                let mut resent = 0u64;
                                let mut resent_bytes = 0u64;
                                let mut failed = false;
                                for frame in &unconfirmed_frames {
                                    resent_bytes = resent_bytes.saturating_add(frame.len() as u64);
                                    if client_for_task.send(frame).await.is_ok() {
                                        resent = resent.saturating_add(1);
                                    } else {
                                        failed = true;
                                        break;
                                    }
                                }

                                if failed {
                                    if let Some(conn) = connections
                                        .write()
                                        .await
                                        .get_mut(&connection_id_for_task)
                                    {
                                        conn.data_transfer_state = DataTransferState::Error;
                                    }
                                    append_master_message(
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Error,
                                        &connection_id_for_task,
                                        "重发未确认 I 帧失败".to_string(),
                                        None,
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    emit_master_runtime_hint(
                                        &runtime_event_handle,
                                        &station_id_for_task,
                                        Some(&connection_id_for_task),
                                        "retransmit-failed",
                                    )
                                    .await;
                                    if recover_protocol_connection(
                                        &task_lifecycle,
                                        &connection_id_for_task,
                                        &client_for_task,
                                        &protocol_for_task,
                                        link_params_for_task,
                                        auto_start_data_transfer,
                                        auto_reconnect,
                                    )
                                    .await
                                    {
                                        pending_test = None;
                                        pending_ack_count = 0;
                                        pending_ack_since = None;
                                        unacked_since = None;
                                        retransmit_attempts = 0;
                                        continue 'monitor;
                                    }
                                    break 'monitor;
                                }

                                last_activity = Instant::now();
                                if let Some(conn) =
                                    connections.write().await.get_mut(&connection_id_for_task)
                                {
                                    conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(resent);
                                    conn.tx_bytes = conn.tx_bytes.saturating_add(resent_bytes);
                                }
                                append_master_message(
                                    &message_records,
                                    &next_message_id,
                                    MessageDirection::Sent,
                                    &connection_id_for_task,
                                    format!(
                                        "t1 超时重发未确认 I 帧 {} 条 (attempt={})",
                                        resent, retransmit_attempts
                                    ),
                                    None,
                                    None,
                                    None,
                                    None,
                                )
                                .await;
                                emit_master_runtime_hint(
                                    &runtime_event_handle,
                                    &station_id_for_task,
                                    Some(&connection_id_for_task),
                                    "retransmit",
                                )
                                .await;
                                continue;
                            }
                        }

                        if last_activity.elapsed() < Duration::from_secs(t3_seconds) {
                            continue;
                        }

                        let frame = match protocol_for_task.lock().await.generate_test_act() {
                            Ok(frame) => frame,
                            Err(e) => {
                                warn!(
                                    "[master] keepalive_generate_failed {} error={}",
                                    iec104_log_kv(
                                        &station_id_for_task,
                                        Some(&connection_id_for_task),
                                        None,
                                        None,
                                        None,
                                        Some(Iec104ErrorCode::ProtocolState.as_str()),
                                        None,
                                    ),
                                    e
                                );
                                continue;
                            }
                        };

                        let sent_len = frame.len() as u64;
                        if client_for_task.send(&frame).await.is_ok() {
                            last_activity = Instant::now();
                            pending_test = Some(Instant::now());

                            if let Some(conn) = connections.write().await.get_mut(&connection_id_for_task) {
                                conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
                                conn.tx_bytes = conn.tx_bytes.saturating_add(sent_len);
                            }
                            append_master_message(
                                &message_records,
                                &next_message_id,
                                MessageDirection::Sent,
                                &connection_id_for_task,
                                format!("发送 TESTFR_ACT (t3 keepalive {}s)", t3_seconds),
                                Some(bytes_to_hex(&frame)),
                                None,
                                None,
                                None,
                            )
                            .await;
                            emit_master_runtime_hint(
                                &runtime_event_handle,
                                &station_id_for_task,
                                Some(&connection_id_for_task),
                                "test-frame-sent",
                            )
                            .await;
                        }
                    }
                }
        }

        // 清理资源（尽量）
        cleanup_connection_task(&task_lifecycle, &station_id_for_task, &connection_id_for_task).await;
    });
    (task_handle, start_gate)
}
