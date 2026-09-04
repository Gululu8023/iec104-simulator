//! 响应原因处理
//!
//! 本模块负责处理 IEC104 协议中的响应原因（Cause of Transmission, COT）和负确认判定逻辑。
//!
//! # 核心功能
//!
//! - **响应原因合并**：将请求的源地址（OA）与响应原因合并
//! - **负确认原因生成**：根据请求类型生成正确的负确认原因
//! - **负确认判定**：判断是否应该对特定错误发送负确认
//! - **控制请求识别**：识别控制类命令的传输原因
//!
//! # 传输原因（COT）结构
//!
//! IEC104 协议中的传输原因是一个 16 位字段，结构如下：
//!
//! ```text
//! 高字节（Byte 1）：
//! Bit 15-8: OA (Originator Address) - 源地址
//!
//! 低字节（Byte 0）：
//! Bit 7: T (Test) - 测试标志
//! Bit 6: P/N (Positive/Negative) - 肯定/否定标志
//!   - 0: 肯定确认
//!   - 1: 否定确认
//! Bit 5-0: COT (Cause of Transmission) - 传输原因
//! ```
//!
//! # 常用传输原因
//!
//! ## 控制命令相关
//!
//! - **6 (ACT)**：激活（Activation）- 主站发送控制命令
//! - **7 (ACT_CON)**：激活确认（Activation Confirmation）- 从站确认接收
//! - **7 | 0x40 (ACT_CON | P/N)**：激活否定确认 - 从站拒绝命令
//! - **8 (DEACT)**：去激活（Deactivation）- 主站取消 Select 命令
//! - **9 (DEACT_CON)**：去激活确认 - 从站确认取消
//! - **9 | 0x40 (DEACT_CON | P/N)**：去激活否定确认 - 从站拒绝取消
//! - **10 (ACT_TERM)**：激活终止（Activation Termination）- 从站完成命令执行
//!
//! ## 召唤相关
//!
//! - **20 (INTERROGATION)**：总召唤响应 - 从站上报数据点
//! - **37 (COUNTER_INTERROGATION)**：计数量召唤响应 - 从站上报累计量
//!
//! ## 其他
//!
//! - **3 (SPONTANEOUS)**：突发传输 - 从站主动上报变化
//! - **5 (REQUEST)**：请求响应 - 从站响应读命令
//! - **44 (UNKNOWN_TYPE_ID)**：未知类型标识
//! - **45 (UNKNOWN_COT)**：未知传输原因
//! - **46 (UNKNOWN_CA)**：未知公共地址
//! - **47 (UNKNOWN_IOA)**：未知信息对象地址
//!
//! # 负确认策略
//!
//! ## 何时发送负确认
//!
//! 从站在以下情况下应发送负确认（P/N=1）：
//!
//! 1. **未知类型标识**：收到不支持的 Type ID
//! 2. **解码错误**：ASDU 解码失败（无效限定词、无效值等）
//! 3. **目标不存在**：控制目标不存在或未映射
//! 4. **类型不匹配**：控制命令类型与目标类型不匹配
//! 5. **数据点被阻塞**：目标数据点的 BL 位为 1
//! 6. **Select 超时**：Execute 命令未在超时时间内到达
//!
//! ## 负确认原因选择
//!
//! - **ACT (6) → ACT_CON | P/N (0x47)**：激活命令被拒绝
//! - **DEACT (8) → DEACT_CON | P/N (0x49)**：去激活命令被拒绝
//!
//! # 源地址（OA）处理
//!
//! 源地址用于标识命令的发起者，从站在响应时必须保留请求中的源地址：
//!
//! ```text
//! 请求：OA=0x12, COT=6 (ACT)        → 0x1206
//! 响应：OA=0x12, COT=7 (ACT_CON)    → 0x1207
//! 否定：OA=0x12, COT=0x47 (ACT_CON|P/N) → 0x1247
//! ```
//!
//! # 使用示例
//!
//! ```rust
//! // 合并响应原因和源地址
//! let request_cause = 0x1206; // OA=0x12, COT=6 (ACT)
//! let response_cause = merge_response_cause_with_request_origin(request_cause, 7);
//! assert_eq!(response_cause, 0x1207); // OA=0x12, COT=7 (ACT_CON)
//!
//! // 生成负确认原因
//! let negative_cause = command_negative_ack_cause(request_cause);
//! assert_eq!(negative_cause, 0x1247); // OA=0x12, COT=0x47 (ACT_CON|P/N)
//!
//! // 判断是否应该发送负确认
//! if should_negative_ack_unknown_type(&asdu, None, true) {
//!     // 发送未知类型的负确认
//! }
//! ```
//!
//! # 线程安全
//!
//! 本模块中的所有函数都是纯函数，不涉及共享状态，可以安全地在多线程环境中调用。

use crate::{
    core::types::AsduInfo,
    errors::{Iec104ErrorCode, Iec104ValidationError},
    network::{DecodedAsdu, DecodedAsduBody},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponsePayloadPolicy {
    Provided,
    EchoRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseTimestampPolicy {
    None,
    Cp24,
    Cp56,
}

impl ResponseTimestampPolicy {
    fn for_type(type_id: u8) -> Self {
        match type_id {
            2 | 4 | 6 | 8 | 10 | 12 | 14 | 16..=19 => Self::Cp24,
            30..=40 | 58..=64 | 103 | 107 | 126 | 127 => Self::Cp56,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResponseEnvelope {
    pub(crate) asdu: AsduInfo,
    pub(crate) object_count: u8,
    pub(crate) sequence: bool,
    pub(crate) payload_policy: ResponsePayloadPolicy,
    pub(crate) timestamp_policy: ResponseTimestampPolicy,
}

impl ResponseEnvelope {
    pub(crate) fn provided_single(request_cause: u16, mut asdu: AsduInfo) -> Self {
        asdu.cause = merge_response_cause_with_request_origin(request_cause, asdu.cause);
        let timestamp_policy = ResponseTimestampPolicy::for_type(asdu.type_id);
        Self {
            asdu,
            object_count: 1,
            sequence: false,
            payload_policy: ResponsePayloadPolicy::Provided,
            timestamp_policy,
        }
    }

    pub(crate) fn echoed_request(request_cause: u16, mut asdu: AsduInfo, request_vsq: u8) -> Self {
        asdu.cause = merge_response_cause_with_request_origin(request_cause, asdu.cause);
        let timestamp_policy = ResponseTimestampPolicy::for_type(asdu.type_id);
        Self {
            asdu,
            object_count: request_vsq & 0x7F,
            sequence: (request_vsq & 0x80) != 0,
            payload_policy: ResponsePayloadPolicy::EchoRequest,
            timestamp_policy,
        }
    }
}

/// 合并响应原因和请求源地址
///
/// 将请求中的源地址（OA，高 8 位）与响应原因（COT，低 8 位）合并为完整的传输原因字段。
/// 这确保了响应报文中保留了请求的源地址，符合 IEC104 协议要求。
///
/// # 参数
///
/// * `request_cause` - 请求的传输原因（包含源地址）
/// * `response_cause` - 响应的传输原因（仅低 8 位有效）
///
/// # 返回
///
/// 合并后的传输原因（高 8 位为源地址，低 8 位为响应原因）
///
/// # 位操作说明
///
/// ```text
/// request_cause  = 0x1206 (OA=0x12, COT=6)
/// response_cause = 0x0007 (COT=7)
///
/// 步骤：
/// 1. request_cause & 0xFF00  = 0x1200 (保留源地址)
/// 2. response_cause & 0x00FF = 0x0007 (保留响应原因)
/// 3. 0x1200 | 0x0007         = 0x1207 (合并结果)
/// ```
///
/// # 示例
///
/// ```rust
/// let request_cause = 0x1206; // OA=0x12, COT=6 (ACT)
/// let response = merge_response_cause_with_request_origin(request_cause, 7);
/// assert_eq!(response, 0x1207); // OA=0x12, COT=7 (ACT_CON)
/// ```
#[inline]
pub(crate) fn merge_response_cause_with_request_origin(request_cause: u16, response_cause: u16) -> u16 {
    (request_cause & 0xFF80) | (response_cause & 0x007F)
}

/// 生成未知类型的负确认原因
///
/// 根据请求的传输原因生成对应的负确认原因。用于响应未知类型标识（Type ID）的命令。
///
/// # 参数
///
/// * `request_cause` - 请求的传输原因
///
/// # 返回
///
/// 负确认的传输原因（保留源地址，设置 P/N 位）
///
/// # 逻辑说明
///
/// 1. 提取请求的传输原因（低 6 位）
/// 2. 根据请求类型选择响应原因：
///    - 如果是 DEACT (8)，返回 DEACT_CON | P/N (0x49)
///    - 否则返回 ACT_CON | P/N (0x47)
/// 3. 合并源地址和响应原因
///
/// # 示例
///
/// ```rust
/// // 激活命令的负确认
/// let request_cause = 0x1206; // OA=0x12, COT=6 (ACT)
/// let negative = unknown_type_negative_ack_cause(request_cause);
/// assert_eq!(negative, 0x1247); // OA=0x12, COT=0x47 (ACT_CON|P/N)
///
/// // 去激活命令的负确认
/// let request_cause = 0x1208; // OA=0x12, COT=8 (DEACT)
/// let negative = unknown_type_negative_ack_cause(request_cause);
/// assert_eq!(negative, 0x1249); // OA=0x12, COT=0x49 (DEACT_CON|P/N)
/// ```
/// 生成命令的负确认原因
///
/// 根据请求的传输原因生成对应的负确认原因。用于响应控制命令执行失败的情况。
///
/// # 参数
///
/// * `request_cause` - 请求的传输原因
///
/// # 返回
///
/// 负确认的传输原因（保留源地址，设置 P/N 位）
///
/// # 逻辑说明
///
/// 1. 提取请求的传输原因（低 6 位）
/// 2. 根据请求类型选择响应原因：
///    - 如果是 DEACT (8)，返回 DEACT_CON | P/N (0x49)
///    - 否则返回 ACT_CON | P/N (0x47)
/// 3. 合并源地址和响应原因
///
/// # 与 unknown_type_negative_ack_cause 的区别
///
/// 虽然实现相同，但语义不同：
/// - `unknown_type_negative_ack_cause`：用于未知类型标识
/// - `command_negative_ack_cause`：用于命令执行失败（目标不存在、类型不匹配、被阻塞等）
///
/// # 示例
///
/// ```rust
/// // 控制命令执行失败
/// let request_cause = 0x1206; // OA=0x12, COT=6 (ACT)
/// let negative = command_negative_ack_cause(request_cause);
/// assert_eq!(negative, 0x1247); // OA=0x12, COT=0x47 (ACT_CON|P/N)
/// ```
#[inline]
pub(crate) fn command_negative_ack_cause(request_cause: u16) -> u16 {
    let request_cot = (request_cause as u8) & 0x3F;
    let low: u8 = if request_cot == 8 { 9 | 0x40 } else { 7 | 0x40 };
    merge_response_cause_with_request_origin(request_cause, u16::from(low))
}

pub(crate) fn decode_error_response_cause(request_cause: u16, decode_error: &Iec104ValidationError) -> u16 {
    let reason = match decode_error.code {
        Iec104ErrorCode::UnsupportedCommand => 44,
        Iec104ErrorCode::InvalidCommonAddress => 46,
        Iec104ErrorCode::InvalidObjectAddress => 47,
        Iec104ErrorCode::InvalidValue
            if decode_error.message.starts_with("unsupported command cause of transmission") =>
        {
            45
        }
        _ => return command_negative_ack_cause(request_cause),
    };
    merge_response_cause_with_request_origin(request_cause, reason)
}

/// 判断是否应该对未知类型发送负确认
///
/// 检查是否应该对未知类型标识（Type ID）的 ASDU 发送负确认响应。
///
/// # 参数
///
/// * `asdu` - 已解码的 ASDU
/// * `decode_error` - 解码错误（可选）
/// * `enabled` - 是否启用未知类型负确认功能
///
/// # 返回
///
/// - `true` - 应该发送负确认
/// - `false` - 不应该发送负确认
///
/// # 判定逻辑
///
/// 满足以下所有条件时返回 `true`：
///
/// 1. **功能已启用**：`enabled` 参数为 `true`
/// 2. **是控制请求**：传输原因是 ACT (6) 或 DEACT (8)
/// 3. **满足以下任一条件**：
///    - ASDU 解码失败（body 为 Raw）
///    - 解码错误代码为 `UnsupportedCommand`
///
/// # 使用场景
///
/// - 从站收到不支持的 Type ID 时，根据配置决定是否发送负确认
/// - 某些主站可能不支持负确认，因此需要可配置
///
/// # 示例
///
/// ```rust
/// let asdu = DecodedAsdu {
///     type_id: 127, // 未知类型
///     vsq: 1,
///     cause: 0x0006, // ACT
///     common_address: 1,
///     body: DecodedAsduBody::Raw, // 解码失败
///     raw_payload: vec![0x00, 0x00, 0x00, 0x01],
/// };
///
/// // 功能启用时应该发送负确认
/// assert!(should_negative_ack_unknown_type(&asdu, None, true));
///
/// // 功能禁用时不应该发送负确认
/// assert!(!should_negative_ack_unknown_type(&asdu, None, false));
/// ```
pub(crate) fn should_negative_ack_unknown_type(
    asdu: &DecodedAsdu,
    decode_error: Option<&Iec104ValidationError>,
    enabled: bool,
) -> bool {
    if !enabled || !is_control_request_cot(asdu.cause) {
        return false;
    }

    if matches!(asdu.body, DecodedAsduBody::Raw) {
        return true;
    }

    matches!(
        decode_error,
        Some(err) if err.code == Iec104ErrorCode::UnsupportedCommand
    )
}

/// 判断是否应该对解码错误发送负确认
///
/// 检查是否应该对 ASDU 解码错误发送负确认响应。
///
/// # 参数
///
/// * `asdu` - 已解码的 ASDU
/// * `decode_error` - 解码错误
/// * `unknown_typeid_negative_ack` - 是否启用未知类型负确认功能
///
/// # 返回
///
/// - `true` - 应该发送负确认
/// - `false` - 不应该发送负确认
///
/// # 判定逻辑
///
/// 满足以下所有条件时返回 `true`：
///
/// 1. **是控制请求**：传输原因是 ACT (6) 或 DEACT (8)
/// 2. **满足以下任一条件**：
///    - 错误代码为 `UnsupportedCommand` 且未知类型负确认功能已启用
///    - 错误代码为以下之一：
///      - `InvalidQualifier` - 无效限定词
///      - `InvalidValue` - 无效值
///      - `InvalidObjectAddress` - 无效信息对象地址
///      - `InvalidCommonAddress` - 无效公共地址
///
/// # 与 should_negative_ack_unknown_type 的区别
///
/// - `should_negative_ack_unknown_type`：专门用于未知类型标识的判定
/// - `should_negative_ack_decode_error`：用于所有解码错误的判定（包括未知类型）
///
/// # 使用场景
///
/// - 从站在解码 ASDU 时发生错误，需要决定是否发送负确认
/// - 不同类型的错误有不同的处理策略
///
/// # 示例
///
/// ```rust
/// let asdu = DecodedAsdu {
///     type_id: 45, // C_SC_NA_1
///     vsq: 1,
///     cause: 0x0006, // ACT
///     common_address: 1,
///     body: DecodedAsduBody::Raw,
///     raw_payload: vec![0x00, 0x00, 0x00, 0xFF], // 无效限定词
/// };
///
/// let error = Iec104ValidationError {
///     code: Iec104ErrorCode::InvalidQualifier,
///     message: "Invalid qualifier".to_string(),
/// };
///
/// // 无效限定词应该发送负确认
/// assert!(should_negative_ack_decode_error(&asdu, &error, true));
/// ```
pub(crate) fn should_negative_ack_decode_error(
    asdu: &DecodedAsdu,
    decode_error: &Iec104ValidationError,
    unknown_typeid_negative_ack: bool,
) -> bool {
    if decode_error.message.starts_with("unsupported command cause of transmission") {
        return true;
    }
    if !is_control_request_cot(asdu.cause) {
        return false;
    }

    if decode_error.code == Iec104ErrorCode::UnsupportedCommand {
        return should_negative_ack_unknown_type(asdu, Some(decode_error), unknown_typeid_negative_ack);
    }

    matches!(
        decode_error.code,
        Iec104ErrorCode::InvalidQualifier
            | Iec104ErrorCode::InvalidValue
            | Iec104ErrorCode::InvalidObjectAddress
            | Iec104ErrorCode::InvalidCommonAddress
    )
}

/// 判断传输原因是否为控制请求
///
/// 检查传输原因是否为控制类命令的请求（ACT 或 DEACT）。
///
/// # 参数
///
/// * `cause` - 传输原因（16 位，包含源地址和 COT）
///
/// # 返回
///
/// - `true` - 是控制请求（ACT 或 DEACT）
/// - `false` - 不是控制请求
///
/// # 逻辑说明
///
/// 1. 提取传输原因的低 6 位（去除 P/N 和 T 标志）
/// 2. 检查是否为 6 (ACT) 或 8 (DEACT)
///
/// # 位操作说明
///
/// ```text
/// cause = 0x1206 (OA=0x12, COT=6)
///
/// 步骤：
/// 1. (cause as u8) = 0x06
/// 2. 0x06 & 0x3F  = 0x06 (去除 P/N 和 T 标志)
/// 3. 0x06 == 6    = true (是 ACT)
/// ```
///
/// # 示例
///
/// ```rust
/// // ACT (激活)
/// assert!(is_control_request_cot(0x1206)); // OA=0x12, COT=6
///
/// // DEACT (去激活)
/// assert!(is_control_request_cot(0x1208)); // OA=0x12, COT=8
///
/// // ACT_CON (激活确认) - 不是请求
/// assert!(!is_control_request_cot(0x1207)); // OA=0x12, COT=7
///
/// // INTERROGATION (总召唤) - 不是控制请求
/// assert!(!is_control_request_cot(0x1214)); // OA=0x12, COT=20
/// ```
#[inline]
fn is_control_request_cot(cause: u16) -> bool {
    matches!((cause as u8) & 0x3F, 6 | 8)
}
