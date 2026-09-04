//! 召唤类 ASDU 解析（TI 100~106）。
//!
//! 本模块负责解析 IEC 60870-5-104 协议中的召唤相关信息，包括：
//!
//! - **总召唤** (TI 100): 召唤所有数据点 (C_IC_NA_1)
//! - **计数器召唤** (TI 101): 召唤计数器值 (C_CI_NA_1)
//! - **时钟同步** (TI 103): 时钟读取/同步命令 (C_CS_NA_1)
//! - **测试命令** (TI 104): 测试命令 (C_TS_NA_1)
//! - **复位进程** (TI 105): 复位进程命令 (C_RP_NA_1)
//! - **延时采集传输** (TI 106): 延时获取或传输命令 (C_CD_NA_1)
//!
//! ## 数据格式
//!
//! 这些类型的 payload 格式相对简单：一个 IOA + 1 字节限定词。
//! 不同类型使用不同的限定词：
//!
//! - TI 100 (总召唤): IOA + QOI (召唤限定词)
//! - TI 101 (计数器召唤): IOA + QCC (计数器召唤限定词)
//! - 其他类型: IOA + 相应的限定词
//!
//! ## 顺序位 (SQ) 支持
//!
//! 本模块支持 SQ=0 和 SQ=1 两种模式：
//!
//! - **SQ=0**: 每个信息对象都包含完整的 IOA
//! - **SQ=1**: 第一个 IOA 显式给出，后续 IOA 自动递增
//!
//! ## 示例
//!
//! ```ignore
//! use iec60870_parser::parser::asdu::{AsduHeader, TypeId, Cot, CotReason};
//! use iec60870_parser::parser::asdu::interrogation::parse_interrogation;
//!
//! // 总召唤示例
//! let header = AsduHeader::new(TypeId::new(100), 1, Cot::new(CotReason::Activation), 1);
//! let payload = [
//!     0x00, 0x00, 0x00,  // IOA = 0 (广播)
//!     0x14,              // QOI = 20 (站召唤)
//! ];
//!
//! let frame = parse_interrogation(header, &payload).unwrap();
//! assert_eq!(frame.items.len(), 1);
//! assert_eq!(frame.items[0].qualifier, 0x14);
//! ```

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, for_each_entry, spec, types::lookup_type_descriptor},
};

/// 单条召唤请求。
///
/// 召唤命令的基本信息结构，包含信息对象地址和限定词。
#[derive(Debug, Clone)]
pub struct Interrogation {
    /// 信息对象地址 (IOA)。
    ///
    /// - IOA=0 通常表示广播地址，用于总召唤
    /// - 其他值表示特定的信息对象或分组
    pub ioa: u32,
    /// 限定词（根据类型不同有不同含义）。
    ///
    /// - TI 100: QOI (召唤限定词)，如 20=站召唤
    /// - TI 101: QCC (计数器召唤限定词)
    /// - 其他: 相应的限定词
    pub qualifier: u8,
}

/// 召唤帧集合。
///
/// 包含 ASDU 头部和若干条召唤请求记录。
/// 通常一个召唤帧只包含一条请求，但协议允许批量操作。
#[derive(Debug, Clone)]
pub struct InterrogationFrame {
    /// ASDU 头部信息。
    pub header: AsduHeader,
    /// 召唤请求列表。
    pub items: SmallVec<[Interrogation; 4]>,
}

/// 解析召唤类 ASDU。
///
/// 支持 TI 100~106 等召唤类型，自动处理 SQ=0 和 SQ=1 两种模式。
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
/// // 总召唤
/// let frame = parse_interrogation(header, &payload)?;
/// for req in &frame.items {
///     println!("召唤 IOA: 0x{:06X}, QOI: 0x{:02X}", req.ioa, req.qualifier);
/// }
/// ```
pub fn parse_interrogation(header: AsduHeader, payload: &[u8]) -> Result<InterrogationFrame, ParseError> {
    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut items = SmallVec::new();
    for_each_entry(header.type_id, payload, descriptor, header.vsq, |ioa, body| {
        if body.is_empty() {
            return Err(ParseError::insufficient_info_object(
                "缺少限定词",
                header.type_id,
                Some(ioa),
                None,
                1,
                body.len(),
            ));
        }
        let qualifier = body[0];
        match header.type_id.raw() {
            100 if !(20..=36).contains(&qualifier) => {
                return Err(ParseError::invalid_info_object(
                    "QOI 取值必须为 20~36",
                    header.type_id,
                    Some(ioa),
                    None,
                    None,
                ));
            }
            101 if !spec::is_valid_qcc_request(qualifier & 0x3F) => {
                return Err(ParseError::invalid_info_object(
                    "QCC 请求组号必须为 1~5",
                    header.type_id,
                    Some(ioa),
                    None,
                    None,
                ));
            }
            _ => {}
        }
        items.push(Interrogation { ioa, qualifier });
        Ok(())
    })?;

    Ok(InterrogationFrame { header, items })
}
