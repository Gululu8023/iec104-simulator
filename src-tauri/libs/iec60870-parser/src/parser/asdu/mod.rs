//! ASDU 解析核心模块：定义头部、工具函数及子模块。
//!
//! 解析流程：
//! 1. 调用 [`parse_asdu_header`] 读取 TI/VSQ/COT/CA。
//! 2. 通过 [`types::lookup_type_descriptor`] 获取结构信息。
//! 3. 使用各子模块（`commands`/`measurements`/`file_transfer` 等）解析 payload。

pub mod commands;
pub mod cot;
pub mod encode;
pub mod events;
pub mod file_transfer;
pub mod initialization;
pub mod interrogation;
pub mod measurements;
pub mod parameters;
pub mod qualifiers;
pub mod security;
pub mod spec;
pub mod test_command;
pub mod timestamp;
pub mod types;

pub use cot::{Cot, CotReason};
pub use timestamp::{CP24Time2a, CP56Time2a, Timestamp};
pub use types::{
    AsduCategory, InformationFamily, QualifierKind, QualityKind, TimestampKind, TypeDescriptor, TypeId, TypeIdEnum,
    ValueKind, lookup_type_descriptor, lookup_type_descriptor_by_kind,
};

use crate::{
    error::ParseError,
    profile::{ProfileAsdu, ProtocolProfile},
};

#[derive(Debug, Clone, Copy)]
pub struct AsduHeader {
    pub type_id: TypeId,
    pub vsq: u8,
    /// 传送原因（COT）
    pub cot: Cot,
    pub common_addr: u16,
}

impl AsduHeader {
    /// 创建新的ASDU头部（用于测试和手动构造）
    pub fn new(type_id: TypeId, vsq: u8, cot: Cot, common_addr: u16) -> Self {
        Self { type_id, vsq, cot, common_addr }
    }

    /// 使用类型安全的枚举创建ASDU头部。
    ///
    /// 这是推荐的构造方式，避免使用魔数。
    ///
    /// # 示例
    ///
    /// ```
    /// # use iec60870_parser::parser::asdu::{AsduHeader, TypeIdEnum, Cot, CotReason};
    /// let header = AsduHeader::from_kind(
    ///     TypeIdEnum::CScNa1,              // 单点命令
    ///     1,                               // vsq: 1个信息体
    ///     Cot::new(CotReason::Activation), // COT: activation
    ///     100,                             // common address
    /// );
    /// ```
    pub fn from_kind(kind: TypeIdEnum, vsq: u8, cot: Cot, common_addr: u16) -> Self {
        Self { type_id: TypeId::new(kind.to_u8()), vsq, cot, common_addr }
    }

    /// 获取传送原因代码（COT的低6位）
    pub fn cause_of_transmission(&self) -> u8 {
        self.cot.cause_of_transmission()
    }

    /// 获取Test位（COT的bit 7）
    pub fn is_test(&self) -> bool {
        self.cot.is_test()
    }

    /// 获取P/N位（COT的bit 6）
    pub fn is_negative_confirm(&self) -> bool {
        self.cot.is_negative_confirm()
    }

    /// 获取信息体数量
    pub fn obj_count(&self) -> u8 {
        self.vsq & 0x7F
    }

    /// 获取SQ位（是否连续地址）
    pub fn is_sequence(&self) -> bool {
        (self.vsq & 0x80) != 0
    }
}

#[derive(Debug)]
pub struct Asdu<'a> {
    pub header: AsduHeader,
    pub payload: &'a [u8],
}

/// 信息体地址（Information Object Address）占 3 字节，小端序。
pub fn parse_ioa(ti: TypeId, input: &[u8], ioa_len: usize) -> Result<(u32, &[u8]), ParseError> {
    if ioa_len == 0 {
        return Ok((0, input));
    }
    if ioa_len > 3 {
        return Err(ParseError::invalid_info_object("IOA 长度非法", ti, None, None, None));
    }
    if input.len() < ioa_len {
        return Err(ParseError::insufficient_info_object("IOA 长度不足", ti, None, None, ioa_len, input.len()));
    }

    let addr = match ioa_len {
        1 => input[0] as u32,
        2 => u16::from_le_bytes([input[0], input[1]]) as u32,
        3 => u32::from_le_bytes([input[0], input[1], input[2], 0]),
        _ => unreachable!("ioa_len 已在上方限制为 1..=3"),
    };

    Ok((addr, &input[ioa_len..]))
}

/// 遍历 ASDU payload 中的每条信息体（兼容 SQ=0/1）。
///
/// - `payload`: 仅包含信息体部分（不含 ASDU 头）。
/// - `descriptor`: 对应 TI 的结构定义。
/// - `vsq`: 原始 VSQ 字节。
/// - `f`: 闭包函数，入参为 `(ioa, body)`，`body` 已剥离 IOA。
pub fn for_each_entry<'a, F>(
    ti: TypeId,
    payload: &'a [u8],
    descriptor: &TypeDescriptor,
    vsq: u8,
    mut f: F,
) -> Result<(), ParseError>
where
    F: FnMut(u32, &'a [u8]) -> Result<(), ParseError>,
{
    let count = (vsq & 0x7F) as usize;
    if count == 0 {
        return Err(ParseError::invalid_info_object("VSQ 信息体数量为 0", ti, None, None, None));
    }
    let entry_len = descriptor.entry_len().ok_or(ParseError::invalid_info_object(
        "信息体长度可变，需专用解析",
        ti,
        None,
        None,
        None,
    ))?;
    let ioa_len = descriptor.ioa_len;
    // SQ 位仅在存在 IOA 时有意义；对 IOA=0 的类型，按 SQ=0 处理（逐条 entry 解析）。
    let sq = (vsq & 0x80 != 0) && ioa_len > 0;

    if !sq {
        let required = entry_len.checked_mul(count).ok_or(ParseError::invalid_info_object(
            "信息体长度溢出",
            ti,
            None,
            None,
            None,
        ))?;
        if payload.len() < required {
            return Err(ParseError::insufficient_info_object(
                "payload 长度不足",
                ti,
                None,
                None,
                required,
                payload.len(),
            ));
        }
        if payload.len() != required {
            return Err(ParseError::invalid_info_object("信息体存在剩余数据", ti, None, None, None));
        }
        for idx in 0..count {
            let entry = &payload[idx * entry_len..(idx + 1) * entry_len];
            let (ioa, body) = parse_ioa(ti, entry, ioa_len)?;
            f(ioa, body)?;
        }
        return Ok(());
    }

    let data_len = entry_len.checked_sub(ioa_len).ok_or(ParseError::invalid_info_object(
        "信息体长度不足",
        ti,
        None,
        None,
        None,
    ))?;
    // 需要 entry_len + data_len * (count-1) 字节
    let required = entry_len
        .checked_add(data_len.checked_mul(count.saturating_sub(1)).ok_or(ParseError::invalid_info_object(
            "信息体长度溢出",
            ti,
            None,
            None,
            None,
        ))?)
        .ok_or(ParseError::invalid_info_object("信息体长度溢出", ti, None, None, None))?;
    if payload.len() < required {
        return Err(ParseError::insufficient_info_object("payload 长度不足", ti, None, None, required, payload.len()));
    }

    let (first, mut rest) = payload.split_at(entry_len);
    let (base_ioa, data) = parse_ioa(ti, first, ioa_len)?;
    f(base_ioa, data)?;
    for offset in 1..count {
        let (data, tail) = rest.split_at(data_len);
        let ioa = base_ioa.checked_add(offset as u32).ok_or(ParseError::invalid_info_object(
            "IOA 递增溢出",
            ti,
            None,
            None,
            None,
        ))?;
        f(ioa, data)?;
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(ParseError::invalid_info_object("信息体存在剩余数据", ti, None, None, None));
    }
    Ok(())
}

/// 根据时间戳长度拆分 payload，支持 CP24Time2a (3B) / CP56Time2a (7B)。
/// 根据 `timestamp_len` 拆分出数据体与尾部时间戳。
/// 若 `timestamp_len=None`，返回 `(input, None)`。
pub fn split_timestamp<'a>(
    ti: TypeId,
    input: &'a [u8],
    timestamp_len: Option<usize>,
) -> Result<(&'a [u8], Option<&'a [u8]>), ParseError> {
    let len = match timestamp_len {
        Some(len) => len,
        None => return Ok((input, None)),
    };
    if input.len() < len {
        return Err(ParseError::insufficient_info_object("缺少时间标签", ti, None, None, len, input.len()));
    }
    let (body, ts) = input.split_at(input.len() - len);
    Ok((body, Some(ts)))
}

/// 解析 ASDU 头部（TI/VSQ/COT/CA）。
///
/// IEC 60870-5-104总是使用2字节COT格式：
/// - TI (1字节): 类型标识
/// - VSQ (1字节): 可变结构限定词
/// - COT (2字节): 传送原因 + P/N + T + 原发地址
/// - CA (2字节): 公共地址
///
/// # 错误处理
///
/// - 对于超出 1..=127 范围的 TI 值，返回错误
/// - 对于 1..=127 范围内的所有值（包括非标准值），均接受
/// - 非标准值将被映射为 `TypeIdEnum::Unknown(val)`，但仍然可以解析
pub fn parse_asdu_header(input: &[u8]) -> Result<AsduHeader, ParseError> {
    if input.len() < 6 {
        return Err(ParseError::insufficient_asdu_header("头部长度不足", 6, input.len()));
    }

    // 使用与旧版本一致的验证逻辑：只拒绝超出 1..=127 范围的值
    // 对于范围内的所有值（包括非标准值），都接受并构造 TypeId
    let type_id =
        TypeId::from_raw(input[0]).ok_or_else(|| ParseError::unsupported_type_id(TypeId::new(input[0]), None))?;

    let vsq = input[1];

    // IEC 60870-5-104总是使用2字节COT模式
    let cot_byte1 = input[2];
    let cot_byte2 = input[3];
    let cot = Cot::from_bytes(cot_byte1, cot_byte2);

    let ca = u16::from_le_bytes([input[4], input[5]]);

    Ok(AsduHeader { type_id, vsq, cot, common_addr: ca })
}

impl<'a> Asdu<'a> {
    /// 创建新的 ASDU 视图。
    pub fn new(header: AsduHeader, payload: &'a [u8]) -> Self {
        Self { header, payload }
    }
}

/// 使用指定 Profile 解析 ASDU 帧。
///
/// 该函数根据 Profile 判断 TypeID 是否为私有类型：
/// - 如果是私有类型，委托给 Profile 的 `parse_private_asdu` 方法处理
/// - 如果不是私有类型，返回原始 ASDU 视图供后续标准解析
///
/// # 参数
///
/// - `header`: 已解析的 ASDU 头部
/// - `payload`: ASDU 载荷（信息体部分）
/// - `profile`: 协议 Profile 实现
///
/// # 返回值
///
/// - `Ok(ParsedAsduResult::Standard(asdu))`: 标准类型，返回原始 ASDU 视图
/// - `Ok(ParsedAsduResult::Private(profile_asdu))`: 私有类型，返回 Profile 解析结果
/// - `Err(ParseError)`: 解析失败
///
/// # 示例
///
/// ```ignore
/// use iec60870_parser::profiles::distribution::DistributionProfile;
/// use iec60870_parser::parser::asdu::{parse_asdu_header, parse_asdu_with_profile};
///
/// let profile = DistributionProfile;
/// let header = parse_asdu_header(&raw_data)?;
/// let payload = &raw_data[6..];
///
/// match parse_asdu_with_profile(header, payload, &profile)? {
///     ParsedAsduResult::Standard(asdu) => {
///         // 使用标准解析器处理
///     }
///     ParsedAsduResult::Private(private_asdu) => {
///         // 处理私有类型数据
///         println!("Private type: {}", private_asdu.type_name());
///     }
/// }
/// ```
pub fn parse_asdu_with_profile<'a, P: ProtocolProfile>(
    header: AsduHeader,
    payload: &'a [u8],
    profile: &P,
) -> Result<ParsedAsduResult<'a>, ParseError> {
    // 检查是否为 Profile 支持的私有类型
    if profile.is_private_type(header.type_id) {
        let private_asdu = profile.parse_private_asdu(&header, payload)?;
        return Ok(ParsedAsduResult::Private(private_asdu));
    }

    // 标准类型，返回原始 ASDU 视图供后续处理
    Ok(ParsedAsduResult::Standard(Asdu::new(header, payload)))
}

/// `parse_asdu_with_profile` 的返回结果。
#[derive(Debug)]
pub enum ParsedAsduResult<'a> {
    /// 标准类型，返回原始 ASDU 视图
    Standard(Asdu<'a>),
    /// Profile 私有类型，返回解析后的 Profile ASDU
    Private(Box<dyn ProfileAsdu + 'a>),
}
