//! 从站消息存储模块。
//!
//! 本模块负责从站的消息记录和 PCAP 抓包数据的存储管理，包括：
//!
//! - **消息记录**：存储解析后的 ASDU 消息记录（最多 5000 条）
//! - **PCAP 抓包**：存储原始网络数据包（最多 10000 条）
//! - **跟踪控制**：支持动态启用/禁用消息跟踪功能
//! - **容量限制**：自动淘汰旧记录以保持固定容量

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use tokio::sync::{Mutex, RwLock};

use crate::{
    core::{
        shared::capture_store::push_capped,
        types::{MessageDirection, SlaveMessageRecord},
    },
    utils::pcap::CapturePacket,
};

/// 从站消息记录的最大容量
pub(crate) const MAX_SLAVE_MESSAGE_RECORDS: usize = 5000;
/// 从站抓包数据的最大容量
pub(crate) const MAX_SLAVE_CAPTURE_PACKETS: usize = 10000;

/// 检查从站消息跟踪是否已启用
pub(crate) fn is_slave_message_tracking_enabled(flag: &Arc<AtomicBool>) -> bool {
    flag.load(Ordering::Relaxed)
}

/// 追加从站消息记录
///
/// 创建新的消息记录并添加到队列，超过容量限制时自动淘汰旧记录。
pub(crate) async fn append_slave_message(
    records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_id: &Arc<AtomicU64>,
    direction: MessageDirection,
    connection_id: &str,
    content: String,
    hex_data: Option<String>,
    type_id: Option<u8>,
    cause: Option<u16>,
    common_address: Option<u16>,
) {
    let record = SlaveMessageRecord {
        id: next_id.fetch_add(1, Ordering::Relaxed) + 1,
        timestamp: chrono::Utc::now(),
        connection_id: connection_id.to_string(),
        direction,
        content,
        hex_data,
        type_id,
        cause,
        common_address,
    };
    let mut guard = records.write().await;
    push_capped(&mut guard, record, MAX_SLAVE_MESSAGE_RECORDS);
}

/// 追加 PCAP 抓包数据
///
/// 将原始网络数据包添加到队列，超过容量限制时自动淘汰旧数据。
pub(crate) async fn append_capture_packet(records: &Arc<RwLock<VecDeque<CapturePacket>>>, packet: CapturePacket) {
    let mut guard = records.write().await;
    push_capped(&mut guard, packet, MAX_SLAVE_CAPTURE_PACKETS);
}

/// 追加受跟踪控制的从站消息记录
///
/// 仅在跟踪启用时追加消息记录，使用互斥锁保证跟踪状态的一致性。
pub(crate) async fn append_tracked_slave_message(
    tracking_guard: &Arc<Mutex<()>>,
    tracking_enabled: &Arc<AtomicBool>,
    records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_id: &Arc<AtomicU64>,
    direction: MessageDirection,
    connection_id: &str,
    content: String,
    hex_data: Option<String>,
    type_id: Option<u8>,
    cause: Option<u16>,
    common_address: Option<u16>,
) {
    let _tracking_guard = tracking_guard.lock().await;
    if !is_slave_message_tracking_enabled(tracking_enabled) {
        return;
    }

    append_slave_message(records, next_id, direction, connection_id, content, hex_data, type_id, cause, common_address)
        .await;
}

/// 追加受跟踪控制的 PCAP 抓包数据
///
/// 仅在跟踪启用时追加抓包数据，使用互斥锁保证跟踪状态的一致性。
pub(crate) async fn append_tracked_capture_packet(
    tracking_guard: &Arc<Mutex<()>>,
    tracking_enabled: &Arc<AtomicBool>,
    records: &Arc<RwLock<VecDeque<CapturePacket>>>,
    packet: CapturePacket,
) {
    let _tracking_guard = tracking_guard.lock().await;
    if !is_slave_message_tracking_enabled(tracking_enabled) {
        return;
    }

    append_capture_packet(records, packet).await;
}

/// 重置从站消息跟踪状态
///
/// 清空所有消息记录和抓包数据，重置消息 ID 计数器，并设置新的跟踪启用状态。
pub(crate) async fn reset_slave_message_tracking_state(
    tracking_guard: &Arc<Mutex<()>>,
    tracking_enabled: &Arc<AtomicBool>,
    enabled: bool,
    records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    next_id: &Arc<AtomicU64>,
) {
    let _tracking_guard = tracking_guard.lock().await;
    tracking_enabled.store(false, Ordering::Relaxed);
    records.write().await.clear();
    capture_packets.write().await.clear();
    next_id.store(0, Ordering::Relaxed);
    tracking_enabled.store(enabled, Ordering::Relaxed);
}
