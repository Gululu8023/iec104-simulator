//! 故障事件信息 ASDU 解析（TI 42）。
//!
//! 本模块实现《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义的故障事件信息。
//!
//! TI 42 (M_FT_NA_1) 用于配电终端在故障时刻上送遥信和遥测数据。
//!
//! ## 报文格式（实现细则 7.5）
//!
//! ```text
//! IOA(3) + 故障类型(1)
//! + 带时标遥信个数(1) + [遥信类型(1) + 点号(2) + 遥信值(1) + CP56Time2a(7)]...
//! + 遥测个数(1) + [遥测类型(1) + 遥测IOA(3) + 遥测值(2/4)]...
//! ```

use std::any::Any;

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, CP56Time2a, TypeId},
    profile::ProfileAsdu,
};

/// 故障类型枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FaultType {
    /// 1: 接地故障
    Ground = 1,
    /// 2: 短路故障
    ShortCircuit = 2,
    /// 3: 过流故障
    Overcurrent = 3,
    /// 4: 过压故障
    Overvoltage = 4,
    /// 5: 欠压故障
    Undervoltage = 5,
    /// 其他故障类型
    Other(u8),
}

impl From<u8> for FaultType {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Ground,
            2 => Self::ShortCircuit,
            3 => Self::Overcurrent,
            4 => Self::Overvoltage,
            5 => Self::Undervoltage,
            _ => Self::Other(v),
        }
    }
}

/// 遥信类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TeleIndicationType {
    /// 1: 单点遥信 (M_SP_NA_1)
    SinglePoint = 1,
    /// 3: 双点遥信 (M_DP_NA_1)
    DoublePoint = 3,
}

impl TryFrom<u8> for TeleIndicationType {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            1 => Ok(Self::SinglePoint),
            3 => Ok(Self::DoublePoint),
            _ => Err(v),
        }
    }
}

/// 遥测类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TelemetryType {
    /// 9: 归一化测量值 (M_ME_NA_1)
    Normalized = 9,
    /// 11: 标度化测量值 (M_ME_NB_1)
    Scaled = 11,
    /// 13: 短浮点测量值 (M_ME_NC_1)
    ShortFloat = 13,
}

impl TryFrom<u8> for TelemetryType {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            9 => Ok(Self::Normalized),
            11 => Ok(Self::Scaled),
            13 => Ok(Self::ShortFloat),
            _ => Err(v),
        }
    }
}

/// 故障时刻遥信条目。
#[derive(Debug, Clone, Copy)]
pub struct FaultTeleIndication {
    /// 遥信点号
    pub point_no: u16,
    /// 遥信值（单点: 0/1, 双点: 0/1/2/3）
    pub value: u8,
    /// 故障时刻时标
    pub timestamp: CP56Time2a,
}

/// 故障时刻遥测条目。
#[derive(Debug, Clone, Copy)]
pub struct FaultTelemetry {
    /// 信息对象地址
    pub ioa: u32,
    /// 遥测值（根据类型解释）
    pub value: FaultTelemetryValue,
}

/// 遥测值类型。
#[derive(Debug, Clone, Copy)]
pub enum FaultTelemetryValue {
    /// 归一化值 (i16)
    Normalized(i16),
    /// 标度化值 (i16)
    Scaled(i16),
    /// 短浮点值 (f32)
    ShortFloat(f32),
}

/// TI 42: 故障事件信息。
#[derive(Debug, Clone)]
pub struct FaultEvent {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 信息对象地址
    pub ioa: u32,
    /// 故障类型
    pub fault_type: FaultType,
    /// 遥信类型（若有遥信）
    pub ti_type: Option<TeleIndicationType>,
    /// 故障时刻遥信列表
    pub teleindications: SmallVec<[FaultTeleIndication; 4]>,
    /// 遥测类型（若有遥测）
    pub tm_type: Option<TelemetryType>,
    /// 故障时刻遥测列表
    pub telemetries: SmallVec<[FaultTelemetry; 4]>,
}

impl ProfileAsdu for FaultEvent {
    fn type_id(&self) -> TypeId {
        TypeId::new(42)
    }

    fn type_name(&self) -> &'static str {
        "M_FT_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析故障事件信息 (TI 42)。
pub fn parse_fault_event<'a>(header: &AsduHeader, payload: &'a [u8]) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    let ti = header.type_id;

    // 最小字段: IOA(3) + 故障类型(1) + 遥信个数(1) + 遥测个数(1) = 6
    if payload.len() < 6 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 6,
            available: payload.len(),
            ti,
            ioa: None,
            entry_index: None,
            reason: "故障事件信息基础字段不足",
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
    let fault_type = FaultType::from(payload[3]);
    let mut pos = 4usize;

    // 遥信
    let ti_count = payload[pos] as usize;
    pos += 1;

    let mut ti_type = None;
    let mut teleindications = SmallVec::new();
    if ti_count > 0 {
        if pos >= payload.len() {
            return Err(ParseError::InsufficientInfoObject {
                needed: pos + 1,
                available: payload.len(),
                ti,
                ioa: Some(ioa),
                entry_index: None,
                reason: "缺少遥信类型字节",
            });
        }

        let ti_type_byte = payload[pos];
        pos += 1;
        let parsed_ti_type = TeleIndicationType::try_from(ti_type_byte).map_err(|_| ParseError::InvalidInfoObject {
            reason: "无效的遥信类型",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: Some(pos - 1),
        })?;
        ti_type = Some(parsed_ti_type);

        for idx in 0..ti_count {
            // 点号(2) + 值(1) + 时标(7)
            if pos + 10 > payload.len() {
                return Err(ParseError::InsufficientInfoObject {
                    needed: pos + 10,
                    available: payload.len(),
                    ti,
                    ioa: Some(ioa),
                    entry_index: Some(idx),
                    reason: "遥信条目数据不足",
                });
            }

            let point_no = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
            let value = payload[pos + 2];
            let timestamp =
                CP56Time2a::parse(&payload[pos + 3..pos + 10]).map_err(|_| ParseError::InvalidInfoObject {
                    reason: "遥信时标解析失败",
                    ti,
                    ioa: Some(ioa),
                    entry_index: Some(idx),
                    offset: Some(pos + 3),
                })?;

            teleindications.push(FaultTeleIndication { point_no, value, timestamp });
            pos += 10;
        }
    }

    // 遥测
    if pos >= payload.len() {
        return Err(ParseError::InsufficientInfoObject {
            needed: pos + 1,
            available: payload.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少遥测个数字节",
        });
    }
    let tm_count = payload[pos] as usize;
    pos += 1;

    let mut tm_type = None;
    let mut telemetries = SmallVec::new();
    if tm_count > 0 {
        if pos >= payload.len() {
            return Err(ParseError::InsufficientInfoObject {
                needed: pos + 1,
                available: payload.len(),
                ti,
                ioa: Some(ioa),
                entry_index: None,
                reason: "缺少遥测类型字节",
            });
        }

        let tm_type_byte = payload[pos];
        pos += 1;
        let parsed_tm_type = TelemetryType::try_from(tm_type_byte).map_err(|_| ParseError::InvalidInfoObject {
            reason: "无效的遥测类型",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: Some(pos - 1),
        })?;
        tm_type = Some(parsed_tm_type);

        let value_len = match parsed_tm_type {
            TelemetryType::Normalized | TelemetryType::Scaled => 2,
            TelemetryType::ShortFloat => 4,
        };
        let entry_len = 3 + value_len;

        for idx in 0..tm_count {
            if pos + entry_len > payload.len() {
                return Err(ParseError::InsufficientInfoObject {
                    needed: pos + entry_len,
                    available: payload.len(),
                    ti,
                    ioa: Some(ioa),
                    entry_index: Some(idx),
                    reason: "遥测条目数据不足",
                });
            }

            let tm_ioa = u32::from_le_bytes([payload[pos], payload[pos + 1], payload[pos + 2], 0]);
            let value = match parsed_tm_type {
                TelemetryType::Normalized => {
                    let v = i16::from_le_bytes([payload[pos + 3], payload[pos + 4]]);
                    FaultTelemetryValue::Normalized(v)
                }
                TelemetryType::Scaled => {
                    let v = i16::from_le_bytes([payload[pos + 3], payload[pos + 4]]);
                    FaultTelemetryValue::Scaled(v)
                }
                TelemetryType::ShortFloat => {
                    let v =
                        f32::from_le_bytes([payload[pos + 3], payload[pos + 4], payload[pos + 5], payload[pos + 6]]);
                    FaultTelemetryValue::ShortFloat(v)
                }
            };

            telemetries.push(FaultTelemetry { ioa: tm_ioa, value });
            pos += entry_len;
        }
    }

    if pos != payload.len() {
        return Err(ParseError::InvalidInfoObject {
            reason: "故障事件信息存在多余数据",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: Some(pos),
        });
    }

    Ok(Box::new(FaultEvent { header: *header, ioa, fault_type, ti_type, teleindications, tm_type, telemetries }))
}
