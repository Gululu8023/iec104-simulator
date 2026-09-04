//! 主站消息存储模块。
//!
//! 本模块负责主站的消息记录和 PCAP 抓包数据的存储管理,包括:
//!
//! - **消息记录**:存储解析后的 ASDU 消息记录(最多 5000 条)
//! - **PCAP 抓包**:存储原始网络数据包(最多 10000 条)
//! - **SOE 记录**:存储事件序列记录(最多 10000 条)
//! - **跟踪控制**:支持动态启用/禁用消息跟踪功能
//! - **容量限制**:自动淘汰旧记录以保持固定容量

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex as StdMutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use tokio::sync::{Mutex, RwLock};

use crate::{
    core::{
        shared::capture_store::push_capped,
        types::{BackendSoeEvent, MasterMessageRecord, MessageDirection},
    },
    utils::pcap::CapturePacket,
};

pub(crate) const MAX_MASTER_MESSAGE_RECORDS: usize = 5000;
pub(crate) const MAX_MASTER_CAPTURE_PACKETS: usize = 10000;
pub(crate) const MAX_MASTER_SOE_RECORDS: usize = 10000;

#[derive(Clone)]
struct MasterMessageTrackingState {
    enabled: Arc<AtomicBool>,
    guard: Arc<Mutex<()>>,
}

type MasterMessageTrackingRegistry = HashMap<usize, MasterMessageTrackingState>;

fn master_message_tracking_registry() -> &'static StdMutex<MasterMessageTrackingRegistry> {
    static REGISTRY: OnceLock<StdMutex<MasterMessageTrackingRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn master_capture_tracking_registry() -> &'static StdMutex<MasterMessageTrackingRegistry> {
    static REGISTRY: OnceLock<StdMutex<MasterMessageTrackingRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn message_records_key(records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>) -> usize {
    Arc::as_ptr(records) as usize
}

fn capture_packets_key(records: &Arc<RwLock<VecDeque<CapturePacket>>>) -> usize {
    Arc::as_ptr(records) as usize
}

fn resolve_master_message_tracking(
    records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>,
) -> Option<MasterMessageTrackingState> {
    master_message_tracking_registry()
        .lock()
        .expect("master message tracking registry lock")
        .get(&message_records_key(records))
        .cloned()
}

fn resolve_master_capture_tracking(
    records: &Arc<RwLock<VecDeque<CapturePacket>>>,
) -> Option<MasterMessageTrackingState> {
    master_capture_tracking_registry()
        .lock()
        .expect("master capture tracking registry lock")
        .get(&capture_packets_key(records))
        .cloned()
}

fn should_bypass_master_message_tracking(command_state: Option<&str>) -> bool {
    matches!(command_state, Some(state) if state != "dispatch")
}

pub(crate) fn is_master_message_tracking_enabled(flag: &Arc<AtomicBool>) -> bool {
    flag.load(Ordering::Relaxed)
}

pub(crate) fn register_master_message_tracking(
    message_records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    tracking_enabled: &Arc<AtomicBool>,
    tracking_guard: &Arc<Mutex<()>>,
) {
    let tracking_state =
        MasterMessageTrackingState { enabled: Arc::clone(tracking_enabled), guard: Arc::clone(tracking_guard) };
    master_message_tracking_registry()
        .lock()
        .expect("master message tracking registry lock")
        .insert(message_records_key(message_records), tracking_state.clone());
    master_capture_tracking_registry()
        .lock()
        .expect("master capture tracking registry lock")
        .insert(capture_packets_key(capture_packets), tracking_state);
}

pub(crate) async fn reset_master_message_tracking_state(
    tracking_guard: &Arc<Mutex<()>>,
    tracking_enabled: &Arc<AtomicBool>,
    enabled: bool,
    records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>,
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

pub(crate) async fn append_master_message(
    records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    next_id: &Arc<AtomicU64>,
    direction: MessageDirection,
    connection_id: &str,
    content: String,
    hex_data: Option<String>,
    type_id: Option<u8>,
    cause: Option<u16>,
    common_address: Option<u16>,
) {
    append_master_message_meta(
        records,
        next_id,
        direction,
        connection_id,
        content,
        hex_data,
        type_id,
        cause,
        common_address,
        None,
        None,
    )
    .await;
}

pub(crate) async fn append_master_message_meta(
    records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    next_id: &Arc<AtomicU64>,
    direction: MessageDirection,
    connection_id: &str,
    content: String,
    hex_data: Option<String>,
    type_id: Option<u8>,
    cause: Option<u16>,
    common_address: Option<u16>,
    command_trace_id: Option<String>,
    command_state: Option<String>,
) {
    if let Some(tracking_state) = resolve_master_message_tracking(records) {
        let _tracking_guard = tracking_state.guard.lock().await;
        let should_append = should_bypass_master_message_tracking(command_state.as_deref())
            || is_master_message_tracking_enabled(&tracking_state.enabled);
        if !should_append {
            return;
        }
    }

    let record = MasterMessageRecord {
        id: next_id.fetch_add(1, Ordering::Relaxed) + 1,
        timestamp: chrono::Utc::now(),
        connection_id: connection_id.to_string(),
        direction,
        content,
        hex_data,
        type_id,
        cause,
        common_address,
        command_trace_id,
        command_state,
    };

    let mut guard = records.write().await;
    push_capped(&mut guard, record, MAX_MASTER_MESSAGE_RECORDS);
}

pub(crate) async fn append_capture_packet(records: &Arc<RwLock<VecDeque<CapturePacket>>>, packet: CapturePacket) {
    if let Some(tracking_state) = resolve_master_capture_tracking(records) {
        let _tracking_guard = tracking_state.guard.lock().await;
        if !is_master_message_tracking_enabled(&tracking_state.enabled) {
            return;
        }
    }

    let mut guard = records.write().await;
    push_capped(&mut guard, packet, MAX_MASTER_CAPTURE_PACKETS);
}

pub(crate) async fn append_soe_records(
    records: &Arc<RwLock<VecDeque<BackendSoeEvent>>>,
    next_id: &Arc<AtomicU64>,
    events: &[BackendSoeEvent],
    max_records: usize,
) {
    if events.is_empty() {
        return;
    }

    let mut guard = records.write().await;
    for item in events {
        let mut event = item.clone();
        event.id = next_id.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        push_capped(&mut guard, event, max_records);
    }
}
