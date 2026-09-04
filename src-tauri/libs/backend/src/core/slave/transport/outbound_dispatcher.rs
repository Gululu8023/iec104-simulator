//! 从站出站调度器模块。
//!
//! 本模块负责从站向主站发送数据的调度和管理,为每个主站连接维护独立的出站队列和调度任务,包括:
//!
//! - **优先级队列**:支持高优先级和普通优先级的 ASDU 发送
//! - **批量发送**:自动合并多个 ASDU 到单个 I 帧以提高效率
//! - **发送窗口管理**:遵守 IEC104 发送窗口限制(k 参数),避免窗口满
//! - **队列容量控制**:限制队列大小,防止内存溢出
//! - **消息跟踪**:记录发送的 ASDU 消息和 PCAP 抓包数据
//! - **指标统计**:跟踪队列深度、等待时间、窗口使用率等指标
//! - **异步调度**:每个连接独立的后台调度任务,互不阻塞
//!
//! # 工作流程
//!
//! ```text
//! ASDU 入队 → 优先级队列 → 调度器唤醒 → 检查发送窗口
//!                                           ↓
//!                                    窗口可用 → 批量编码 → TCP 发送
//!                                           ↓
//!                                    窗口满 → 等待确认
//! ```

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex as StdMutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64},
    },
    time::{Duration, Instant},
};

use log::{debug, warn};
use tokio::{
    sync::{Mutex, Notify, RwLock, oneshot},
    task::JoinHandle,
};

use crate::{
    core::{
        shared::runtime_hint::emit_slave_runtime_hint,
        slave::{
            connection::connection_metrics::{
                increment_window_full_avoided_count, sync_send_window_used, update_outbound_queue_metrics,
                update_queue_wait_ms,
            },
            data::message_store::{
                append_tracked_capture_packet, append_tracked_slave_message, is_slave_message_tracking_enabled,
            },
            protocol::response_cause::ResponseEnvelope,
            transport::{
                outbound_frame::{
                    EncodedOutboundFrame, encode_batched_outbound_frame, encode_response_outbound_frame,
                    encode_single_outbound_frame,
                },
                outbound_queue::{
                    OutboundPriority, OutboundWorkItem, OutboundWorkKind, PeerOutboundState,
                    sanitize_outbound_queue_capacity,
                },
            },
        },
        types::{AsduInfo, ConnectionInfo, LinkParams, MessageDirection, SlaveMessageRecord},
    },
    errors::{AppError, AppResult, NetworkError},
    network::{Iec104Protocol, ProtocolState, TcpServer},
    utils::{bytes_to_hex_preview, pcap::CapturePacket},
};

struct PeerOutboundDispatcher {
    peer_addr: String,
    server: Arc<TcpServer>,
    protocol: Arc<Mutex<Iec104Protocol>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    message_tracking_enabled: Arc<AtomicBool>,
    message_tracking_guard: Arc<Mutex<()>>,
    next_message_id: Arc<AtomicU64>,
    capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    link_params: Arc<RwLock<LinkParams>>,
    station_id: String,
    runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    state: Arc<Mutex<PeerOutboundState>>,
    queue_notify: Arc<Notify>,
    ack_notify: Arc<Notify>,
    capacity_notify: Arc<Notify>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl PeerOutboundDispatcher {
    async fn spawn(
        peer_addr: String,
        server: Arc<TcpServer>,
        protocol: Arc<Mutex<Iec104Protocol>>,
        connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
        peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
        message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
        message_tracking_enabled: Arc<AtomicBool>,
        message_tracking_guard: Arc<Mutex<()>>,
        next_message_id: Arc<AtomicU64>,
        capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
        link_params: Arc<RwLock<LinkParams>>,
        station_id: String,
        runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    ) -> Arc<Self> {
        let dispatcher = Arc::new(Self {
            peer_addr,
            server,
            protocol,
            connections,
            peer_local_addrs,
            message_records,
            message_tracking_enabled,
            message_tracking_guard,
            next_message_id,
            capture_packets,
            link_params,
            station_id,
            runtime_event_handle,
            state: Arc::new(Mutex::new(PeerOutboundState::default())),
            queue_notify: Arc::new(Notify::new()),
            ack_notify: Arc::new(Notify::new()),
            capacity_notify: Arc::new(Notify::new()),
            task: Mutex::new(None),
        });
        let dispatcher_for_task = Arc::clone(&dispatcher);
        let handle = tokio::spawn(async move {
            dispatcher_for_task.run().await;
        });
        *dispatcher.task.lock().await = Some(handle);
        dispatcher
    }

    async fn enqueue(&self, item: OutboundWorkItem) -> AppResult<()> {
        let mut pending_item = Some(item);
        loop {
            let capacity_available = self.capacity_notify.notified();
            tokio::pin!(capacity_available);
            capacity_available.as_mut().enable();
            let queue_capacity = {
                let params = *self.link_params.read().await;
                sanitize_outbound_queue_capacity(params.outbound_queue_capacity)
            };
            let (queue_len, interrogation_active) = {
                let mut state = self.state.lock().await;
                if state.stopped {
                    return Err(AppError::Network(NetworkError::ConnectionClosed));
                }
                if state.total_len() < queue_capacity {
                    state.push(pending_item.take().expect("pending outbound item"));
                    (state.total_len(), state.interrogation_active)
                } else {
                    drop(state);
                    capacity_available.await;
                    continue;
                }
            };
            update_outbound_queue_metrics(&self.connections, &self.peer_addr, queue_len, interrogation_active).await;
            self.queue_notify.notify_one();
            return Ok(());
        }
    }

    async fn shutdown(&self) {
        let pending = {
            let mut state = self.state.lock().await;
            state.stopped = true;
            state.drain_all()
        };
        for mut item in pending {
            if let Some(tx) = item.completion.take() {
                let _ = tx.send(Err(AppError::Network(NetworkError::ConnectionClosed)));
            }
        }
        update_outbound_queue_metrics(&self.connections, &self.peer_addr, 0, false).await;
        self.queue_notify.notify_waiters();
        self.ack_notify.notify_waiters();
        self.capacity_notify.notify_waiters();
        if let Some(handle) = self.task.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    fn notify_window_available(&self) {
        self.ack_notify.notify_one();
    }

    async fn run(self: Arc<Self>) {
        let mut last_tx_hint_at: Option<Instant> = None;
        loop {
            let next_item = {
                let mut state = self.state.lock().await;
                if state.stopped {
                    return;
                }
                let item = state.pop_next();
                let queue_len = state.total_len();
                let interrogation_active = state.interrogation_active;
                (item, queue_len, interrogation_active)
            };

            if let Some(mut item) = next_item.0 {
                update_outbound_queue_metrics(&self.connections, &self.peer_addr, next_item.1, next_item.2).await;
                self.capacity_notify.notify_waiters();
                let is_interrogation = item.priority == OutboundPriority::Interrogation;
                let result = self.process_item(&item).await;
                let should_emit_tx_hint = result.is_ok() && !item.frames.is_empty();
                if is_interrogation {
                    let (queue_len, interrogation_active) = {
                        let mut state = self.state.lock().await;
                        state.interrogation_active = !state.interrogation_queue.is_empty();
                        (state.total_len(), state.interrogation_active)
                    };
                    update_outbound_queue_metrics(&self.connections, &self.peer_addr, queue_len, interrogation_active)
                        .await;
                }
                self.capacity_notify.notify_waiters();
                if let Some(tx) = item.completion.take() {
                    let _ = tx.send(result);
                }
                let should_notify_frontend =
                    last_tx_hint_at.map(|last_at| last_at.elapsed() >= Duration::from_millis(250)).unwrap_or(true);
                if should_emit_tx_hint
                    && should_notify_frontend
                    && is_slave_message_tracking_enabled(&self.message_tracking_enabled)
                {
                    last_tx_hint_at = Some(Instant::now());
                    emit_slave_runtime_hint(
                        &self.runtime_event_handle,
                        &self.station_id,
                        Some(&self.peer_addr),
                        "tx-data",
                    )
                    .await;
                }
                continue;
            }

            update_outbound_queue_metrics(&self.connections, &self.peer_addr, 0, false).await;
            tokio::select! {
                _ = self.queue_notify.notified() => {}
                _ = self.ack_notify.notified() => {}
            }
        }
    }

    async fn process_item(&self, item: &OutboundWorkItem) -> AppResult<()> {
        let sequence_group = item.sequence_group.as_deref().unwrap_or("none");
        let dedupe_key = item.dedupe_key.as_deref().unwrap_or("none");
        debug!(
            "[slave] outbound_dispatch peer={} priority={:?} kind={:?} frames={} sequence_group={} dedupe_key={}",
            self.peer_addr,
            item.priority,
            item.kind,
            item.frames.len(),
            sequence_group,
            dedupe_key
        );
        for frame in &item.frames {
            let mut window_blocked = false;
            let wire_frame = loop {
                let window_available = self.ack_notify.notified();
                tokio::pin!(window_available);
                window_available.as_mut().enable();
                if self.state.lock().await.stopped {
                    return Err(AppError::Network(NetworkError::ConnectionClosed));
                }
                let maybe_frame = {
                    let mut protocol = self.protocol.lock().await;
                    if protocol.state() != &ProtocolState::DataTransfer {
                        return Err(AppError::Network(NetworkError::ConnectionClosed));
                    }
                    if protocol.unconfirmed_count() >= protocol.max_unconfirmed() {
                        None
                    } else {
                        Some(protocol.send_asdu(&frame.asdu_bytes))
                    }
                };

                match maybe_frame {
                    Some(Ok(wire_frame)) => break wire_frame,
                    Some(Err(err)) => return Err(err.into()),
                    None => {
                        if !window_blocked {
                            increment_window_full_avoided_count(&self.connections, &self.peer_addr).await;
                            window_blocked = true;
                        }
                        window_available.await;
                    }
                }
            };

            self.server.send_to(&self.peer_addr, &wire_frame).await?;
            record_slave_sent_frame(
                &self.peer_addr,
                &wire_frame,
                &self.connections,
                &self.peer_local_addrs,
                &self.message_tracking_enabled,
                &self.message_tracking_guard,
                &self.message_records,
                &self.next_message_id,
                &self.capture_packets,
                frame.content.clone(),
                frame.type_id,
                frame.cause,
                frame.common_address,
            )
            .await;
            sync_send_window_used(&self.peer_addr, &self.protocol, &self.connections).await;
        }

        let wait_ms = item.enqueued_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        update_queue_wait_ms(&self.connections, &self.peer_addr, wait_ms).await;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PeerOutboundDispatcherKey {
    peer_addr: String,
    protocol_ptr: usize,
}

type PeerOutboundDispatcherRegistry = HashMap<PeerOutboundDispatcherKey, Weak<PeerOutboundDispatcher>>;

fn outbound_dispatcher_registry() -> &'static StdMutex<PeerOutboundDispatcherRegistry> {
    static REGISTRY: OnceLock<StdMutex<PeerOutboundDispatcherRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn outbound_dispatcher_key(peer_addr: &str, protocol: &Arc<Mutex<Iec104Protocol>>) -> PeerOutboundDispatcherKey {
    PeerOutboundDispatcherKey { peer_addr: peer_addr.to_string(), protocol_ptr: Arc::as_ptr(protocol) as usize }
}

pub(crate) async fn record_slave_sent_frame(
    peer_addr: &str,
    frame: &[u8],
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_tracking_enabled: &Arc<AtomicBool>,
    message_tracking_guard: &Arc<Mutex<()>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    content: String,
    type_id: Option<u8>,
    cause: Option<u16>,
    common_address: Option<u16>,
) {
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
        conn.tx_bytes = conn.tx_bytes.saturating_add(frame.len() as u64);
    }

    append_tracked_slave_message(
        message_tracking_guard,
        message_tracking_enabled,
        message_records,
        next_message_id,
        MessageDirection::Sent,
        peer_addr,
        content,
        Some(bytes_to_hex_preview(frame)),
        type_id,
        cause,
        common_address,
    )
    .await;

    if let (Some(local), Ok(remote)) =
        (peer_local_addrs.read().await.get(peer_addr).copied(), peer_addr.parse::<std::net::SocketAddr>())
    {
        append_tracked_capture_packet(
            message_tracking_guard,
            message_tracking_enabled,
            capture_packets,
            CapturePacket {
                timestamp: chrono::Utc::now(),
                connection_id: peer_addr.to_string(),
                src: local,
                dst: remote,
                payload: frame.to_vec(),
            },
        )
        .await;
    }
}

pub(crate) async fn send_asdu_to_peer_wait_window(
    peer_addr: &str,
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    asdu: &AsduInfo,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
) -> AppResult<()> {
    send_asdu_to_peer_with_priority(
        peer_addr,
        server,
        protocol,
        asdu,
        connections,
        peer_local_addrs,
        message_records,
        next_message_id,
        capture_packets,
        OutboundPriority::Control,
    )
    .await
}

pub(crate) async fn send_asdu_to_peer_with_priority(
    peer_addr: &str,
    _server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    asdu: &AsduInfo,
    _connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    _peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    _message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    _next_message_id: &Arc<AtomicU64>,
    _capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    priority: OutboundPriority,
) -> AppResult<()> {
    let dispatcher = get_registered_outbound_dispatcher(peer_addr, protocol)
        .ok_or(AppError::Network(NetworkError::ConnectionClosed))?;
    let (item, rx) = OutboundWorkItem::single(
        priority,
        OutboundWorkKind::SingleAsdu,
        None,
        None,
        encode_single_outbound_frame(asdu),
    );
    dispatcher.enqueue(item).await?;
    await_outbound_completion(rx).await
}

pub(crate) async fn send_response_to_peer_wait_window(
    peer_addr: &str,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    response: &ResponseEnvelope,
) -> AppResult<()> {
    let dispatcher = get_registered_outbound_dispatcher(peer_addr, protocol)
        .ok_or(AppError::Network(NetworkError::ConnectionClosed))?;
    let (item, rx) = OutboundWorkItem::single(
        OutboundPriority::Control,
        OutboundWorkKind::SingleAsdu,
        None,
        None,
        encode_response_outbound_frame(response),
    );
    dispatcher.enqueue(item).await?;
    await_outbound_completion(rx).await
}

pub(crate) async fn enqueue_response_to_peer_with_priority(
    peer_addr: &str,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    response: &ResponseEnvelope,
) -> AppResult<()> {
    let dispatcher = get_registered_outbound_dispatcher(peer_addr, protocol)
        .ok_or(AppError::Network(NetworkError::ConnectionClosed))?;
    let (item, rx) = OutboundWorkItem::single(
        OutboundPriority::Control,
        OutboundWorkKind::SingleAsdu,
        None,
        None,
        encode_response_outbound_frame(response),
    );
    dispatcher.enqueue(item).await?;
    let peer_addr = peer_addr.to_string();
    tokio::spawn(async move {
        if let Ok(Err(err)) = rx.await {
            warn!("[slave] async response completion failed peer={} error={}", peer_addr, err);
        }
    });
    Ok(())
}

pub(crate) async fn send_asdu_batched_to_peer_with_priority(
    peer_addr: &str,
    _server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    asdu: &AsduInfo,
    object_count: u8,
    sq_sequence: bool,
    _connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    _peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    _message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    _next_message_id: &Arc<AtomicU64>,
    _capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    priority: OutboundPriority,
) -> AppResult<()> {
    let dispatcher = get_registered_outbound_dispatcher(peer_addr, protocol)
        .ok_or(AppError::Network(NetworkError::ConnectionClosed))?;
    let (item, rx) = OutboundWorkItem::single(
        priority,
        OutboundWorkKind::BatchedAsdu,
        None,
        None,
        encode_batched_outbound_frame(asdu, object_count, sq_sequence),
    );
    dispatcher.enqueue(item).await?;
    await_outbound_completion(rx).await
}

pub(crate) async fn enqueue_interrogation_sequence(
    peer_addr: &str,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    sequence_group: String,
    frames: Vec<EncodedOutboundFrame>,
) -> AppResult<()> {
    let dispatcher = get_registered_outbound_dispatcher(peer_addr, protocol)
        .ok_or(AppError::Network(NetworkError::ConnectionClosed))?;
    let (item, rx) = OutboundWorkItem::sequence(OutboundPriority::Interrogation, Some(sequence_group), frames);
    dispatcher.enqueue(item).await?;
    await_outbound_completion(rx).await
}

async fn await_outbound_completion(rx: oneshot::Receiver<AppResult<()>>) -> AppResult<()> {
    match rx.await {
        Ok(result) => result,
        Err(_) => Err(AppError::Network(NetworkError::ConnectionClosed)),
    }
}

pub(crate) async fn register_outbound_dispatcher(
    station_id: &str,
    peer_addr: &str,
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    message_tracking_enabled: &Arc<AtomicBool>,
    message_tracking_guard: &Arc<Mutex<()>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    link_params: &Arc<RwLock<LinkParams>>,
    runtime_event_handle: &Arc<crate::services::RuntimeEventSink>,
) {
    let key = outbound_dispatcher_key(peer_addr, protocol);
    if let Some(dispatcher) = outbound_dispatcher_registry()
        .lock()
        .expect("outbound dispatcher registry lock")
        .get(&key)
        .and_then(|dispatcher| dispatcher.upgrade())
    {
        dispatcher.notify_window_available();
        return;
    }

    let dispatcher = PeerOutboundDispatcher::spawn(
        peer_addr.to_string(),
        Arc::clone(server),
        Arc::clone(protocol),
        Arc::clone(connections),
        Arc::clone(peer_local_addrs),
        Arc::clone(message_records),
        Arc::clone(message_tracking_enabled),
        Arc::clone(message_tracking_guard),
        Arc::clone(next_message_id),
        Arc::clone(capture_packets),
        Arc::clone(link_params),
        station_id.to_string(),
        Arc::clone(runtime_event_handle),
    )
    .await;
    outbound_dispatcher_registry()
        .lock()
        .expect("outbound dispatcher registry lock")
        .insert(key, Arc::downgrade(&dispatcher));
}

fn get_registered_outbound_dispatcher(
    peer_addr: &str,
    protocol: &Arc<Mutex<Iec104Protocol>>,
) -> Option<Arc<PeerOutboundDispatcher>> {
    let key = outbound_dispatcher_key(peer_addr, protocol);
    let mut registry = outbound_dispatcher_registry().lock().expect("outbound dispatcher registry lock");
    let dispatcher = registry.get(&key).and_then(|dispatcher| dispatcher.upgrade());
    if dispatcher.is_none() {
        registry.remove(&key);
    }
    dispatcher
}

pub(crate) async fn shutdown_registered_outbound_dispatcher(peer_addr: &str, protocol: &Arc<Mutex<Iec104Protocol>>) {
    let key = outbound_dispatcher_key(peer_addr, protocol);
    let dispatcher = outbound_dispatcher_registry()
        .lock()
        .expect("outbound dispatcher registry lock")
        .remove(&key)
        .and_then(|dispatcher| dispatcher.upgrade());
    if let Some(dispatcher) = dispatcher {
        dispatcher.shutdown().await;
    }
}

pub(crate) fn notify_registered_outbound_dispatcher(peer_addr: &str, protocol: &Arc<Mutex<Iec104Protocol>>) {
    if let Some(dispatcher) = get_registered_outbound_dispatcher(peer_addr, protocol) {
        dispatcher.notify_window_available();
    }
}
