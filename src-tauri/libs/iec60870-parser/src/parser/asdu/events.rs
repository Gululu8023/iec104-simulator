//! 保护事件 ASDU 解析（TI 17~20, 30~40）。
//!
//! 本模块负责解析 IEC 60870-5-104 协议中的保护设备事件信息，包括：
//!
//! - **单点保护事件** (TI 17, 38): 单一保护事件带时标
//! - **打包启动事件** (TI 18, 39): 继电保护设备启动事件
//! - **打包输出电路事件** (TI 19, 40): 继电保护设备输出电路信息
//! - **其他保护事件** (TI 20, 30~37): 通用保护事件
//!
//! ## 时标支持
//!
//! - TI 17~20: 使用 3 字节时标 (CP24Time2a)
//! - TI 30~40: 使用 7 字节时标 (CP56Time2a)
//!
//! ## 解析策略
//!
//! 当前实现采用零拷贝设计，将事件数据存储为字节切片 (`&[u8]`)，
//! 便于上层根据具体业务需求进行详细解析。未来可扩展为结构化解析。
//!
//! ## 示例
//!
//! ```ignore
//! use iec60870_parser::parser::asdu::{AsduHeader, TypeId, Cot, CotReason};
//! use iec60870_parser::parser::asdu::events::parse_protection_events;
//!
//! let header = AsduHeader::new(TypeId::new(38), 1, Cot::new(CotReason::Spontaneous), 1);
//! let payload = [
//!     0x02, 0x01, 0x00,        // IOA
//!     0xAA, 0xBB, 0xCC,        // 保护事件数据
//!     1, 2, 3, 4, 5, 6, 7,     // CP56 时标
//! ];
//!
//! let events = parse_protection_events(header, &payload).unwrap();
//! assert_eq!(events.items.len(), 1);
//! assert_eq!(events.items[0].ioa, 0x000102);
//! ```

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, for_each_entry, lookup_type_descriptor, split_timestamp},
};

/// 保护事件集合。
///
/// 包含 ASDU 头部和若干条保护事件记录。
#[derive(Debug, Clone)]
pub struct ProtectionEvents<'a> {
    /// ASDU 头部信息。
    pub header: AsduHeader,
    /// 保护事件记录列表。
    pub items: SmallVec<[ProtectionEvent<'a>; 4]>,
}

/// 单条保护事件记录。
#[derive(Debug, Clone)]
pub struct ProtectionEvent<'a> {
    /// 信息对象地址 (IOA)。
    pub ioa: u32,
    /// 保护事件数据载荷。
    pub payload: ProtectionEventData<'a>,
    /// 时标（CP24 或 CP56），如果存在。
    pub timestamp: Option<&'a [u8]>,
}

/// 保护事件数据类型。
///
/// 根据 Type ID 区分不同类型的保护事件：
/// - `Single`: 单点保护事件 (TI 17, 38)
/// - `PackedStart`: 打包启动事件 (TI 18, 39)
/// - `PackedOutput`: 打包输出电路事件 (TI 19, 40)
/// - `Raw`: 其他未特定分类的事件
#[derive(Debug, Clone)]
pub enum ProtectionEventData<'a> {
    /// 单点保护事件数据（原始字节）。
    Single(&'a [u8]),
    /// 打包启动事件数据（原始字节）。
    PackedStart(&'a [u8]),
    /// 打包输出电路事件数据（原始字节）。
    PackedOutput(&'a [u8]),
    /// 未分类的保护事件数据（原始字节）。
    Raw(&'a [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionQuality {
    pub raw: u8,
    pub elapsed_time_invalid: bool,
    pub blocked: bool,
    pub substituted: bool,
    pub not_topical: bool,
    pub invalid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionEventDetails {
    Single { state: u8, quality: ProtectionQuality, elapsed_ms: u16 },
    PackedStart { flags: u8, quality: ProtectionQuality, elapsed_ms: u16 },
    PackedOutput { flags: u8, quality: ProtectionQuality, elapsed_ms: u16 },
}

impl ProtectionQuality {
    fn from_raw(raw: u8) -> Self {
        Self {
            raw,
            elapsed_time_invalid: raw & 0x08 != 0,
            blocked: raw & 0x10 != 0,
            substituted: raw & 0x20 != 0,
            not_topical: raw & 0x40 != 0,
            invalid: raw & 0x80 != 0,
        }
    }
}

impl ProtectionEventData<'_> {
    pub fn details(&self) -> Option<ProtectionEventDetails> {
        match self {
            Self::Single(bytes) if bytes.len() >= 3 => Some(ProtectionEventDetails::Single {
                state: bytes[0] & 0x03,
                quality: ProtectionQuality::from_raw(bytes[0]),
                elapsed_ms: u16::from_le_bytes([bytes[1], bytes[2]]),
            }),
            Self::PackedStart(bytes) if bytes.len() >= 4 => Some(ProtectionEventDetails::PackedStart {
                flags: bytes[0] & 0x3F,
                quality: ProtectionQuality::from_raw(bytes[1]),
                elapsed_ms: u16::from_le_bytes([bytes[2], bytes[3]]),
            }),
            Self::PackedOutput(bytes) if bytes.len() >= 4 => Some(ProtectionEventDetails::PackedOutput {
                flags: bytes[0] & 0x0F,
                quality: ProtectionQuality::from_raw(bytes[1]),
                elapsed_ms: u16::from_le_bytes([bytes[2], bytes[3]]),
            }),
            _ => None,
        }
    }
}

/// 解析保护事件 ASDU。
///
/// 支持 TI 17~20, 30~40 等保护事件类型。解析后的事件数据
/// 保留为原始字节切片，由上层根据业务需求进一步解析。
///
/// # 参数
///
/// - `header`: ASDU 头部信息
/// - `payload`: ASDU 负载数据
///
/// # 错误
///
/// - 如果 Type ID 不支持，返回 `ParseError::UnsupportedTypeId`
/// - 如果数据长度不足，返回 `ParseError::InsufficientInfoObject`
///
/// # 示例
///
/// ```ignore
/// let events = parse_protection_events(header, &payload)?;
/// for event in &events.items {
///     println!("IOA: 0x{:06X}", event.ioa);
/// }
/// ```
pub fn parse_protection_events<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<ProtectionEvents<'a>, ParseError> {
    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut records = SmallVec::new();
    for_each_entry(header.type_id, payload, descriptor, header.vsq, |ioa, body| {
        let (core, timestamp) = split_timestamp(header.type_id, body, descriptor.timestamp_len)?;

        let data = match header.type_id.raw() {
            17 | 38 => ProtectionEventData::Single(core),
            18 | 39 => ProtectionEventData::PackedStart(core),
            19 | 40 => ProtectionEventData::PackedOutput(core),
            _ => ProtectionEventData::Raw(core),
        };

        records.push(ProtectionEvent { ioa, payload: data, timestamp });
        Ok(())
    })?;

    Ok(ProtectionEvents { header, items: records })
}

#[derive(Debug)]
pub struct EventView<'a> {
    pub header: AsduHeader,
    pub payload: &'a [u8],
}

pub fn parse_event<'a>(header: AsduHeader, payload: &'a [u8]) -> EventView<'a> {
    EventView { header, payload }
}
