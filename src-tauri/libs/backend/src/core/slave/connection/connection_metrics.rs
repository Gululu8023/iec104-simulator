//! 从站连接指标跟踪模块。
//!
//! 本模块提供连接统计信息的更新和管理功能，包括：
//!
//! - **队列指标**：出站队列长度、队列等待时间
//! - **窗口指标**：发送窗口使用情况、窗口满避免计数
//! - **错误指标**：丢帧计数、重传计数、未知类型计数
//! - **运行时统计**：SOE 队列峰值、合并更新计数、总召唤状态
//!
//! 所有函数都是线程安全的，通过 RwLock 保护共享状态。

use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, RwLock};

use crate::{core::types::ConnectionInfo, network::Iec104Protocol};

/// 更新出站队列指标
///
/// 更新指定连接的出站队列长度和总召唤活动状态。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `peer_addr` - 对端地址
/// * `queue_len` - 当前队列长度
/// * `interrogation_active` - 总召唤是否活动
///
/// # 线程安全
///
/// 通过 RwLock 写锁保护，支持并发调用。
pub(crate) async fn update_outbound_queue_metrics(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
    queue_len: usize,
    interrogation_active: bool,
) {
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.outbound_queue_len = queue_len as u64;
        conn.interrogation_active = interrogation_active;
    }
}

/// 更新队列等待时间
///
/// 更新指定连接的队列等待时间（毫秒）。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `peer_addr` - 对端地址
/// * `queue_wait_ms` - 队列等待时间（毫秒）
///
/// # 线程安全
///
/// 通过 RwLock 写锁保护，支持并发调用。
pub(crate) async fn update_queue_wait_ms(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
    queue_wait_ms: u64,
) {
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.queue_wait_ms = queue_wait_ms;
    }
}

/// 增加窗口满避免计数
///
/// 当检测到发送窗口已满但通过队列机制避免了阻塞时，增加避免计数。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `peer_addr` - 对端地址
///
/// # 线程安全
///
/// 通过 RwLock 写锁保护，支持并发调用。使用 saturating_add 防止溢出。
pub(crate) async fn increment_window_full_avoided_count(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
) {
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.window_full_avoided_count = conn.window_full_avoided_count.saturating_add(1);
    }
}

/// 同步发送窗口使用情况
///
/// 从协议实例读取未确认帧数量，并更新到连接信息中。
///
/// # 参数
///
/// * `peer_addr` - 对端地址
/// * `protocol` - 协议实例（用于读取未确认帧数量）
/// * `connections` - 连接信息映射表
///
/// # 线程安全
///
/// 通过 Mutex 和 RwLock 保护，支持并发调用。
pub(crate) async fn sync_send_window_used(
    peer_addr: &str,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
) {
    let send_window_used = protocol.lock().await.unconfirmed_count() as u64;
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.send_window_used = send_window_used;
    }
}

/// 增加丢帧计数
///
/// 增加指定连接的丢帧计数，用于统计协议解析失败或其他原因导致的丢帧。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `peer_addr` - 对端地址
/// * `delta` - 增加的丢帧数量（0 则不执行任何操作）
///
/// # 线程安全
///
/// 通过 RwLock 写锁保护，支持并发调用。使用 saturating_add 防止溢出。
pub(crate) async fn increment_dropped_frame_count(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
    delta: u64,
) {
    if delta == 0 {
        return;
    }
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.dropped_frame_count = conn.dropped_frame_count.saturating_add(delta);
    }
}

/// 重置连接运行时统计
///
/// 将指定连接的所有运行时统计字段重置为初始值（0 或 false）。
/// 不影响连接的基本信息（地址、状态、收发字节数等）。
///
/// # 参数
///
/// * `connections` - 连接信息映射表
/// * `peer_addr` - 对端地址
///
/// # 重置的字段
///
/// - `retransmit_count` - 重传计数
/// - `unknown_typeid_count` - 未知类型计数
/// - `dropped_frame_count` - 丢帧计数
/// - `soe_queue_peak` - SOE 队列峰值
/// - `send_window_used` - 发送窗口使用
/// - `outbound_queue_len` - 出站队列长度
/// - `queue_wait_ms` - 队列等待时间
/// - `coalesced_updates` - 合并更新计数
/// - `soe_backlog` - SOE 积压
/// - `interrogation_active` - 总召唤活动状态
/// - `window_full_avoided_count` - 窗口满避免计数
///
/// # 线程安全
///
/// 通过 RwLock 写锁保护，支持并发调用。
pub(crate) async fn reset_connection_runtime_stats(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
) {
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.retransmit_count = 0;
        conn.unknown_typeid_count = 0;
        conn.dropped_frame_count = 0;
        conn.soe_queue_peak = 0;
        conn.send_window_used = 0;
        conn.outbound_queue_len = 0;
        conn.queue_wait_ms = 0;
        conn.coalesced_updates = 0;
        conn.soe_backlog = 0;
        conn.interrogation_active = false;
        conn.window_full_avoided_count = 0;
    }
}
