//! 配电文件传输和软件升级 ASDU 解析（TI 210/211）。
//!
//! 本模块实现《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义的：
//! - TI 210 (F_FR_NA_1): 文件传输（配电特有格式，10种操作类型）
//! - TI 211 (F_SR_NA_1): 软件升级

use std::any::Any;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, CP56Time2a, TypeId},
    profile::ProfileAsdu,
};

// ============================================================================
// TI 210: 文件传输（配电特有）- 枚举与辅助类型
// ============================================================================

/// 配电文件传输操作类型（实施细则表22）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FileOperation {
    /// 1: 文件目录召唤
    DirectoryRequest = 1,
    /// 2: 目录召唤确认
    DirectoryConfirm = 2,
    /// 3: 读文件激活
    ReadActivate = 3,
    /// 4: 读文件激活确认
    ReadActivateConfirm = 4,
    /// 5: 读文件数据传输
    ReadData = 5,
    /// 6: 读文件数据传输确认
    ReadDataConfirm = 6,
    /// 7: 写文件激活
    WriteActivate = 7,
    /// 8: 写文件激活确认
    WriteActivateConfirm = 8,
    /// 9: 写文件数据传输
    WriteData = 9,
    /// 10: 写文件数据传输确认
    WriteDataConfirm = 10,
}

impl TryFrom<u8> for FileOperation {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            1 => Ok(Self::DirectoryRequest),
            2 => Ok(Self::DirectoryConfirm),
            3 => Ok(Self::ReadActivate),
            4 => Ok(Self::ReadActivateConfirm),
            5 => Ok(Self::ReadData),
            6 => Ok(Self::ReadDataConfirm),
            7 => Ok(Self::WriteActivate),
            8 => Ok(Self::WriteActivateConfirm),
            9 => Ok(Self::WriteData),
            10 => Ok(Self::WriteDataConfirm),
            _ => Err(v),
        }
    }
}

/// 目录召唤标志（实施细则）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryCallFlag {
    /// 0: 列出目录下所有文件
    ListAll = 0,
    /// 1: 列出指定时间范围内的文件
    ListByTime = 1,
}

/// 文件操作结果描述字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FileResult {
    /// 0: 成功
    Success = 0,
    /// 1: 失败（无此文件/目录）
    NotFound = 1,
    /// 2: 失败（校验错误）
    ChecksumError = 2,
    /// 3: 失败（其他错误）
    OtherError = 3,
}

impl From<u8> for FileResult {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Success,
            1 => Self::NotFound,
            2 => Self::ChecksumError,
            _ => Self::OtherError,
        }
    }
}

/// 后续标志。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowFlag {
    /// 0: 无后续
    NoMore = 0,
    /// 1: 有后续
    More = 1,
}

impl TryFrom<u8> for FollowFlag {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::NoMore),
            1 => Ok(Self::More),
            _ => Err(v),
        }
    }
}

/// 目录信息中的单个文件条目。
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// 文件名
    pub filename: String,
    /// 文件属性（bit0: 目录标志）
    pub attributes: u8,
    /// 文件大小（字节）
    pub size: u32,
    /// 文件创建/修改时间
    pub timestamp: CP56Time2a,
}

// ============================================================================
// TI 210: 各操作类型的专用结构体
// ============================================================================

/// 操作1: 文件目录召唤（控制方向）。
/// 格式: IOA(3) + 操作标识(1) + 目录ID(4) + 目录名长度(1) + 目录名(N) + 召唤标志(1) + [起始时间(7)
/// + 终止时间(7)]
#[derive(Debug, Clone)]
pub struct FileDirectoryRequest {
    pub header: AsduHeader,
    pub ioa: u32,
    pub dir_id: u32,
    pub dir_name: String,
    pub call_flag: DirectoryCallFlag,
    /// 召唤标志=1时有效
    pub start_time: Option<CP56Time2a>,
    pub end_time: Option<CP56Time2a>,
}

/// 操作2: 目录召唤确认（监视方向）。
/// 格式: IOA(3) + 操作标识(1) + 结果(1) + 目录ID(4) + 后续标志(1) + 文件数量(1) + 目录信息(M)
#[derive(Debug, Clone)]
pub struct FileDirectoryConfirm {
    pub header: AsduHeader,
    pub ioa: u32,
    pub result: FileResult,
    pub dir_id: u32,
    pub follow_flag: FollowFlag,
    pub file_count: u8,
    pub entries: Vec<DirectoryEntry>,
}

/// 操作3: 读文件激活（控制方向）。
/// 格式: IOA(3) + 操作标识(1) + 文件名长度(1) + 文件名(N)
#[derive(Debug, Clone)]
pub struct FileReadActivate {
    pub header: AsduHeader,
    pub ioa: u32,
    pub filename: String,
}

/// 操作4: 读文件激活确认（监视方向）。
/// 格式: IOA(3) + 操作标识(1) + 结果(1) + 文件名长度(1) + 文件名(N) + 文件ID(4) + 文件大小(4)
#[derive(Debug, Clone)]
pub struct FileReadActivateConfirm {
    pub header: AsduHeader,
    pub ioa: u32,
    pub result: FileResult,
    pub filename: String,
    pub file_id: u32,
    pub file_size: u32,
}

/// 操作5: 读文件数据传输（监视方向）。
/// 格式: IOA(3) + 操作标识(1) + 文件ID(4) + 数据段号(4) + 后续标志(1) + 文件数据(N) + 校验码(1)
#[derive(Debug, Clone)]
pub struct FileReadData {
    pub header: AsduHeader,
    pub ioa: u32,
    pub file_id: u32,
    pub segment_no: u32,
    pub follow_flag: FollowFlag,
    pub data: Vec<u8>,
    pub checksum: u8,
}

/// 操作6: 读文件数据传输确认（控制方向）。
/// 格式: IOA(3) + 操作标识(1) + 文件ID(4) + 数据段号(4) + 结果(1)
#[derive(Debug, Clone)]
pub struct FileReadDataConfirm {
    pub header: AsduHeader,
    pub ioa: u32,
    pub file_id: u32,
    pub segment_no: u32,
    pub result: FileResult,
}

/// 操作7: 写文件激活（控制方向）。
/// 格式: IOA(3) + 操作标识(1) + 文件名长度(1) + 文件名(N) + 文件ID(4) + 文件大小(4)
#[derive(Debug, Clone)]
pub struct FileWriteActivate {
    pub header: AsduHeader,
    pub ioa: u32,
    pub filename: String,
    pub file_id: u32,
    pub file_size: u32,
}

/// 操作8: 写文件激活确认（监视方向）。
/// 格式: IOA(3) + 操作标识(1) + 结果(1) + 文件名长度(1) + 文件名(N) + 文件ID(4)
#[derive(Debug, Clone)]
pub struct FileWriteActivateConfirm {
    pub header: AsduHeader,
    pub ioa: u32,
    pub result: FileResult,
    pub filename: String,
    pub file_id: u32,
}

/// 操作9: 写文件数据传输（控制方向）。
/// 格式: IOA(3) + 操作标识(1) + 文件ID(4) + 数据段号(4) + 后续标志(1) + 文件数据(N) + 校验码(1)
#[derive(Debug, Clone)]
pub struct FileWriteData {
    pub header: AsduHeader,
    pub ioa: u32,
    pub file_id: u32,
    pub segment_no: u32,
    pub follow_flag: FollowFlag,
    pub data: Vec<u8>,
    pub checksum: u8,
}

/// 操作10: 写文件数据传输确认（监视方向）。
/// 格式: IOA(3) + 操作标识(1) + 文件ID(4) + 数据段号(4) + 结果(1)
#[derive(Debug, Clone)]
pub struct FileWriteDataConfirm {
    pub header: AsduHeader,
    pub ioa: u32,
    pub file_id: u32,
    pub segment_no: u32,
    pub result: FileResult,
}

/// TI 210 文件传输的统一枚举包装。
#[derive(Debug, Clone)]
pub enum FileTransfer {
    DirectoryRequest(FileDirectoryRequest),
    DirectoryConfirm(FileDirectoryConfirm),
    ReadActivate(FileReadActivate),
    ReadActivateConfirm(FileReadActivateConfirm),
    ReadData(FileReadData),
    ReadDataConfirm(FileReadDataConfirm),
    WriteActivate(FileWriteActivate),
    WriteActivateConfirm(FileWriteActivateConfirm),
    WriteData(FileWriteData),
    WriteDataConfirm(FileWriteDataConfirm),
}

impl FileTransfer {
    pub fn header(&self) -> &AsduHeader {
        match self {
            Self::DirectoryRequest(x) => &x.header,
            Self::DirectoryConfirm(x) => &x.header,
            Self::ReadActivate(x) => &x.header,
            Self::ReadActivateConfirm(x) => &x.header,
            Self::ReadData(x) => &x.header,
            Self::ReadDataConfirm(x) => &x.header,
            Self::WriteActivate(x) => &x.header,
            Self::WriteActivateConfirm(x) => &x.header,
            Self::WriteData(x) => &x.header,
            Self::WriteDataConfirm(x) => &x.header,
        }
    }

    pub fn operation(&self) -> FileOperation {
        match self {
            Self::DirectoryRequest(_) => FileOperation::DirectoryRequest,
            Self::DirectoryConfirm(_) => FileOperation::DirectoryConfirm,
            Self::ReadActivate(_) => FileOperation::ReadActivate,
            Self::ReadActivateConfirm(_) => FileOperation::ReadActivateConfirm,
            Self::ReadData(_) => FileOperation::ReadData,
            Self::ReadDataConfirm(_) => FileOperation::ReadDataConfirm,
            Self::WriteActivate(_) => FileOperation::WriteActivate,
            Self::WriteActivateConfirm(_) => FileOperation::WriteActivateConfirm,
            Self::WriteData(_) => FileOperation::WriteData,
            Self::WriteDataConfirm(_) => FileOperation::WriteDataConfirm,
        }
    }
}

impl ProfileAsdu for FileTransfer {
    fn type_id(&self) -> TypeId {
        TypeId::new(210)
    }

    fn type_name(&self) -> &'static str {
        "F_FR_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ============================================================================
// TI 210: 解析函数
// ============================================================================

/// 辅助: 读取长度前缀的字符串
fn read_length_prefixed_string(data: &[u8], ti: TypeId) -> Result<(String, usize), ParseError> {
    if data.is_empty() {
        return Err(ParseError::InsufficientInfoObject {
            needed: 1,
            available: 0,
            ti,
            ioa: None,
            entry_index: None,
            reason: "缺少字符串长度字节",
        });
    }
    let len = data[0] as usize;
    if data.len() < 1 + len {
        return Err(ParseError::InsufficientInfoObject {
            needed: 1 + len,
            available: data.len(),
            ti,
            ioa: None,
            entry_index: None,
            reason: "字符串数据不足",
        });
    }
    let s = String::from_utf8_lossy(&data[1..1 + len]).to_string();
    Ok((s, 1 + len))
}

fn parse_follow_flag(flag: u8, ti: TypeId, ioa: u32, offset: usize) -> Result<FollowFlag, ParseError> {
    FollowFlag::try_from(flag).map_err(|_| ParseError::InvalidInfoObject {
        reason: "无效的后续标志（只能为 0/1）",
        ti,
        ioa: Some(ioa),
        entry_index: None,
        offset: Some(offset),
    })
}

/// 解析配电文件传输 (TI 210)。
pub fn parse_file_transfer<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    let ti = header.type_id;
    // 最小长度: IOA(3) + 操作标识(1) = 4
    if payload.len() < 4 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 4,
            available: payload.len(),
            ti,
            ioa: None,
            entry_index: None,
            reason: "文件传输数据不足",
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
    let op_byte = payload[3];
    let data = &payload[4..];

    let operation = FileOperation::try_from(op_byte).map_err(|_| ParseError::InvalidInfoObject {
        reason: "无效的文件操作标识",
        ti,
        ioa: Some(ioa),
        entry_index: None,
        offset: Some(3),
    })?;

    let transfer = match operation {
        FileOperation::DirectoryRequest => parse_op1_directory_request(header, ioa, data, ti)?,
        FileOperation::DirectoryConfirm => parse_op2_directory_confirm(header, ioa, data, ti)?,
        FileOperation::ReadActivate => parse_op3_read_activate(header, ioa, data, ti)?,
        FileOperation::ReadActivateConfirm => parse_op4_read_activate_confirm(header, ioa, data, ti)?,
        FileOperation::ReadData => parse_op5_read_data(header, ioa, data, ti)?,
        FileOperation::ReadDataConfirm => parse_op6_read_data_confirm(header, ioa, data, ti)?,
        FileOperation::WriteActivate => parse_op7_write_activate(header, ioa, data, ti)?,
        FileOperation::WriteActivateConfirm => parse_op8_write_activate_confirm(header, ioa, data, ti)?,
        FileOperation::WriteData => parse_op9_write_data(header, ioa, data, ti)?,
        FileOperation::WriteDataConfirm => parse_op10_write_data_confirm(header, ioa, data, ti)?,
    };

    Ok(Box::new(transfer))
}

/// 操作1: 文件目录召唤
fn parse_op1_directory_request(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 目录ID(4) + 目录名长度(1) + 目录名(N) + 召唤标志(1) + [起始时间(7) + 终止时间(7)]
    if data.len() < 6 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 6,
            available: data.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "目录召唤数据不足",
        });
    }

    let dir_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let (dir_name, consumed) = read_length_prefixed_string(&data[4..], ti)?;
    let rest = &data[4 + consumed..];
    if rest.is_empty() {
        return Err(ParseError::InsufficientInfoObject {
            needed: 1,
            available: 0,
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少召唤标志",
        });
    }

    let call_flag_byte = rest[0];
    let call_flag = match call_flag_byte {
        0 => DirectoryCallFlag::ListAll,
        1 => DirectoryCallFlag::ListByTime,
        _ => {
            return Err(ParseError::InvalidInfoObject {
                reason: "无效的目录召唤标志",
                ti,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(4 + consumed),
            });
        }
    };

    let (start_time, end_time) = if call_flag == DirectoryCallFlag::ListByTime {
        if rest.len() < 1 + 7 + 7 {
            return Err(ParseError::InsufficientInfoObject {
                needed: 1 + 7 + 7,
                available: rest.len(),
                ti,
                ioa: Some(ioa),
                entry_index: None,
                reason: "按时间召唤需要起始和终止时间",
            });
        }
        let start = CP56Time2a::parse(&rest[1..8]).map_err(|_| ParseError::InvalidInfoObject {
            reason: "起始时间解析失败",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: None,
        })?;
        let end = CP56Time2a::parse(&rest[8..15]).map_err(|_| ParseError::InvalidInfoObject {
            reason: "终止时间解析失败",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: None,
        })?;
        if rest.len() != 15 {
            return Err(ParseError::InvalidInfoObject {
                reason: "按时间召唤存在多余数据",
                ti,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(4 + consumed + 15),
            });
        }
        (Some(start), Some(end))
    } else {
        if rest.len() != 1 {
            return Err(ParseError::InvalidInfoObject {
                reason: "目录召唤存在多余数据",
                ti,
                ioa: Some(ioa),
                entry_index: None,
                offset: Some(4 + consumed + 1),
            });
        }
        (None, None)
    };

    Ok(FileTransfer::DirectoryRequest(FileDirectoryRequest {
        header: *header,
        ioa,
        dir_id,
        dir_name,
        call_flag,
        start_time,
        end_time,
    }))
}

/// 操作2: 目录召唤确认
fn parse_op2_directory_confirm(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 结果(1) + 目录ID(4) + 后续标志(1) + 文件数量(1) + 目录信息(M)
    if data.len() < 7 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 7,
            available: data.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "目录召唤确认数据不足",
        });
    }
    let result = FileResult::from(data[0]);
    let dir_id = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let follow_flag = parse_follow_flag(data[5], ti, ioa, 5)?;
    let file_count = data[6];

    let mut entries = Vec::with_capacity(file_count as usize);
    let mut pos = 7usize;
    for idx in 0..file_count as usize {
        if pos >= data.len() {
            return Err(ParseError::InsufficientInfoObject {
                needed: pos + 1,
                available: data.len(),
                ti,
                ioa: Some(ioa),
                entry_index: Some(idx),
                reason: "目录条目缺少文件名长度",
            });
        }

        let name_len = data[pos] as usize;
        let entry_total = 1 + name_len + 1 + 4 + 7;
        if pos + entry_total > data.len() {
            return Err(ParseError::InsufficientInfoObject {
                needed: pos + entry_total,
                available: data.len(),
                ti,
                ioa: Some(ioa),
                entry_index: Some(idx),
                reason: "目录条目数据不足",
            });
        }

        let filename = String::from_utf8_lossy(&data[pos + 1..pos + 1 + name_len]).to_string();
        let attr_pos = pos + 1 + name_len;
        let attributes = data[attr_pos];
        let size = u32::from_le_bytes([data[attr_pos + 1], data[attr_pos + 2], data[attr_pos + 3], data[attr_pos + 4]]);
        let ts_start = attr_pos + 5;
        let timestamp =
            CP56Time2a::parse(&data[ts_start..ts_start + 7]).map_err(|_| ParseError::InvalidInfoObject {
                reason: "目录条目时间戳解析失败",
                ti,
                ioa: Some(ioa),
                entry_index: Some(idx),
                offset: Some(ts_start),
            })?;

        entries.push(DirectoryEntry { filename, attributes, size, timestamp });
        pos += entry_total;
    }

    if pos != data.len() {
        return Err(ParseError::InvalidInfoObject {
            reason: "目录召唤确认存在多余数据",
            ti,
            ioa: Some(ioa),
            entry_index: None,
            offset: Some(pos),
        });
    }

    Ok(FileTransfer::DirectoryConfirm(FileDirectoryConfirm {
        header: *header,
        ioa,
        result,
        dir_id,
        follow_flag,
        file_count,
        entries,
    }))
}

/// 操作3: 读文件激活
fn parse_op3_read_activate(header: &AsduHeader, ioa: u32, data: &[u8], ti: TypeId) -> Result<FileTransfer, ParseError> {
    // 文件名长度(1) + 文件名(N)
    let (filename, _) = read_length_prefixed_string(data, ti)?;
    Ok(FileTransfer::ReadActivate(FileReadActivate { header: *header, ioa, filename }))
}

/// 操作4: 读文件激活确认
fn parse_op4_read_activate_confirm(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 结果(1) + 文件名长度(1) + 文件名(N) + 文件ID(4) + 文件大小(4)
    if data.is_empty() {
        return Err(ParseError::InsufficientInfoObject {
            needed: 1,
            available: 0,
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少结果描述字",
        });
    }
    let result = FileResult::from(data[0]);
    let (filename, consumed) = read_length_prefixed_string(&data[1..], ti)?;
    let rest = &data[1 + consumed..];

    if rest.len() < 8 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 8,
            available: rest.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少文件ID和大小",
        });
    }

    let file_id = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    let file_size = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]);

    Ok(FileTransfer::ReadActivateConfirm(FileReadActivateConfirm {
        header: *header,
        ioa,
        result,
        filename,
        file_id,
        file_size,
    }))
}

/// 操作5: 读文件数据传输
fn parse_op5_read_data(header: &AsduHeader, ioa: u32, data: &[u8], ti: TypeId) -> Result<FileTransfer, ParseError> {
    // 文件ID(4) + 数据段号(4) + 后续标志(1) + 文件数据(N) + 校验码(1)
    if data.len() < 10 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 10,
            available: data.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "读文件数据最小长度不足",
        });
    }

    let file_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let segment_no = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let follow_flag = parse_follow_flag(data[8], ti, ioa, 12)?;
    // 最后一个字节是校验码
    let checksum = data[data.len() - 1];
    let file_data = data[9..data.len() - 1].to_vec();

    Ok(FileTransfer::ReadData(FileReadData {
        header: *header,
        ioa,
        file_id,
        segment_no,
        follow_flag,
        data: file_data,
        checksum,
    }))
}

/// 操作6: 读文件数据传输确认
fn parse_op6_read_data_confirm(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 文件ID(4) + 数据段号(4) + 结果(1)
    if data.len() < 9 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 9,
            available: data.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "读文件确认数据不足",
        });
    }

    let file_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let segment_no = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let result = FileResult::from(data[8]);

    Ok(FileTransfer::ReadDataConfirm(FileReadDataConfirm { header: *header, ioa, file_id, segment_no, result }))
}

/// 操作7: 写文件激活
fn parse_op7_write_activate(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 文件名长度(1) + 文件名(N) + 文件ID(4) + 文件大小(4)
    let (filename, consumed) = read_length_prefixed_string(data, ti)?;
    let rest = &data[consumed..];

    if rest.len() < 8 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 8,
            available: rest.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少文件ID和大小",
        });
    }

    let file_id = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    let file_size = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]);

    Ok(FileTransfer::WriteActivate(FileWriteActivate { header: *header, ioa, filename, file_id, file_size }))
}

/// 操作8: 写文件激活确认
fn parse_op8_write_activate_confirm(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 结果(1) + 文件名长度(1) + 文件名(N) + 文件ID(4)
    if data.is_empty() {
        return Err(ParseError::InsufficientInfoObject {
            needed: 1,
            available: 0,
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少结果描述字",
        });
    }

    let result = FileResult::from(data[0]);
    let (filename, consumed) = read_length_prefixed_string(&data[1..], ti)?;
    let rest = &data[1 + consumed..];

    if rest.len() < 4 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 4,
            available: rest.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "缺少文件ID",
        });
    }

    let file_id = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);

    Ok(FileTransfer::WriteActivateConfirm(FileWriteActivateConfirm { header: *header, ioa, result, filename, file_id }))
}

/// 操作9: 写文件数据传输
fn parse_op9_write_data(header: &AsduHeader, ioa: u32, data: &[u8], ti: TypeId) -> Result<FileTransfer, ParseError> {
    // 文件ID(4) + 数据段号(4) + 后续标志(1) + 文件数据(N) + 校验码(1)
    if data.len() < 10 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 10,
            available: data.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "写文件数据最小长度不足",
        });
    }

    let file_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let segment_no = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let follow_flag = parse_follow_flag(data[8], ti, ioa, 12)?;
    let checksum = data[data.len() - 1];
    let file_data = data[9..data.len() - 1].to_vec();

    Ok(FileTransfer::WriteData(FileWriteData {
        header: *header,
        ioa,
        file_id,
        segment_no,
        follow_flag,
        data: file_data,
        checksum,
    }))
}

/// 操作10: 写文件数据传输确认
fn parse_op10_write_data_confirm(
    header: &AsduHeader,
    ioa: u32,
    data: &[u8],
    ti: TypeId,
) -> Result<FileTransfer, ParseError> {
    // 文件ID(4) + 数据段号(4) + 结果(1)
    if data.len() < 9 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 9,
            available: data.len(),
            ti,
            ioa: Some(ioa),
            entry_index: None,
            reason: "写文件确认数据不足",
        });
    }

    let file_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let segment_no = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let result = FileResult::from(data[8]);

    Ok(FileTransfer::WriteDataConfirm(FileWriteDataConfirm { header: *header, ioa, file_id, segment_no, result }))
}

// ============================================================================
// TI 211: 软件升级
// ============================================================================

/// 软件升级命令类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareUpgradeCommand {
    /// S/E=1: 软件升级启动
    Start,
    /// S/E=0: 软件升级结束
    Finish,
    /// 未知命令
    Unknown(u8),
}

impl From<u8> for SoftwareUpgradeCommand {
    fn from(value: u8) -> Self {
        if value & 0x7F != 0 {
            return Self::Unknown(value);
        }
        if (value & 0x80) != 0 { Self::Start } else { Self::Finish }
    }
}

/// TI 211: 软件升级命令。
#[derive(Debug, Clone)]
pub struct SoftwareUpgrade {
    /// ASDU 头部
    pub header: AsduHeader,
    /// 信息对象地址
    pub ioa: u32,
    /// 命令类型 (CTYPE)
    pub command: SoftwareUpgradeCommand,
}

impl ProfileAsdu for SoftwareUpgrade {
    fn type_id(&self) -> TypeId {
        TypeId::new(211)
    }

    fn type_name(&self) -> &'static str {
        "F_SR_NA_1"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 解析软件升级命令 (TI 211)。
pub fn parse_software_upgrade<'a>(
    header: &AsduHeader,
    payload: &'a [u8],
) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
    // 格式: IOA(3) + CTYPE(1)
    if payload.len() < 4 {
        return Err(ParseError::InsufficientInfoObject {
            needed: 4,
            available: payload.len(),
            ti: header.type_id,
            ioa: None,
            entry_index: None,
            reason: "软件升级命令数据不足",
        });
    }

    let ioa = u32::from_le_bytes([payload[0], payload[1], payload[2], 0]);
    let command = SoftwareUpgradeCommand::from(payload[3]);

    Ok(Box::new(SoftwareUpgrade { header: *header, ioa, command }))
}
