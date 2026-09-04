//! 文件传输 ASDU 解析（TI 120~127）。
//!
//! 标准将文件传输拆为 FileReady/SectionReady/Segment/Directory 等多种记录。
//! 它们的结构各不相同，因此本模块为每种 TI 编写专用解析函数，
//! 并在长度不足时返回 [`ParseError::InvalidInfoObject`]。

use core::fmt;

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{
        AsduHeader, TypeId, split_timestamp,
        types::{TypeDescriptor, lookup_type_descriptor},
    },
};

#[derive(Debug, Clone)]
pub struct FileTransferSet<'a> {
    pub header: AsduHeader,
    pub items: SmallVec<[FileTransferRecord<'a>; 2]>,
}

#[derive(Debug, Clone)]
pub enum FileTransferRecord<'a> {
    FileReady(FileReady),
    SectionReady(SectionReady),
    SelectCall(SelectCall),
    LastSection(LastSection),
    AckSection(AckSection),
    Segment(SegmentData<'a>),
    Directory(DirectoryListing<'a>),
    QueryLog(QueryLog),
    Raw(&'a [u8]),
}

#[derive(Debug, Clone, Copy)]
pub struct FileReady {
    pub ioa: u32,
    pub file_name: u16,
    pub qualifier: u8,
    pub length: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SectionReady {
    pub ioa: u32,
    pub file_name: u16,
    pub qualifier: u8,
    pub section: u8,
    pub length: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SelectCall {
    pub ioa: u32,
    pub file_name: u16,
    pub qualifier: u8,
    pub section: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct LastSection {
    pub ioa: u32,
    pub file_name: u16,
    pub qualifier: u8,
    pub last_section_number: u8,
    pub last_segment_number: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct AckSection {
    pub ioa: u32,
    pub file_name: u16,
    pub qualifier: u8,
    pub section: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct DirectoryAttributes {
    pub length: u32,
    pub last_section: u16,
    pub last_segment: u16,
}

/// 目录状态 (SOF - Status of File)
/// 对应 SDK 中 sIEC104FileAttributes 的状态位
#[derive(Debug, Clone, Copy)]
pub struct DirectoryStatus {
    pub raw: u8,
    pub is_file: bool,
    pub is_last_file_of_directory: bool,
}

impl DirectoryStatus {
    const FILE_ENTRY_FLAG: u8 = 0x01;
    const LAST_ENTRY_FLAG: u8 = 0x02;

    pub fn from_raw(raw: u8) -> Self {
        Self {
            raw,
            is_file: raw & Self::FILE_ENTRY_FLAG != 0,
            is_last_file_of_directory: raw & Self::LAST_ENTRY_FLAG != 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirectoryListing<'a> {
    pub ioa: u32,
    pub file_name: u16,
    pub qualifier: u8, // TI=126 中无此字段，固定为 0
    pub section: u16,  // TI=126 中无此字段，固定为 0
    pub attributes: DirectoryAttributes,
    pub status: DirectoryStatus,
    pub timestamp: Option<&'a [u8]>, // CP56Time2a
}

/// 文件段数据 (TI 125, F_SC_NA_1)
///
/// IEC 60870-5-104 标准格式:
/// - IOA (3 bytes): 信息对象地址
/// - NOF (2 bytes): 文件名
/// - NOS (1 byte): 节号
/// - LOS (1 byte): 段长度
/// - Data: 段数据
#[derive(Debug, Clone)]
pub struct SegmentData<'a> {
    pub ioa: u32,
    pub file_name: u16,
    pub section: u8,
    pub los: u8,
    pub data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct QueryLog {
    pub ioa: u32,
    pub file_name: u16,
    pub range_start_time: [u8; 7], // CP56Time2a
    pub range_end_time: [u8; 7],   // CP56Time2a
}

impl<'a> From<FileReady> for FileTransferRecord<'a> {
    fn from(value: FileReady) -> Self {
        FileTransferRecord::FileReady(value)
    }
}

impl<'a> From<SectionReady> for FileTransferRecord<'a> {
    fn from(value: SectionReady) -> Self {
        FileTransferRecord::SectionReady(value)
    }
}

impl<'a> From<SelectCall> for FileTransferRecord<'a> {
    fn from(value: SelectCall) -> Self {
        FileTransferRecord::SelectCall(value)
    }
}

impl<'a> From<LastSection> for FileTransferRecord<'a> {
    fn from(value: LastSection) -> Self {
        FileTransferRecord::LastSection(value)
    }
}

impl<'a> From<AckSection> for FileTransferRecord<'a> {
    fn from(value: AckSection) -> Self {
        FileTransferRecord::AckSection(value)
    }
}

impl<'a> From<DirectoryListing<'a>> for FileTransferRecord<'a> {
    fn from(value: DirectoryListing<'a>) -> Self {
        FileTransferRecord::Directory(value)
    }
}

impl<'a> From<QueryLog> for FileTransferRecord<'a> {
    fn from(value: QueryLog) -> Self {
        FileTransferRecord::QueryLog(value)
    }
}

// ============================================================================
// Display 实现 - 提供人类可读的格式化输出
// ============================================================================

impl fmt::Display for FileReady {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "文件准备就绪: 文件名={} (0x{:04X}), 大小={} 字节, 限定词=0x{:02X} ({})",
            self.file_name,
            self.file_name,
            self.length,
            self.qualifier,
            decode_frq(self.qualifier)
        )
    }
}

impl fmt::Display for SectionReady {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "节准备就绪: 文件名={} (0x{:04X}), 节号={}, 大小={} 字节, 限定词=0x{:02X} ({})",
            self.file_name,
            self.file_name,
            self.section,
            self.length,
            self.qualifier,
            decode_srq(self.qualifier)
        )
    }
}

impl fmt::Display for SelectCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "选择/调用: 文件名={} (0x{:04X}), 节号={}, 限定词=0x{:02X} ({})",
            self.file_name,
            self.file_name,
            self.section,
            self.qualifier,
            decode_scq(self.qualifier)
        )
    }
}

impl fmt::Display for LastSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "最后节/段: 文件名={} (0x{:04X}), 最后节号={}, 最后段号={}, 限定词=0x{:02X} ({})",
            self.file_name,
            self.file_name,
            self.last_section_number,
            self.last_segment_number,
            self.qualifier,
            decode_lsq(self.qualifier)
        )
    }
}

impl fmt::Display for AckSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "确认: 文件名={} (0x{:04X}), 节号={}, 限定词=0x{:02X} ({})",
            self.file_name,
            self.file_name,
            self.section,
            self.qualifier,
            decode_afq(self.qualifier)
        )
    }
}

impl<'a> fmt::Display for SegmentData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "段数据: 文件名={} (0x{:04X}), 节号={}, LOS={}",
            self.file_name, self.file_name, self.section, self.los
        )?;

        // 显示前32字节数据
        if !self.data.is_empty() {
            write!(f, ", 数据(前{}字节)=[", self.data.len().min(32))?;
            for (i, byte) in self.data.iter().take(32).enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{:02X}", byte)?;
            }
            write!(f, "]")?;
            if self.data.len() > 32 {
                write!(f, "... (共{}字节)", self.data.len())?;
            }
        }
        Ok(())
    }
}

impl<'a> fmt::Display for DirectoryListing<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entry_type = if self.status.is_file { "文件" } else { "目录" };
        write!(
            f,
            "目录: 文件名={} (0x{:04X}), 类型={}, 大小={} 字节",
            self.file_name, self.file_name, entry_type, self.attributes.length
        )?;

        if self.status.is_last_file_of_directory {
            write!(f, ", [最后条目]")?;
        }

        if let Some(ts) = self.timestamp {
            write!(f, ", 时间戳={}", format_cp56time2a(ts))?;
        }
        Ok(())
    }
}

impl fmt::Display for QueryLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "查询归档: 文件名={} (0x{:04X}), 起始={}, 结束={}",
            self.file_name,
            self.file_name,
            format_cp56time2a(&self.range_start_time),
            format_cp56time2a(&self.range_end_time)
        )
    }
}

impl<'a> fmt::Display for FileTransferRecord<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileTransferRecord::FileReady(r) => write!(f, "{}", r),
            FileTransferRecord::SectionReady(r) => write!(f, "{}", r),
            FileTransferRecord::SelectCall(r) => write!(f, "{}", r),
            FileTransferRecord::LastSection(r) => write!(f, "{}", r),
            FileTransferRecord::AckSection(r) => write!(f, "{}", r),
            FileTransferRecord::Segment(r) => write!(f, "{}", r),
            FileTransferRecord::Directory(r) => write!(f, "{}", r),
            FileTransferRecord::QueryLog(r) => write!(f, "{}", r),
            FileTransferRecord::Raw(data) => {
                write!(f, "原始数据: {} 字节 [", data.len())?;
                for (i, byte) in data.iter().take(32).enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{:02X}", byte)?;
                }
                if data.len() > 32 {
                    write!(f, "... (共{}字节)", data.len())?;
                }
                write!(f, "]")
            }
        }
    }
}

// ============================================================================
// 辅助函数 - 解码限定词和时间戳
// ============================================================================

/// 解码文件准备限定词 (FRQ - File Ready Qualifier)
/// 根据 IEC 60870-5-104 标准和实际观察：
/// 0x00: 默认 (无特殊状态)
/// 0x7D: 文件传输中/文件活跃 (SDK常见值)
/// bit 7: 0=文件可用，1=文件不可用
fn decode_frq(frq: u8) -> String {
    match frq {
        0x00 => "默认".to_string(),
        0x7D => "文件传输活跃".to_string(),
        _ => {
            if (frq & 0x80) != 0 {
                format!("文件不可用, 状态码=0x{:02X}", frq & 0x7F)
            } else {
                format!("文件可用, 状态码=0x{:02X}", frq & 0x7F)
            }
        }
    }
}

/// 解码节准备限定词 (SRQ - Section Ready Qualifier)
/// 0x00: 默认
/// 0x01: 节准备就绪/节传输活跃 (SDK常见值)
/// bit 7: 0=节可用，1=节不可用
fn decode_srq(srq: u8) -> String {
    match srq {
        0x00 => "默认".to_string(),
        0x01 => "节传输活跃".to_string(),
        _ => {
            if (srq & 0x80) != 0 {
                format!("节不可用, 状态码=0x{:02X}", srq & 0x7F)
            } else {
                format!("节可用, 状态码=0x{:02X}", srq & 0x7F)
            }
        }
    }
}

/// 解码最后节/段限定词 (LSQ - Last Section/Segment Qualifier)
/// bit 0: 文件传输完成
/// bit 1: 节传输完成
/// bit 2-6: 保留
/// bit 7: 0=成功，1=不成功
fn decode_lsq(lsq: u8) -> String {
    if lsq == 0 {
        return "无特殊标志".to_string();
    }

    let mut parts = Vec::new();

    if (lsq & 0x01) != 0 {
        parts.push("文件传输完成");
    }
    if (lsq & 0x02) != 0 {
        parts.push("节传输完成");
    }
    if (lsq & 0x80) != 0 {
        parts.push("传输不成功");
    }

    if parts.is_empty() { "无特殊标志".to_string() } else { parts.join(", ") }
}

/// 解码选择/调用限定词 (SCQ)
fn decode_scq(scq: u8) -> String {
    let bits = [
        (0x01, "选择文件"),
        (0x02, "请求文件"),
        (0x04, "停用文件"),
        (0x08, "删除文件"),
        (0x10, "选择节"),
        (0x20, "请求节"),
        (0x40, "停用节"),
    ];

    let active: Vec<&str> = bits.iter().filter(|(mask, _)| (scq & mask) != 0).map(|(_, name)| *name).collect();

    if active.is_empty() { "无".to_string() } else { active.join(", ") }
}

/// 解码确认限定词 (AFQ)
fn decode_afq(afq: u8) -> String {
    let bits = [(0x01, "确认文件正面"), (0x02, "确认文件负面"), (0x04, "确认节正面"), (0x08, "确认节负面")];

    let active: Vec<&str> = bits.iter().filter(|(mask, _)| (afq & mask) != 0).map(|(_, name)| *name).collect();

    if active.is_empty() { "无".to_string() } else { active.join(", ") }
}

/// 格式化 CP56Time2a 时间戳
fn format_cp56time2a(ts: &[u8]) -> String {
    if ts.len() < 7 {
        return format!("无效({} 字节)", ts.len());
    }

    let ms = u16::from_le_bytes([ts[0], ts[1]]);
    let minute = ts[2] & 0x3F;
    let hour = ts[3] & 0x1F;
    let day = ts[4] & 0x1F;
    let month = ts[5] & 0x0F;
    let year = ts[6] & 0x7F;

    let iv = (ts[2] & 0x80) != 0;
    let su = (ts[3] & 0x80) != 0;

    let second = ms / 1000;
    let millisecond = ms % 1000;

    let mut result =
        format!("20{:02}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}", year, month, day, hour, minute, second, millisecond);

    if iv {
        result.push_str(" [IV]");
    }
    if su {
        result.push_str(" [SU]");
    }

    result
}

// ============================================================================
// 解析函数
// ============================================================================

pub fn parse_file_transfer<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<FileTransferSet<'a>, ParseError> {
    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let ti = header.type_id;
    let count = (header.vsq & 0x7F) as usize;
    let sq = (header.vsq & 0x80) != 0;

    if count == 0 {
        return Err(ParseError::invalid_info_object("VSQ count 为 0", ti, None, None, None));
    }

    let mut records = SmallVec::with_capacity(count.min(4));
    let mut cursor = payload;

    // 根据不同的 TypeId 选择解析函数
    type ParseFn = for<'b> fn(TypeId, &'b TypeDescriptor, u32, &'b [u8]) -> Result<FileTransferRecord<'b>, ParseError>;
    let parse_fn: ParseFn = match header.type_id.raw() {
        120 => parse_file_ready_with_ioa,
        121 => parse_section_ready_with_ioa,
        122 => parse_select_call_with_ioa,
        123 => parse_last_section_with_ioa,
        124 => parse_ack_section_with_ioa,
        125 => {
            // 段传输是变长的，特殊处理
            // TI=125 通常使用 SQ=0 (每个段都有完整 IOA)
            if sq {
                return Err(ParseError::invalid_info_object("TI=125 不支持 SQ=1 模式", ti, None, None, None));
            }

            for _ in 0..count {
                let (record, consumed) = parse_segment(ti, cursor)?;
                cursor = &cursor[consumed..];
                records.push(record);
            }

            if !cursor.is_empty() {
                return Err(ParseError::invalid_info_object("文件段 ASDU 存在多余字节", ti, None, None, None));
            }

            return Ok(FileTransferSet { header, items: records });
        }
        126 => parse_directory_with_ioa,
        127 => parse_query_log_with_ioa,
        _ => {
            // 未知类型，返回 Raw
            for _ in 0..count {
                records.push(FileTransferRecord::Raw(cursor));
            }
            return Ok(FileTransferSet { header, items: records });
        }
    };

    // 使用统一的 SQ 处理逻辑
    let ioa_len = descriptor.ioa_len;
    let entry_len = descriptor.entry_len().ok_or(ParseError::invalid_info_object(
        "文件传输信息体需要固定长度",
        ti,
        None,
        None,
        None,
    ))?;

    if !sq {
        // SQ=0: 每个信息体都有完整的 IOA
        let required = entry_len.checked_mul(count).ok_or(ParseError::invalid_info_object(
            "信息体长度溢出",
            ti,
            None,
            None,
            None,
        ))?;

        if cursor.len() < required {
            return Err(ParseError::insufficient_info_object(
                "payload 长度不足",
                ti,
                None,
                None,
                required,
                cursor.len(),
            ));
        }

        for _ in 0..count {
            let entry = &cursor[..entry_len];
            let (ioa, body) = parse_ioa(ti, entry, ioa_len)?;
            let record = parse_fn(ti, descriptor, ioa, body)?;
            records.push(record);
            cursor = &cursor[entry_len..];
        }
    } else {
        // SQ=1: 只有第一个信息体有 IOA，后续 IOA 递增
        if ioa_len == 0 {
            return Err(ParseError::invalid_info_object("SQ=1 但 IOA 长度为 0", ti, None, None, None));
        }

        let data_len = entry_len.checked_sub(ioa_len).ok_or(ParseError::invalid_info_object(
            "信息体长度不足",
            ti,
            None,
            None,
            None,
        ))?;

        let required = entry_len
            .checked_add(data_len.checked_mul(count.saturating_sub(1)).ok_or(ParseError::invalid_info_object(
                "信息体长度溢出",
                ti,
                None,
                None,
                None,
            ))?)
            .ok_or(ParseError::invalid_info_object("信息体长度溢出", ti, None, None, None))?;

        if cursor.len() < required {
            return Err(ParseError::insufficient_info_object(
                "payload 长度不足",
                ti,
                None,
                None,
                required,
                cursor.len(),
            ));
        }

        // 第一个信息体：包含 IOA
        let (first, rest) = cursor.split_at(entry_len);
        let (base_ioa, body) = parse_ioa(ti, first, ioa_len)?;
        let record = parse_fn(ti, descriptor, base_ioa, body)?;
        records.push(record);

        // 后续信息体：没有 IOA，IOA 自动递增
        cursor = rest;
        for offset in 1..count {
            let body = &cursor[..data_len];
            let ioa = base_ioa.checked_add(offset as u32).ok_or(ParseError::invalid_info_object(
                "IOA 递增溢出",
                ti,
                None,
                None,
                None,
            ))?;
            let record = parse_fn(ti, descriptor, ioa, body)?;
            records.push(record);
            cursor = &cursor[data_len..];
        }
    }

    if !cursor.is_empty() {
        return Err(ParseError::invalid_info_object("文件 ASDU 存在多余字节", ti, None, None, None));
    }

    Ok(FileTransferSet { header, items: records })
}

// 辅助函数：解析 IOA
fn parse_ioa(ti: TypeId, entry: &[u8], ioa_len: usize) -> Result<(u32, &[u8]), ParseError> {
    if entry.len() < ioa_len {
        return Err(ParseError::insufficient_info_object("IOA 长度不足", ti, None, None, ioa_len, entry.len()));
    }

    let ioa = match ioa_len {
        0 => 0,
        1 => entry[0] as u32,
        2 => u16::from_le_bytes([entry[0], entry[1]]) as u32,
        3 => u32::from_le_bytes([entry[0], entry[1], entry[2], 0]) & 0xFFFFFF,
        _ => return Err(ParseError::invalid_info_object("IOA 长度非法", ti, None, None, None)),
    };

    Ok((ioa, &entry[ioa_len..]))
}

// 带 IOA 参数的解析函数
fn parse_file_ready_with_ioa<'b>(
    _ti: TypeId,
    _descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    if body.len() < 6 {
        return Err(ParseError::insufficient_info_object("文件准备信息体长度不足", ti, Some(ioa), None, 6, body.len()));
    }
    // Format: NOF(2) + FRQ(1) + LOF(3)
    Ok(FileReady {
        ioa,
        file_name: u16::from_le_bytes([body[0], body[1]]),
        qualifier: body[2],
        length: read_u24(&body[3..6]),
    }
    .into())
}

fn parse_section_ready_with_ioa<'b>(
    _ti: TypeId,
    _descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    if body.len() < 7 {
        return Err(ParseError::insufficient_info_object("节准备信息体长度不足", ti, Some(ioa), None, 7, body.len()));
    }
    // Format: NOF(2) + SRQ(1) + NOS(1) + LOS(3)
    Ok(SectionReady {
        ioa,
        file_name: u16::from_le_bytes([body[0], body[1]]),
        qualifier: body[2],
        section: body[3],
        length: read_u24(&body[4..7]),
    }
    .into())
}

fn parse_select_call_with_ioa<'b>(
    _ti: TypeId,
    _descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    if body.len() < 4 {
        return Err(ParseError::insufficient_info_object(
            "选择/调用信息体长度不足",
            ti,
            Some(ioa),
            None,
            4,
            body.len(),
        ));
    }
    // Format: NOF(2) + SCQ(1) + NOS(1)
    Ok(SelectCall { ioa, file_name: u16::from_le_bytes([body[0], body[1]]), qualifier: body[2], section: body[3] }
        .into())
}

fn parse_last_section_with_ioa<'b>(
    _ti: TypeId,
    _descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    if body.len() < 5 {
        return Err(ParseError::insufficient_info_object("最后节信息体长度不足", ti, Some(ioa), None, 5, body.len()));
    }
    // Format: NOF(2) + LSQ(1) + NOS(2)
    // Note: NOS is 2 bytes containing last_section_number and last_segment_number
    Ok(LastSection {
        ioa,
        file_name: u16::from_le_bytes([body[0], body[1]]),
        qualifier: body[2],
        last_section_number: body[3],
        last_segment_number: body[4],
    }
    .into())
}

fn parse_ack_section_with_ioa<'b>(
    _ti: TypeId,
    _descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    if body.len() < 4 {
        return Err(ParseError::insufficient_info_object("确认信息体长度不足", ti, Some(ioa), None, 4, body.len()));
    }
    // Format: NOF(2) + AFQ(1) + NOS(1)
    Ok(AckSection { ioa, file_name: u16::from_le_bytes([body[0], body[1]]), qualifier: body[2], section: body[3] }
        .into())
}

fn parse_directory_with_ioa<'b>(
    _ti: TypeId,
    descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    let (body_data, timestamp) = split_timestamp(ti, body, descriptor.timestamp_len)?;
    if body_data.len() < 6 {
        return Err(ParseError::insufficient_info_object(
            "目录信息体长度不足",
            ti,
            Some(ioa),
            None,
            6,
            body_data.len(),
        ));
    }

    let file_name = u16::from_le_bytes([body_data[0], body_data[1]]);
    let length = read_u24(&body_data[2..5]);
    let status = DirectoryStatus::from_raw(body_data[5]);

    Ok(DirectoryListing {
        ioa,
        qualifier: 0, // 对于 TI=126，没有独立的 qualifier 字段
        file_name,
        section: 0,
        attributes: DirectoryAttributes { length, last_section: 0, last_segment: 0 },
        status,
        timestamp,
    }
    .into())
}

fn parse_query_log_with_ioa<'b>(
    _ti: TypeId,
    _descriptor: &'b TypeDescriptor,
    ioa: u32,
    body: &'b [u8],
) -> Result<FileTransferRecord<'b>, ParseError> {
    let ti = _ti;
    if body.len() < 16 {
        return Err(ParseError::insufficient_info_object(
            "查询归档信息体长度不足",
            ti,
            Some(ioa),
            None,
            16,
            body.len(),
        ));
    }

    let file_name = u16::from_le_bytes([body[0], body[1]]);
    let mut range_start_time = [0u8; 7];
    range_start_time.copy_from_slice(&body[2..9]);
    let mut range_end_time = [0u8; 7];
    range_end_time.copy_from_slice(&body[9..16]);

    Ok(QueryLog { ioa, file_name, range_start_time, range_end_time }.into())
}

// parse_segment 需要处理 IOA，重写此函数
fn parse_segment<'a>(ti: TypeId, input: &'a [u8]) -> Result<(FileTransferRecord<'a>, usize), ParseError> {
    // IEC 60870-5-104 标准格式: IOA (3) + NOF (2) + NOS (1) + LOS (1) + Data
    // 头部固定 7 字节

    if input.len() < 7 {
        return Err(ParseError::insufficient_info_object("文件段信息体头部长度不足", ti, None, None, 7, input.len()));
    }

    // 解析 IOA (3 bytes)
    let ioa = u32::from_le_bytes([input[0], input[1], input[2], 0]) & 0xFFFFFF;

    // 解析 body: NOF (2 bytes), NOS (1 byte), LOS (1 byte)
    let file_name = u16::from_le_bytes([input[3], input[4]]);
    let section = input[5]; // NOS - 节号
    let los = input[6]; // LOS - 段长度

    // 验证数据长度
    let expected_len = 7 + los as usize;
    if input.len() < expected_len {
        return Err(ParseError::insufficient_info_object(
            "文件段数据长度不足",
            ti,
            Some(ioa),
            None,
            expected_len,
            input.len(),
        ));
    }

    // 数据部分：从 byte[7] 开始，长度由 LOS 指定
    let data = &input[7..expected_len];

    Ok((
        FileTransferRecord::Segment(SegmentData { ioa, file_name, section, los, data }),
        expected_len, // 消费实际使用的字节
    ))
}

fn read_u24(bytes: &[u8]) -> u32 {
    (bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16)
}
