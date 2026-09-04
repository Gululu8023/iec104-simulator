//! 参数类 ASDU 解析（TI 110~113）。
//!
//! 本模块负责解析 IEC 60870-5-104 协议中的参数信息，包括：
//!
//! - **归一化值参数** (TI 110): 带限定词的归一化参数 (NVA + QPM)
//! - **标度化值参数** (TI 111): 带限定词的标度化参数 (SVA + QPM)
//! - **短浮点数参数** (TI 112): 带限定词的 IEEE 754 单精度浮点数 (R32 + QPM)
//! - **参数激活** (TI 113): 激活参数的标识符 (QPA)
//!
//! ## 数据格式
//!
//! 不同类型参数的数据结构：
//!
//! - TI 110 (归一化): 2 字节值 + 1 字节限定词 (QPM)
//! - TI 111 (标度化): 2 字节值 + 1 字节限定词 (QPM)
//! - TI 112 (短浮点): 4 字节 IEEE 754 + 1 字节限定词 (QPM)
//! - TI 113 (激活): 1 字节激活限定词 (QPA)
//!
//! ## 限定词说明
//!
//! - **QPM (参数限定词)**: 指示参数的变化状态和操作模式
//! - **QPA (参数激活限定词)**: 指示参数激活或解除激活
//!
//! ## 示例
//!
//! ```ignore
//! use iec60870_parser::parser::asdu::{AsduHeader, TypeId, Cot, CotReason};
//! use iec60870_parser::parser::asdu::parameters::parse_parameters;
//!
//! let header = AsduHeader::new(TypeId::new(110), 1, Cot::new(CotReason::Activation), 1);
//! let payload = [
//!     0x01, 0x00, 0x00,  // IOA
//!     0x34, 0x12,        // 归一化值 (小端序)
//!     0xAA,              // QPM
//! ];
//!
//! let params = parse_parameters(header, &payload).unwrap();
//! assert_eq!(params.items.len(), 1);
//! ```

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, TypeId, for_each_entry, lookup_type_descriptor, spec},
};

/// 参数集合。
///
/// 包含 ASDU 头部和若干条参数记录。
#[derive(Debug, Clone)]
pub struct ParameterSet<'a> {
    /// ASDU 头部信息。
    pub header: AsduHeader,
    /// 参数记录列表。
    pub items: SmallVec<[Parameter<'a>; 4]>,
}

/// 单条参数记录。
#[derive(Debug, Clone)]
pub struct Parameter<'a> {
    /// 信息对象地址 (IOA)。
    pub ioa: u32,
    /// 参数值。
    pub value: ParameterValue<'a>,
}

/// 参数值类型。
///
/// 根据 Type ID 区分不同类型的参数：
/// - `Normalized`: 归一化值参数 (TI 110)，范围 -1.0 到 +1.0
/// - `Scaled`: 标度化值参数 (TI 111)，整数值
/// - `ShortFloat`: IEEE 754 单精度浮点数参数 (TI 112)
/// - `Activation`: 参数激活标识符 (TI 113)
/// - `Raw`: 未识别的参数类型，保留原始字节
#[derive(Debug, Clone)]
pub enum ParameterValue<'a> {
    /// 归一化值参数 (TI 110)。
    ///
    /// - `value`: 归一化值 (-32768 到 +32767，映射到 -1.0 到 +1.0)
    /// - `qualifier`: 参数限定词 (QPM)
    Normalized { value: i16, qualifier: u8 },
    /// 标度化值参数 (TI 111)。
    ///
    /// - `value`: 标度化整数值
    /// - `qualifier`: 参数限定词 (QPM)
    Scaled { value: i16, qualifier: u8 },
    /// 短浮点数参数 (TI 112)。
    ///
    /// - `value`: IEEE 754 单精度浮点数
    /// - `qualifier`: 参数限定词 (QPM)
    ShortFloat { value: f32, qualifier: u8 },
    /// 参数激活 (TI 113)。
    ///
    /// - 包含激活限定词 (QPA)
    Activation(u8),
    /// 未识别的参数类型（原始字节）。
    Raw(&'a [u8]),
}

/// 解析参数 ASDU。
///
/// 支持 TI 110~113 等参数类型。根据 Type ID 自动选择
/// 相应的解析方式。
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
/// let params = parse_parameters(header, &payload)?;
/// for param in &params.items {
///     match param.value {
///         ParameterValue::Normalized { value, qualifier } => {
///             println!("Normalized: {} (QPM: 0x{:02X})", value, qualifier);
///         }
///         _ => {}
///     }
/// }
/// ```
pub fn parse_parameters<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<ParameterSet<'a>, ParseError> {
    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut items = SmallVec::new();
    for_each_entry(header.type_id, payload, descriptor, header.vsq, |ioa, body| {
        let value = match header.type_id.raw() {
            110 => decode_parameter_normalized(header.type_id, body)?,
            111 => decode_parameter_scaled(header.type_id, body)?,
            112 => decode_parameter_float(header.type_id, body)?,
            113 => decode_parameter_activation(header.type_id, body)?,
            _ => ParameterValue::Raw(body),
        };
        items.push(Parameter { ioa, value });
        Ok(())
    })?;

    Ok(ParameterSet { header, items })
}

fn decode_parameter_normalized(ti: TypeId, input: &[u8]) -> Result<ParameterValue<'_>, ParseError> {
    if input.len() < 3 {
        return Err(ParseError::insufficient_info_object("参数长度不足", ti, None, None, 3, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    let qualifier = input[2];
    if !spec::is_valid_qpm_kind(qualifier & 0x3F) {
        return Err(ParseError::invalid_info_object("QPM 参数类型非法", ti, None, None, None));
    }
    Ok(ParameterValue::Normalized { value, qualifier })
}

fn decode_parameter_scaled(ti: TypeId, input: &[u8]) -> Result<ParameterValue<'_>, ParseError> {
    if input.len() < 3 {
        return Err(ParseError::insufficient_info_object("参数长度不足", ti, None, None, 3, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    let qualifier = input[2];
    if !spec::is_valid_qpm_kind(qualifier & 0x3F) {
        return Err(ParseError::invalid_info_object("QPM 参数类型非法", ti, None, None, None));
    }
    Ok(ParameterValue::Scaled { value, qualifier })
}

fn decode_parameter_float(ti: TypeId, input: &[u8]) -> Result<ParameterValue<'_>, ParseError> {
    if input.len() < 5 {
        return Err(ParseError::insufficient_info_object("参数长度不足", ti, None, None, 5, input.len()));
    }
    let value = f32::from_bits(u32::from_le_bytes([input[0], input[1], input[2], input[3]]));
    let qualifier = input[4];
    if !spec::is_valid_qpm_kind(qualifier & 0x3F) {
        return Err(ParseError::invalid_info_object("QPM 参数类型非法", ti, None, None, None));
    }
    Ok(ParameterValue::ShortFloat { value, qualifier })
}

fn decode_parameter_activation(ti: TypeId, input: &[u8]) -> Result<ParameterValue<'_>, ParseError> {
    if input.is_empty() {
        return Err(ParseError::insufficient_info_object("参数激活长度不足", ti, None, None, 1, input.len()));
    }
    let qualifier = input[0];
    if !spec::is_valid_qpa(qualifier) {
        return Err(ParseError::invalid_info_object("QPA 参数激活限定词非法", ti, None, None, None));
    }
    Ok(ParameterValue::Activation(qualifier))
}
