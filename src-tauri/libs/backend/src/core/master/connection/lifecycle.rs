//! 连接生命周期管理
//!
//! 本模块负责管理主站连接的完整生命周期,包括连接的创建、监控任务注册、
//! 断开和资源清理。
//!
//! # 核心功能
//!
//! - **连接准备**：建立 TCP 连接,初始化协议处理器,可选自动启动数据传输
//! - **任务注册**：为每个连接注册后台监控任务,处理数据收发和超时检测
//! - **连接断开**：关闭 TCP 连接,取消后台任务,清理所有相关资源
//! - **资源清理**：清理连接的所有运行时资源（客户端、协议处理器、待完成命令等）
//!
//! # 连接生命周期
//!
//! 1. **准备阶段**：`prepare_connection()` - 建立 TCP 连接,初始化协议,可选发送 STARTDT
//! 2. **运行阶段**：`register_monitor_task()` - 注册后台任务,处理数据收发
//! 3. **清理阶段**：`cleanup_connection_task()` - 清理任务资源,移除待完成命令
//! 4. **断开阶段**：`disconnect_connection()` - 关闭连接,清理所有资源
//!
//! # 自动功能
//!
//! - **自动 STARTDT**：连接建立后自动发送 STARTDT_ACT 启动数据传输
//! - **自动重连**：连接断开后自动尝试重连（可配置重试次数和间隔）
//! - **重复连接检测**：防止同一从站地址建立多个连接

use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{Arc, atomic::AtomicU64},
};

use chrono::Utc;
use log::info;
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use super::{
    super::{
        command::store::{PendingCommand, remove_pending_commands_for_connection},
        file_transfer::store::MasterFileTransferSessionRuntime,
        message_store::append_master_message,
    },
    registry::{
        clear_connection_runtime, insert_connection_runtime, register_connection_task, remove_connection_profile_id,
        store_connection_profile_id, take_connection_task,
    },
};
use crate::{
    core::{
        shared::runtime_hint::emit_master_runtime_hint,
        types::{
            ConnectionInfo, DataTransferState, LinkParams, MasterMessageRecord, MessageDirection, StationStatus,
            TransportState,
        },
    },
    errors::{AppError, AppResult, ProtocolError},
    network::{Iec104Protocol, NetworkConfig, ProtocolState, TcpClient},
    utils::bytes_to_hex,
};

/// 主站生命周期运行时
///
/// 封装了连接生命周期管理所需的所有共享状态和资源引用。
#[derive(Clone)]
pub(crate) struct MasterLifecycleRuntime {
    /// 主站状态（Stopped/Starting/Running/Stopping）
    pub(crate) status: Arc<RwLock<StationStatus>>,
    /// 连接列表（key: connection_id, value: 连接信息）
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    /// 正在建立的连接目标。
    pub(crate) connecting_targets: Arc<Mutex<Vec<(SocketAddr, Option<i64>)>>>,
    /// 通信报文缓存（用于记录连接事件）
    pub(crate) message_records: Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    /// 待完成命令队列（用于清理连接的待完成命令）
    pub(crate) pending_commands: Arc<RwLock<VecDeque<PendingCommand>>>,
    /// 报文 ID 生成器（原子递增）
    pub(crate) next_message_id: Arc<AtomicU64>,
    /// TCP 客户端映射（key: connection_id, value: TCP 客户端实例）
    pub(crate) clients: Arc<RwLock<HashMap<String, Arc<TcpClient>>>>,
    /// IEC104 协议处理器映射（key: connection_id, value: 协议编解码器）
    pub(crate) protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    /// 连接事件监控任务（key: connection_id, value: 后台任务句柄）
    pub(crate) connection_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    /// 运行时事件发布器
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    /// 运行时连接 UUID → profile_id 映射（用于关联配置文件）
    pub(crate) connection_profile_map: Arc<RwLock<HashMap<String, i64>>>,
    /// 文件传输会话（key: connection_id, value: 会话运行时）
    pub(crate) file_transfer_sessions: Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
}

fn targets_conflict(
    left_addr: SocketAddr,
    left_profile: Option<i64>,
    right_addr: SocketAddr,
    right_profile: Option<i64>,
) -> bool {
    left_addr == right_addr && (left_profile == right_profile || left_profile.is_none() || right_profile.is_none())
}

pub(crate) async fn reserve_connection_target(
    runtime: &MasterLifecycleRuntime,
    slave_addr: SocketAddr,
    profile_id: Option<i64>,
) -> AppResult<()> {
    let mut connecting_targets = runtime.connecting_targets.lock().await;
    let connections = runtime.connections.read().await;
    if let Some(existing) = connections
        .values()
        .find(|connection| targets_conflict(connection.remote_addr, connection.profile_id, slave_addr, profile_id))
    {
        return Err(AppError::Protocol(ProtocolError::InvalidData(format!(
            "重复连接被拒绝: {} 已存在活动会话 {}",
            slave_addr, existing.id
        ))));
    }
    if connecting_targets
        .iter()
        .any(|(addr, reserved_profile)| targets_conflict(*addr, *reserved_profile, slave_addr, profile_id))
    {
        return Err(AppError::Protocol(ProtocolError::InvalidData(format!(
            "重复连接被拒绝: {} 正在建立连接",
            slave_addr
        ))));
    }
    connecting_targets.push((slave_addr, profile_id));
    Ok(())
}

pub(crate) async fn release_connection_target(
    runtime: &MasterLifecycleRuntime,
    slave_addr: SocketAddr,
    profile_id: Option<i64>,
) {
    let mut connecting_targets = runtime.connecting_targets.lock().await;
    if let Some(index) = connecting_targets.iter().position(|target| *target == (slave_addr, profile_id)) {
        connecting_targets.swap_remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::targets_conflict;

    #[test]
    fn connection_target_conflict_preserves_profile_rules() {
        let address = "127.0.0.1:2404".parse().expect("address");
        let other_address = "127.0.0.1:2405".parse().expect("other address");

        assert!(targets_conflict(address, Some(1), address, Some(1)));
        assert!(targets_conflict(address, None, address, Some(1)));
        assert!(!targets_conflict(address, Some(1), address, Some(2)));
        assert!(!targets_conflict(address, Some(1), other_address, Some(1)));
    }
}

/// 已准备的主站连接
///
/// 封装了连接准备阶段的输出结果,包含连接 ID、TCP 客户端、
/// 协议处理器和链路参数。
pub(crate) struct PreparedMasterConnection {
    /// 连接 ID（UUID 格式）
    pub(crate) connection_id: String,
    /// TCP 客户端实例
    pub(crate) client: Arc<TcpClient>,
    /// IEC104 协议处理器
    pub(crate) protocol: Arc<Mutex<Iec104Protocol>>,
    /// 链路参数（t0/t1/t2/t3/k/w 等）
    pub(crate) link_params: LinkParams,
}

/// 准备连接
///
/// 建立到从站的 TCP 连接,初始化协议处理器,可选自动启动数据传输。
/// 这是连接生命周期的第一步,完成后需要调用 `register_monitor_task()` 启动监控任务。
///
/// # 参数
///
/// * `runtime` - 生命周期运行时
/// * `station_id` - 主站 ID
/// * `station_name` - 主站名称
/// * `slave_addr` - 从站地址（IP:Port）
/// * `profile_id` - 可选的配置文件 ID（用于数据库关联）
/// * `link_params` - 可选的链路参数（如果为 None 则使用默认值）
/// * `auto_start_data_transfer` - 是否自动发送 STARTDT_ACT
/// * `auto_reconnect` - 是否启用自动重连
/// * `retry_count` - 重连重试次数
/// * `retry_interval_seconds` - 重连间隔（秒）
///
/// # 返回
///
/// - `Ok(PreparedMasterConnection)` - 返回已准备的连接信息
/// - `Err(AppError)` - 连接失败
///
/// # 执行流程
///
/// 1. **状态检查**：验证主站是否处于 Running 状态
/// 2. **重复连接检测**：检查是否已存在到该从站地址的连接
/// 3. **生成连接 ID**：生成唯一的连接 ID（格式: `conn_{UUID}`）
/// 4. **配置网络参数**：设置自动重连参数（重试次数、重试间隔）
/// 5. **建立 TCP 连接**：通过 TcpClient 连接到从站
/// 6. **初始化协议**：创建 Iec104Protocol 实例并配置链路参数（k/w/max_asdu_len）
/// 7. **可选发送 STARTDT**：如果 auto_start_data_transfer=true,生成并发送 STARTDT_ACT 帧
/// 8. **创建连接信息**：构建 ConnectionInfo 结构体,记录连接状态和统计信息
/// 9. **注册运行时资源**：将连接信息、TCP 客户端、协议处理器注册到映射表
/// 10. **记录报文**：记录 STARTDT 或连接建立事件到报文缓存
/// 11. **推送事件**：向前端推送 "connection-opened" 事件
/// 12. **存储配置关联**：存储连接 ID 到配置文件 ID 的映射
///
/// # 错误处理
///
/// - 主站未启动：返回 ConfigError
/// - 重复连接：返回 ProtocolError::InvalidData
/// - TCP 连接失败：返回 NetworkError
/// - 协议初始化失败：返回 ProtocolError
///
/// # 线程安全
///
/// 本函数是异步的,所有共享状态访问都通过 RwLock/Mutex 保护,支持并发调用。
pub(crate) async fn prepare_connection(
    runtime: &MasterLifecycleRuntime,
    station_id: &str,
    station_name: &str,
    slave_addr: SocketAddr,
    profile_id: Option<i64>,
    link_params: Option<LinkParams>,
    auto_start_data_transfer: bool,
    auto_reconnect: bool,
    retry_count: u32,
    retry_interval_seconds: u32,
) -> AppResult<PreparedMasterConnection> {
    info!("主站 {} 连接到从站: {}", station_name, slave_addr);

    let status = *runtime.status.read().await;
    if status != StationStatus::Running {
        return Err(crate::errors::AppError::Config(crate::errors::ConfigError::Other(
            "主站未启动，无法连接到从站".to_string(),
        )));
    }

    {
        let connections = runtime.connections.read().await;
        if let Some(existing) = connections.values().find(|conn| {
            conn.remote_addr == slave_addr
                && (conn.profile_id == profile_id || conn.profile_id.is_none() || profile_id.is_none())
        }) {
            return Err(AppError::Protocol(ProtocolError::InvalidData(format!(
                "重复连接被拒绝: {} 已存在活动会话 {}",
                slave_addr, existing.id
            ))));
        }
    }

    let connection_id = format!("conn_{}", uuid::Uuid::new_v4());
    let mut network_config = NetworkConfig::default();
    network_config.max_reconnect_attempts = if auto_reconnect { retry_count } else { 0 };
    network_config.reconnect_interval = u64::from(retry_interval_seconds.max(1)) * 1000;
    let client = Arc::new(TcpClient::new(network_config));
    client.connect(&slave_addr.to_string()).await?;

    let link_params = link_params.unwrap_or_default();
    let protocol = Arc::new(Mutex::new(Iec104Protocol::new()));
    let start_frame = {
        let mut protocol = protocol.lock().await;
        protocol.set_max_unconfirmed(link_params.k_value);
        protocol.set_max_asdu_len(link_params.max_asdu_bytes as usize);
        protocol.set_state(ProtocolState::Connected);
        if auto_start_data_transfer { protocol.begin_data_transfer().map(Some) } else { Ok(None) }
    };
    let start_frame = match start_frame {
        Ok(frame) => frame,
        Err(error) => {
            let _ = client.disconnect().await;
            return Err(error.into());
        }
    };
    if let Some(frame) = start_frame.as_ref() {
        if let Err(error) = client.send(frame).await {
            let _ = client.disconnect().await;
            return Err(error.into());
        }
    }

    let connection = ConnectionInfo {
        id: connection_id.clone(),
        station_id: station_id.to_string(),
        profile_id,
        remote_addr: slave_addr,
        transport_state: TransportState::Connected,
        data_transfer_state: if auto_start_data_transfer {
            DataTransferState::Starting
        } else {
            DataTransferState::Stopped
        },
        reconnecting: false,
        reconnect_attempts: 0,
        max_reconnect_attempts: if auto_reconnect { retry_count } else { 0 },
        connected_at: Some(Utc::now()),
        last_clock_sync_at: None,
        tx_apdu_count: if auto_start_data_transfer { 1 } else { 0 },
        rx_apdu_count: 0,
        tx_bytes: start_frame.as_ref().map(|frame| frame.len() as u64).unwrap_or(0),
        rx_bytes: 0,
        retransmit_count: 0,
        unknown_typeid_count: 0,
        dropped_frame_count: 0,
        soe_queue_peak: 0,
        send_window_used: 0,
        outbound_queue_len: 0,
        queue_wait_ms: 0,
        coalesced_updates: 0,
        soe_backlog: 0,
        interrogation_active: false,
        window_full_avoided_count: 0,
    };

    insert_connection_runtime(
        &runtime.connections,
        &runtime.clients,
        &runtime.protocols,
        connection,
        client.clone(),
        protocol.clone(),
    )
    .await;

    if let Some(frame) = start_frame.as_ref() {
        append_master_message(
            &runtime.message_records,
            &runtime.next_message_id,
            MessageDirection::Sent,
            &connection_id,
            format!("发送 STARTDT_ACT 至 {}", slave_addr),
            Some(bytes_to_hex(frame)),
            None,
            None,
            None,
        )
        .await;
    } else {
        append_master_message(
            &runtime.message_records,
            &runtime.next_message_id,
            MessageDirection::Received,
            &connection_id,
            format!("TCP连接已建立，等待手动 STARTDT: {}", slave_addr),
            None,
            None,
            None,
            None,
        )
        .await;
    }
    emit_master_runtime_hint(&runtime.runtime_event_handle, station_id, Some(&connection_id), "connection-opened")
        .await;
    store_connection_profile_id(&runtime.connection_profile_map, &connection_id, profile_id).await;

    Ok(PreparedMasterConnection { connection_id, client, protocol, link_params })
}

/// 注册监控任务
///
/// 将连接的后台监控任务句柄注册到任务映射表中。
/// 通常在连接准备完成后调用,用于启动数据收发和超时检测。
///
/// # 参数
///
/// * `runtime` - 生命周期运行时
/// * `connection_id` - 连接 ID
/// * `task_handle` - 后台任务句柄（JoinHandle）
///
/// # 线程安全
///
/// 本函数通过 Mutex 保护共享状态,支持并发调用。
pub(crate) async fn register_monitor_task(
    runtime: &MasterLifecycleRuntime,
    connection_id: String,
    task_handle: JoinHandle<()>,
) {
    register_connection_task(&runtime.connection_tasks, connection_id, task_handle).await;
}

/// 清理连接任务
///
/// 清理连接的运行时资源和待完成命令,通常在后台监控任务结束时调用。
/// 不会关闭 TCP 连接,只清理内存中的资源。
///
/// # 参数
///
/// * `runtime` - 生命周期运行时
/// * `station_id` - 主站 ID
/// * `connection_id` - 连接 ID
///
/// # 执行流程
///
/// 1. 清理连接的运行时资源（连接信息、TCP 客户端、协议处理器、文件传输会话）
/// 2. 移除连接的所有待完成命令
/// 3. 推送 "task-ended" 事件到前端
/// 4. 移除后台任务句柄
///
/// # 线程安全
///
/// 本函数是异步的,所有共享状态访问都通过 RwLock/Mutex 保护,支持并发调用。
pub(crate) async fn cleanup_connection_task(runtime: &MasterLifecycleRuntime, station_id: &str, connection_id: &str) {
    clear_connection_lifecycle_runtime(runtime, connection_id).await;
    remove_pending_commands_for_connection(&runtime.pending_commands, connection_id).await;
    emit_master_runtime_hint(&runtime.runtime_event_handle, station_id, Some(connection_id), "task-ended").await;
    take_connection_task(&runtime.connection_tasks, connection_id).await;
}

/// 断开连接
///
/// 主动断开到从站的连接,关闭 TCP 连接并清理所有相关资源。
/// 这是连接生命周期的最后一步。
///
/// # 参数
///
/// * `runtime` - 生命周期运行时
/// * `station_id` - 主站 ID
/// * `station_name` - 主站名称
/// * `connection_id` - 连接 ID
///
/// # 返回
///
/// - `Ok(())` - 断开成功
/// - `Err(AppError)` - 断开失败（实际上总是返回 Ok）
///
/// # 执行流程
///
/// 1. **取消后台任务**：从任务映射表中移除并中止后台监控任务
/// 2. **清理运行时资源**：清理连接信息、TCP 客户端、协议处理器、文件传输会话
/// 3. **关闭 TCP 连接**：调用 TcpClient 的 disconnect() 方法关闭连接
/// 4. **移除待完成命令**：清理该连接的所有待完成命令
/// 5. **推送事件**：向前端推送 "connection-removed" 事件
///
/// # 线程安全
///
/// 本函数是异步的,所有共享状态访问都通过 RwLock/Mutex 保护,支持并发调用。
pub(crate) async fn disconnect_connection(
    runtime: &MasterLifecycleRuntime,
    station_id: &str,
    station_name: &str,
    connection_id: &str,
) -> AppResult<()> {
    info!("主站 {} 断开连接: {}", station_name, connection_id);

    if let Some(handle) = take_connection_task(&runtime.connection_tasks, connection_id).await {
        handle.abort();
        let _ = handle.await;
    }

    if let Some(client) = clear_connection_lifecycle_runtime(runtime, connection_id).await {
        let _ = client.disconnect().await;
    }
    remove_pending_commands_for_connection(&runtime.pending_commands, connection_id).await;
    emit_master_runtime_hint(&runtime.runtime_event_handle, station_id, Some(connection_id), "connection-removed")
        .await;
    Ok(())
}

/// 清理连接生命周期运行时资源
///
/// 内部辅助函数,用于清理连接的所有运行时资源。
/// 包括连接信息、TCP 客户端、协议处理器、文件传输会话和配置文件映射。
///
/// # 参数
///
/// * `runtime` - 生命周期运行时
/// * `connection_id` - 连接 ID
///
/// # 返回
///
/// - `Some(Arc<TcpClient>)` - 返回被移除的 TCP 客户端（用于关闭连接）
/// - `None` - 连接不存在
///
/// # 线程安全
///
/// 本函数是异步的,所有共享状态访问都通过 RwLock 保护,支持并发调用。
async fn clear_connection_lifecycle_runtime(
    runtime: &MasterLifecycleRuntime,
    connection_id: &str,
) -> Option<Arc<TcpClient>> {
    let client = clear_connection_runtime(
        &runtime.connections,
        &runtime.clients,
        &runtime.protocols,
        &runtime.file_transfer_sessions,
        connection_id,
    )
    .await;
    remove_connection_profile_id(&runtime.connection_profile_map, connection_id).await;
    client
}
