//! 核心数据类型定义

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointSyncCursor {
    pub epoch: u64,
    pub revision: u64,
}

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolType {
    /// IEC 60870-5-104 协议
    Iec104,
}

/// 站点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationType {
    /// 主站（客户端）
    Master,
    /// 从站（服务器）
    Slave,
}

/// 站点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationStatus {
    /// 已停止
    Stopped,
    /// 启动中
    Starting,
    /// 运行中
    Running,
    /// 停止中
    Stopping,
    /// 错误
    Error,
}

/// IEC104 链路层参数
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkParams {
    /// 最大未确认 APDU 数
    pub k_value: u16,
    /// 确认窗口大小
    pub w_value: u16,
    /// 连接建立超时（秒）
    pub t0_seconds: u64,
    /// 发送或测试 APDU 超时（秒）
    pub t1_seconds: u64,
    /// 无数据报文 t2 < t1 确认超时（秒）
    pub t2_seconds: u64,
    /// 长期空闲测试帧超时（秒）
    pub t3_seconds: u64,
    /// ASDU 最大字节数
    #[serde(default = "default_max_asdu_bytes")]
    pub max_asdu_bytes: u16,
    /// 选择超时（秒）
    #[serde(default = "default_select_timeout_seconds")]
    pub select_timeout_seconds: u64,
    /// 是否启用 SQ=1 上送
    #[serde(default)]
    pub enable_sq1_upload: bool,
    /// 是否启用 SOE 事件记录
    #[serde(default)]
    pub enable_soe: bool,
    /// 未知类型标识是否返回否定确认
    #[serde(default = "default_unknown_typeid_negative_ack")]
    pub unknown_typeid_negative_ack: bool,
    /// 出站队列容量
    #[serde(default = "default_outbound_queue_capacity")]
    pub outbound_queue_capacity: usize,
    /// 自发上送刷新间隔（毫秒）
    #[serde(default = "default_spontaneous_flush_ms")]
    pub spontaneous_flush_ms: u64,
    /// 模拟模式是否只保留最新值
    #[serde(default = "default_true")]
    pub simulation_latest_only: bool,
}

/// 从站应用层策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ControlExecutionMode {
    Direct,
    #[default]
    Sbo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlaveStationPolicy {
    /// 选择超时（秒）
    #[serde(default = "default_select_timeout_seconds")]
    pub select_timeout_seconds: u64,
    /// 是否启用 SQ=1 上送
    #[serde(default)]
    pub enable_sq1_upload: bool,
    /// 是否启用 SOE 事件记录
    #[serde(default)]
    pub enable_soe: bool,
    #[serde(default)]
    pub control_execution_mode: ControlExecutionMode,
}

impl Default for SlaveStationPolicy {
    fn default() -> Self {
        Self {
            select_timeout_seconds: default_select_timeout_seconds(),
            enable_sq1_upload: false,
            enable_soe: false,
            control_execution_mode: ControlExecutionMode::Sbo,
        }
    }
}

/// 从站应用层策略覆盖项（None = 继承上层）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SlaveStationPolicyOverride {
    /// 选择超时（秒）
    #[serde(default)]
    pub select_timeout_seconds: Option<u64>,
    /// 是否启用 SQ=1 上送
    #[serde(default)]
    pub enable_sq1_upload: Option<bool>,
    /// 是否启用 SOE 事件记录
    #[serde(default)]
    pub enable_soe: Option<bool>,
    #[serde(default)]
    pub control_execution_mode: Option<ControlExecutionMode>,
}

impl SlaveStationPolicyOverride {
    pub fn is_empty(&self) -> bool {
        self.select_timeout_seconds.is_none()
            && self.enable_sq1_upload.is_none()
            && self.enable_soe.is_none()
            && self.control_execution_mode.is_none()
    }
}

impl Default for LinkParams {
    fn default() -> Self {
        Self {
            k_value: 12,
            w_value: 8,
            t0_seconds: 30,
            t1_seconds: 15,
            t2_seconds: 10,
            t3_seconds: 20,
            max_asdu_bytes: default_max_asdu_bytes(),
            select_timeout_seconds: default_select_timeout_seconds(),
            enable_sq1_upload: false,
            enable_soe: false,
            unknown_typeid_negative_ack: default_unknown_typeid_negative_ack(),
            outbound_queue_capacity: default_outbound_queue_capacity(),
            spontaneous_flush_ms: default_spontaneous_flush_ms(),
            simulation_latest_only: default_true(),
        }
    }
}

fn default_max_asdu_bytes() -> u16 {
    249
}

fn default_select_timeout_seconds() -> u64 {
    15
}

fn default_unknown_typeid_negative_ack() -> bool {
    true
}

fn default_outbound_queue_capacity() -> usize {
    2048
}

fn default_spontaneous_flush_ms() -> u64 {
    15
}

fn default_true() -> bool {
    true
}

/// 点表来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PointSource {
    /// 内置点表
    BuiltIn,
    /// 导入点表
    Imported,
    /// 手动创建
    #[default]
    Manual,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlQualifierMode {
    #[default]
    Standard,
    RawTest,
}

/// 点表定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointDef {
    /// 信息对象地址
    pub address: u32,
    /// 点名称
    pub name: String,
    /// 类型标识
    pub type_id: u8,
    /// 数据类型
    pub data_type: String,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
    /// 控制点地址
    #[serde(default)]
    pub control_ioa: Option<u32>,
    /// 总召唤组（1-16）；未配置时仅参与站召唤。
    #[serde(default)]
    pub gi_group: Option<u8>,
    /// 计数量召唤组（1-4）；未配置时仅参与全部累计量操作。
    #[serde(default)]
    pub counter_group: Option<u8>,
    /// 默认值
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
    /// 是否启用
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    /// 点表来源
    #[serde(default)]
    pub source: PointSource,
}

/// TCP 承载层状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportState {
    /// 已断开
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 错误
    Error,
}

/// IEC104 数据传输层状态（STARTDT/STOPDT）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataTransferState {
    /// 已停止
    Stopped,
    /// 启动中
    Starting,
    /// 已启动
    Started,
    /// 停止中
    Stopping,
    /// 超时
    Timeout,
    /// 错误
    Error,
}

/// 站点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationConfig {
    /// 站点 ID
    pub id: String,
    /// 站点名称
    pub name: String,
    /// 协议类型
    pub protocol: ProtocolType,
    /// 站点类型
    pub station_type: StationType,
    /// 地址
    pub address: SocketAddr,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    pub enabled: bool,
}

/// 连接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// 连接 ID
    pub id: String,
    /// 站点 ID
    pub station_id: String,
    /// 主站侧连接配置 ID（从站侧为 None）
    #[serde(default)]
    pub profile_id: Option<i64>,
    /// 远端地址
    pub remote_addr: SocketAddr,
    /// TCP 传输层状态
    pub transport_state: TransportState,
    /// 数据传输层状态
    pub data_transfer_state: DataTransferState,
    /// 是否正在重连
    #[serde(default)]
    pub reconnecting: bool,
    /// 重连尝试次数
    #[serde(default)]
    pub reconnect_attempts: u32,
    /// 最大重连次数
    #[serde(default)]
    pub max_reconnect_attempts: u32,
    /// 连接建立时间
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 最后时钟同步时间
    #[serde(default)]
    pub last_clock_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 发送 APDU 计数
    pub tx_apdu_count: u64,
    /// 接收 APDU 计数
    pub rx_apdu_count: u64,
    /// 发送字节数
    pub tx_bytes: u64,
    /// 接收字节数
    pub rx_bytes: u64,
    /// 重传计数
    #[serde(default)]
    pub retransmit_count: u64,
    /// 未知类型标识计数
    #[serde(default)]
    pub unknown_typeid_count: u64,
    /// 丢帧计数
    #[serde(default)]
    pub dropped_frame_count: u64,
    /// SOE 队列峰值
    #[serde(default)]
    pub soe_queue_peak: u64,
    /// 发送窗口使用量
    #[serde(default)]
    pub send_window_used: u64,
    /// 出站队列长度
    #[serde(default)]
    pub outbound_queue_len: u64,
    /// 队列等待时间（毫秒）
    #[serde(default)]
    pub queue_wait_ms: u64,
    /// 合并更新计数
    #[serde(default)]
    pub coalesced_updates: u64,
    /// SOE 积压数
    #[serde(default)]
    pub soe_backlog: u64,
    /// 是否正在总召
    #[serde(default)]
    pub interrogation_active: bool,
    /// 窗口满避免计数
    #[serde(default)]
    pub window_full_avoided_count: u64,
}

/// 报文方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageDirection {
    /// 发送
    Sent,
    /// 接收
    Received,
    /// 错误
    Error,
}

/// 主站通信报文记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterMessageRecord {
    /// 记录 ID
    pub id: u64,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 连接 ID
    pub connection_id: String,
    /// 报文方向
    pub direction: MessageDirection,
    /// 报文内容
    pub content: String,
    /// 十六进制数据
    pub hex_data: Option<String>,
    /// 类型标识
    pub type_id: Option<u8>,
    /// 传输原因
    pub cause: Option<u16>,
    /// 公共地址
    pub common_address: Option<u16>,
    /// 命令跟踪 ID
    pub command_trace_id: Option<String>,
    /// 命令状态
    pub command_state: Option<String>,
}

/// 控制超时类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ControlTimeoutType {
    /// 选择超时
    #[default]
    Select,
    /// 执行超时
    Execute,
}

/// 控制状态类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ControlStatusKind {
    /// 空闲
    #[default]
    Idle,
    /// 已选择
    Selected,
    /// 执行中
    Executing,
    /// 成功
    Ok,
    /// 失败
    Ng,
    /// 超时
    Timeout,
}

/// 控制状态快照
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlStatusSnapshot {
    /// 状态
    pub state: ControlStatusKind,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 最新传输原因
    #[serde(default)]
    pub latest_cause: Option<u16>,
    /// ACTCON 是否为否定确认
    #[serde(default)]
    pub actcon_negative: Option<bool>,
    /// ACTCON 时间
    #[serde(default)]
    pub actcon_at: Option<chrono::DateTime<chrono::Utc>>,
    /// ACTTERM 时间
    #[serde(default)]
    pub actterm_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 超时类型
    #[serde(default)]
    pub timeout_type: Option<ControlTimeoutType>,
    /// 选择/执行标志（S/E）
    #[serde(default)]
    pub select_execute: Option<bool>,
    /// 命令时间戳（CP56Time2a）
    #[serde(default)]
    pub command_cp56: Option<String>,
}

/// 主站运行时变更提示事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterRuntimeHintEvent {
    /// 站点 ID
    pub station_id: String,
    /// 连接 ID
    pub connection_id: Option<String>,
    /// 变更原因
    pub reason: String,
    /// 重连尝试次数
    #[serde(default)]
    pub attempt: Option<u32>,
    /// 最大重连次数
    #[serde(default)]
    pub max_attempts: Option<u32>,
    /// 数据点存储游标
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_cursor: Option<PointSyncCursor>,
}

/// 主站文件传输模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MasterFileTransferMode {
    /// 目录查询
    Directory,
    /// 下载
    Download,
    /// 上传
    Upload,
    /// 日志查询
    #[serde(rename = "query-log")]
    QueryLog,
}

/// 主站文件传输状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MasterFileTransferState {
    /// 空闲
    Idle,
    /// 运行中
    Running,
    /// 成功
    Success,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 主站文件传输运行阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterFileTransferStage {
    /// 会话已创建，尚未发送协议请求
    Preparing,
    /// 等待目录查询结果
    DirectoryListing,
    /// 等待日志查询结果
    QueryLogListing,
    /// 等待下载文件准备响应
    AwaitingDownloadFileReady,
    /// 等待下载节准备响应并接收该节数据
    ReceivingDownloadSection,
    /// 等待上传文件准备确认
    AwaitingUploadFileAck,
    /// 等待上传节准备确认
    AwaitingUploadSectionAck,
    /// 正在发送上传文件段
    SendingUploadData,
    /// 等待上传完成确认
    AwaitingUploadCompletionAck,
    /// 本地收尾阶段，不再接受协议记录
    Finalizing,
    /// 会话已进入终态
    Finished,
}

/// 主站远程目录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterRemoteDirectoryEntry {
    /// 信息对象地址
    pub ioa: u32,
    /// 文件名称标识
    pub nof: u16,
    /// 文件长度
    pub length: u32,
    /// 状态原始值
    pub status_raw: u8,
    /// 是否为文件
    pub is_file: bool,
    /// 是否为最后一个条目
    pub is_last: bool,
    /// 时间戳
    #[serde(default)]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// 主站文件传输单节容量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterFileTransferLimits {
    /// 单个文件段可承载的最大数据字节数
    pub segment_payload_max: u16,
    /// 单节最多 255 段可承载的文件字节数
    pub single_section_max_file_size: u32,
}

/// 主站文件传输会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterFileTransferSession {
    /// 会话 ID
    pub session_id: String,
    /// 站点 ID
    pub station_id: String,
    /// 连接 ID
    pub connection_id: String,
    /// 从站 ID
    pub slave_id: i64,
    /// 公共地址
    pub common_address: u16,
    /// 传输模式
    pub mode: MasterFileTransferMode,
    /// 传输状态
    pub state: MasterFileTransferState,
    /// 信息对象地址
    pub ioa: u32,
    /// 文件名称标识
    #[serde(default)]
    pub nof: Option<u16>,
    /// 本地路径
    #[serde(default)]
    pub local_path: Option<String>,
    /// 总字节数
    pub bytes_total: u64,
    /// 已完成字节数
    pub bytes_done: u64,
    /// 当前节
    pub current_section: u8,
    /// 当前段
    pub current_segment: u32,
    /// 失败原因
    #[serde(default)]
    pub failure_reason: Option<String>,
    /// 目录条目列表
    #[serde(default)]
    pub directory_entries: Vec<MasterRemoteDirectoryEntry>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 从站通信报文记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveMessageRecord {
    /// 记录 ID
    pub id: u64,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 连接 ID
    pub connection_id: String,
    /// 报文方向
    pub direction: MessageDirection,
    /// 报文内容
    pub content: String,
    /// 十六进制数据
    pub hex_data: Option<String>,
    /// 类型标识
    pub type_id: Option<u8>,
    /// 传输原因
    pub cause: Option<u16>,
    /// 公共地址
    pub common_address: Option<u16>,
}

/// 从站运行时变更提示事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveRuntimeHintEvent {
    /// 站点 ID
    pub station_id: String,
    /// 连接 ID
    pub connection_id: Option<String>,
    /// 变更原因
    pub reason: String,
    /// 数据点存储游标
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_cursor: Option<PointSyncCursor>,
}

/// SOE 事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSoeEvent {
    /// 记录 ID
    pub id: u64,
    /// 记录时间
    pub logged_at: chrono::DateTime<chrono::Utc>,
    /// 事件时间戳
    pub event_timestamp: chrono::DateTime<chrono::Utc>,
    /// 连接 ID
    pub connection_id: String,
    /// 从站 ID
    #[serde(default)]
    pub slave_id: Option<i64>,
    /// 点名称
    #[serde(default)]
    pub point_name: Option<String>,
    /// 公共地址
    pub common_address: u16,
    /// 信息对象地址
    pub ioa: u32,
    /// 类型标识
    pub type_id: u8,
    /// 传输原因
    pub cause: u16,
    /// 数值
    pub value: f64,
    /// 品质
    pub quality: u8,
    /// 通用品质字段
    #[serde(default)]
    pub quality_common: Option<DataPointQualityCommon>,
    /// 类型相关品质字段
    #[serde(default)]
    pub quality_detail: Option<DataPointQualityDetail>,
    /// 线路时标字段
    #[serde(default)]
    pub timestamp_detail: Option<DataPointTimestampDetail>,
    /// 业务可用性
    #[serde(default = "default_business_usable")]
    pub business_usable: bool,
}

/// 数据点品质通用字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DataPointQualityCommon {
    /// 无效（Invalid）
    pub iv: bool,
    /// 非当前值（Not Topical）
    pub nt: bool,
    /// 被替代（Substituted）
    pub sb: bool,
    /// 被闭锁（Blocked）
    pub bl: bool,
}

/// 数据点品质详细信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataPointQualityDetail {
    /// 单点信息品质（SIQ）
    Siq { raw: u8, spi: bool, iv: bool, nt: bool, sb: bool, bl: bool },
    /// 双点信息品质（DIQ）
    Diq { raw: u8, dpi: u8, iv: bool, nt: bool, sb: bool, bl: bool },
    /// 品质描述词（QDS）
    Qds {
        raw: u8,
        ov: bool,
        iv: bool,
        nt: bool,
        sb: bool,
        bl: bool,
        transient: Option<bool>,
        scd_status: Option<u16>,
        scd_change: Option<u16>,
    },
    /// 二进制计数器品质（BCR）
    Bcr { raw: u8, sq: u8, cy: bool, ca: bool, iv: bool },
    /// 无品质信息
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataPointTimestampDetail {
    Cp24 {
        raw: Vec<u8>,
        invalid: bool,
        resolved: Option<chrono::DateTime<chrono::Utc>>,
    },
    Cp56 {
        raw: Vec<u8>,
        invalid: bool,
        summer_time: bool,
        weekday: u8,
        resolved: Option<chrono::DateTime<chrono::Utc>>,
    },
}

fn default_business_usable() -> bool {
    true
}

pub fn is_quality_business_usable(type_id: u8, quality_raw: u8) -> bool {
    use crate::core::iec104_registry::{QualityModel, lookup_capability};

    match lookup_capability(type_id).map(|capability| capability.quality_model) {
        Some(QualityModel::None) | None => true,
        Some(QualityModel::Bcr) => (quality_raw & 0x80) == 0,
        Some(QualityModel::Siq | QualityModel::Diq | QualityModel::Qds | QualityModel::Qdp) => {
            (quality_raw & 0x90) == 0
        }
    }
}

pub fn sync_data_point_business_usable(point: &mut DataPoint) {
    point.business_usable = is_quality_business_usable(point.type_id, point.quality);
}

/// 数据点信息
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPoint {
    /// 连接 ID
    pub connection_id: String,
    /// 主站侧连接配置 ID
    #[serde(default)]
    pub link_profile_id: Option<i64>,
    /// 从站 ID
    #[serde(default, rename = "slave_id")]
    pub slave_id: Option<i64>,
    /// 公共地址
    #[serde(default)]
    pub common_address: Option<u16>,
    /// 信息对象地址
    pub address: u32,
    /// 点名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 控制点地址
    pub control_ioa: Option<u32>,
    /// 总召唤组（1-16）
    #[serde(default)]
    pub gi_group: Option<u8>,
    /// 计数量召唤组（1-4）
    #[serde(default)]
    pub counter_group: Option<u8>,
    /// 类型标识
    pub type_id: u8,
    /// 数据类型
    pub data_type: String,
    /// 数值
    pub value: f64,
    /// 品质
    pub quality: u8,
    /// 品质通用字段
    #[serde(default)]
    pub quality_common: Option<DataPointQualityCommon>,
    /// 品质详细信息
    #[serde(default)]
    pub quality_detail: Option<DataPointQualityDetail>,
    /// 业务可用性
    #[serde(default = "default_business_usable")]
    pub business_usable: bool,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 最新事件时间戳
    #[serde(default)]
    pub latest_event_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    /// 线路时标及其 IV/SU/DOW 语义
    #[serde(default)]
    pub timestamp_detail: Option<DataPointTimestampDetail>,
    /// 上报计数
    #[serde(default)]
    pub report_count: Option<u64>,
    /// 最新传输原因
    #[serde(default)]
    pub latest_cause: Option<u16>,
    /// 点表来源
    #[serde(default)]
    pub point_source: Option<PointSource>,
    /// 控制状态快照
    #[serde(default)]
    pub control_status_snapshot: Option<ControlStatusSnapshot>,
}

/// ASDU 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsduInfo {
    /// 类型标识
    pub type_id: u8,
    /// 传输原因
    pub cause: u16,
    /// 公共地址
    pub common_address: u16,
    /// 数据
    pub data: Vec<u8>,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
