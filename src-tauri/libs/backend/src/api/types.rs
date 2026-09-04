//! API 层类型定义模块
//!
//! 定义所有 Tauri
//! 命令接口使用的请求和响应类型，是前后端通信的契约层。包括通用响应类型、站点管理类型、主站/
//! 从站类型、点表导入类型、消息解析器类型、控制命令类型、模拟类型等。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::core::types::{
    LinkParams, MasterFileTransferLimits, MasterFileTransferMode, MasterFileTransferSession, MasterFileTransferState,
    MasterRemoteDirectoryEntry, PointDef, SlaveStationPolicy, SlaveStationPolicyOverride,
};
use crate::{
    core::{
        master::{Iec104Command, QualifierMode},
        slave::{InitializationProfile, SimulationProfile, SimulationScenario},
        types::StationStatus,
    },
    errors::Iec104ErrorCode,
};

/// 默认值辅助函数：返回 true（用于 serde 的 `#[serde(default = "default_true")]` 属性）
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiErrorCode {
    AppError,
    CommonAddressMismatch,
    ConflictAsduAlias,
    ConflictCoaDuplicate,
    ConflictIoaDuplicate,
    ConflictIoaExisting,
    ConnectionNotFound,
    DbReadFailed,
    DbTransactionFailed,
    DbWriteFailed,
    DuplicateConnection,
    EmptyPointTable,
    ExportDirectoryFailed,
    ExportWriteFailed,
    FileTooLarge,
    FileTransferBusy,
    ForeignKeyViolation,
    InternalError,
    InvalidAsduType,
    InvalidExportData,
    InvalidExportPath,
    InvalidLocale,
    NotConnected,
    PointTableImportFailed,
    ProfileNotFound,
    ProtocolState,
    SlaveNotFound,
    StationNameConflict,
    StationNotFound,
    StationStatusFailed,
    StationStopFailed,
    TargetUnsupportedControlMapping,
    UploadFileMissing,
    UserSettingParseFailed,
    ValidationError,
}

impl ApiErrorCode {
    pub fn from_code(code: &str) -> Self {
        match code {
            "APP_ERROR" => Self::AppError,
            "COMMON_ADDRESS_MISMATCH" => Self::CommonAddressMismatch,
            "CONFLICT_ASDU_ALIAS" => Self::ConflictAsduAlias,
            "CONFLICT_COA_DUPLICATE" => Self::ConflictCoaDuplicate,
            "CONFLICT_IOA_DUPLICATE" => Self::ConflictIoaDuplicate,
            "CONFLICT_IOA_EXISTING" => Self::ConflictIoaExisting,
            "CONNECTION_NOT_FOUND" => Self::ConnectionNotFound,
            "DB_READ_FAILED" => Self::DbReadFailed,
            "DB_TRANSACTION_FAILED" => Self::DbTransactionFailed,
            "DB_WRITE_FAILED" => Self::DbWriteFailed,
            "DUPLICATE_CONNECTION" => Self::DuplicateConnection,
            "EMPTY_POINT_TABLE" => Self::EmptyPointTable,
            "EXPORT_DIRECTORY_FAILED" => Self::ExportDirectoryFailed,
            "EXPORT_WRITE_FAILED" => Self::ExportWriteFailed,
            "FILE_TOO_LARGE" => Self::FileTooLarge,
            "FILE_TRANSFER_BUSY" => Self::FileTransferBusy,
            "FOREIGN_KEY_VIOLATION" => Self::ForeignKeyViolation,
            "INVALID_ASDU_TYPE" => Self::InvalidAsduType,
            "INVALID_EXPORT_DATA" => Self::InvalidExportData,
            "INVALID_EXPORT_PATH" => Self::InvalidExportPath,
            "INVALID_LOCALE" => Self::InvalidLocale,
            "NOT_CONNECTED" => Self::NotConnected,
            "POINT_TABLE_IMPORT_FAILED" => Self::PointTableImportFailed,
            "PROFILE_NOT_FOUND" => Self::ProfileNotFound,
            "PROTOCOL_STATE" => Self::ProtocolState,
            "SLAVE_NOT_FOUND" => Self::SlaveNotFound,
            "STATION_NAME_CONFLICT" => Self::StationNameConflict,
            "STATION_NOT_FOUND" => Self::StationNotFound,
            "STATION_STATUS_FAILED" => Self::StationStatusFailed,
            "STATION_STOP_FAILED" => Self::StationStopFailed,
            "TARGET_UNSUPPORTED_CONTROL_MAPPING" => Self::TargetUnsupportedControlMapping,
            "UPLOAD_FILE_MISSING" => Self::UploadFileMissing,
            "USER_SETTING_PARSE_FAILED" => Self::UserSettingParseFailed,
            "VALIDATION_ERROR" => Self::ValidationError,
            _ => Self::InternalError,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiError {
    pub code: ApiErrorCode,
    /// 错误消息
    pub message: String,
    /// 详细错误信息（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 参数化错误上下文（可选，供前端按错误码本地化渲染）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<String, serde_json::Value>>,
}

impl ApiError {
    pub fn app(message: impl Into<String>) -> Self {
        Self::new("APP_ERROR", message)
    }

    pub fn new(code: impl AsRef<str>, message: impl Into<String>) -> Self {
        Self { code: ApiErrorCode::from_code(code.as_ref()), message: message.into(), detail: None, params: None }
    }

    pub fn with_params(
        code: impl AsRef<str>,
        message: impl Into<String>,
        params: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            code: ApiErrorCode::from_code(code.as_ref()),
            message: message.into(),
            detail: None,
            params: if params.is_empty() { None } else { Some(params) },
        }
    }
}

pub type CommandResult<T> = Result<T, ApiError>;

/// 导出抓包文件响应（PCAP/PCAPNG）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportCaptureResponse {
    /// 文件名
    pub file_name: String,
    /// MIME 类型
    pub mime_type: String,
    /// Base64 编码的文件数据
    pub data_base64: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PointTableTemplateExportResponse {
    pub file_name: String,
    pub mime_type: String,
    pub data_base64: String,
}

/// 站点协议类型
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StationProtocol {
    /// IEC 60870-5-104 协议
    Iec104,
    /// 不支持的协议
    #[serde(other)]
    Unsupported,
}

/// 从站侧冗余组模式（仅用于配置标识，不直接影响运行时行为）
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RedundancyGroupMode {
    /// 单连接模式
    Single,
    /// 连接级冗余
    Connection,
    /// 多从站冗余
    Multi,
}

/// 站点创建请求
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateStationRequest {
    /// 站点名称
    pub name: String,
    /// 协议类型
    pub protocol: StationProtocol,
    /// 站点类型（"master" 或 "slave"）
    pub station_type: String,
    /// 监听/连接地址
    pub host: String,
    /// 监听/连接端口
    pub port: u16,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    pub enabled: bool,
    /// 点表模板（可选）
    #[serde(default)]
    pub point_template: Option<StationPointTemplate>,
}

/// 点表模板对象
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StationPointTemplateObject {
    /// 对象名称
    pub name: String,
    /// ASDU 类型标识
    pub asdu_type: String,
    /// 起始信息对象地址
    pub ioa_start: u32,
    /// 信息对象数量
    pub ioa_count: u16,
}

/// 点表模板（从站创建时的点表初始化模板）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StationPointTemplate {
    /// 是否使用默认模板
    #[serde(default)]
    pub use_default_template: bool,
    /// 自定义对象列表
    #[serde(default)]
    pub objects: Vec<StationPointTemplateObject>,
}

/// 站点配置响应
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StationConfigResponse {
    /// 站点唯一标识符
    pub station_id: String,
    /// 站点名称
    pub name: String,
    /// 协议类型
    pub protocol: StationProtocol,
    /// 站点类型
    pub station_type: String,
    /// 监听/连接地址
    pub host: String,
    /// 监听/连接端口
    pub port: u16,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    pub enabled: bool,
}

/// 站点列表摘要（包含配置信息和运行状态）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StationSummary {
    /// 站点唯一标识符
    pub station_id: String,
    /// 站点名称
    pub name: String,
    /// 站点类型
    pub station_type: String,
    /// 监听/连接地址
    pub host: String,
    /// 监听/连接端口
    pub port: u16,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用（配置）
    pub enabled: bool,
    /// 运行状态（运行时）
    pub status: StationStatus,
}

/// 连接请求（主站临时连接，不保存配置）
#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    /// 从站地址
    pub host: String,
    /// 从站端口
    pub port: u16,
    /// 连接配置 ID（可选）
    #[serde(default)]
    pub profile_id: Option<i64>,
    /// 是否自动启动数据传输
    #[serde(default)]
    pub auto_start_data_transfer: Option<bool>,
}

/// 主站侧：链路配置创建/更新请求（多公共地址模式，不包含公共地址，支持多个从站）
#[derive(Debug, Deserialize, Clone)]
pub struct UpsertLinkProfileRequest {
    /// 配置名称
    pub name: String,
    /// 从站地址
    pub host: String,
    /// 从站端口
    pub port: u16,
    /// 链路参数
    #[serde(default)]
    pub link_params: LinkParams,
    /// 是否自动连接
    #[serde(default)]
    pub auto_connect: bool,
    /// 是否自动启动数据传输
    #[serde(default = "default_true")]
    pub auto_start_data_transfer: bool,
    /// 是否自动总召唤
    #[serde(default)]
    pub auto_gi: bool,
    /// 重连次数
    #[serde(default)]
    pub retry_count: u32,
    /// 重连间隔（秒）
    #[serde(default)]
    pub retry_interval: u32,
}

/// 主站侧：链路从站创建/更新请求
#[derive(Debug, Deserialize, Clone)]
pub struct UpsertLinkSlaveRequest {
    /// 从站名称
    pub name: String,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 主站侧：从站 + 点表批量提交请求
#[derive(Debug, Deserialize, Clone)]
pub struct UpsertLinkSlaveBundleRequest {
    /// 从站配置
    pub slave: UpsertLinkSlaveRequest,
    /// 点表定义列表
    #[serde(default)]
    pub point_defs: Vec<PointDef>,
}

/// 主站侧：从站 + 点表批量提交响应
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpsertLinkSlaveBundleResponse {
    /// 从站数据库 ID
    pub slave_id: i64,
    /// 更新的点表数量
    pub point_defs_updated: usize,
}

/// 主站侧：链路配置响应
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LinkProfileResponse {
    /// 配置数据库 ID
    pub id: i64,
    /// 所属站点 ID
    pub station_id: String,
    /// 配置名称
    pub name: String,
    /// 从站地址
    pub host: String,
    /// 从站端口
    pub port: u16,
    /// 链路参数
    pub link_params: LinkParams,
    /// 是否自动连接
    pub auto_connect: bool,
    /// 是否自动启动数据传输
    pub auto_start_data_transfer: bool,
    /// 是否自动总召唤
    pub auto_gi: bool,
    /// 重连次数
    pub retry_count: u32,
    /// 重连间隔（秒）
    pub retry_interval: u32,
}

/// 主站侧：链路从站配置响应
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LinkSlaveResponse {
    /// 从站数据库 ID
    pub id: i64,
    /// 所属链路配置 ID
    pub connection_id: i64,
    /// 从站名称
    pub name: String,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    pub enabled: bool,
}

/// 从站侧：从站创建/更新请求
#[derive(Debug, Deserialize, Clone)]
pub struct UpsertSlaveRequest {
    /// 从站名称
    pub name: String,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 策略覆盖配置
    #[serde(default)]
    pub station_policy_override: SlaveStationPolicyOverride,
}

/// 从站侧：从站 + 点表批量提交请求
#[derive(Debug, Deserialize, Clone)]
pub struct UpsertSlaveBundleRequest {
    /// 从站配置
    pub slave: UpsertSlaveRequest,
    /// 点表定义列表
    #[serde(default)]
    pub point_defs: Vec<PointDef>,
}

/// 从站侧：从站 + 点表批量提交响应
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpsertSlaveBundleResponse {
    /// 从站数据库 ID
    pub slave_id: i64,
    /// 更新的点表数量
    pub point_defs_updated: usize,
}

/// 从站侧：从站配置响应（包括策略配置）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlaveResponse {
    /// 从站数据库 ID
    pub id: i64,
    /// 所属站点 ID
    pub station_id: i64,
    /// 从站名称
    pub name: String,
    /// 公共地址
    pub common_address: u16,
    /// 是否启用
    pub enabled: bool,
    /// 策略覆盖配置
    #[serde(default)]
    pub station_policy_override: SlaveStationPolicyOverride,
    /// 生效的策略配置
    #[serde(default)]
    pub effective_station_policy: SlaveStationPolicy,
}

/// 从站侧：监听端点配置更新请求
#[derive(Debug, Deserialize, Clone)]
pub struct UpdateSlaveStationConfigBundleRequest {
    /// 站点配置
    pub config: CreateStationRequest,
}

/// 从站侧：站点配置 + 点表批量更新响应
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateSlaveStationConfigBundleResponse {
    /// 更新后的站点配置
    pub station: StationConfigResponse,
    /// 更新的点表数量
    pub point_defs_updated: usize,
}

/// 点表导入目标（主站或从站的不同配置层级）
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PointTableImportTarget {
    /// 主站链路下的从站
    MasterLinkSlave {
        /// 从站数据库 ID
        slave_id: i64,
    },
    /// 从站站点下的从站
    SlaveStationSlave {
        /// 从站数据库 ID
        slave_id: i64,
    },
}

/// 点表导入格式
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PointTableImportFormat {
    /// XML 格式
    Xml,
    /// CSV 格式
    Csv,
    /// JSON 格式
    Json,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointTableImportMode {
    AppendOnly,
    ReplaceAll,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PointTableImportSourceCommonAddress {
    pub common_address: u16,
    pub location: String,
}

/// 点表导入问题
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PointTableImportIssue {
    /// 问题代码
    pub code: String,
    /// 问题描述
    pub message: String,
    /// 问题位置（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// 点表导入摘要
///
/// 点表导入预览的统计信息。
///
/// # 字段说明
///
/// - `total_points`: 文件中的总点数
/// - `normalized_points`: 规范化后的点数
/// - `preview_points`: 预览显示的点数
/// - `duplicate_address_count`: 重复地址数量
/// - `control_mapping_count`: 控制映射数量
/// - `asdu_alias_count`: ASDU 别名数量
/// - `existing_points_count`: 已存在的点数
/// - `error_count`: 错误数量
/// - `warning_count`: 警告数量
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PointTableImportSummary {
    /// 文件中的总点数
    pub total_points: usize,
    /// 规范化后的点数
    pub normalized_points: usize,
    /// 预览显示的点数
    pub preview_points: usize,
    /// 重复地址数量
    pub duplicate_address_count: usize,
    /// 控制映射数量
    pub control_mapping_count: usize,
    /// ASDU 别名数量
    pub asdu_alias_count: usize,
    /// 已存在的点数
    pub existing_points_count: usize,
    /// 错误数量
    pub error_count: usize,
    /// 警告数量
    pub warning_count: usize,
}

/// 点表导入规范化载荷
///
/// 包含规范化后的点表数据和 ASDU 别名映射。
///
/// # 字段说明
///
/// - `point_defs`: 规范化后的点表定义列表
/// - `asdu_aliases`: ASDU 类型别名映射（如 "M_SP" -> "M_SP_NA_1"）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PointTableImportNormalizedPayload {
    /// 规范化后的点表定义列表
    #[serde(default)]
    pub point_defs: Vec<PointDef>,
    /// ASDU 类型别名映射
    #[serde(default)]
    pub asdu_aliases: HashMap<String, String>,
    #[serde(default)]
    pub source_common_addresses: Vec<PointTableImportSourceCommonAddress>,
}

/// 点表导入预览
///
/// 点表导入前的预览信息，包含文件信息、统计、问题和预览数据。
///
/// # 字段说明
///
/// - `target`: 导入目标
/// - `file_path`: 文件路径
/// - `file_name`: 文件名
/// - `file_size`: 文件大小（字节）
/// - `format`: 文件格式
/// - `encoding`: 文件编码
/// - `can_apply`: 是否可以应用（无错误时为 true）
/// - `summary`: 统计摘要
/// - `errors`: 错误列表
/// - `warnings`: 警告列表
/// - `points_preview`: 点表预览（前 N 条）
/// - `asdu_aliases_preview`: ASDU 别名预览
/// - `normalized_payload`: 规范化后的完整载荷
///
/// # 使用场景
///
/// - 用户选择文件后，后端解析并返回预览
/// - 前端展示预览信息，用户确认后调用应用接口
/// - 如果有错误（`can_apply=false`），不允许应用
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PointTableImportPreview {
    /// 导入目标
    pub target: PointTableImportTarget,
    /// 文件路径
    pub file_path: String,
    /// 文件名
    pub file_name: String,
    /// 文件大小（字节）
    pub file_size: u64,
    /// 文件格式
    pub format: PointTableImportFormat,
    /// 文件编码
    pub encoding: String,
    /// 是否可以应用
    pub can_apply: bool,
    /// 统计摘要
    pub summary: PointTableImportSummary,
    /// 错误列表
    #[serde(default)]
    pub errors: Vec<PointTableImportIssue>,
    /// 警告列表
    #[serde(default)]
    pub warnings: Vec<PointTableImportIssue>,
    /// 点表预览（前 N 条）
    #[serde(default)]
    pub points_preview: Vec<PointDef>,
    /// ASDU 别名预览
    #[serde(default)]
    pub asdu_aliases_preview: HashMap<String, String>,
    /// 规范化后的完整载荷
    pub normalized_payload: PointTableImportNormalizedPayload,
}

/// 点表导入应用结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PointTableImportApplyResult {
    /// 导入目标
    pub target: PointTableImportTarget,
    /// 导入的点数
    pub imported_points: usize,
    /// 已存在的点数
    pub existing_points: usize,
    /// 更新的 ASDU 别名数量
    pub asdu_aliases_updated: usize,
    /// 是否已应用到运行时
    pub runtime_applied: bool,
    /// 已应用到运行时的数据点游标
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_point_cursor: Option<crate::core::types::PointSyncCursor>,
}

/// 消息详情解析视图模式
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MessageDetailParseViewMode {
    /// 表格模式
    Table,
    /// 树形模式
    Tree,
}

/// 消息解析器状态
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MessageParserStatus {
    /// 解析成功
    Success,
    /// 部分解析
    Partial,
    /// 解析失败
    Error,
}

/// 消息解析器输入类型
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MessageParserInputKind {
    /// 连续十六进制字符串
    HexContinuous,
    /// 空格分隔的十六进制字节
    HexByteTokens,
    /// 0x 前缀的十六进制字节
    HexPrefixedTokens,
    /// 转义字节序列
    EscapedBytes,
    /// 字节数组格式
    ByteArray,
}

/// 消息解析器节点色调
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageParserNodeTone {
    /// 中性
    Neutral,
    /// 警告
    Warning,
    /// 危险
    Danger,
}

/// 消息解析器字节范围（用于高亮显示）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MessageParserByteRange {
    /// 起始字节索引
    pub start: usize,
    /// 结束字节索引
    pub end: usize,
}

/// 消息解析器问题
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MessageParserIssue {
    /// 问题代码
    pub code: String,
    /// 问题描述
    pub message: String,
    /// 详细信息（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 问题相关的字节范围（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_range: Option<MessageParserByteRange>,
}

/// 消息解析器树形节点
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MessageParserTreeNode {
    /// 节点唯一标识符
    pub id: String,
    /// 节点标签
    pub label: String,
    /// 节点值（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// 节点摘要（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// 节点对应的字节范围（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_range: Option<MessageParserByteRange>,
    /// 节点色调（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<MessageParserNodeTone>,
    /// 是否默认展开
    #[serde(default)]
    pub default_expanded: bool,
    /// 子节点列表
    #[serde(default)]
    pub children: Vec<MessageParserTreeNode>,
}

/// 消息帧解析摘要
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageFrameParseSummary {
    /// 输入格式类型（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_kind: Option<MessageParserInputKind>,
    /// 输入格式标签（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_kind_label: Option<String>,
    /// 输入字节数
    pub input_byte_count: usize,
    /// 帧字节数
    pub frame_byte_count: usize,
    /// 额外字节数
    pub extra_byte_count: usize,
    /// 声明的帧字节数（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_frame_byte_count: Option<usize>,
    /// 输入十六进制字符串
    pub input_hex: String,
    /// 帧十六进制字符串
    pub frame_hex: String,
    /// 规范化后的字节数组
    #[serde(default)]
    pub normalized_bytes: Vec<u8>,
    /// 帧类型（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_type: Option<String>,
    /// 帧类型标签（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_type_label: Option<String>,
    /// APDU 长度（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apdu_length: Option<u8>,
    /// 发送序号（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_seq: Option<u16>,
    /// 接收序号（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recv_seq: Option<u16>,
    /// 类型标识（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u8>,
    /// 类型名称（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// 传输原因（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause_of_transmission: Option<u8>,
    /// 传输原因描述（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause_description: Option<String>,
    /// 公共地址（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_address: Option<u16>,
    /// 信息对象数量（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_count: Option<usize>,
}

/// 消息帧解析结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageFrameParseResult {
    /// 解析状态
    pub status: MessageParserStatus,
    /// 解析摘要
    pub summary: MessageFrameParseSummary,
    /// 警告列表
    #[serde(default)]
    pub warnings: Vec<MessageParserIssue>,
    /// 错误信息（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<MessageParserIssue>,
    /// 树形结构节点列表
    #[serde(default)]
    pub tree: Vec<MessageParserTreeNode>,
}

/// 解析消息帧请求
#[derive(Debug, Deserialize, Clone)]
pub struct ParseMessageFrameRequest {
    /// 输入文本
    pub input_text: String,
}

/// 统一 IEC104 命令请求
#[derive(Debug, Deserialize)]
pub struct Iec104CommandRequest {
    /// 运行时连接 ID
    pub connection_id: String,
    /// 从站数据库 ID
    pub slave_id: i64,
    /// 控制限定词校验模式
    #[serde(default)]
    pub qualifier_mode: QualifierMode,
    /// 命令体
    pub command: Iec104Command,
}

/// 主站文件传输请求（Type ID 120-127）
#[derive(Debug, Deserialize)]
pub struct MasterFileTransferRequest {
    /// 运行时连接 ID
    pub connection_id: String,
    /// 从站数据库 ID
    pub slave_id: i64,
    /// 文件传输模式
    pub mode: MasterFileTransferMode,
    /// 信息对象地址（可选）
    #[serde(default)]
    pub ioa: Option<u32>,
    /// 文件名或目录名（可选）
    #[serde(default)]
    pub nof: Option<u16>,
    /// 本地文件路径（可选）
    #[serde(default)]
    pub local_path: Option<String>,
    /// 查询起始时间（可选）
    #[serde(default)]
    pub query_start_time: Option<String>,
    /// 查询结束时间（可选）
    #[serde(default)]
    pub query_end_time: Option<String>,
}

/// 统一 IEC104 命令响应
#[derive(Debug, Serialize, Deserialize)]
pub struct Iec104CommandResult {
    /// 命令是否被接受
    pub accepted: bool,
    /// 跟踪 ID（可选）
    pub trace_id: Option<String>,
    /// 错误码（可选）
    pub error_code: Option<Iec104ErrorCode>,
    /// 错误消息（可选）
    pub error_message: Option<String>,
}

impl Iec104CommandResult {
    /// 创建接受响应
    pub fn accepted(trace_id: impl Into<String>) -> Self {
        Self { accepted: true, trace_id: Some(trace_id.into()), error_code: None, error_message: None }
    }

    /// 创建拒绝响应
    pub fn rejected(code: Iec104ErrorCode, error_message: impl Into<String>) -> Self {
        Self { accepted: false, trace_id: None, error_code: Some(code), error_message: Some(error_message.into()) }
    }
}

/// 数据点更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateDataPointRequest {
    /// 信息对象地址
    pub address: u32,
    /// 数据点值
    pub value: f64,
    /// 质量描述符
    pub quality: u8,
    /// 公共地址（可选）
    #[serde(default)]
    pub common_address: Option<u16>,
}

/// 模拟配置更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateSimulationProfileRequest {
    /// 模拟配置
    pub profile: SimulationProfile,
}

/// 初始化模板更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateInitializationProfileRequest {
    /// 初始化模板
    pub profile: InitializationProfile,
}

/// 场景触发请求
#[derive(Debug, Deserialize)]
pub struct TriggerSimulationScenarioRequest {
    /// 场景配置
    pub scenario: SimulationScenario,
}
