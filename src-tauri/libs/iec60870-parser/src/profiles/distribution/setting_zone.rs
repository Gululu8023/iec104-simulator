//! 定值区 ASDU 解析（TI 200-203）。
//!
//! 本模块实现《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义的定值区命令：
//! - TI 200 (C_SR_NA_1): 切换定值区
//! - TI 201 (C_RR_NA_1): 读定值区号
//! - TI 202 (C_RS_NA_1): 读参数和定值
//! - TI 203 (C_WS_NA_1): 写参数和定值
//!
//! ## 重要说明
//!
//! 根据实施细则，定值区号 (SN) 为 **2字节** 无符号整数。
//! 参数特征标识 PI 为单字节位字段：CP8{CONT, RES, CR, S/E}。

use std::any::Any;

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, CotReason, TypeId},
    profile::ProfileAsdu,
};

// ============================================================================
// TI 200: 切换定值区
// ============================================================================

/// TI 200: 切换定值区命令。
///
/// 用于切换终端使用的定值区。
///
/// ## 报文格式
/// - IOA(3): 信息对象地址，固定为 0
/// - SN(2): 定值区号（2字节）
#[derive(Debug, Clone)]
pub struct SwitchSettingZone {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 信息对象地址（通常为 0）
    pub ioa: u32,
    /// 定值区号 (SN)，2字节
    pub sn: u16,
}

impl ProfileAsdu for SwitchSettingZone {
    fn type_id(&self) -> TypeId {
        TypeId::new(200)
    }

    fn type_name(&self) -> &'static str {
        "C_SR_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析切换定值区命令 (TI 200)。
pub fn parse_switch_setting_zone<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    // 格式: IOA(3) + SN(2) = 5字节
    if payload.len() < 5 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 5,
            available: payload.len(),
            ti: header.type_id,
            ioa: None,
            entry_index: None,
            reason: "切换定值区命令数据不足",
        });
    }
    if payload.len() != 5 {
        return Err(ParseError::InvalidInfoObject {
            reason: "切换定值区命令存在多余数据",
            ti: header.type_id,
            ioa: None,
            entry_index: None,
            offset: Some(5),
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
    let sn = u16::from_le_bytes([payload[3], payload[4]]);

    Ok(Box::new(SwitchSettingZone { header: *header, ioa, sn }))
}

// ============================================================================
// TI 201: 读定值区号
// ============================================================================

/// TI 201: 读定值区号命令/响应。
///
/// ## 报文格式
/// - 控制方向: IOA(3)，仅信息对象地址=0
/// - 监视方向: IOA(3) + SN1(2) + SN2(2) + SN3(2)
///   - SN1: 当前定值区号
///   - SN2: 最小定值区号
///   - SN3: 最大定值区号
#[derive(Debug, Clone)]
pub struct ReadSettingZoneNumber {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 信息对象地址
    pub ioa: u32,
    /// 当前定值区号 (SN1)，控制方向时为 None
    pub current_sn: Option<u16>,
    /// 最小定值区号 (SN2)，仅监视方向
    pub min_sn: Option<u16>,
    /// 最大定值区号 (SN3)，仅监视方向
    pub max_sn: Option<u16>,
}

impl ProfileAsdu for ReadSettingZoneNumber {
    fn type_id(&self) -> TypeId {
        TypeId::new(201)
    }

    fn type_name(&self) -> &'static str {
        "C_RR_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析读定值区号命令/响应 (TI 201)。
pub fn parse_read_setting_zone_number<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    // 格式:
    // 控制方向: IOA(3) = 3字节
    // 监视方向: IOA(3) + SN1(2) + SN2(2) + SN3(2) = 9字节

    if payload.len() < 3 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 3,
            available: payload.len(),
            ti: header.type_id,
            ioa: None,
            entry_index: None,
            reason: "读定值区号命令数据不足",
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);

    // 实施细则 COT:
    // - 控制方向: act(6)
    // - 监视方向: actcon(7), actterm(10)
    let monitor_direction =
        matches!(header.cot.reason, CotReason::ActivationConfirmation | CotReason::ActivationTermination);
    let control_direction = matches!(header.cot.reason, CotReason::Activation);

    if monitor_direction {
        if payload.len() < 9 {
            return Err(ParseError::InsufficientInfoObject {
                needed: 9,
                available: payload.len(),
                ti: header.type_id,
                ioa: Some(ioa),
                entry_index: None,
                reason: "监视方向读定值区号数据不足",
            });
        }
        if payload.len() != 9 {
            return Err(ParseError::InvalidInfoObject {
                reason: "监视方向读定值区号存在多余数据",
                ti: header.type_id,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(9),
            });
        }

        let current_sn = u16::from_le_bytes([payload[3], payload[4]]);
        let min_sn = u16::from_le_bytes([payload[5], payload[6]]);
        let max_sn = u16::from_le_bytes([payload[7], payload[8]]);

        return Ok(Box::new(ReadSettingZoneNumber {
            header: *header,
            ioa,
            current_sn: Some(current_sn),
            min_sn: Some(min_sn),
            max_sn: Some(max_sn),
        }));
    }

    if control_direction {
        if payload.len() != 3 {
            return Err(ParseError::InvalidInfoObject {
                reason: "控制方向读定值区号长度非法",
                ti: header.type_id,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(3),
            });
        }

        return Ok(Box::new(ReadSettingZoneNumber {
            header: *header,
            ioa,
            current_sn: None,
            min_sn: None,
            max_sn: None,
        }));
    }

    // 未知 COT 兜底：按长度判定，保证兼容性。
    match payload.len() {
        3 => Ok(Box::new(ReadSettingZoneNumber { header: *header, ioa, current_sn: None, min_sn: None, max_sn: None })),
        9 => {
            let current_sn = u16::from_le_bytes([payload[3], payload[4]]);
            let min_sn = u16::from_le_bytes([payload[5], payload[6]]);
            let max_sn = u16::from_le_bytes([payload[7], payload[8]]);
            Ok(Box::new(ReadSettingZoneNumber {
                header: *header,
                ioa,
                current_sn: Some(current_sn),
                min_sn: Some(min_sn),
                max_sn: Some(max_sn),
            }))
        }
        _ => Err(ParseError::InvalidInfoObject {
            reason: "读定值区号长度与方向不匹配",
            ti: header.type_id,
            ioa: Some(ioa),
            entry_index: None,
            offset: Some(payload.len()),
        }),
    }
}

// ============================================================================
// TI 202: 读参数和定值
// ============================================================================

/// 参数特征标识 PI: CP8{CONT, RES, CR, S/E}。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamIdentifier {
    /// 原始 PI 字节。
    pub raw: u8,
}

impl ParamIdentifier {
    /// CONT 位（bit0）: 0=无后续, 1=有后续。
    pub fn has_follow(self) -> bool {
        (self.raw & 0x01) != 0
    }

    /// CR 位（bit6）: 1=取消预置。
    pub fn cancel_preset(self) -> bool {
        (self.raw & 0x40) != 0
    }

    /// S/E 位（bit7）: 1=预置, 0=固化。
    pub fn is_preset(self) -> bool {
        (self.raw & 0x80) != 0
    }

    /// RES 位（bit1..bit5）应为 0。
    pub fn reserved_bits(self) -> u8 {
        self.raw & 0x3E
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadSettingParamDirection {
    Control,
    Monitor,
}

/// TI 202: 读参数和定值命令/响应。
///
/// ## 控制方向报文格式（召唤）
/// - IOA(3) + SN(2) + [参数IOA列表(N*3)]
///
/// ## 监视方向报文格式（响应）
/// - IOA(3) + SN(2) + PI(1) + [参数条目...]
/// - 参数条目: IOA(3) + Tag(1) + 长度(1) + 值(N)
#[derive(Debug, Clone)]
pub struct ReadSettingParam {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 信息对象地址
    pub ioa: u32,
    /// 定值区号 (SN)，2字节
    pub sn: u16,
    /// 报文方向
    pub direction: ReadSettingParamDirection,
    /// 参数特征标识 (PI)，仅监视方向存在
    pub pi: Option<ParamIdentifier>,
    /// 参数 IOA 列表（控制方向）
    pub param_ioas: SmallVec<[u32; 8]>,
    /// 解析后的参数条目（监视方向）
    pub params: SmallVec<[SettingParam; 8]>,
}

/// 单个定值参数。
#[derive(Debug, Clone)]
pub struct SettingParam {
    /// 参数信息对象地址
    pub ioa: u32,
    /// 数据类型标签 (Tag)
    pub tag: u8,
    /// 参数值（原始字节）
    pub value: Vec<u8>,
}

impl ProfileAsdu for ReadSettingParam {
    fn type_id(&self) -> TypeId {
        TypeId::new(202)
    }

    fn type_name(&self) -> &'static str {
        "C_RS_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn parse_param_entries(param_data: &[u8], ti: TypeId, ioa: u32) -> Result<SmallVec<[SettingParam; 8]>, ParseError> {
    let mut params = SmallVec::new();
    let mut pos = 0usize;

    while pos < param_data.len() {
        if pos + 5 > param_data.len() {
            return Err(ParseError::InsufficientInfoObject {
                needed: pos + 5,
                available: param_data.len(),
                ti,
                ioa: Some(ioa),
                entry_index: Some(params.len()),
                reason: "参数条目头部不完整",
            });
        }

        let param_ioa = u32::from_le_bytes([param_data[pos], param_data[pos + 1], param_data[pos + 2], 0]);
        let tag = param_data[pos + 3];
        let value_len = param_data[pos + 4] as usize;

        if pos + 5 + value_len > param_data.len() {
            return Err(ParseError::InsufficientInfoObject {
                needed: pos + 5 + value_len,
                available: param_data.len(),
                ti,
                ioa: Some(ioa),
                entry_index: Some(params.len()),
                reason: "参数值数据不足",
            });
        }

        let value = param_data[pos + 5..pos + 5 + value_len].to_vec();
        params.push(SettingParam { ioa: param_ioa, tag, value });
        pos += 5 + value_len;
    }

    Ok(params)
}

/// 解析读参数和定值命令/响应 (TI 202)。
pub fn parse_read_setting_param<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    let ti = header.type_id;

    // 最小格式: IOA(3) + SN(2) = 5字节
    if payload.len() < 5 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 5,
            available: payload.len(),
            ti,
            ioa: None,
            entry_index: None,
            reason: "读参数和定值命令数据不足",
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
    let sn = u16::from_le_bytes([payload[3], payload[4]]);
    let remaining = &payload[5..];

    let parse_as_control = || -> Result<ReadSettingParam, ParseError> {
        if remaining.len() % 3 != 0 {
            return Err(ParseError::InvalidInfoObject {
                reason: "控制方向参数 IOA 列表长度非法",
                ti,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(5),
            });
        }

        let mut param_ioas = SmallVec::new();
        let mut pos = 0usize;
        while pos < remaining.len() {
            let param_ioa = u32::from_le_bytes([remaining[pos], remaining[pos + 1], remaining[pos + 2], 0]);
            param_ioas.push(param_ioa);
            pos += 3;
        }

        Ok(ReadSettingParam {
            header: *header,
            ioa,
            sn,
            direction: ReadSettingParamDirection::Control,
            pi: None,
            param_ioas,
            params: SmallVec::new(),
        })
    };

    let parse_as_monitor = || -> Result<ReadSettingParam, ParseError> {
        if remaining.is_empty() {
            return Err(ParseError::InsufficientInfoObject {
                needed: 6,
                available: payload.len(),
                ti,
                ioa: Some(ioa),
                entry_index: None,
                reason: "监视方向缺少 PI 字段",
            });
        }

        let pi = ParamIdentifier { raw: remaining[0] };
        if pi.reserved_bits() != 0 {
            return Err(ParseError::InvalidInfoObject {
                reason: "PI 保留位必须为 0",
                ti,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(5),
            });
        }

        let params = parse_param_entries(&remaining[1..], ti, ioa)?;

        Ok(ReadSettingParam {
            header: *header,
            ioa,
            sn,
            direction: ReadSettingParamDirection::Monitor,
            pi: Some(pi),
            param_ioas: SmallVec::new(),
            params,
        })
    };

    let parsed = match header.cot.reason {
        CotReason::Activation => parse_as_control()?,
        CotReason::ActivationConfirmation | CotReason::ActivationTermination => parse_as_monitor()?,
        _ => {
            // 未知 COT 时尝试双分支，保证兼容并避免启发式误判。
            match (parse_as_control(), parse_as_monitor()) {
                (Ok(control), Err(_)) => control,
                (Err(_), Ok(monitor)) => monitor,
                (Ok(_), Ok(monitor)) => monitor,
                (Err(control_err), Err(_)) => return Err(control_err),
            }
        }
    };

    Ok(Box::new(parsed))
}

// ============================================================================
// TI 203: 写参数和定值
// ============================================================================

/// 写操作类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WriteOperationType {
    /// 预置（S/E=1, CR=0）
    Preset,
    /// 固化（S/E=0, CR=0）
    Solidify,
    /// 撤销预置（CR=1）
    CancelPreset,
}

impl WriteOperationType {
    fn from_pi(pi: ParamIdentifier) -> Self {
        if pi.cancel_preset() {
            Self::CancelPreset
        } else if pi.is_preset() {
            Self::Preset
        } else {
            Self::Solidify
        }
    }
}

/// TI 203: 写参数和定值命令。
///
/// ## 预置报文格式
/// - IOA(3) + SN(2) + PI(1) + [参数条目...]
/// - 参数条目: IOA(3) + Tag(1) + 长度(1) + 值(N)
///
/// ## 固化/撤销报文格式
/// - IOA(3) + SN(2) + PI(1)
#[derive(Debug, Clone)]
pub struct WriteSettingParam {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 信息对象地址
    pub ioa: u32,
    /// 定值区号 (SN)，2字节
    pub sn: u16,
    /// 参数特征标识 PI
    pub pi: ParamIdentifier,
    /// 操作类型
    pub operation: WriteOperationType,
    /// 参数条目（仅预置时有效）
    pub params: SmallVec<[SettingParam; 8]>,
}

impl ProfileAsdu for WriteSettingParam {
    fn type_id(&self) -> TypeId {
        TypeId::new(203)
    }

    fn type_name(&self) -> &'static str {
        "C_WS_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析写参数和定值命令 (TI 203)。
pub fn parse_write_setting_param<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    let ti = header.type_id;

    // 最小格式: IOA(3) + SN(2) + PI(1) = 6字节
    if payload.len() < 6 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 6,
            available: payload.len(),
            ti,
            ioa: None,
            entry_index: None,
            reason: "写参数和定值命令数据不足",
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
    let sn = u16::from_le_bytes([payload[3], payload[4]]);
    let pi = ParamIdentifier { raw: payload[5] };
    if pi.reserved_bits() != 0 {
        return Err(ParseError::InvalidInfoObject {
            reason: "PI 保留位必须为 0",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: Some(5),
        });
    }

    let operation = WriteOperationType::from_pi(pi);
    let param_data = &payload[6..];

    let params = match operation {
        WriteOperationType::Preset => {
            let params = parse_param_entries(param_data, ti, ioa)?;
            if params.is_empty() {
                return Err(ParseError::InvalidInfoObject {
                    reason: "预置操作缺少参数条目",
                    ti,
                    ioa: Some(ioa),
                    entry_index: None,
                    offset: Some(6),
                });
            }
            params
        }
        WriteOperationType::Solidify | WriteOperationType::CancelPreset => {
            if !param_data.is_empty() {
                return Err(ParseError::InvalidInfoObject {
                    reason: "固化/撤销报文不应包含参数条目",
                    ti,
                    ioa: Some(ioa),
                    entry_index: None,
                    offset: Some(6),
                });
            }
            SmallVec::new()
        }
    };

    Ok(Box::new(WriteSettingParam { header: *header, ioa, sn, pi, operation, params }))
}
