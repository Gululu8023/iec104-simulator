//! 测试命令 ASDU 解析（TI 104）。
//!
//! 本模块负责解析 DL/T 634.5104-2009 国内标准中定义的测试命令 (C_TS_NA_1)。
//!
//! ## 数据格式
//!
//! TI 104 测试命令格式：
//! - IOA (3 bytes): 信息对象地址
//! - FBP (2 bytes): 固定测试码，固定值 0x55AA
//!
//! ## 示例
//!
//! ```ignore
//! use iec60870_parser::parser::asdu::{AsduHeader, TypeId, Cot, CotReason};
//! use iec60870_parser::parser::asdu::test_command::parse_test_command;
//!
//! let header = AsduHeader::new(TypeId::new(104), 1, Cot::new(CotReason::Activation), 1);
//! let payload = [
//!     0x00, 0x00, 0x00,  // IOA = 0
//!     0xAA, 0x55,        // FBP = 0x55AA (小端序)
//! ];
//!
//! let frame = parse_test_command(header, &payload).unwrap();
//! assert_eq!(frame.items[0].fbp, 0x55AA);
//! ```

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, for_each_entry, spec, split_timestamp, types::lookup_type_descriptor},
};

/// 单条测试命令。
///
/// DL/T 634.5104-2009 定义的测试命令信息结构。
#[derive(Debug, Clone, Copy)]
pub struct TestCommand<'a> {
    /// 信息对象地址 (IOA)。
    pub ioa: u32,
    /// 固定测试码 (FBP)。
    ///
    /// 标准值为 0x55AA，用于链路测试。
    pub fbp: u16,
    /// Type 107 携带的 CP56Time2a。
    pub timestamp: Option<&'a [u8]>,
}

impl TestCommand<'_> {
    /// 检查 FBP 是否为标准值 0x55AA。
    pub fn is_valid_fbp(&self) -> bool {
        self.fbp == spec::DLT_TEST_FBP
    }
}

/// 测试命令帧集合。
///
/// 包含 ASDU 头部和若干条测试命令记录。
#[derive(Debug, Clone)]
pub struct TestCommandFrame<'a> {
    /// ASDU 头部信息。
    pub header: AsduHeader,
    /// 测试命令列表。
    pub items: SmallVec<[TestCommand<'a>; 1]>,
}

/// 解析测试命令 ASDU (TI 104, C_TS_NA_1)。
///
/// # 参数
///
/// - `header`: ASDU 头部信息
/// - `payload`: ASDU 负载数据
///
/// # 错误
///
/// - 如果 Type ID 不是 104，返回 `ParseError::UnsupportedTypeId`
/// - 如果数据长度不足，返回 `ParseError::InsufficientInfoObject`
///
/// # 示例
///
/// ```ignore
/// let frame = parse_test_command(header, &payload)?;
/// for cmd in &frame.items {
///     println!("测试命令 IOA: 0x{:06X}, FBP: 0x{:04X}", cmd.ioa, cmd.fbp);
///     if cmd.is_valid_fbp() {
///         println!("FBP 校验通过");
///     }
/// }
/// ```
pub fn parse_test_command<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<TestCommandFrame<'a>, ParseError> {
    if !matches!(header.type_id.raw(), 104 | 107) {
        return Err(ParseError::unsupported_type_id(header.type_id, None));
    }

    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut items = SmallVec::new();

    for_each_entry(header.type_id, payload, descriptor, header.vsq, |ioa, body| {
        let (core, timestamp) = split_timestamp(header.type_id, body, descriptor.timestamp_len)?;
        if core.len() < 2 {
            return Err(ParseError::insufficient_info_object(
                "测试命令缺少 FBP 字段",
                header.type_id,
                Some(ioa),
                None,
                2,
                core.len(),
            ));
        }
        let fbp = u16::from_le_bytes([core[0], core[1]]);
        if fbp != spec::DLT_TEST_FBP {
            return Err(ParseError::invalid_info_object(
                "测试命令 FBP 必须为 0x55AA",
                header.type_id,
                Some(ioa),
                None,
                None,
            ));
        }
        items.push(TestCommand { ioa, fbp, timestamp });
        Ok(())
    })?;

    Ok(TestCommandFrame { header, items })
}
