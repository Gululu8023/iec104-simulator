//! 连接注册表管理
//!
//! 本模块提供连接运行时资源的注册、查询和清理功能。
//! 负责维护连接 ID 到各种资源（TCP 客户端、协议处理器、后台任务等）的映射关系。
//!
//! # 核心功能
//!
//! - **资源注册**：将新建连接的资源注册到各个映射表
//! - **资源清理**：断开连接时清理所有相关资源
//! - **任务管理**：注册和移除连接的后台监控任务
//! - **配置关联**：维护连接 ID 到配置文件 ID 的映射
//!
//! # 资源映射
//!
//! 每个连接在运行时维护以下资源：
//! - **ConnectionInfo**：连接元信息（状态、统计等）
//! - **TcpClient**：TCP 客户端实例（用于网络发送）
//! - **Iec104Protocol**：协议处理器（用于帧编解码）
//! - **JoinHandle**：后台任务句柄（用于取消任务）
//! - **Profile ID**：配置文件 ID（用于数据库关联）
//! - **FileTransferSession**：文件传输会话（如果有）

use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    core::{master::file_transfer::store::MasterFileTransferSessionRuntime, types::ConnectionInfo},
    network::{Iec104Protocol, TcpClient},
};

/// 插入连接运行时资源
///
/// 将新建连接的所有运行时资源注册到各个映射表中。
/// 通常在连接成功建立后调用。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `clients` - TCP 客户端映射表
/// * `protocols` - 协议处理器映射表
/// * `connection` - 连接信息
/// * `client` - TCP 客户端实例
/// * `protocol` - 协议处理器实例
///
/// # 执行流程
///
/// 1. 提取连接 ID
/// 2. 将连接信息插入 connections 映射表
/// 3. 将 TCP 客户端插入 clients 映射表
/// 4. 将协议处理器插入 protocols 映射表
///
/// # 线程安全
///
/// 本函数通过 RwLock 保护共享状态,支持并发调用。
pub(crate) async fn insert_connection_runtime(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    clients: &Arc<RwLock<HashMap<String, Arc<TcpClient>>>>,
    protocols: &Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    connection: ConnectionInfo,
    client: Arc<TcpClient>,
    protocol: Arc<Mutex<Iec104Protocol>>,
) {
    let connection_id = connection.id.clone();
    connections.write().await.insert(connection_id.clone(), connection);
    clients.write().await.insert(connection_id.clone(), client);
    protocols.write().await.insert(connection_id, protocol);
}

/// 清理连接运行时资源
///
/// 从所有映射表中移除指定连接的资源,用于连接断开时的清理操作。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `clients` - TCP 客户端映射表
/// * `protocols` - 协议处理器映射表
/// * `file_transfer_sessions` - 文件传输会话映射表
/// * `connection_id` - 要清理的连接 ID
///
/// # 返回
///
/// - `Some(Arc<TcpClient>)` - 返回被移除的 TCP 客户端（用于关闭连接）
/// - `None` - 连接不存在
///
/// # 执行流程
///
/// 1. 从 clients 映射表中移除并获取 TCP 客户端
/// 2. 从 protocols 映射表中移除协议处理器
/// 3. 从 connections 映射表中移除连接信息
/// 4. 从 file_transfer_sessions 映射表中移除文件传输会话（如果有）
/// 5. 返回 TCP 客户端供调用者关闭连接
///
/// # 线程安全
///
/// 本函数通过 RwLock 保护共享状态,支持并发调用。
pub(crate) async fn clear_connection_runtime(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    clients: &Arc<RwLock<HashMap<String, Arc<TcpClient>>>>,
    protocols: &Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    file_transfer_sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
) -> Option<Arc<TcpClient>> {
    let client = clients.write().await.remove(connection_id);
    protocols.write().await.remove(connection_id);
    connections.write().await.remove(connection_id);
    file_transfer_sessions.write().await.remove(connection_id);
    client
}

/// 注册连接后台任务
///
/// 将连接的后台监控任务句柄注册到任务映射表中。
/// 用于后续取消任务或等待任务完成。
///
/// # 参数
///
/// * `connection_tasks` - 连接任务映射表
/// * `connection_id` - 连接 ID
/// * `task_handle` - 后台任务句柄（JoinHandle）
///
/// # 线程安全
///
/// 本函数通过 Mutex 保护共享状态,支持并发调用。
pub(crate) async fn register_connection_task(
    connection_tasks: &Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    connection_id: String,
    task_handle: JoinHandle<()>,
) {
    connection_tasks.lock().await.insert(connection_id, task_handle);
}

/// 移除连接后台任务
///
/// 从任务映射表中移除并返回指定连接的后台任务句柄。
/// 通常在连接断开时调用,用于取消或等待任务完成。
///
/// # 参数
///
/// * `connection_tasks` - 连接任务映射表
/// * `connection_id` - 连接 ID
///
/// # 返回
///
/// - `Some(JoinHandle<()>)` - 返回被移除的任务句柄
/// - `None` - 任务不存在
///
/// # 线程安全
///
/// 本函数通过 Mutex 保护共享状态,支持并发调用。
pub(crate) async fn take_connection_task(
    connection_tasks: &Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    connection_id: &str,
) -> Option<JoinHandle<()>> {
    connection_tasks.lock().await.remove(connection_id)
}

/// 存储连接的配置文件 ID
///
/// 将连接 ID 到配置文件 ID 的映射关系存储到映射表中。
/// 用于后续通过连接 ID 查询对应的配置文件 ID（用于数据库关联）。
///
/// # 参数
///
/// * `connection_profile_map` - 连接配置文件映射表
/// * `connection_id` - 连接 ID
/// * `profile_id` - 配置文件 ID（如果为 None 则不存储）
///
/// # 线程安全
///
/// 本函数通过 RwLock 保护共享状态,支持并发调用。
pub(crate) async fn store_connection_profile_id(
    connection_profile_map: &Arc<RwLock<HashMap<String, i64>>>,
    connection_id: &str,
    profile_id: Option<i64>,
) {
    if let Some(profile_id) = profile_id {
        connection_profile_map.write().await.insert(connection_id.to_string(), profile_id);
    }
}

/// 移除连接的配置文件 ID
///
/// 从映射表中移除指定连接的配置文件 ID 映射关系。
/// 通常在连接断开时调用。
///
/// # 参数
///
/// * `connection_profile_map` - 连接配置文件映射表
/// * `connection_id` - 连接 ID
///
/// # 返回
///
/// - `Some(i64)` - 返回被移除的配置文件 ID
/// - `None` - 映射关系不存在
///
/// # 线程安全
///
/// 本函数通过 RwLock 保护共享状态,支持并发调用。
pub(crate) async fn remove_connection_profile_id(
    connection_profile_map: &Arc<RwLock<HashMap<String, i64>>>,
    connection_id: &str,
) -> Option<i64> {
    connection_profile_map.write().await.remove(connection_id)
}

/// 尝试获取连接的配置文件 ID
///
/// 以非阻塞方式尝试从映射表中查询指定连接的配置文件 ID。
/// 如果映射表被其他线程锁定,则立即返回 None 而不等待。
///
/// # 参数
///
/// * `connection_profile_map` - 连接配置文件映射表
/// * `connection_id` - 连接 ID
///
/// # 返回
///
/// - `Some(i64)` - 返回配置文件 ID
/// - `None` - 映射关系不存在或映射表被锁定
///
/// # 线程安全
///
/// 本函数使用 try_read() 进行非阻塞读取,如果无法获取锁则立即返回 None。
/// 适用于不希望阻塞的场景（如命令分发时快速查询配置文件 ID）。
pub(crate) fn try_get_connection_profile_id(
    connection_profile_map: &Arc<RwLock<HashMap<String, i64>>>,
    connection_id: &str,
) -> Option<i64> {
    connection_profile_map.try_read().ok().and_then(|map| map.get(connection_id).copied())
}
