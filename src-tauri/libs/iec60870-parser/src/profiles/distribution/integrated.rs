//! 累计量 ASDU 解析（TI 206/207）。
//!
//! 本模块实现《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义的累计量类型：
//! - TI 206 (M_IT_NB_1): 累计量（短浮点数）
//! - TI 207 (M_IT_TC_1): 带时标累计量（短浮点数）

use std::any::Any;

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, CP56Time2a, TypeId},
    profile::ProfileAsdu,
};

/// 单条累计量（短浮点数）。
#[derive(Debug, Clone, Copy)]
pub struct IntegratedTotalFloatItem {
    /// 信息对象地址
    pub ioa: u32,
    /// 电能量值（IEEE 754 短浮点数）
    pub value: f32,
    /// 品质描述词 (QDS)
    pub qds: u8,
}

impl IntegratedTotalFloatItem {
    /// 检查是否有效（IV 位为 0）
    pub fn is_valid(&self) -> bool {
        (self.qds & 0x80) == 0
    }

    /// 检查是否溢出（OV 位为 1）
    pub fn is_overflow(&self) -> bool {
        (self.qds & 0x01) != 0
    }

    /// 检查是否已被取代（SB 位为 1）
    pub fn is_substituted(&self) -> bool {
        (self.qds & 0x20) != 0
    }

    /// 检查是否被封锁（BL 位为 1）
    pub fn is_blocked(&self) -> bool {
        (self.qds & 0x10) != 0
    }
}

/// TI 206: 累计量（短浮点数）帧。
#[derive(Debug, Clone)]
pub struct IntegratedTotalFloat {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 累计量列表
    pub items: SmallVec<[IntegratedTotalFloatItem; 4]>,
}

impl ProfileAsdu for IntegratedTotalFloat {
    fn type_id(&self) -> TypeId {
        TypeId::new(206)
    }

    fn type_name(&self) -> &'static str {
        "M_IT_NB_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析累计量（短浮点数）(TI 206)。
pub fn parse_integrated_total_float<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    // 格式: IOA(3) + 电能量值(4) + QDS(1) = 8 字节/条
    const ENTRY_LEN: usize = 8;

    let vsq = header.vsq;
    let count = (vsq & 0x7F) as usize;
    let sq = (vsq & 0x80) != 0;

    if count == 0 {
        return Err(ParseError::InvalidAsduHeader {
            reason: "VSQ 数量为 0", ti: Some(header.type_id), offset: None
        });
    }

    let mut items = SmallVec::new();

    if sq {
        // SQ=1: 连续地址
        if payload.len() < 3 + count * 5 {
            return Err(ParseError::InsufficientInfoObject {
                needed: 3 + count * 5,
                available: payload.len(),
                ti: header.type_id,
                ioa: None,
                entry_index: None,
                reason: "累计量数据不足",
            });
        }

        let base_ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
        let data = &payload[3..];

        for i in 0..count {
            let offset = i * 5;
            if offset + 5 > data.len() {
                break;
            }

            let value = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            let qds = data[offset + 4];

            items.push(IntegratedTotalFloatItem { ioa: base_ioa + i as u32, value, qds });
        }
    } else {
        // SQ=0: 独立地址
        if payload.len() < count * ENTRY_LEN {
            return Err(ParseError::InsufficientInfoObject {
                needed: count * ENTRY_LEN,
                available: payload.len(),
                ti: header.type_id,
                ioa: None,
                entry_index: None,
                reason: "累计量数据不足",
            });
        }

        for i in 0..count {
            let offset = i * ENTRY_LEN;
            if offset + ENTRY_LEN > payload.len() {
                break;
            }

            let ioa = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], 0]);
            let value = f32::from_le_bytes([
                payload[offset + 3],
                payload[offset + 4],
                payload[offset + 5],
                payload[offset + 6],
            ]);
            let qds = payload[offset + 7];

            items.push(IntegratedTotalFloatItem { ioa, value, qds });
        }
    }

    Ok(Box::new(IntegratedTotalFloat { header: *header, items }))
}

/// 单条带时标累计量（短浮点数）。
#[derive(Debug, Clone)]
pub struct IntegratedTotalFloatTimeItem {
    /// 信息对象地址
    pub ioa: u32,
    /// 电能量值（IEEE 754 短浮点数）
    pub value: f32,
    /// 品质描述词 (QDS)
    pub qds: u8,
    /// CP56Time2a 时间戳
    pub timestamp: CP56Time2a,
}

/// TI 207: 带时标累计量（短浮点数）帧。
#[derive(Debug, Clone)]
pub struct IntegratedTotalFloatTime {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 累计量列表
    pub items: SmallVec<[IntegratedTotalFloatTimeItem; 4]>,
}

impl ProfileAsdu for IntegratedTotalFloatTime {
    fn type_id(&self) -> TypeId {
        TypeId::new(207)
    }

    fn type_name(&self) -> &'static str {
        "M_IT_TC_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析带时标累计量（短浮点数）(TI 207)。
pub fn parse_integrated_total_float_time<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    // 格式: IOA(3) + 电能量值(4) + QDS(1) + CP56Time2a(7) = 15 字节/条
    const ENTRY_LEN: usize = 15;

    let vsq = header.vsq;
    let count = (vsq & 0x7F) as usize;
    let sq = (vsq & 0x80) != 0;

    if count == 0 {
        return Err(ParseError::InvalidAsduHeader {
            reason: "VSQ 数量为 0", ti: Some(header.type_id), offset: None
        });
    }

    let mut items = SmallVec::new();

    if sq {
        // SQ=1: 连续地址
        if payload.len() < 3 + count * 12 {
            return Err(ParseError::InsufficientInfoObject {
                needed: 3 + count * 12,
                available: payload.len(),
                ti: header.type_id,
                ioa: None,
                entry_index: None,
                reason: "带时标累计量数据不足",
            });
        }

        let base_ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
        let data = &payload[3..];

        for i in 0..count {
            let offset = i * 12;
            if offset + 12 > data.len() {
                break;
            }

            let value = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            let qds = data[offset + 4];
            let timestamp =
                CP56Time2a::parse(&data[offset + 5..offset + 12]).map_err(|_| ParseError::InsufficientInfoObject {
                    needed: 7,
                    available: 0,
                    ti: header.type_id,
                    ioa: Some(base_ioa + i as u32),
                    entry_index: Some(i),
                    reason: "时间戳解析失败",
                })?;

            items.push(IntegratedTotalFloatTimeItem { ioa: base_ioa + i as u32, value, qds, timestamp });
        }
    } else {
        // SQ=0: 独立地址
        if payload.len() < count * ENTRY_LEN {
            return Err(ParseError::InsufficientInfoObject {
                needed: count * ENTRY_LEN,
                available: payload.len(),
                ti: header.type_id,
                ioa: None,
                entry_index: None,
                reason: "带时标累计量数据不足",
            });
        }

        for i in 0..count {
            let offset = i * ENTRY_LEN;
            if offset + ENTRY_LEN > payload.len() {
                break;
            }

            let ioa = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], 0]);
            let value = f32::from_le_bytes([
                payload[offset + 3],
                payload[offset + 4],
                payload[offset + 5],
                payload[offset + 6],
            ]);
            let qds = payload[offset + 7];
            let timestamp = CP56Time2a::parse(&payload[offset + 8..offset + 15]).map_err(|_| {
                ParseError::InsufficientInfoObject {
                    needed: 7,
                    available: 0,
                    ti: header.type_id,
                    ioa: Some(ioa),
                    entry_index: Some(i),
                    reason: "时间戳解析失败",
                }
            })?;

            items.push(IntegratedTotalFloatTimeItem { ioa, value, qds, timestamp });
        }
    }

    Ok(Box::new(IntegratedTotalFloatTime { header: *header, items }))
}
