//! 文件传输 ASDU 校验规则

use crate::{
    error::ValidationError,
    parser::asdu::file_transfer::{self, FileTransferRecord},
    types::FrameView,
    validator::FrameRule,
};

/// 校验文件传输 ASDU 中的限定词及段长度。
#[derive(Debug)]
pub struct FileTransferRule;

impl FrameRule for FileTransferRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let asdu = match frame.asdu() {
            Some(asdu) => asdu,
            None => return Ok(()),
        };
        // 使用枚举分类方法，替代魔法数字范围判断
        if !asdu.header.type_id.kind().is_file_transfer() {
            return Ok(());
        }
        let set = file_transfer::parse_file_transfer(asdu.header, asdu.payload).map_err(|_| {
            ValidationError::InvalidFileTransfer { reason: "文件 ASDU 解析失败", field: "ASDU", value: 0 }
        })?;
        for rec in set.items {
            match rec {
                FileTransferRecord::FileReady(r) => check_file_qualifier(r.qualifier, "FRQ")?,
                FileTransferRecord::SectionReady(r) => check_file_qualifier(r.qualifier, "SRQ")?,
                FileTransferRecord::SelectCall(r) => check_file_qualifier(r.qualifier, "SCQ")?,
                FileTransferRecord::LastSection(r) => check_file_qualifier(r.qualifier, "LSQ")?,
                FileTransferRecord::AckSection(r) => check_file_qualifier(r.qualifier, "AFQ")?,
                FileTransferRecord::Directory(r) => check_file_qualifier(r.qualifier, "DRQ")?,
                FileTransferRecord::QueryLog(_) => {}
                FileTransferRecord::Segment(seg) => {
                    // LOS 字段指定的长度应与实际数据长度匹配
                    if seg.los as usize != seg.data.len() {
                        return Err(ValidationError::InvalidFileTransfer {
                            reason: "文件段 LOS 与数据长度不匹配",
                            field: "LOS",
                            value: seg.los as u32,
                        });
                    }
                }
                FileTransferRecord::Raw(_) => {}
            }
        }
        Ok(())
    }
}

fn check_file_qualifier(qualifier: u8, name: &'static str) -> Result<(), ValidationError> {
    // SCQ（选择和调用限定词）的所有位在实际使用中均有含义：
    // 0x01=选择文件, 0x02=目录请求, 0x03=下载选择, 0x20=段请求 等
    // 作为模拟器应兼容接收，不做预留位限制。
    if name == "SCQ" {
        return Ok(());
    }
    // AFQ 的低四位分别表示文件正/负确认和节正/负确认，只有高四位预留。
    let reserved_mask = if name == "AFQ" { 0b1111_0000 } else { 0b0011_1100 };
    if qualifier & reserved_mask != 0 {
        let reason = match name {
            "FRQ" => "FRQ 预留位需为 0",
            "SRQ" => "SRQ 预留位需为 0",
            "LSQ" => "LSQ 预留位需为 0",
            "AFQ" => "AFQ 预留位需为 0",
            "DRQ" => "DRQ 预留位需为 0",
            "QLQ" => "QLQ 预留位需为 0",
            _ => "文件限定词预留位需为 0",
        };
        Err(ValidationError::InvalidFileTransfer { reason, field: name, value: qualifier as u32 })
    } else {
        Ok(())
    }
}
