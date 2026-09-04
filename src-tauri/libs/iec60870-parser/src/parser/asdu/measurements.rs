//! 遥测 / 测量类 ASDU 解析（TI 1~21, 30~37 等）。
//!
//! 根据 `docs/ARCHITECTURE.md` 中的定义，解析逻辑需要保证零拷贝：除时间戳等字段外，
//! 全部字段均为切片视图，调用方可按需拷贝。

use std::fmt;

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{
        AsduHeader, TypeId,
        commands::{DoublePointState, SinglePointState},
        for_each_entry,
        qualifiers::{QualityFlags, parse_wire_quality},
        split_timestamp,
        timestamp::Timestamp,
        types::{TypeDescriptor, lookup_type_descriptor},
    },
};

/// 测量值集合，一个 ASDU 对应多个信息体。
#[derive(Debug, Clone)]
pub struct MeasurementSet<'a> {
    /// ASDU 头部。
    pub header: AsduHeader,
    /// 信息体列表。
    pub items: SmallVec<[Measurement<'a>; 4]>,
}

/// 单条测量值记录。
#[derive(Debug, Clone)]
pub struct Measurement<'a> {
    /// 信息体地址。
    pub ioa: u32,
    /// 具体类型及数值。
    pub value: MeasurementValue<'a>,
    /// 质量标志。
    pub quality: QualityFlags,
    /// 描述词（SIQ/DIQ/QDS/BCR）细节。
    pub descriptor: MeasurementDescriptor,
    /// 可选时间戳（CP24/CP56）。
    pub timestamp: Option<&'a [u8]>,
}

impl<'a> Measurement<'a> {
    /// 解析时间戳（如果存在）
    pub fn parsed_timestamp(&self) -> Result<Option<Timestamp>, ParseError> {
        self.timestamp.map(Timestamp::parse).transpose()
    }
}

/// 描述词细节（不再把所有语义折叠为统一质量结构）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementDescriptor {
    Siq {
        raw: u8,
        spi: bool,
        iv: bool,
        nt: bool,
        sb: bool,
        bl: bool,
    },
    Diq {
        raw: u8,
        dpi: u8,
        iv: bool,
        nt: bool,
        sb: bool,
        bl: bool,
    },
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
    Bcr {
        raw: u8,
        sq: u8,
        cy: bool,
        ca: bool,
        iv: bool,
    },
    None,
}

/// 测量值类型。
#[derive(Debug, Clone)]
pub enum MeasurementValue<'a> {
    SinglePoint { state: SinglePointState },
    DoublePoint { state: DoublePointState },
    StepPosition { value: i8, transient: bool },
    Normalized(i16),
    Scaled(i16),
    NormalizedNoQuality(i16),
    ShortFloat(f32),
    BitString(&'a [u8]),
    IntegratedTotal { value: i32, sequence: u8 },
    PackedStatus { status: u16, change: u16 },
    Raw(&'a [u8]),
}

impl<'a> MeasurementValue<'a> {
    /// 获取单点测量的状态（如果是SinglePoint）
    pub fn single_state(&self) -> Option<SinglePointState> {
        match self {
            Self::SinglePoint { state } => Some(*state),
            _ => None,
        }
    }

    /// 获取双点测量的状态（如果是DoublePoint）
    pub fn double_state(&self) -> Option<DoublePointState> {
        match self {
            Self::DoublePoint { state } => Some(*state),
            _ => None,
        }
    }

    /// 测量值是否为On状态（适用于SinglePoint/DoublePoint）
    pub fn is_on(&self) -> Option<bool> {
        match self {
            Self::SinglePoint { state } => Some(state.is_on()),
            Self::DoublePoint { state } => Some(state.is_on()),
            _ => None,
        }
    }

    /// 测量值是否为Off状态（适用于SinglePoint/DoublePoint）
    pub fn is_off(&self) -> Option<bool> {
        match self {
            Self::SinglePoint { state } => Some(state.is_off()),
            Self::DoublePoint { state } => Some(state.is_off()),
            _ => None,
        }
    }

    /// 获取归一化数值（-1.0 到 +1.0）
    ///
    /// 根据IEC 60870-5-4标准：
    /// - 归一化值使用16位有符号整数（-32768 到 +32767）
    /// - 映射公式：normalized = value / 2^15 = value / 32768.0
    /// - 范围：-1.0 ≤ normalized < +1.0
    /// - 注意：+32767/32768 ≈ 0.999969，无法精确表示 +1.0
    ///
    /// 注意：此方法仅适用于Normalized和NormalizedNoQuality类型。
    /// ShortFloat类型表示实际物理量，不进行归一化，请使用`as_float()`获取其值。
    pub fn as_normalized(&self) -> Option<f32> {
        match self {
            Self::Normalized(v) | Self::NormalizedNoQuality(v) => {
                // IEC 60870-5-4标准：归一化值 = 原始值 / 32768
                Some(*v as f32 / 32768.0)
            }
            _ => None,
        }
    }

    /// 获取原始整数值（适用于 Normalized/Scaled/StepPosition/IntegratedTotal）
    pub fn as_raw_int(&self) -> Option<i32> {
        match self {
            Self::Normalized(v) | Self::Scaled(v) | Self::NormalizedNoQuality(v) => Some(*v as i32),
            Self::StepPosition { value, .. } => Some(*value as i32),
            Self::IntegratedTotal { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// 获取标度化值（直接返回16位整数，不进行归一化）
    ///
    /// 标度化值是直接的物理量表示，需要配合系统配置的缩放系数使用。
    /// 例如：如果缩放系数为0.01，则值1234表示12.34的实际物理量。
    pub fn as_scaled(&self) -> Option<i16> {
        match self {
            Self::Scaled(v) => Some(*v),
            _ => None,
        }
    }

    /// 获取浮点数值
    ///
    /// 注意：
    /// - 对于Normalized：返回原始整数值转换的f32（未归一化）
    /// - 对于Scaled：返回标度化整数值转换的f32
    /// - 对于ShortFloat：返回IEEE 754单精度浮点数
    /// - 使用 `as_normalized()` 获取归一化后的值（-1.0到+1.0）
    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Normalized(v) | Self::Scaled(v) | Self::NormalizedNoQuality(v) => Some(*v as f32),
            Self::StepPosition { value, .. } => Some(*value as f32),
            Self::ShortFloat(v) => Some(*v),
            Self::IntegratedTotal { value, .. } => Some(*value as f32),
            _ => None,
        }
    }

    /// 检查短浮点数是否为有效数值（非NaN、非Infinity）
    ///
    /// 根据IEC 60870-5-4标准，短浮点数使用标准IEEE 754单精度格式。
    /// 在电力系统中，NaN和Infinity通常表示无效或异常状态。
    pub fn is_valid_float(&self) -> bool {
        match self {
            Self::ShortFloat(v) => v.is_finite(),
            _ => true,
        }
    }

    /// 检查归一化值是否在有效范围内（-1.0 到 +1.0）
    ///
    /// 根据IEC 60870-5-4标准：
    /// - 归一化值范围：-1.0 ≤ normalized < +1.0
    /// - 注意：最大值是+0.999969，无法精确表示+1.0
    ///
    /// 对于Normalized/NormalizedNoQuality类型，由于使用16位整数表示，
    /// 理论上不会超出范围，此方法总是返回true。
    ///
    /// 对于ShortFloat类型，此方法检查是否在归一化范围内（-1.0 ≤ v < +1.0）。
    pub fn is_valid_normalized(&self) -> bool {
        match self {
            Self::Normalized(_) | Self::NormalizedNoQuality(_) => {
                // 16位有符号整数的完整范围都是有效的
                true
            }
            Self::Scaled(_) => true,
            Self::ShortFloat(v) => {
                // IEC标准：-1.0 ≤ normalized < +1.0，不包括+1.0
                v.is_finite() && *v >= -1.0 && *v < 1.0
            }
            _ => true,
        }
    }
}

impl<'a> fmt::Display for MeasurementValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SinglePoint { state } => {
                write!(f, "SinglePoint({})", state)
            }
            Self::DoublePoint { state } => {
                write!(f, "DoublePoint({})", state)
            }
            Self::StepPosition { value, transient } => {
                write!(f, "StepPosition(value={}, transient={})", value, transient)
            }
            Self::Normalized(v) => {
                write!(f, "Normalized({}, {:.3})", v, *v as f32 / 32768.0)
            }
            Self::Scaled(v) => {
                write!(f, "Scaled({})", v)
            }
            Self::NormalizedNoQuality(v) => {
                write!(f, "NormalizedNoQuality({}, {:.3})", v, *v as f32 / 32768.0)
            }
            Self::ShortFloat(v) => {
                write!(f, "ShortFloat({:.6})", v)
            }
            Self::BitString(bits) => {
                write!(f, "BitString(len={})", bits.len())
            }
            Self::IntegratedTotal { value, sequence } => {
                write!(f, "IntegratedTotal(value={}, seq={})", value, sequence)
            }
            Self::PackedStatus { status, change } => {
                write!(f, "PackedStatus(status=0x{status:04X}, change=0x{change:04X})")
            }
            Self::Raw(data) => {
                write!(f, "Raw(len={})", data.len())
            }
        }
    }
}

/// 解析测量类 ASDU（TI = 1/2/3/4/5/7/9/11/13/15/20/21/30…）。
pub fn parse_measurements<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<MeasurementSet<'a>, ParseError> {
    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut items = SmallVec::with_capacity(((header.vsq & 0x7F) as usize).min(4));
    for_each_entry(header.type_id, payload, descriptor, header.vsq, |ioa, body| {
        let measurement = decode_measurement(header.type_id, descriptor, ioa, body)?;
        items.push(measurement);
        Ok(())
    })?;
    Ok(MeasurementSet { header, items })
}

fn decode_measurement<'a>(
    ti: TypeId,
    descriptor: &TypeDescriptor,
    ioa: u32,
    body: &'a [u8],
) -> Result<Measurement<'a>, ParseError> {
    let (core, timestamp) = split_timestamp(ti, body, descriptor.timestamp_len)?;
    let (value, quality, descriptor) = match ti.raw() {
        1 | 2 | 30 => decode_single_point(ti, core)?,
        3 | 4 | 31 => decode_double_point(ti, core)?,
        5 | 6 | 32 => decode_step_position(ti, core)?,
        7 | 8 | 33 => decode_bitstring(ti, core)?,
        9 | 10 | 34 => decode_normalized(ti, core)?,
        11 | 12 | 35 => decode_scaled(ti, core)?,
        13 | 14 | 36 => decode_short_float(ti, core)?,
        15 | 16 | 37 => decode_integrated_total(ti, core)?,
        20 => decode_packed_status(ti, core)?,
        21 => decode_without_quality(ti, core)?,
        _ => (MeasurementValue::Raw(core), QualityFlags::default(), MeasurementDescriptor::None),
    };

    Ok(Measurement { ioa, value, quality, descriptor, timestamp })
}

fn decode_single_point<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.is_empty() {
        return Err(ParseError::insufficient_info_object("SIQ 长度不足", ti, None, None, 1, input.len()));
    }
    let siq = input[0];
    let state = SinglePointState::from_bool(siq & 0x01 != 0);
    let quality = parse_wire_quality(siq);
    Ok((
        MeasurementValue::SinglePoint { state },
        // SIQ 线路格式: bit0=SPI(值), bit4=BL, bit5=SB, bit6=NT, bit7=IV
        quality,
        MeasurementDescriptor::Siq {
            raw: siq,
            spi: state.as_bool(),
            iv: quality.iv,
            nt: quality.nt,
            sb: quality.sb,
            bl: quality.bl,
        },
    ))
}

fn decode_double_point<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.is_empty() {
        return Err(ParseError::insufficient_info_object("DIQ 长度不足", ti, None, None, 1, input.len()));
    }
    let diq = input[0];
    let state = DoublePointState::from_u8(diq & 0x03);
    let quality = parse_wire_quality(diq);
    Ok((
        MeasurementValue::DoublePoint { state },
        // DIQ 线路格式: bit0-1=DPI(值), bit4=BL, bit5=SB, bit6=NT, bit7=IV
        quality,
        MeasurementDescriptor::Diq {
            raw: diq,
            dpi: state.as_u8(),
            iv: quality.iv,
            nt: quality.nt,
            sb: quality.sb,
            bl: quality.bl,
        },
    ))
}

fn decode_step_position<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 2 {
        return Err(ParseError::insufficient_info_object("步进位置长度不足", ti, None, None, 2, input.len()));
    }
    // VTI: bit0..6 = 7bit two's complement step value, bit7 = transient state
    let vti = input[0];
    let raw = (vti & 0x7F) as i8;
    let value = if raw & 0x40 != 0 { raw | !0x7F_i8 } else { raw };
    let transient = (vti & 0x80) != 0;
    let qds = input[1];
    let quality = parse_wire_quality(qds);
    Ok((MeasurementValue::StepPosition { value, transient }, quality, MeasurementDescriptor::Qds {
        raw: qds,
        ov: (qds & 0x01) != 0,
        iv: quality.iv,
        nt: quality.nt,
        sb: quality.sb,
        bl: quality.bl,
        transient: Some(transient),
        scd_status: None,
        scd_change: None,
    }))
}

fn decode_bitstring<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 5 {
        return Err(ParseError::insufficient_info_object("位串长度不足", ti, None, None, 5, input.len()));
    }
    let (bits, qds) = input.split_at(4);
    let raw = qds[0];
    let quality = parse_wire_quality(raw);
    Ok((MeasurementValue::BitString(bits), quality, MeasurementDescriptor::Qds {
        raw,
        ov: (raw & 0x01) != 0,
        iv: quality.iv,
        nt: quality.nt,
        sb: quality.sb,
        bl: quality.bl,
        transient: None,
        scd_status: None,
        scd_change: None,
    }))
}

fn decode_normalized<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 3 {
        return Err(ParseError::insufficient_info_object("测量值长度不足", ti, None, None, 3, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    let raw = input[2];
    let quality = parse_wire_quality(raw);
    Ok((MeasurementValue::Normalized(value), quality, MeasurementDescriptor::Qds {
        raw,
        ov: (raw & 0x01) != 0,
        iv: quality.iv,
        nt: quality.nt,
        sb: quality.sb,
        bl: quality.bl,
        transient: None,
        scd_status: None,
        scd_change: None,
    }))
}

fn decode_scaled<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 3 {
        return Err(ParseError::insufficient_info_object("标度化测量值长度不足", ti, None, None, 3, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    let raw = input[2];
    let quality = parse_wire_quality(raw);
    Ok((MeasurementValue::Scaled(value), quality, MeasurementDescriptor::Qds {
        raw,
        ov: (raw & 0x01) != 0,
        iv: quality.iv,
        nt: quality.nt,
        sb: quality.sb,
        bl: quality.bl,
        transient: None,
        scd_status: None,
        scd_change: None,
    }))
}

/// 解析短浮点数（IEEE 754 单精度）。
///
/// 根据IEC 60870-5-4标准：
/// - 使用标准IEEE 754单精度浮点数格式（32位）
/// - 字节序：小端序（LSB first）
/// - 格式：符号位(1) + 指数(8) + 尾数(23)
/// - 支持特殊值：±0, ±Infinity, NaN
/// - 有效范围：±3.4E+38（理论值），实际使用范围根据应用而定
fn decode_short_float<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 5 {
        return Err(ParseError::insufficient_info_object("浮点测量值长度不足", ti, None, None, 5, input.len()));
    }
    // IEEE 754单精度浮点数：小端序读取4字节
    let value = f32::from_bits(u32::from_le_bytes([input[0], input[1], input[2], input[3]]));
    let raw = input[4];
    let quality = parse_wire_quality(raw);
    Ok((MeasurementValue::ShortFloat(value), quality, MeasurementDescriptor::Qds {
        raw,
        ov: (raw & 0x01) != 0,
        iv: quality.iv,
        nt: quality.nt,
        sb: quality.sb,
        bl: quality.bl,
        transient: None,
        scd_status: None,
        scd_change: None,
    }))
}

fn decode_integrated_total<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 5 {
        return Err(ParseError::insufficient_info_object("累计量长度不足", ti, None, None, 5, input.len()));
    }
    let value = i32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    // BCR (5th byte): bit0-4=SQ(sequence), bit5=CY, bit6=CA, bit7=IV
    let bcr = input[4];
    let seq = bcr & 0x1F;
    let quality = QualityFlags { iv: bcr & 0x80 != 0, nt: false, sb: false, bl: false, ei: false };
    Ok((MeasurementValue::IntegratedTotal { value, sequence: seq }, quality, MeasurementDescriptor::Bcr {
        raw: bcr,
        sq: seq,
        cy: (bcr & 0x20) != 0,
        ca: (bcr & 0x40) != 0,
        iv: quality.iv,
    }))
}

fn decode_packed_status<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 5 {
        return Err(ParseError::insufficient_info_object("打包状态长度不足", ti, None, None, 5, input.len()));
    }
    let status = u16::from_le_bytes([input[0], input[1]]);
    let change = u16::from_le_bytes([input[2], input[3]]);
    let qds = &input[4..];
    let raw = qds[0];
    let quality = parse_wire_quality(raw);
    Ok((MeasurementValue::PackedStatus { status, change }, quality, MeasurementDescriptor::Qds {
        raw,
        ov: (raw & 0x01) != 0,
        iv: quality.iv,
        nt: quality.nt,
        sb: quality.sb,
        bl: quality.bl,
        transient: None,
        scd_status: Some(status),
        scd_change: Some(change),
    }))
}

fn decode_without_quality<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(MeasurementValue<'a>, QualityFlags, MeasurementDescriptor), ParseError> {
    if input.len() < 2 {
        return Err(ParseError::insufficient_info_object("测量值长度不足", ti, None, None, 2, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    Ok((MeasurementValue::NormalizedNoQuality(value), QualityFlags::default(), MeasurementDescriptor::None))
}
