//! 从站连接注册表模块。
//!
//! 本模块管理从站的连接注册表和协议实例，包括：
//!
//! - **选择-执行（SBO）管理**：跟踪待执行的选择命令及其超时
//! - **文件传输会话**：管理文件传输的状态和缓冲区
//! - **对端链路状态**：跟踪每个连接的链路层状态（重传、确认、测试等）
//! - **协议实例管理**：为每个连接创建和管理 IEC104 协议实例
//! - **连接生命周期**：注册、清理和更新连接信息

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::{Mutex, RwLock};

use crate::{
    core::{
        protocol_adapter::SetPointCommandValue,
        types::{ConnectionInfo, DataTransferState, TransportState},
    },
    network::Iec104Protocol,
};

/// 选择命令类型。
///
/// 用于 SBO（Select-Before-Operate）模式，标识待执行的选择命令类型和参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SelectCommandKind {
    /// 单点控制（C_SC_NA_1/C_SC_TA_1）
    Single { value: bool, qu: u8, timestamp: Option<[u8; 7]> },
    /// 双点控制（C_DC_NA_1/C_DC_TA_1）
    Double { value: u8, qu: u8, timestamp: Option<[u8; 7]> },
    /// 升降命令（C_RC_NA_1/C_RC_TA_1）
    RegulatingStep { step: u8, qu: u8, timestamp: Option<[u8; 7]> },
    /// 设点命令（C_SE_NA_1/C_SE_NB_1/C_SE_NC_1 等）
    SetPoint { type_id: u8, value: SetPointCommandValue, ql: u8, timestamp: Option<[u8; 7]> },
}

/// 待执行选择命令条目。
///
/// 存储选择命令的类型和过期时间，用于 SBO 模式的超时管理。
#[derive(Debug, Clone)]
struct PendingSelectEntry {
    /// 命令类型和参数
    kind: SelectCommandKind,
    /// 过期时间（超过此时间后选择命令失效）
    expires_at: Instant,
}

/// 从站文件传输模式。
///
/// 标识文件传输的方向（从站→主站 或 主站→从站）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlaveFileTransferMode {
    /// 下载到主站（从站→主站）
    DownloadToMaster,
    /// 从主站上传（主站→从站）
    UploadFromMaster,
}

/// 从站文件传输会话。
///
/// 管理单个文件传输的状态和缓冲区。
#[derive(Debug, Clone)]
pub(crate) struct SlaveFileTransferSession {
    /// 传输模式（下载/上传）
    pub(crate) mode: SlaveFileTransferMode,
    pub(crate) common_address: u16,
    pub(crate) ioa: u32,
    /// 文件名称（NOF）
    pub(crate) nof: u16,
    /// 预期文件长度（字节）
    pub(crate) expected_length: u32,
    /// 当前节号
    pub(crate) section: u8,
    /// 本地文件路径
    pub(crate) local_path: PathBuf,
    /// 传输缓冲区
    pub(crate) buffer: Vec<u8>,
    pub(crate) expires_at: Instant,
}

/// 对端链路状态。
///
/// 跟踪单个主站连接的链路层状态，包括超时、重传、SBO 和文件传输。
#[derive(Debug)]
pub(crate) struct PeerLinkState {
    /// 最后活动时间（用于 t3 空闲超时）
    pub(crate) last_activity: Instant,
    /// 待确认的测试帧发送时间（用于 t1 测试超时）
    pub(crate) pending_test: Option<Instant>,
    /// 未确认 I 帧的起始时间（用于 t1 重传超时）
    pub(crate) unacked_since: Option<Instant>,
    /// 重传尝试次数
    pub(crate) retransmit_attempts: u32,
    /// 对端最后确认的序号（用于检测确认进度）
    pub(crate) last_peer_ack_sequence: u16,
    /// 待发送 S 帧确认的 I 帧数量
    pub(crate) pending_ack_count: u16,
    /// 待确认的起始时间（用于 t2 延迟确认超时）
    pub(crate) pending_ack_since: Option<Instant>,
    /// 是否已尝试帧计数恢复（防止重复恢复）
    pub(crate) frame_count_recovery_attempted: bool,
    /// 待执行的选择命令映射表（key: (common_address, type_id, ioa)）
    pending_selects: HashMap<(u16, u8, u32), PendingSelectEntry>,
    /// 文件传输会话（如果正在进行文件传输）
    pub(crate) file_transfer: Option<SlaveFileTransferSession>,
}

impl PeerLinkState {
    /// 创建新的对端链路状态
    pub(crate) fn new() -> Self {
        Self {
            last_activity: Instant::now(),
            pending_test: None,
            unacked_since: None,
            retransmit_attempts: 0,
            last_peer_ack_sequence: 0,
            pending_ack_count: 0,
            pending_ack_since: None,
            frame_count_recovery_attempted: false,
            pending_selects: HashMap::new(),
            file_transfer: None,
        }
    }

    /// 清空所有待执行的选择命令
    pub(crate) fn clear_pending_selects(&mut self) {
        self.pending_selects.clear();
    }

    /// 清理已过期的选择命令
    pub(crate) fn cleanup_expired_selects(&mut self, now: Instant) {
        self.pending_selects.retain(|_, entry| entry.expires_at > now);
    }

    /// 注册选择命令并设置超时时间
    pub(crate) fn register_select_with_timeout(
        &mut self,
        common_address: u16,
        type_id: u8,
        ioa: u32,
        kind: SelectCommandKind,
        timeout_seconds: u64,
    ) {
        let expires_at = Instant::now() + Duration::from_secs(timeout_seconds.max(1));
        self.pending_selects.insert((common_address, type_id, ioa), PendingSelectEntry { kind, expires_at });
    }

    /// 消费选择命令（执行阶段）
    pub(crate) fn consume_select(
        &mut self,
        common_address: u16,
        type_id: u8,
        ioa: u32,
        kind: SelectCommandKind,
    ) -> bool {
        self.take_select_if_matches(common_address, type_id, ioa, kind)
    }

    /// 取消选择命令（取消阶段）
    pub(crate) fn cancel_select(
        &mut self,
        common_address: u16,
        type_id: u8,
        ioa: u32,
        kind: SelectCommandKind,
    ) -> bool {
        self.take_select_if_matches(common_address, type_id, ioa, kind)
    }

    /// 取出匹配的选择命令（内部方法）
    fn take_select_if_matches(&mut self, common_address: u16, type_id: u8, ioa: u32, kind: SelectCommandKind) -> bool {
        let now = Instant::now();
        self.cleanup_expired_selects(now);

        match self.pending_selects.get(&(common_address, type_id, ioa)) {
            Some(entry) if entry.kind == kind => {
                self.pending_selects.remove(&(common_address, type_id, ioa));
                true
            }
            _ => false,
        }
    }
}

/// 注册对端本地地址映射
///
/// 将对端地址与本地地址的映射关系存储到映射表中，用于 PCAP 抓包。
pub(crate) async fn register_peer_local_addr(
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    peer_addr: &str,
    local_addr: &str,
) {
    if let Ok(addr) = local_addr.parse::<std::net::SocketAddr>() {
        peer_local_addrs.write().await.insert(peer_addr.to_string(), addr);
    }
}

/// 插入已连接的对端信息
///
/// 在连接映射表中创建新的连接信息条目。
pub(crate) async fn insert_connected_peer(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    station_id: &str,
    peer_addr: &str,
) {
    let Ok(remote_addr) = peer_addr.parse::<std::net::SocketAddr>() else {
        return;
    };

    let conn = ConnectionInfo {
        id: peer_addr.to_string(),
        station_id: station_id.to_string(),
        profile_id: None,
        remote_addr,
        transport_state: TransportState::Connected,
        data_transfer_state: DataTransferState::Stopped,
        reconnecting: false,
        reconnect_attempts: 0,
        max_reconnect_attempts: 0,
        connected_at: None,
        last_clock_sync_at: None,
        tx_apdu_count: 0,
        rx_apdu_count: 0,
        tx_bytes: 0,
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
    connections.write().await.insert(peer_addr.to_string(), conn);
}

/// 确保对端协议实例存在
///
/// 如果对端协议实例不存在则创建，返回协议实例的引用。
pub(crate) async fn ensure_peer_protocol(
    protocols: &Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    peer_addr: &str,
) -> Arc<Mutex<Iec104Protocol>> {
    let mut guard = protocols.write().await;
    guard.entry(peer_addr.to_string()).or_insert_with(|| Arc::new(Mutex::new(Iec104Protocol::new()))).clone()
}

/// 清理对端连接运行时资源
///
/// 从所有映射表中移除对端的连接信息、地址映射和协议实例。
pub(crate) async fn clear_peer_connection_runtime(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    protocols: &Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    peer_addr: &str,
) {
    peer_local_addrs.write().await.remove(peer_addr);
    connections.write().await.remove(peer_addr);
    protocols.write().await.remove(peer_addr);
}

/// 增加所有对端的合并更新计数
///
/// 为所有已连接的对端增加合并更新计数器。
pub(crate) async fn increment_coalesced_updates_for_all_peers(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    delta: u64,
) {
    if delta == 0 {
        return;
    }
    let mut guard = connections.write().await;
    for conn in guard.values_mut() {
        conn.coalesced_updates = conn.coalesced_updates.saturating_add(delta);
    }
}

/// 更新所有对端的 SOE 积压值
///
/// 为所有已连接的对端设置相同的 SOE 积压值。
pub(crate) async fn update_soe_backlog_for_all_peers(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    backlog: u64,
) {
    let mut guard = connections.write().await;
    for conn in guard.values_mut() {
        conn.soe_backlog = backlog;
    }
}

/// 增加指定对端的未知类型计数
///
/// 为指定对端增加未知类型 ID 的计数器。
pub(crate) async fn increment_unknown_typeid_count(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
    delta: u64,
) {
    if delta == 0 {
        return;
    }
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.unknown_typeid_count = conn.unknown_typeid_count.saturating_add(delta);
    }
}

/// 增加指定对端的重传计数
///
/// 为指定对端增加重传帧的计数器。
pub(crate) async fn increment_retransmit_count(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
    delta: u64,
) {
    if delta == 0 {
        return;
    }
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.retransmit_count = conn.retransmit_count.saturating_add(delta);
    }
}

/// 更新指定对端的 SOE 队列峰值
///
/// 更新指定对端的 SOE 队列峰值和当前积压值。
pub(crate) async fn update_soe_queue_peak_for_peer(
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_addr: &str,
    queue_len_peak: usize,
) {
    if let Some(conn) = connections.write().await.get_mut(peer_addr) {
        conn.soe_queue_peak = conn.soe_queue_peak.max(queue_len_peak as u64);
        conn.soe_backlog = queue_len_peak as u64;
    }
}
