use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use log::{debug, error, info, warn};
use tokio::time::Duration;

use super::{
    FrameCountRecoveryStrategy, IEC104_LINK_MAX_RETRANSMIT_ATTEMPTS, SlaveService, first_ioa_from_decoded_asdu,
};
use crate::{
    core::{
        shared::runtime_hint::emit_slave_runtime_hint,
        slave::{
            connection::{
                connection_metrics::{increment_dropped_frame_count, sync_send_window_used},
                connection_registry::{PeerLinkState, ensure_peer_protocol, increment_retransmit_count},
                listener_lifecycle::{
                    PreparedSlaveListener, handle_peer_connected, handle_peer_disconnected, register_listener_task,
                    start_listener, stop_listener,
                },
            },
            data::{
                file_repo::{build_slave_file_repo_dir, create_slave_file_repo_dir, resolve_slave_file_repo_base_dir},
                message_store::{
                    append_tracked_capture_packet, append_tracked_slave_message, is_slave_message_tracking_enabled,
                },
            },
            protocol::asdu_dispatch::{RequestContext, handle_master_asdu},
            simulation::runtime::{
                resume_simulation_task as run_resume_simulation_task,
                suspend_simulation_task as run_suspend_simulation_task,
            },
            transport::outbound_dispatcher::{notify_registered_outbound_dispatcher, record_slave_sent_frame},
        },
        types::{DataTransferState, MessageDirection, StationConfig, StationStatus, TransportState},
    },
    errors::{AppError, AppResult, ConfigError},
    network::{NetworkEvent, ProtocolMessage, ProtocolState, UFrameType},
    utils::{bytes_to_hex, logger::iec104_log_kv, pcap::CapturePacket},
};

impl SlaveService {
    /// 启动从站服务
    ///
    /// 启动 IEC104 从站服务，开始监听主站连接请求。
    /// 这是从站服务的核心方法，负责初始化所有必要的资源并启动监听任务。
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 启动成功
    /// - `Err(AppError)` - 启动失败（如端口被占用、文件仓目录创建失败等）
    ///
    /// # 执行流程
    ///
    /// 1. **状态检查**：
    ///    - 检查当前状态是否为 Stopped
    ///    - 如果已经在运行或正在启动，直接返回
    ///    - 将状态设置为 Starting
    ///
    /// 2. **清理和初始化**：
    ///    - 清空消息记录队列
    ///    - 清空 PCAP 抓包队列
    ///    - 清空 SOE 事件队列
    ///    - 重置突发上报合并器
    ///    - 重置从站时间偏移
    ///    - 重置消息 ID 计数器
    ///    - 如果数据点列表为空，初始化默认数据点
    ///
    /// 3. **文件仓目录创建**：
    ///    - 解析文件仓基础目录
    ///    - 为当前从站创建专用文件仓目录
    ///    - 如果创建失败，返回错误并将状态设置为 Stopped
    ///
    /// 4. **TCP 监听器启动**：
    ///    - 创建监听运行时上下文
    ///    - 调用 start_listener 启动 TCP 服务器
    ///    - 获取服务器实例和事件接收器
    ///
    /// 5. **监听任务启动**：
    ///    - 克隆所有必要的共享状态
    ///    - 启动异步任务处理网络事件
    ///    - 实现完整的 IEC104 协议处理逻辑
    ///    - 实现链路超时机制（t0/t1/t2/t3）
    ///    - 实现帧计数错误恢复策略
    ///
    /// 6. **模拟任务恢复**：
    ///    - 如果之前有模拟任务在运行，恢复模拟任务
    ///
    /// # 网络事件处理
    ///
    /// 监听任务会处理以下网络事件：
    /// - **Connected**：主站连接成功，创建协议实例
    /// - **Disconnected**：主站断开连接，清理资源
    /// - **Error**：网络错误，记录日志并更新连接状态
    /// - **DataSent**：数据发送成功，更新链路状态
    /// - **DataReceived**：接收到数据，解析协议帧并处理
    ///
    /// # 协议帧处理
    ///
    /// 监听任务会处理以下协议帧：
    /// - **U 帧**：STARTDT/STOPDT/TESTFR（链路控制）
    /// - **S 帧**：确认帧（流量控制）
    /// - **I 帧**：信息帧（ASDU 数据）
    ///
    /// # 链路超时机制
    ///
    /// - **t0**：连接建立超时（等待 STARTDT_CON）
    /// - **t1**：发送或测试 APDU 超时（未确认 I 帧或 TESTFR_CON）
    /// - **t2**：接收方确认超时（延迟发送 S 帧）
    /// - **t3**：空闲超时（发送 TESTFR_ACT 保活）
    ///
    /// # 帧计数错误恢复
    ///
    /// 当检测到帧计数错误（V(R) 与 N(S) 不匹配）时：
    /// - **TcpReconnect 策略**：断开连接并等待主站重连（默认）
    /// - **StopDtStartDt 策略**：发送 STOPDT 和 STARTDT 重置帧计数
    ///
    /// # 错误处理
    ///
    /// - 如果从站已经在运行，记录警告并返回 Ok
    /// - 如果文件仓目录创建失败，返回配置错误
    /// - 如果 TCP 监听器启动失败，返回网络错误
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    /// 监听任务在独立的 tokio 任务中运行，不会阻塞调用线程。
    ///
    /// # 使用场景
    ///
    /// 在从站服务创建后调用，通常在应用启动时或用户手动启动从站时调用。
    pub async fn start(&self) -> AppResult<()> {
        {
            let mut status = self.transport.status.write().await;
            if *status != StationStatus::Stopped {
                warn!("从站 {} 已经在运行或正在启动", self.config.name);
                return Ok(());
            }
            info!("启动从站: {}", self.config.name);
            *status = StationStatus::Starting;
        }

        self.telemetry.message_records.write().await.clear();
        self.telemetry.capture_packets.write().await.clear();
        self.telemetry.soe_events.write().await.clear();
        {
            let mut coalescer = self.telemetry.spontaneous_coalescer.lock().await;
            coalescer.pending_points.clear();
            coalescer.flush_active = false;
        }
        *self.transport.station_time_offset_ms.write().await = 0;
        self.telemetry.next_message_id.store(0, Ordering::Relaxed);
        if self.points.data_points.read().await.is_empty() {
            self.initialize_data_points().await;
        }

        let file_repo_base_dir = resolve_slave_file_repo_base_dir();
        if let Err(err) = create_slave_file_repo_dir(&file_repo_base_dir, &self.config.id) {
            let repo_dir = build_slave_file_repo_dir(&file_repo_base_dir, &self.config.id);
            error!("从站 {} 创建文件仓目录失败: path={} error={}", self.config.name, repo_dir.display(), err);
            *self.transport.status.write().await = StationStatus::Stopped;
            return Err(AppError::Config(ConfigError::Other(format!(
                "创建从站文件仓目录失败: {} ({err})",
                repo_dir.display()
            ))));
        }

        let listener_runtime = self.listener_runtime();
        let PreparedSlaveListener { server, mut event_receiver } =
            start_listener(&listener_runtime, &self.config.id, &self.config.name, self.config.address.to_string())
                .await?;
        let connections = Arc::clone(&self.transport.connections);
        let peer_local_addrs = Arc::clone(&self.transport.peer_local_addrs);
        let data_points = Arc::clone(&self.points.data_points);
        let protocols = Arc::clone(&self.transport.protocols);
        let link_params = Arc::clone(&self.transport.link_params);
        let station_policy_defaults = Arc::clone(&self.policy.station_policy_defaults);
        let station_policy_overrides = Arc::clone(&self.policy.station_policy_overrides);
        let station_policy_connection_override_enabled =
            Arc::clone(&self.policy.station_policy_connection_override_enabled);
        let message_records = Arc::clone(&self.telemetry.message_records);
        let capture_packets = Arc::clone(&self.telemetry.capture_packets);
        let message_tracking_enabled = Arc::clone(&self.telemetry.message_tracking_enabled);
        let message_tracking_guard = Arc::clone(&self.telemetry.message_tracking_guard);
        let next_message_id = Arc::clone(&self.telemetry.next_message_id);
        let command_mismatch_policy = Arc::clone(&self.policy.command_mismatch_policy);
        let db_service = Arc::clone(&self.context.db_service);
        let runtime_event_handle = Arc::clone(&self.context.runtime_event_handle);
        let soe_events = Arc::clone(&self.telemetry.soe_events);
        let soe_records = Arc::clone(&self.telemetry.soe_records);
        let next_soe_id = Arc::clone(&self.telemetry.next_soe_id);
        let station_time_offset_ms = Arc::clone(&self.transport.station_time_offset_ms);

        let station_id = self.config.id.clone();
        let common_address = self.config.common_address;
        let frame_count_recovery = FrameCountRecoveryStrategy::from_env();
        let task_listener_runtime = listener_runtime.clone();

        let server_task = tokio::spawn(async move {
            let mut last_rx_hint_at = Instant::now() - Duration::from_secs(60);
            let mut peer_links: HashMap<String, PeerLinkState> = HashMap::new();

            loop {
                match tokio::time::timeout(Duration::from_secs(1), event_receiver.recv()).await {
                    Ok(Some(event)) => match event {
                        NetworkEvent::Connected { peer_addr, local_addr } => {
                            handle_peer_connected(
                                &task_listener_runtime,
                                &station_id,
                                &server,
                                &peer_addr,
                                &local_addr,
                            )
                            .await;
                            peer_links.insert(peer_addr, PeerLinkState::new());
                        }
                        NetworkEvent::Disconnected { peer_addr, reason } => {
                            peer_links.remove(&peer_addr);
                            handle_peer_disconnected(&task_listener_runtime, &station_id, &peer_addr, &reason).await;
                        }
                        NetworkEvent::Error { peer_addr, error } => {
                            error!(
                                "[slave] network_error station_id={} peer_addr={:?} error={}",
                                station_id, peer_addr, error
                            );
                            if let Some(peer) = peer_addr {
                                if let Some(conn) = connections.write().await.get_mut(&peer) {
                                    conn.transport_state = TransportState::Error;
                                    conn.data_transfer_state = DataTransferState::Error;
                                }
                                append_tracked_slave_message(
                                    &message_tracking_guard,
                                    &message_tracking_enabled,
                                    &message_records,
                                    &next_message_id,
                                    MessageDirection::Error,
                                    &peer,
                                    format!("网络错误: {}", error),
                                    None,
                                    None,
                                    None,
                                    None,
                                )
                                .await;
                                emit_slave_runtime_hint(
                                    &runtime_event_handle,
                                    &station_id,
                                    Some(&peer),
                                    "connection-changed",
                                )
                                .await;
                            }
                        }
                        NetworkEvent::DataReceived { peer_addr, data } => {
                            if let Some(state) = peer_links.get_mut(&peer_addr) {
                                state.last_activity = Instant::now();
                                state.pending_test = None;
                            }
                            if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                conn.rx_bytes = conn.rx_bytes.saturating_add(data.len() as u64);
                            }

                            if let (Some(local), Ok(remote)) = (
                                peer_local_addrs.read().await.get(&peer_addr).copied(),
                                peer_addr.parse::<std::net::SocketAddr>(),
                            ) {
                                append_tracked_capture_packet(
                                    &message_tracking_guard,
                                    &message_tracking_enabled,
                                    &capture_packets,
                                    CapturePacket {
                                        timestamp: chrono::Utc::now(),
                                        connection_id: peer_addr.clone(),
                                        src: remote,
                                        dst: local,
                                        payload: data.clone(),
                                    },
                                )
                                .await;
                            }

                            let (k_value, w_value, max_asdu_bytes) = {
                                let guard = link_params.read().await;
                                (guard.k_value, guard.w_value, guard.max_asdu_bytes)
                            };

                            let protocol = ensure_peer_protocol(&protocols, &peer_addr).await;

                            let messages = {
                                let mut protocol = protocol.lock().await;
                                protocol.set_max_unconfirmed(k_value);
                                protocol.set_max_asdu_len(max_asdu_bytes as usize);
                                match protocol.handle_received_data(&data) {
                                    Ok(msgs) => msgs,
                                    Err(e) => {
                                        warn!(
                                            "[slave] protocol_parse_error {} error={}",
                                            iec104_log_kv(
                                                &station_id,
                                                Some(&peer_addr),
                                                None,
                                                None,
                                                None,
                                                Some("PROTOCOL_PARSE_FAILED"),
                                                None,
                                            ),
                                            e
                                        );
                                        increment_dropped_frame_count(&connections, &peer_addr, 1).await;
                                        continue;
                                    }
                                }
                            };
                            notify_registered_outbound_dispatcher(&peer_addr, &protocol);
                            sync_send_window_used(&peer_addr, &protocol, &connections).await;

                            if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                conn.rx_apdu_count = conn.rx_apdu_count.saturating_add(messages.len() as u64);
                            }

                            // 更新对端确认进度（用于 t1 重发策略）
                            let (peer_ack_sequence, unconfirmed_count) = {
                                let protocol_guard = protocol.lock().await;
                                (protocol_guard.ack_sequence(), protocol_guard.unconfirmed_count())
                            };
                            if let Some(state) = peer_links.get_mut(&peer_addr) {
                                if peer_ack_sequence != state.last_peer_ack_sequence {
                                    state.last_peer_ack_sequence = peer_ack_sequence;
                                    state.retransmit_attempts = 0;
                                    state.unacked_since =
                                        if unconfirmed_count > 0 { Some(Instant::now()) } else { None };
                                } else if unconfirmed_count == 0 {
                                    state.retransmit_attempts = 0;
                                    state.unacked_since = None;
                                }
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
                                    ProtocolMessage::AckRequired { expected_receive_sequence, got_send_sequence } => {
                                        debug!(
                                            "[slave] duplicate iframe dropped station_id={} peer_addr={} expected_vr={} got_ns={}",
                                            station_id, peer_addr, expected_receive_sequence, got_send_sequence
                                        );
                                        increment_dropped_frame_count(&connections, &peer_addr, 1).await;

                                        let ack = protocol.lock().await.generate_ack();
                                        if let Ok(ack) = ack {
                                            if server.send_to(&peer_addr, &ack).await.is_ok() {
                                                record_slave_sent_frame(
                                                    &peer_addr,
                                                    &ack,
                                                    &connections,
                                                    &peer_local_addrs,
                                                    &message_tracking_enabled,
                                                    &message_tracking_guard,
                                                    &message_records,
                                                    &next_message_id,
                                                    &capture_packets,
                                                    format!("发送 S 帧确认 seq={}", expected_receive_sequence),
                                                    None,
                                                    None,
                                                    None,
                                                )
                                                .await;
                                            }
                                            if let Some(state) = peer_links.get_mut(&peer_addr) {
                                                state.pending_ack_count = 0;
                                                state.pending_ack_since = None;
                                                state.last_activity = Instant::now();
                                            }
                                        }
                                    }
                                    ProtocolMessage::UFrame { u_type: UFrameType::StartDtAct, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            "收到 STARTDT_ACT".to_string(),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        if let Ok(frame) = protocol.lock().await.generate_startdt_con() {
                                            if server.send_to(&peer_addr, &frame).await.is_ok() {
                                                record_slave_sent_frame(
                                                    &peer_addr,
                                                    &frame,
                                                    &connections,
                                                    &peer_local_addrs,
                                                    &message_tracking_enabled,
                                                    &message_tracking_guard,
                                                    &message_records,
                                                    &next_message_id,
                                                    &capture_packets,
                                                    "发送 STARTDT_CON".to_string(),
                                                    None,
                                                    None,
                                                    None,
                                                )
                                                .await;
                                            }
                                        }
                                        if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                            conn.transport_state = TransportState::Connected;
                                            conn.data_transfer_state = DataTransferState::Started;
                                            if conn.connected_at.is_none() {
                                                conn.connected_at = Some(chrono::Utc::now());
                                            }
                                        }
                                        if let Some(state) = peer_links.get_mut(&peer_addr) {
                                            state.last_activity = Instant::now();
                                            state.pending_test = None;
                                            state.unacked_since = None;
                                            state.pending_ack_since = None;
                                            state.frame_count_recovery_attempted = false;
                                        }
                                        emit_slave_runtime_hint(
                                            &runtime_event_handle,
                                            &station_id,
                                            Some(&peer_addr),
                                            "connection-changed",
                                        )
                                        .await;
                                    }
                                    ProtocolMessage::UFrame { u_type: UFrameType::StartDtCon, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            "收到 STARTDT_CON".to_string(),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                            conn.transport_state = TransportState::Connected;
                                            conn.data_transfer_state = DataTransferState::Started;
                                            if conn.connected_at.is_none() {
                                                conn.connected_at = Some(chrono::Utc::now());
                                            }
                                        }
                                        if let Some(state) = peer_links.get_mut(&peer_addr) {
                                            state.last_activity = Instant::now();
                                            state.pending_test = None;
                                            state.unacked_since = None;
                                            state.pending_ack_since = None;
                                            state.frame_count_recovery_attempted = false;
                                        }
                                        emit_slave_runtime_hint(
                                            &runtime_event_handle,
                                            &station_id,
                                            Some(&peer_addr),
                                            "connection-changed",
                                        )
                                        .await;
                                    }
                                    ProtocolMessage::UFrame { u_type: UFrameType::StopDtAct, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            "收到 STOPDT_ACT".to_string(),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        if let Some(state) = peer_links.get_mut(&peer_addr) {
                                            state.clear_pending_selects();
                                            state.file_transfer = None;
                                        }
                                        if let Ok(frame) = protocol.lock().await.generate_stopdt_con() {
                                            if server.send_to(&peer_addr, &frame).await.is_ok() {
                                                record_slave_sent_frame(
                                                    &peer_addr,
                                                    &frame,
                                                    &connections,
                                                    &peer_local_addrs,
                                                    &message_tracking_enabled,
                                                    &message_tracking_guard,
                                                    &message_records,
                                                    &next_message_id,
                                                    &capture_packets,
                                                    "发送 STOPDT_CON".to_string(),
                                                    None,
                                                    None,
                                                    None,
                                                )
                                                .await;
                                            }
                                        }
                                        if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                            conn.transport_state = TransportState::Connected;
                                            conn.data_transfer_state = DataTransferState::Stopped;
                                        }
                                        emit_slave_runtime_hint(
                                            &runtime_event_handle,
                                            &station_id,
                                            Some(&peer_addr),
                                            "connection-changed",
                                        )
                                        .await;
                                    }
                                    ProtocolMessage::UFrame { u_type: UFrameType::StopDtCon, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            "收到 STOPDT_CON".to_string(),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        if let Some(state) = peer_links.get_mut(&peer_addr) {
                                            state.clear_pending_selects();
                                            state.file_transfer = None;
                                        }
                                        if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                            conn.transport_state = TransportState::Connected;
                                            conn.data_transfer_state = DataTransferState::Stopped;
                                        }
                                        emit_slave_runtime_hint(
                                            &runtime_event_handle,
                                            &station_id,
                                            Some(&peer_addr),
                                            "connection-changed",
                                        )
                                        .await;
                                    }
                                    ProtocolMessage::UFrame { u_type: UFrameType::TestFrAct, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            "收到 TESTFR_ACT".to_string(),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        if let Ok(frame) = protocol.lock().await.generate_test_response() {
                                            match server.send_to(&peer_addr, &frame).await {
                                                Ok(_) => {
                                                    record_slave_sent_frame(
                                                        &peer_addr,
                                                        &frame,
                                                        &connections,
                                                        &peer_local_addrs,
                                                        &message_tracking_enabled,
                                                        &message_tracking_guard,
                                                        &message_records,
                                                        &next_message_id,
                                                        &capture_packets,
                                                        "发送 TESTFR_CON".to_string(),
                                                        None,
                                                        None,
                                                        None,
                                                    )
                                                    .await;
                                                }
                                                Err(err) => {
                                                    warn!(
                                                        "[slave] send_testfr_con_failed {} error={}",
                                                        iec104_log_kv(
                                                            &station_id,
                                                            Some(&peer_addr),
                                                            None,
                                                            None,
                                                            None,
                                                            Some("SEND_FAILED"),
                                                            None,
                                                        ),
                                                        err
                                                    );
                                                    append_tracked_slave_message(
                                                        &message_tracking_guard,
                                                        &message_tracking_enabled,
                                                        &message_records,
                                                        &next_message_id,
                                                        MessageDirection::Error,
                                                        &peer_addr,
                                                        format!("发送 TESTFR_CON 失败: {}", err),
                                                        None,
                                                        None,
                                                        None,
                                                        None,
                                                    )
                                                    .await;
                                                }
                                            }
                                        }
                                    }
                                    ProtocolMessage::UFrame { u_type: UFrameType::TestFrCon, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            "收到 TESTFR_CON".to_string(),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                    }
                                    ProtocolMessage::IgnoredFrame { frame_type, reason } => {
                                        warn!(
                                            "[slave] ignore {} frame before STARTDT station_id={} peer_addr={} reason={}",
                                            frame_type, station_id, peer_addr, reason
                                        );
                                        increment_dropped_frame_count(&connections, &peer_addr, 1).await;
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Error,
                                            &peer_addr,
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
                                        let ioa = first_ioa_from_decoded_asdu(&decoded_asdu);
                                        let content = match ioa {
                                            Some(ioa) => format!(
                                                "收到 I 帧 type_id={} cot={} ca={} ioa={}",
                                                decoded_asdu.type_id,
                                                decoded_asdu.cause,
                                                decoded_asdu.common_address,
                                                ioa
                                            ),
                                            None => format!(
                                                "收到 I 帧 type_id={} cot={} ca={}",
                                                decoded_asdu.type_id, decoded_asdu.cause, decoded_asdu.common_address
                                            ),
                                        };
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
                                            content,
                                            Some(bytes_to_hex(&raw_frame)),
                                            Some(decoded_asdu.type_id),
                                            Some(decoded_asdu.cause),
                                            Some(decoded_asdu.common_address),
                                        )
                                        .await;
                                        if let Err(err) = {
                                            let peer_link_state =
                                                peer_links.entry(peer_addr.clone()).or_insert_with(PeerLinkState::new);
                                            let request_context = RequestContext {
                                                station_id: station_id.clone(),
                                                peer_addr: peer_addr.clone(),
                                                slave_common_address: common_address,
                                                server: Arc::clone(&server),
                                                protocol: Arc::clone(&protocol),
                                                connections: Arc::clone(&connections),
                                                peer_local_addrs: Arc::clone(&peer_local_addrs),
                                                message_records: Arc::clone(&message_records),
                                                next_message_id: Arc::clone(&next_message_id),
                                                capture_packets: Arc::clone(&capture_packets),
                                                data_points: Arc::clone(&data_points),
                                                link_params: Arc::clone(&link_params),
                                                station_policy_defaults: Arc::clone(&station_policy_defaults),
                                                station_policy_overrides: Arc::clone(&station_policy_overrides),
                                                station_policy_connection_override_enabled: Arc::clone(
                                                    &station_policy_connection_override_enabled,
                                                ),
                                                command_mismatch_policy: Arc::clone(&command_mismatch_policy),
                                                db_service: Arc::clone(&db_service),
                                                runtime_event_handle: Arc::clone(&runtime_event_handle),
                                                soe_events: Arc::clone(&soe_events),
                                                soe_records: Arc::clone(&soe_records),
                                                next_soe_id: Arc::clone(&next_soe_id),
                                                station_time_offset_ms: Arc::clone(&station_time_offset_ms),
                                            };
                                            handle_master_asdu(&request_context, &decoded_asdu, peer_link_state).await
                                        } {
                                            warn!(
                                                "[slave] handle_master_asdu_failed {} message={}",
                                                iec104_log_kv(
                                                    &station_id,
                                                    Some(&peer_addr),
                                                    Some(decoded_asdu.type_id),
                                                    Some(decoded_asdu.cause),
                                                    ioa,
                                                    None,
                                                    None,
                                                ),
                                                err
                                            );
                                        }
                                    }
                                    ProtocolMessage::IFrameDecodeError {
                                        send_sequence,
                                        receive_sequence,
                                        error,
                                        raw_frame,
                                    } => {
                                        iframe_count_in_batch = iframe_count_in_batch.saturating_add(1);
                                        warn!(
                                            "[slave] ASDU decode failed after accepting I-frame station_id={} peer_addr={} ns={} nr={} error={}",
                                            station_id, peer_addr, send_sequence, receive_sequence, error
                                        );
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Error,
                                            &peer_addr,
                                            format!("I 帧已接收，但 ASDU 解码失败: {}", error),
                                            Some(bytes_to_hex(&raw_frame)),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                    }
                                    ProtocolMessage::SFrame { receive_sequence, raw_frame } => {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Received,
                                            &peer_addr,
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

                            if let Some((expected_receive_sequence, got_send_sequence)) = frame_count_error {
                                increment_dropped_frame_count(&connections, &peer_addr, 1).await;
                                let error_text = format!(
                                    "Frame count error: expected V(R)={}, got N(S)={}",
                                    expected_receive_sequence, got_send_sequence
                                );
                                warn!("[slave] {} station_id={} peer_addr={}", error_text, station_id, peer_addr);
                                append_tracked_slave_message(
                                    &message_tracking_guard,
                                    &message_tracking_enabled,
                                    &message_records,
                                    &next_message_id,
                                    MessageDirection::Error,
                                    &peer_addr,
                                    error_text,
                                    None,
                                    None,
                                    None,
                                    None,
                                )
                                .await;

                                let attempted = peer_links
                                    .get(&peer_addr)
                                    .map(|s| s.frame_count_recovery_attempted)
                                    .unwrap_or(false);

                                if frame_count_recovery == FrameCountRecoveryStrategy::StopDtStartDt && !attempted {
                                    if let Some(state) = peer_links.get_mut(&peer_addr) {
                                        state.frame_count_recovery_attempted = true;
                                    }
                                    if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                        conn.transport_state = TransportState::Connected;
                                        conn.data_transfer_state = DataTransferState::Starting;
                                    }
                                    emit_slave_runtime_hint(
                                        &runtime_event_handle,
                                        &station_id,
                                        Some(&peer_addr),
                                        "connection-changed",
                                    )
                                    .await;

                                    info!(
                                        "[slave] frame count recovery: STOPDT/STARTDT station_id={} peer_addr={}",
                                        station_id, peer_addr
                                    );

                                    let stop_frame = {
                                        let mut protocol = protocol.lock().await;
                                        protocol.begin_stop_data_transfer()
                                    };
                                    if let Ok(stop_frame) = stop_frame {
                                        if server.send_to(&peer_addr, &stop_frame).await.is_ok() {
                                            record_slave_sent_frame(
                                                &peer_addr,
                                                &stop_frame,
                                                &connections,
                                                &peer_local_addrs,
                                                &message_tracking_enabled,
                                                &message_tracking_guard,
                                                &message_records,
                                                &next_message_id,
                                                &capture_packets,
                                                "发送 STOPDT_ACT (frame-count-recovery)".to_string(),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                        }
                                    }

                                    let start_frame = {
                                        let mut protocol = protocol.lock().await;
                                        protocol.begin_data_transfer()
                                    };
                                    if let Ok(start_frame) = start_frame {
                                        if let Some(state) = peer_links.get_mut(&peer_addr) {
                                            state.last_activity = Instant::now();
                                            state.pending_test = None;
                                            state.unacked_since = None;
                                            state.retransmit_attempts = 0;
                                            state.last_peer_ack_sequence = 0;
                                            state.pending_ack_count = 0;
                                            state.pending_ack_since = None;
                                            state.clear_pending_selects();
                                            state.file_transfer = None;
                                        }
                                        if server.send_to(&peer_addr, &start_frame).await.is_ok() {
                                            record_slave_sent_frame(
                                                &peer_addr,
                                                &start_frame,
                                                &connections,
                                                &peer_local_addrs,
                                                &message_tracking_enabled,
                                                &message_tracking_guard,
                                                &message_records,
                                                &next_message_id,
                                                &capture_packets,
                                                "发送 STARTDT_ACT (frame-count-recovery)".to_string(),
                                                None,
                                                None,
                                                None,
                                            )
                                            .await;
                                        }
                                    } else {
                                        let _ = server.disconnect(&peer_addr).await;
                                        peer_links.remove(&peer_addr);
                                        handle_peer_disconnected(
                                            &task_listener_runtime,
                                            &station_id,
                                            &peer_addr,
                                            "frame count recovery failed",
                                        )
                                        .await;
                                    }
                                } else {
                                    let _ = server.disconnect(&peer_addr).await;
                                    peer_links.remove(&peer_addr);
                                    handle_peer_disconnected(
                                        &task_listener_runtime,
                                        &station_id,
                                        &peer_addr,
                                        "frame count error",
                                    )
                                    .await;
                                }

                                continue;
                            }

                            // S 帧确认（w/t2 策略：达到 w 即立即确认；否则等待 tick 触发 t2 确认）
                            let mut should_send_ack_now = false;
                            if iframe_count_in_batch > 0 {
                                if let Some(state) = peer_links.get_mut(&peer_addr) {
                                    state.pending_ack_count =
                                        state.pending_ack_count.saturating_add(iframe_count_in_batch);
                                    if state.pending_ack_since.is_none() {
                                        state.pending_ack_since = Some(Instant::now());
                                    }
                                    should_send_ack_now = state.pending_ack_count >= w_value;
                                }
                            }
                            if should_send_ack_now {
                                let ack = protocol.lock().await.generate_ack();
                                if let Ok(ack) = ack {
                                    if server.send_to(&peer_addr, &ack).await.is_ok() {
                                        record_slave_sent_frame(
                                            &peer_addr,
                                            &ack,
                                            &connections,
                                            &peer_local_addrs,
                                            &message_tracking_enabled,
                                            &message_tracking_guard,
                                            &message_records,
                                            &next_message_id,
                                            &capture_packets,
                                            "发送 S 帧确认".to_string(),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                        if let Some(state) = peer_links.get_mut(&peer_addr) {
                                            state.pending_ack_count = 0;
                                            state.pending_ack_since = None;
                                            state.last_activity = Instant::now();
                                        }
                                    }
                                }
                            }

                            if is_slave_message_tracking_enabled(&message_tracking_enabled)
                                && last_rx_hint_at.elapsed() >= Duration::from_millis(250)
                            {
                                last_rx_hint_at = Instant::now();
                                emit_slave_runtime_hint(
                                    &runtime_event_handle,
                                    &station_id,
                                    Some(&peer_addr),
                                    "rx-data",
                                )
                                .await;
                            }
                        }
                        _ => {}
                    },
                    Ok(None) => break,
                    Err(_) => {
                        let now = Instant::now();
                        let peer_addrs: Vec<String> = peer_links.keys().cloned().collect();
                        let link_params_snapshot = link_params.read().await.clone();

                        for peer_addr in peer_addrs {
                            let (
                                last_activity,
                                pending_test,
                                unacked_since,
                                retransmit_attempts,
                                pending_ack_count,
                                pending_ack_since,
                            ) = match peer_links.get(&peer_addr) {
                                Some(snapshot) => (
                                    snapshot.last_activity,
                                    snapshot.pending_test,
                                    snapshot.unacked_since,
                                    snapshot.retransmit_attempts,
                                    snapshot.pending_ack_count,
                                    snapshot.pending_ack_since,
                                ),
                                None => continue,
                            };
                            let data_transfer_state =
                                connections.read().await.get(&peer_addr).map(|conn| conn.data_transfer_state.clone());

                            // 1) t2 delayed S-ack
                            if pending_ack_count > 0 {
                                if let Some(since) = pending_ack_since {
                                    if since.elapsed() >= Duration::from_secs(link_params_snapshot.t2_seconds) {
                                        let protocol = protocols.read().await.get(&peer_addr).cloned();
                                        if let Some(protocol) = protocol {
                                            if let Ok(ack) = protocol.lock().await.generate_ack() {
                                                if server.send_to(&peer_addr, &ack).await.is_ok() {
                                                    record_slave_sent_frame(
                                                        &peer_addr,
                                                        &ack,
                                                        &connections,
                                                        &peer_local_addrs,
                                                        &message_tracking_enabled,
                                                        &message_tracking_guard,
                                                        &message_records,
                                                        &next_message_id,
                                                        &capture_packets,
                                                        "发送 S 帧确认 (t2)".to_string(),
                                                        None,
                                                        None,
                                                        None,
                                                    )
                                                    .await;
                                                    if let Some(state) = peer_links.get_mut(&peer_addr) {
                                                        state.pending_ack_count = 0;
                                                        state.pending_ack_since = None;
                                                        state.last_activity = now;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 2) keepalive timeout (waiting TESTFR_CON)
                            if let Some(sent_at) = pending_test {
                                if sent_at.elapsed() >= Duration::from_secs(link_params_snapshot.t1_seconds) {
                                    if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                        conn.data_transfer_state = DataTransferState::Timeout;
                                    }
                                    append_tracked_slave_message(
                                        &message_tracking_guard,
                                        &message_tracking_enabled,
                                        &message_records,
                                        &next_message_id,
                                        MessageDirection::Error,
                                        &peer_addr,
                                        format!("链路超时: 等待 TESTFR_CON 超过 {}s", link_params_snapshot.t1_seconds),
                                        None,
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;

                                    let _ = server.disconnect(&peer_addr).await;
                                    peer_links.remove(&peer_addr);
                                    handle_peer_disconnected(
                                        &task_listener_runtime,
                                        &station_id,
                                        &peer_addr,
                                        "test frame timeout",
                                    )
                                    .await;
                                    continue;
                                }
                            }

                            if data_transfer_state == Some(DataTransferState::Starting)
                                && last_activity.elapsed() >= Duration::from_secs(link_params_snapshot.t0_seconds)
                            {
                                if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                    conn.data_transfer_state = DataTransferState::Timeout;
                                }
                                append_tracked_slave_message(
                                    &message_tracking_guard,
                                    &message_tracking_enabled,
                                    &message_records,
                                    &next_message_id,
                                    MessageDirection::Error,
                                    &peer_addr,
                                    format!("链路超时: 等待 STARTDT_CON 超过 {}s", link_params_snapshot.t0_seconds),
                                    None,
                                    None,
                                    None,
                                    None,
                                )
                                .await;
                                emit_slave_runtime_hint(
                                    &runtime_event_handle,
                                    &station_id,
                                    Some(&peer_addr),
                                    "startdt-timeout",
                                )
                                .await;
                                let _ = server.disconnect(&peer_addr).await;
                                peer_links.remove(&peer_addr);
                                handle_peer_disconnected(
                                    &task_listener_runtime,
                                    &station_id,
                                    &peer_addr,
                                    "STARTDT timeout",
                                )
                                .await;
                                continue;
                            }

                            // 3) t1 retransmit/disconnect strategy for unconfirmed I-frames
                            let protocol = protocols.read().await.get(&peer_addr).cloned();
                            let Some(protocol) = protocol else {
                                continue;
                            };

                            let (protocol_state, unconfirmed_count) = {
                                let guard = protocol.lock().await;
                                (guard.state().clone(), guard.unconfirmed_count())
                            };

                            if unconfirmed_count > 0 {
                                if unacked_since.is_none() {
                                    if let Some(state) = peer_links.get_mut(&peer_addr) {
                                        state.unacked_since = Some(now);
                                    }
                                } else if unacked_since
                                    .map(|at| at.elapsed() >= Duration::from_secs(link_params_snapshot.t1_seconds))
                                    .unwrap_or(false)
                                {
                                    if retransmit_attempts >= IEC104_LINK_MAX_RETRANSMIT_ATTEMPTS {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Error,
                                            &peer_addr,
                                            format!(
                                                "链路超时: I 帧未确认超过 {}s (重发次数 {})",
                                                link_params_snapshot.t1_seconds, retransmit_attempts
                                            ),
                                            None,
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;

                                        if let Some(conn) = connections.write().await.get_mut(&peer_addr) {
                                            conn.data_transfer_state = DataTransferState::Timeout;
                                        }
                                        let _ = server.disconnect(&peer_addr).await;
                                        peer_links.remove(&peer_addr);
                                        handle_peer_disconnected(
                                            &task_listener_runtime,
                                            &station_id,
                                            &peer_addr,
                                            "I-frame acknowledgement timeout",
                                        )
                                        .await;
                                        continue;
                                    }

                                    let frames = { protocol.lock().await.unconfirmed_frames() };
                                    let retransmit_frames = frames.len() as u64;
                                    let mut resend_failed = false;
                                    for frame in frames {
                                        if server.send_to(&peer_addr, &frame).await.is_err() {
                                            resend_failed = true;
                                            break;
                                        }
                                        record_slave_sent_frame(
                                            &peer_addr,
                                            &frame,
                                            &connections,
                                            &peer_local_addrs,
                                            &message_tracking_enabled,
                                            &message_tracking_guard,
                                            &message_records,
                                            &next_message_id,
                                            &capture_packets,
                                            "重发未确认 I 帧".to_string(),
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;
                                    }

                                    if resend_failed {
                                        append_tracked_slave_message(
                                            &message_tracking_guard,
                                            &message_tracking_enabled,
                                            &message_records,
                                            &next_message_id,
                                            MessageDirection::Error,
                                            &peer_addr,
                                            "重发未确认 I 帧失败".to_string(),
                                            None,
                                            None,
                                            None,
                                            None,
                                        )
                                        .await;

                                        let _ = server.disconnect(&peer_addr).await;
                                        peer_links.remove(&peer_addr);
                                        handle_peer_disconnected(
                                            &task_listener_runtime,
                                            &station_id,
                                            &peer_addr,
                                            "I-frame retransmission failed",
                                        )
                                        .await;
                                        continue;
                                    }
                                    increment_retransmit_count(&connections, &peer_addr, retransmit_frames).await;

                                    if let Some(state) = peer_links.get_mut(&peer_addr) {
                                        state.retransmit_attempts = state.retransmit_attempts.saturating_add(1);
                                        state.unacked_since = Some(now);
                                        state.last_activity = now;
                                    }
                                }
                            }

                            // 4) t3 keepalive (idle -> send TESTFR_ACT)
                            if protocol_state != ProtocolState::DataTransfer {
                                continue;
                            }
                            if unconfirmed_count > 0 {
                                continue;
                            }
                            if pending_test.is_some() {
                                continue;
                            }
                            if last_activity.elapsed() < Duration::from_secs(link_params_snapshot.t3_seconds) {
                                continue;
                            }

                            if let Ok(frame) = protocol.lock().await.generate_test_act() {
                                if server.send_to(&peer_addr, &frame).await.is_ok() {
                                    record_slave_sent_frame(
                                        &peer_addr,
                                        &frame,
                                        &connections,
                                        &peer_local_addrs,
                                        &message_tracking_enabled,
                                        &message_tracking_guard,
                                        &message_records,
                                        &next_message_id,
                                        &capture_packets,
                                        "发送 TESTFR_ACT".to_string(),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                    if let Some(state) = peer_links.get_mut(&peer_addr) {
                                        state.last_activity = now;
                                        state.pending_test = Some(now);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        register_listener_task(&listener_runtime, &self.config.id, server_task).await;

        if let Err(error) = run_resume_simulation_task(&self.simulation_task_runtime(), self.clone_handle()).await {
            let _ = stop_listener(&self.listener_runtime(), &self.config.id).await;
            return Err(error);
        }
        Ok(())
    }

    /// 停止从站服务
    ///
    /// 停止 IEC104 从站服务，断开所有主站连接并清理资源。
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 停止成功
    /// - `Err(AppError)` - 停止失败
    ///
    /// # 执行流程
    ///
    /// 1. 调用 stop_listener 停止监听任务
    /// 2. 断开所有已连接的主站
    /// 3. 清理协议实例
    /// 4. 将状态设置为 Stopped
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    ///
    /// # 使用场景
    ///
    /// 在应用关闭时或用户手动停止从站时调用。
    pub async fn stop(&self) -> AppResult<()> {
        {
            let mut status = self.transport.status.write().await;
            if *status == StationStatus::Stopped {
                return Ok(());
            }
            *status = StationStatus::Stopping;
        }
        run_suspend_simulation_task(&self.simulation_task_runtime()).await;
        stop_listener(&self.listener_runtime(), &self.config.id).await
    }

    /// 获取从站状态
    ///
    /// 返回从站的当前运行状态。
    ///
    /// # 返回
    ///
    /// 返回 StationStatus 枚举值：
    /// - `Stopped` - 已停止
    /// - `Starting` - 正在启动
    /// - `Running` - 正在运行
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_status(&self) -> StationStatus {
        *self.transport.status.read().await
    }

    /// 获取从站配置
    ///
    /// 返回从站的配置信息引用。
    ///
    /// # 返回
    ///
    /// 返回 StationConfig 的不可变引用，包含以下字段：
    /// - `id` - 从站 ID
    /// - `name` - 从站名称
    /// - `address` - 监听地址（IP:Port）
    /// - `common_address` - 公共地址
    ///
    /// # 线程安全
    ///
    /// 配置在创建后不可变，因此无需锁保护。
    pub fn get_config(&self) -> &StationConfig {
        &self.config
    }
}
