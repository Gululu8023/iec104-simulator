//! 质量描述符处理
//!
//! 本模块负责处理 IEC104 协议中的质量描述符（Quality
//! Descriptor），包括质量位的转换、规范化和元数据提取。
//!
//! # 核心功能
//!
//! - **质量位转换**：将内部质量表示转换为 IEC104 协议格式
//! - **质量规范化**：根据数据点类型规范化质量描述符
//! - **质量元数据提取**：从质量位中提取通用和详细的质量信息
//! - **阻塞质量检测**：检测质量位是否表示数据点被阻塞
//!
//! # 质量描述符类型
//!
//! IEC104 协议定义了多种质量描述符格式，不同类型的数据点使用不同的质量格式：
//!
//! ## SIQ（Single-point Information with Quality）
//!
//! 用于单点信息（Type ID 1, 2, 30）：
//!
//! ```text
//! Bit 7: IV (Invalid)
//! Bit 6: NT (Not Topical)
//! Bit 5: SB (Substituted)
//! Bit 4: BL (Blocked)
//! Bit 3-1: 保留
//! Bit 0: SPI (Single Point Information)
//! ```
//!
//! ## DIQ（Double-point Information with Quality）
//!
//! 用于双点信息（Type ID 3, 4, 31）：
//!
//! ```text
//! Bit 7: IV (Invalid)
//! Bit 6: NT (Not Topical)
//! Bit 5: SB (Substituted)
//! Bit 4: BL (Blocked)
//! Bit 3-2: 保留
//! Bit 1-0: DPI (Double Point Information)
//! ```
//!
//! ## QDS（Quality Descriptor for Events）
//!
//! 用于测量值（Type ID 5-14, 20, 32-36）：
//!
//! ```text
//! Bit 7: IV (Invalid)
//! Bit 6: NT (Not Topical)
//! Bit 5: SB (Substituted)
//! Bit 4: BL (Blocked)
//! Bit 3-1: 保留
//! Bit 0: OV (Overflow)
//! ```
//!
//! ## BCR（Binary Counter Reading）
//!
//! 用于累计量（Type ID 15, 16, 37）：
//!
//! ```text
//! Bit 7: IV (Invalid)
//! Bit 6: CA (Counter Adjusted)
//! Bit 5: CY (Counter Carry)
//! Bit 4-0: SQ (Sequence Number)
//! ```
//!
//! # 质量位含义
//!
//! ## 通用质量位
//!
//! - **IV (Invalid, 0x80)**：数据无效，不应用于控制或计算
//! - **NT (Not Topical, 0x40)**：数据不是最新的，可能已过时
//! - **SB (Substituted, 0x20)**：数据是替代值，不是实际测量值
//! - **BL (Blocked, 0x10)**：数据点被阻塞，不应更新
//!
//! ## 特殊质量位
//!
//! - **OV (Overflow, 0x01)**：测量值溢出，超出量程
//! - **CA (Counter Adjusted, 0x40)**：计数器已调整
//! - **CY (Counter Carry, 0x20)**：计数器进位
//!
//! # 质量转换
//!
//! ## 内部表示 → 协议格式
//!
//! 内部使用统一的质量表示（低 4 位），发送时需要转换为协议格式（高 4 位）：
//!
//! ```text
//! 内部格式（低 4 位）:
//! Bit 3: BL (Blocked)
//! Bit 2: SB (Substituted)
//! Bit 1: NT (Not Topical)
//! Bit 0: IV (Invalid)
//!
//! 协议格式（高 4 位）:
//! Bit 7: IV (Invalid)
//! Bit 6: NT (Not Topical)
//! Bit 5: SB (Substituted)
//! Bit 4: BL (Blocked)
//! ```
//!
//! # 业务可用性
//!
//! 数据点的业务可用性由质量位决定：
//!
//! - **IV=1**：数据无效，业务不可用
//! - **BL=1**：数据被阻塞，业务不可用
//! - **其他**：业务可用（即使 NT=1 或 SB=1）

use iec60870_parser::parser::asdu::{QualityKind, TypeId, lookup_type_descriptor, qualifiers::normalize_quality_byte};

use crate::core::types::{DataPoint, DataPointQualityCommon, DataPointQualityDetail, is_quality_business_usable};

#[inline]
fn quality_kind(type_id: u8) -> QualityKind {
    lookup_type_descriptor(TypeId::new(type_id)).map(|descriptor| descriptor.quality_kind).unwrap_or(QualityKind::None)
}

#[inline]
pub(crate) fn quality_to_wire(q: u8) -> u8 {
    normalize_quality_byte(QualityKind::Siq, q)
}

#[inline]
fn is_siq_quality_type(type_id: u8) -> bool {
    quality_kind(type_id) == QualityKind::Siq
}

#[inline]
fn is_diq_quality_type(type_id: u8) -> bool {
    quality_kind(type_id) == QualityKind::Diq
}

#[inline]
fn is_qds_quality_type(type_id: u8) -> bool {
    quality_kind(type_id) == QualityKind::Qds
}

#[inline]
fn is_bcr_quality_type(type_id: u8) -> bool {
    quality_kind(type_id) == QualityKind::Bcr
}

#[inline]
pub(crate) fn normalize_quality_raw_for_type(type_id: u8, q: u8) -> u8 {
    normalize_quality_byte(quality_kind(type_id), q)
}

#[inline]
pub(crate) fn quality_common_from_raw(type_id: u8, q: u8) -> DataPointQualityCommon {
    let raw = normalize_quality_raw_for_type(type_id, q);
    DataPointQualityCommon {
        iv: (raw & 0x80) != 0,
        nt: if is_bcr_quality_type(type_id) { false } else { (raw & 0x40) != 0 },
        sb: if is_bcr_quality_type(type_id) { false } else { (raw & 0x20) != 0 },
        bl: if is_bcr_quality_type(type_id) { false } else { (raw & 0x10) != 0 },
    }
}

#[inline]
fn quality_detail_from_raw(type_id: u8, q: u8) -> DataPointQualityDetail {
    let raw = normalize_quality_raw_for_type(type_id, q);
    if is_bcr_quality_type(type_id) {
        return DataPointQualityDetail::Bcr {
            raw,
            sq: raw & 0x1F,
            cy: (raw & 0x20) != 0,
            ca: (raw & 0x40) != 0,
            iv: (raw & 0x80) != 0,
        };
    }
    if is_qds_quality_type(type_id) {
        return DataPointQualityDetail::Qds {
            raw,
            ov: (raw & 0x01) != 0,
            iv: (raw & 0x80) != 0,
            nt: (raw & 0x40) != 0,
            sb: (raw & 0x20) != 0,
            bl: (raw & 0x10) != 0,
            transient: matches!(type_id, 5 | 6 | 32).then_some(false),
            scd_status: None,
            scd_change: None,
        };
    }
    DataPointQualityDetail::None
}

#[inline]
pub(crate) fn quality_detail_from_point(type_id: u8, q: u8, value: f64) -> DataPointQualityDetail {
    let raw = normalize_quality_raw_for_type(type_id, q);
    if is_siq_quality_type(type_id) {
        return DataPointQualityDetail::Siq {
            raw,
            spi: value > 0.5,
            iv: (raw & 0x80) != 0,
            nt: (raw & 0x40) != 0,
            sb: (raw & 0x20) != 0,
            bl: (raw & 0x10) != 0,
        };
    }
    if is_diq_quality_type(type_id) {
        let dpi = iec60870_parser::parser::asdu::encode::encode_double_point(value)
            .expect("decoded double-point value must remain in the DPI wire domain");
        return DataPointQualityDetail::Diq {
            raw,
            dpi,
            iv: (raw & 0x80) != 0,
            nt: (raw & 0x40) != 0,
            sb: (raw & 0x20) != 0,
            bl: (raw & 0x10) != 0,
        };
    }
    quality_detail_from_raw(type_id, raw)
}

#[inline]
pub(crate) fn apply_quality_metadata(point: &mut DataPoint, quality: u8) {
    let raw = normalize_quality_raw_for_type(point.type_id, quality);
    point.quality = raw;
    point.quality_common = Some(quality_common_from_raw(point.type_id, raw));
    point.quality_detail = Some(quality_detail_from_point(point.type_id, raw, point.value));
    point.business_usable = is_quality_business_usable(point.type_id, raw);
}

#[inline]
pub(crate) fn is_blocked_quality(quality: u8) -> bool {
    (quality_to_wire(quality) & 0x10) != 0
}
