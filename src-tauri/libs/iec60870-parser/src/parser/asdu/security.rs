//! 安全认证 ASDU 解析（TI 81~87, 90~95）。
//!
//! 本模块负责解析 IEC 60870-5-104 协议中的安全认证相关信息。
//! 这些报文实现了端到端的会话安全机制，支持质询响应认证和会话密钥管理。
//!
//! ## 支持的消息类型
//!
//! ### 认证交互（TI 81~83）
//! - **TI 81 (S_CH_NA_1)**: 安全质询 - 服务器向客户端发送随机数质询
//! - **TI 82 (S_RP_NA_1)**: 安全响应 - 客户端使用 HMAC 响应质询
//! - **TI 83 (S_AR_NA_1)**: 强制请求 - 在一条消息中同时发送请求和认证信息
//!
//! ### 会话密钥管理（TI 84~86）
//! - **TI 84 (S_KR_NA_1)**: 会话密钥状态请求 - 查询会话密钥状态
//! - **TI 85 (S_KS_NA_1)**: 会话密钥状态 - 返回密钥状态和质询信息
//! - **TI 86 (S_KC_NA_1)**: 会话密钥更改 - 传输加密包裹的新会话密钥
//!
//! ### 错误处理（TI 87）
//! - **TI 87 (S_ER_NA_1)**: 认证错误 - 报告认证失败及原因
//!
//! ### 管理消息（TI 90~95）
//! - 预留用于安全管理功能，当前作为原始数据处理
//!
//! ## 分段支持
//!
//! 所有安全消息都支持分段传输（Segmentation）：
//! - **FIN 标志**: 指示是否为最后一个分段
//! - **FIR 标志**: 指示是否为第一个分段
//! - 当 `FIN && FIR` 同时为真时，表示消息未分段，可直接解析
//! - 否则返回 `SecurityFrame::Segment`，由上层处理分段重组
//!
//! ## 强制请求 (Aggressive Request)
//!
//! TI 83 是一种特殊的优化机制，允许在认证握手期间内嵌一个完整的 ASDU：
//! ```text
//! +--------+------------------+-----+-----+------+
//! | Header | Embedded ASDU    | CSQ | USR | HMAC |
//! +--------+------------------+-----+-----+------+
//! ```
//! 这样可以在完成认证的同时发送实际的控制命令，减少往返次数。
//!
//! ## 安全算法
//!
//! - **MAC 算法 (MAL)**: 用于 HMAC 计算的算法标识
//!   - 3: HMAC-SHA-1-64 (8 字节)
//!   - 4: HMAC-SHA-256-128 (16 字节)
//!   - 6: HMAC-SHA-256-96 (12 字节)
//! - **密钥包裹算法 (WAL)**: 用于会话密钥加密的算法
//!
//! ## 使用示例
//!
//! ```ignore
//! use iec60870_parser::parser::asdu::security::parse_security;
//!
//! match parse_security(header, payload)? {
//!     SecurityFrame::Challenge(ch) => {
//!         println!("质询序列号: {}, 用户: {}", ch.sequence, ch.user);
//!         println!("质询数据: {} 字节", ch.challenge.len());
//!     }
//!     SecurityFrame::Reply(rep) => {
//!         println!("响应序列号: {}, HMAC 长度: {}", rep.sequence, rep.hmac.len());
//!     }
//!     SecurityFrame::Segment(seg) => {
//!         println!("收到分段: FIN={}, FIR={}", seg.fin, seg.fir);
//!         // 上层需要处理分段重组
//!     }
//!     _ => {}
//! }
//! ```
//!
//! ## 安全注意事项
//!
//! - 质询值必须使用密码学安全的随机数生成器
//! - 序列号必须单调递增，防止重放攻击
//! - HMAC 验证失败时应立即拒绝，不要提供详细错误信息
//! - 会话密钥应定期更换
//! - 分段消息的重组必须检查序列号连续性

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, TypeId, types::lookup_type_descriptor},
};

/// 安全认证帧类型。
///
/// 枚举了所有支持的安全认证消息类型。每个 variant 对应一个特定的
/// TI (Type Identification) 值。
#[derive(Debug, Clone)]
pub enum SecurityFrame<'a> {
    /// 安全质询 (TI 81, S_CH_NA_1)。
    Challenge(SecureChallenge<'a>),
    /// 安全响应 (TI 82, S_RP_NA_1)。
    Reply(SecureReply<'a>),
    /// 强制请求 (TI 83, S_AR_NA_1) - 内嵌 ASDU 的认证请求。
    AggressiveRequest(SecureAggressiveRequest<'a>),
    /// 会话密钥状态请求 (TI 84, S_KR_NA_1)。
    StatusRequest {
        /// ASDU 头部。
        header: AsduHeader,
        /// 用户标识符。
        user: u16,
    },
    /// 会话密钥状态 (TI 85, S_KS_NA_1)。
    SessionKeyStatus(SecureKeyStatus<'a>),
    /// 会话密钥更改 (TI 86, S_KC_NA_1)。
    SessionKeyChange(SecureKeyChange<'a>),
    /// 认证错误 (TI 87, S_ER_NA_1)。
    AuthenticationError(SecureAuthError<'a>),
    /// 管理消息 (TI 90~95) - 预留的安全管理功能。
    Management {
        /// ASDU 头部。
        header: AsduHeader,
        /// 原始载荷数据。
        payload: &'a [u8],
    },
    /// 分段数据 - FIN/FIR 标志未同时置位，需上层重组。
    Segment(SecuritySegment<'a>),
    /// 未识别的安全帧类型 - 保留原始数据。
    Raw {
        /// ASDU 头部。
        header: AsduHeader,
        /// 原始载荷数据。
        payload: &'a [u8],
    },
}

/// 安全分段信息。
///
/// 当安全消息过大时，可以分段传输。本结构体包含分段控制标志和数据。
#[derive(Debug, Clone)]
pub struct SecuritySegment<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// FIN 标志 - 是否为最后一个分段。
    pub fin: bool,
    /// FIR 标志 - 是否为第一个分段。
    pub fir: bool,
    /// 去掉首字节后的分段数据。
    pub fragment: &'a [u8],
}

/// 安全质询消息 (TI 81)。
///
/// 服务器向客户端发送随机质询，客户端需使用 HMAC 响应。
#[derive(Debug, Clone)]
pub struct SecureChallenge<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 质询序列号 - 用于防止重放攻击。
    pub sequence: u32,
    /// 用户标识符。
    pub user: u16,
    /// MAC 算法标识 (MAL)。
    pub mac_algorithm: u8,
    /// 质询原因代码 (RSC)。
    pub reason: u8,
    /// 质询数据（随机数）。
    pub challenge: &'a [u8],
}

/// 安全响应消息 (TI 82)。
///
/// 客户端使用 HMAC 响应服务器的质询。
#[derive(Debug, Clone)]
pub struct SecureReply<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 响应序列号 - 必须与质询的序列号匹配。
    pub sequence: u32,
    /// 用户标识符。
    pub user: u16,
    /// HMAC 值。
    pub hmac: &'a [u8],
}

/// 强制请求消息 (TI 83)。
///
/// 在认证握手期间内嵌一个完整的 ASDU，减少往返次数。
#[derive(Debug, Clone)]
pub struct SecureAggressiveRequest<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 内嵌的 ASDU 数据（完整的 ASDU，包括头部）。
    pub embedded_asdu: &'a [u8],
    /// 认证序列号。
    pub sequence: u32,
    /// 用户标识符。
    pub user: u16,
    /// HMAC 值。
    pub hmac: &'a [u8],
}

/// 会话密钥状态消息 (TI 85)。
///
/// 返回当前会话密钥的状态和新的质询信息。
#[derive(Debug, Clone)]
pub struct SecureKeyStatus<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 密钥序列号。
    pub sequence: u32,
    /// 用户标识符。
    pub user: u16,
    /// 密钥包裹算法标识 (WAL)。
    pub wrap_algorithm: u8,
    /// 密钥状态代码。
    pub status: u8,
    /// 质询数据。
    pub challenge: &'a [u8],
    /// HMAC 值。
    pub hmac: &'a [u8],
}

/// 会话密钥更改消息 (TI 86)。
///
/// 传输使用密钥包裹算法加密的新会话密钥。
#[derive(Debug, Clone)]
pub struct SecureKeyChange<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 密钥序列号。
    pub sequence: u32,
    /// 用户标识符。
    pub user: u16,
    /// 加密包裹的会话密钥。
    pub wrapped_key: &'a [u8],
}

/// 认证错误消息 (TI 87)。
///
/// 报告认证失败及详细的错误信息。
#[derive(Debug, Clone)]
pub struct SecureAuthError<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 错误序列号。
    pub sequence: u32,
    /// 用户标识符。
    pub user: u16,
    /// 关联标识符。
    pub association: u16,
    /// 错误代码。
    pub error_code: u8,
    /// 时间戳（7 字节 CP56Time2a）。
    pub timestamp: &'a [u8],
    /// 错误文本描述。
    pub text: &'a [u8],
}

impl<'a> SecurityFrame<'a> {
    /// 获取安全帧的 ASDU 头部。
    ///
    /// 所有 variant 都包含 AsduHeader，本方法提供统一访问接口。
    pub fn header(&self) -> AsduHeader {
        match self {
            SecurityFrame::Challenge(inner) => inner.header,
            SecurityFrame::Reply(inner) => inner.header,
            SecurityFrame::AggressiveRequest(inner) => inner.header,
            SecurityFrame::StatusRequest { header, .. } => *header,
            SecurityFrame::SessionKeyStatus(inner) => inner.header,
            SecurityFrame::SessionKeyChange(inner) => inner.header,
            SecurityFrame::AuthenticationError(inner) => inner.header,
            SecurityFrame::Management { header, .. } => *header,
            SecurityFrame::Segment(seg) => seg.header,
            SecurityFrame::Raw { header, .. } => *header,
        }
    }
}

/// 解析安全认证 ASDU。
///
/// 根据 Type ID 自动选择相应的安全消息解析方式。所有安全消息都支持分段传输，
/// 当检测到分段时（FIN 和 FIR 标志不同时为真），返回 `SecurityFrame::Segment`。
///
/// # 参数
///
/// - `header`: ASDU 头部信息
/// - `payload`: ASDU 负载数据（包含分段标志字节）
///
/// # 返回值
///
/// - 对于完整消息（FIN && FIR），返回相应的具体消息类型
/// - 对于分段消息，返回 `SecurityFrame::Segment`，需上层处理重组
/// - 对于未识别的 TI，返回 `SecurityFrame::Raw`
///
/// # 错误
///
/// - 如果数据长度不足，返回 `ParseError::InsufficientInfoObject`
/// - 如果数据格式非法，返回 `ParseError::InvalidInfoObject`
///
/// # 示例
///
/// ```ignore
/// let frame = parse_security(header, payload)?;
/// match frame {
///     SecurityFrame::Challenge(ch) => {
///         // 处理质询
///     }
///     SecurityFrame::Segment(seg) => {
///         // 处理分段，可能需要缓存并等待后续分段
///     }
///     _ => {}
/// }
/// ```
pub fn parse_security<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    match header.type_id.raw() {
        81 => parse_challenge(header, payload),
        82 => parse_reply(header, payload),
        83 => parse_aggressive_request(header, payload),
        84 => parse_status_request(header, payload),
        85 => parse_session_key_status(header, payload),
        86 => parse_session_key_change(header, payload),
        87 => parse_auth_error(header, payload),
        90..=95 => {
            with_single_segment(header, payload, |fragment| Ok(SecurityFrame::Management { header, payload: fragment }))
        }
        _ => Ok(SecurityFrame::Raw { header, payload }),
    }
}

fn parse_challenge<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |mut cursor| {
        let sequence = take_u32(ti, &mut cursor)?;
        let user = take_u16(ti, &mut cursor)?;
        let mac_algorithm = take_u8(ti, &mut cursor)?;
        let reason = take_u8(ti, &mut cursor)?;
        let len = take_u16(ti, &mut cursor)? as usize;
        let challenge = take_bytes(ti, &mut cursor, len)?;
        ensure_done(ti, cursor)?;
        Ok(SecurityFrame::Challenge(SecureChallenge { header, sequence, user, mac_algorithm, reason, challenge }))
    })
}

fn parse_reply<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |mut cursor| {
        let sequence = take_u32(ti, &mut cursor)?;
        let user = take_u16(ti, &mut cursor)?;
        let len = take_u16(ti, &mut cursor)? as usize;
        let hmac = take_bytes(ti, &mut cursor, len)?;
        ensure_done(ti, cursor)?;
        Ok(SecurityFrame::Reply(SecureReply { header, sequence, user, hmac }))
    })
}

fn parse_aggressive_request<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |cursor| {
        let fragment = cursor;
        let embedded_len = estimate_embedded_len(fragment)?;
        if fragment.len() < embedded_len + 6 {
            return Err(ParseError::insufficient_info_object(
                "强制请求帧长度不足",
                ti,
                None,
                None,
                embedded_len + 6,
                fragment.len(),
            ));
        }
        let embedded = &fragment[..embedded_len];
        let mut tail = &fragment[embedded_len..];
        let sequence = take_u32(ti, &mut tail)?;
        let user = take_u16(ti, &mut tail)?;
        let hmac = tail;
        Ok(SecurityFrame::AggressiveRequest(SecureAggressiveRequest {
            header,
            embedded_asdu: embedded,
            sequence,
            user,
            hmac,
        }))
    })
}

fn parse_status_request<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |mut cursor| {
        let user = take_u16(ti, &mut cursor)?;
        ensure_done(ti, cursor)?;
        Ok(SecurityFrame::StatusRequest { header, user })
    })
}

fn parse_session_key_status<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |mut cursor| {
        let sequence = take_u32(ti, &mut cursor)?;
        let user = take_u16(ti, &mut cursor)?;
        let wrap_algorithm = take_u8(ti, &mut cursor)?;
        let status = take_u8(ti, &mut cursor)?;
        let hmac_len = mac_len_from_hal(ti, take_u8(ti, &mut cursor)?)?;
        let challenge_len = take_u16(ti, &mut cursor)? as usize;
        let challenge = take_bytes(ti, &mut cursor, challenge_len)?;
        let hmac = take_bytes(ti, &mut cursor, hmac_len)?;
        ensure_done(ti, cursor)?;
        Ok(SecurityFrame::SessionKeyStatus(SecureKeyStatus {
            header,
            sequence,
            user,
            wrap_algorithm,
            status,
            challenge,
            hmac,
        }))
    })
}

fn parse_session_key_change<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |mut cursor| {
        let sequence = take_u32(ti, &mut cursor)?;
        let user = take_u16(ti, &mut cursor)?;
        let wrapped_len = take_u16(ti, &mut cursor)? as usize;
        let wrapped_key = take_bytes(ti, &mut cursor, wrapped_len)?;
        ensure_done(ti, cursor)?;
        Ok(SecurityFrame::SessionKeyChange(SecureKeyChange { header, sequence, user, wrapped_key }))
    })
}

fn parse_auth_error<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let ti = header.type_id;
    with_single_segment(header, payload, |mut cursor| {
        let sequence = take_u32(ti, &mut cursor)?;
        let user = take_u16(ti, &mut cursor)?;
        let association = take_u16(ti, &mut cursor)?;
        let error_code = take_u8(ti, &mut cursor)?;
        let timestamp = take_bytes(ti, &mut cursor, 7)?;
        let text_len = take_u16(ti, &mut cursor)? as usize;
        let text = take_bytes(ti, &mut cursor, text_len)?;
        ensure_done(ti, cursor)?;
        Ok(SecurityFrame::AuthenticationError(SecureAuthError {
            header,
            sequence,
            user,
            association,
            error_code,
            timestamp,
            text,
        }))
    })
}

fn with_single_segment<'a, F>(header: AsduHeader, payload: &'a [u8], f: F) -> Result<SecurityFrame<'a>, ParseError>
where F: FnOnce(&'a [u8]) -> Result<SecurityFrame<'a>, ParseError> {
    let segment = parse_segment_header(header, payload)?;
    if !(segment.fin && segment.fir) {
        return Ok(SecurityFrame::Segment(segment));
    }
    f(segment.fragment)
}

fn parse_segment_header<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<SecuritySegment<'a>, ParseError> {
    let ti = header.type_id;
    if payload.is_empty() {
        return Err(ParseError::invalid_info_object("安全分段缺少首字节", ti, None, None, None));
    }
    let flags = payload[0];
    let fin = flags & 0x01 != 0;
    let fir = flags & 0x02 != 0;
    Ok(SecuritySegment { header, fin, fir, fragment: &payload[1..] })
}

fn estimate_embedded_len(payload: &[u8]) -> Result<usize, ParseError> {
    const HEADER_LEN: usize = 1 + 1 + 2 + 2;
    if payload.len() < HEADER_LEN {
        return Err(ParseError::invalid_asdu_header("强制请求内嵌 ASDU 头部不足", None, None));
    }
    let embedded_ti =
        TypeId::from_raw(payload[0]).ok_or(ParseError::invalid_asdu_header("强制请求内嵌 TI 非法", None, None))?;
    let descriptor = lookup_type_descriptor(embedded_ti).ok_or(ParseError::unsupported_type_id(embedded_ti, None))?;
    let vsq = payload[1];
    let count = (vsq & 0x7F) as usize;
    if count == 0 {
        return Err(ParseError::invalid_info_object("强制请求 VSQ 计数为 0", embedded_ti, None, None, None));
    }
    let entry_len = descriptor.entry_len().ok_or(ParseError::invalid_info_object(
        "强制请求内嵌 TI 长度可变，暂不支持",
        embedded_ti,
        None,
        None,
        None,
    ))?;
    let sq = descriptor.ioa_len > 0 && (vsq & 0x80 != 0);
    let entries = if sq {
        let data_len = entry_len.checked_sub(descriptor.ioa_len).ok_or(ParseError::invalid_info_object(
            "Aggressive request 信息体长度不足",
            embedded_ti,
            None,
            None,
            None,
        ))?;
        entry_len
            .checked_add(data_len.checked_mul(count.saturating_sub(1)).ok_or(ParseError::invalid_info_object(
                "Aggressive request 长度溢出",
                embedded_ti,
                None,
                None,
                None,
            ))?)
            .ok_or(ParseError::invalid_info_object("Aggressive request 长度溢出", embedded_ti, None, None, None))?
    } else {
        entry_len.checked_mul(count).ok_or(ParseError::invalid_info_object(
            "Aggressive request 长度溢出",
            embedded_ti,
            None,
            None,
            None,
        ))?
    };
    let total = HEADER_LEN.checked_add(entries).ok_or(ParseError::invalid_info_object(
        "Aggressive request 长度溢出",
        embedded_ti,
        None,
        None,
        None,
    ))?;
    if payload.len() < total {
        return Err(ParseError::invalid_info_object(
            "Aggressive request 内嵌 ASDU 数据不足",
            embedded_ti,
            None,
            None,
            None,
        ));
    }
    Ok(total)
}

fn take_u32(ti: TypeId, cursor: &mut &[u8]) -> Result<u32, ParseError> {
    if cursor.len() < 4 {
        return Err(ParseError::insufficient_info_object("字段长度不足(u32)", ti, None, None, 4, cursor.len()));
    }
    let (head, rest) = cursor.split_at(4);
    *cursor = rest;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

fn take_u16(ti: TypeId, cursor: &mut &[u8]) -> Result<u16, ParseError> {
    if cursor.len() < 2 {
        return Err(ParseError::insufficient_info_object("字段长度不足(u16)", ti, None, None, 2, cursor.len()));
    }
    let (head, rest) = cursor.split_at(2);
    *cursor = rest;
    Ok(u16::from_le_bytes([head[0], head[1]]))
}

fn take_u8(ti: TypeId, cursor: &mut &[u8]) -> Result<u8, ParseError> {
    if cursor.is_empty() {
        return Err(ParseError::insufficient_info_object("字段长度不足(u8)", ti, None, None, 1, cursor.len()));
    }
    let value = cursor[0];
    *cursor = &cursor[1..];
    Ok(value)
}

fn take_bytes<'a>(ti: TypeId, cursor: &mut &'a [u8], len: usize) -> Result<&'a [u8], ParseError> {
    if cursor.len() < len {
        return Err(ParseError::insufficient_info_object("字段长度不足(bytes)", ti, None, None, len, cursor.len()));
    }
    let (head, rest) = cursor.split_at(len);
    *cursor = rest;
    Ok(head)
}

fn ensure_done(ti: TypeId, cursor: &[u8]) -> Result<(), ParseError> {
    if cursor.is_empty() {
        Ok(())
    } else {
        Err(ParseError::invalid_info_object("存在未解析数据", ti, None, None, None))
    }
}

fn mac_len_from_hal(ti: TypeId, value: u8) -> Result<usize, ParseError> {
    match value {
        3 => Ok(8),
        4 => Ok(16),
        6 => Ok(12),
        _ => Err(ParseError::invalid_info_object("未知 HAL 算法标识", ti, None, None, None)),
    }
}
