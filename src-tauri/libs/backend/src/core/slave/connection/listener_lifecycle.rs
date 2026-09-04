//! 从站监听器生命周期管理模块。
//!
//! 本模块负责从站 TCP 监听器的完整生命周期管理，包括：
//!
//! - **监听器启动**：创建 TCP 服务器并开始监听主站连接
//! - **任务注册**：注册监听任务句柄并更新从站状态
//! - **监听器停止**：停止 TCP 服务器并清理所有连接
//! - **连接事件处理**：处理主站连接和断开事件

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};

use log::{error, info};
use tokio::{
    sync::{Mutex, RwLock, mpsc},
    task::JoinHandle,
};

use super::connection_registry::{
    clear_peer_connection_runtime, ensure_peer_protocol, insert_connected_peer, register_peer_local_addr,
};
use crate::{
    core::{
        shared::runtime_hint::emit_slave_runtime_hint,
        slave::{
            reporting::{soe_support::SoeEvent, spontaneous::SpontaneousCoalescer},
            transport::outbound_dispatcher::{register_outbound_dispatcher, shutdown_registered_outbound_dispatcher},
        },
        types::{ConnectionInfo, LinkParams, SlaveMessageRecord, StationStatus},
    },
    errors::AppResult,
    network::{Iec104Protocol, NetworkConfig, NetworkEvent, TcpServer},
    utils::pcap::CapturePacket,
};

/// 从站监听器运行时上下文
///
/// 包含监听器运行所需的所有共享状态和资源。
#[derive(Clone)]
pub(crate) struct SlaveListenerRuntime {
    pub(crate) status: Arc<RwLock<StationStatus>>,
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    pub(crate) peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    pub(crate) message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    pub(crate) capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    pub(crate) message_tracking_enabled: Arc<AtomicBool>,
    pub(crate) message_tracking_guard: Arc<Mutex<()>>,
    pub(crate) next_message_id: Arc<AtomicU64>,
    pub(crate) server: Arc<RwLock<Option<Arc<TcpServer>>>>,
    pub(crate) protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    pub(crate) link_params: Arc<RwLock<LinkParams>>,
    pub(crate) server_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub(crate) spontaneous_coalescer: Arc<Mutex<SpontaneousCoalescer>>,
    pub(crate) soe_events: Arc<RwLock<VecDeque<SoeEvent>>>,
    pub(crate) station_time_offset_ms: Arc<RwLock<i64>>,
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
}

/// 已准备的从站监听器
///
/// 包含启动后的 TCP 服务器实例和网络事件接收器。
pub(crate) struct PreparedSlaveListener {
    pub(crate) server: Arc<TcpServer>,
    pub(crate) event_receiver: mpsc::Receiver<NetworkEvent>,
}

/// 启动从站监听器
///
/// 创建并启动 TCP 服务器，返回服务器实例和事件接收器。
pub(crate) async fn start_listener(
    runtime: &SlaveListenerRuntime,
    station_id: &str,
    station_name: &str,
    bind_addr: String,
) -> AppResult<PreparedSlaveListener> {
    let server = Arc::new(TcpServer::new(bind_addr, NetworkConfig::default()));
    if let Err(err) = server.start().await {
        error!("从站 {} 启动监听失败: {}", station_name, err);
        *runtime.status.write().await = StationStatus::Stopped;
        return Err(err.into());
    }
    *runtime.server.write().await = Some(server.clone());
    let event_receiver = server.get_event_receiver().await?;
    info!("从站 {} 监听已启动: {}", station_id, station_name);
    Ok(PreparedSlaveListener { server, event_receiver })
}

/// 注册监听任务句柄
///
/// 保存监听任务句柄并更新从站状态为运行中。
pub(crate) async fn register_listener_task(
    runtime: &SlaveListenerRuntime,
    station_id: &str,
    task_handle: JoinHandle<()>,
) {
    *runtime.server_task.lock().await = Some(task_handle);
    *runtime.status.write().await = StationStatus::Running;
    emit_slave_runtime_hint(&runtime.runtime_event_handle, station_id, None, "station-started").await;
}

/// 停止从站监听器
///
/// 停止监听任务和 TCP 服务器，清理所有连接资源。
pub(crate) async fn stop_listener(runtime: &SlaveListenerRuntime, station_id: &str) -> AppResult<()> {
    {
        let mut status = runtime.status.write().await;
        if *status == StationStatus::Stopped {
            return Ok(());
        }
        *status = StationStatus::Stopping;
    }
    if let Some(handle) = runtime.server_task.lock().await.take() {
        handle.abort();
        let _ = handle.await;
    }
    if let Some(server) = runtime.server.write().await.take() {
        let _ = server.stop().await;
    }
    {
        let mut coalescer = runtime.spontaneous_coalescer.lock().await;
        coalescer.pending_points.clear();
        coalescer.flush_active = false;
    }
    let protocol_entries = runtime
        .protocols
        .read()
        .await
        .iter()
        .map(|(peer_addr, protocol)| (peer_addr.clone(), Arc::clone(protocol)))
        .collect::<Vec<_>>();
    for (peer_addr, protocol) in protocol_entries {
        shutdown_registered_outbound_dispatcher(&peer_addr, &protocol).await;
    }
    runtime.protocols.write().await.clear();
    runtime.connections.write().await.clear();
    runtime.peer_local_addrs.write().await.clear();
    runtime.soe_events.write().await.clear();
    *runtime.station_time_offset_ms.write().await = 0;
    *runtime.status.write().await = StationStatus::Stopped;
    emit_slave_runtime_hint(&runtime.runtime_event_handle, station_id, None, "station-stopped").await;
    Ok(())
}

/// 处理对端连接事件
///
/// 当主站连接成功时，注册连接信息并初始化协议实例。
pub(crate) async fn handle_peer_connected(
    runtime: &SlaveListenerRuntime,
    station_id: &str,
    server: &Arc<TcpServer>,
    peer_addr: &str,
    local_addr: &str,
) {
    register_peer_local_addr(&runtime.peer_local_addrs, peer_addr, local_addr).await;
    insert_connected_peer(&runtime.connections, station_id, peer_addr).await;
    let protocol = ensure_peer_protocol(&runtime.protocols, peer_addr).await;
    let (k_value, max_asdu_bytes) = {
        let guard = runtime.link_params.read().await;
        (guard.k_value, guard.max_asdu_bytes)
    };
    {
        let mut protocol_guard = protocol.lock().await;
        protocol_guard.set_max_unconfirmed(k_value);
        protocol_guard.set_max_asdu_len(max_asdu_bytes as usize);
    }
    register_outbound_dispatcher(
        station_id,
        peer_addr,
        server,
        &protocol,
        &runtime.connections,
        &runtime.peer_local_addrs,
        &runtime.message_records,
        &runtime.message_tracking_enabled,
        &runtime.message_tracking_guard,
        &runtime.next_message_id,
        &runtime.capture_packets,
        &runtime.link_params,
        &runtime.runtime_event_handle,
    )
    .await;
    emit_slave_runtime_hint(&runtime.runtime_event_handle, station_id, Some(peer_addr), "connection-changed").await;
}

/// 处理对端断开事件
///
/// 当主站断开连接时，清理连接资源和协议实例。
pub(crate) async fn handle_peer_disconnected(
    runtime: &SlaveListenerRuntime,
    station_id: &str,
    peer_addr: &str,
    _reason: &str,
) {
    let protocol = runtime.protocols.read().await.get(peer_addr).cloned();
    if let Some(protocol) = protocol.as_ref() {
        shutdown_registered_outbound_dispatcher(peer_addr, protocol).await;
    }
    clear_peer_connection_runtime(&runtime.connections, &runtime.peer_local_addrs, &runtime.protocols, peer_addr).await;
    emit_slave_runtime_hint(&runtime.runtime_event_handle, station_id, Some(peer_addr), "connection-changed").await;
}
