//! 测量类 ASDU 质量标志校验规则

use crate::{
    error::ValidationError,
    parser::asdu::{
        Asdu, TypeIdEnum, for_each_entry, lookup_type_descriptor, split_timestamp,
        types::{TypeDescriptor, TypeId},
    },
    types::FrameView,
    validator::FrameRule,
};

/// 校验测量类 ASDU 的质量字段布局。
///
/// IEC 60870-5-104 中的 IV / NT / SB / BL 属于合法线路语义，不能作为协议错误拒收。
/// 这里仅确保质量字段在对应位置存在，避免截断/错位帧绕过解析。
#[derive(Default)]
pub struct QualityRule;

impl FrameRule for QualityRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let asdu = match frame.asdu() {
            Some(asdu) => asdu,
            None => return Ok(()),
        };
        let ti = asdu.header.type_id;
        let layout = match quality_layout(ti) {
            Some(layout) => layout,
            None => return Ok(()),
        };
        let descriptor = match lookup_type_descriptor(ti) {
            Some(desc) => desc,
            None => return Ok(()),
        };
        check_quality(layout, descriptor, &asdu)
    }
}

fn check_quality(layout: QualityLayout, descriptor: &TypeDescriptor, asdu: &Asdu<'_>) -> Result<(), ValidationError> {
    let ti = asdu.header.type_id;
    for_each_entry(ti, asdu.payload, descriptor, asdu.header.vsq, |_, body| {
        let (core, _) = split_timestamp(ti, body, descriptor.timestamp_len)?;
        let _ = take_quality_byte(ti, layout, core)?;
        Ok(())
    })
    .map_err(map_quality_error)
}

#[derive(Clone, Copy)]
enum QualityLayout {
    Inline { offset: usize },
}

fn quality_layout(ti: TypeId) -> Option<QualityLayout> {
    use QualityLayout::*;
    match ti.kind() {
        // 单点信息（SIQ 在偏移 0）
        TypeIdEnum::MSpNa1 | TypeIdEnum::MSpTa1 | TypeIdEnum::MSpTb1 => Some(Inline { offset: 0 }),
        // 双点信息（DIQ 在偏移 0）
        TypeIdEnum::MDpNa1 | TypeIdEnum::MDpTa1 | TypeIdEnum::MDpTb1 => Some(Inline { offset: 0 }),
        // 步位置信息（VTI + QDS，QDS 在偏移 1）
        TypeIdEnum::MStNa1 | TypeIdEnum::MStTa1 | TypeIdEnum::MStTb1 => Some(Inline { offset: 1 }),
        // 比特串（4B 状态 + QDS，QDS 在偏移 4）
        TypeIdEnum::MBoNa1 | TypeIdEnum::MBoTa1 | TypeIdEnum::MPsNa1 | TypeIdEnum::MBoTb1 => Some(Inline { offset: 4 }),
        // 归一化遥测值（16-bit + QDS，QDS 在偏移 2）
        TypeIdEnum::MMeNa1 | TypeIdEnum::MMeTa1 | TypeIdEnum::MMeTd1 => Some(Inline { offset: 2 }),
        // 标度化遥测值（16-bit + QDS，QDS 在偏移 2）
        TypeIdEnum::MMeNb1 | TypeIdEnum::MMeTb1 | TypeIdEnum::MMeTe1 => Some(Inline { offset: 2 }),
        // 短浮点遥测值（32-bit float + QDS，QDS 在偏移 4）
        TypeIdEnum::MMeNc1 | TypeIdEnum::MMeTc1 | TypeIdEnum::MMeTf1 => Some(Inline { offset: 4 }),
        // 累计量（BCR，IV 在第 5 字节 bit7，BCR 偏移 4）
        TypeIdEnum::MItNa1 | TypeIdEnum::MItTa1 | TypeIdEnum::MItTb1 => Some(Inline { offset: 4 }),
        // 其他类型不需要质量检查
        _ => None,
    }
}

fn take_quality_byte(ti: TypeId, layout: QualityLayout, core: &[u8]) -> Result<Option<u8>, crate::error::ParseError> {
    let offset = match layout {
        QualityLayout::Inline { offset } => offset,
    };
    if offset >= core.len() {
        return Err(crate::error::ParseError::insufficient_info_object(
            "质量字段长度不足",
            ti,
            None,
            None,
            offset + 1,
            core.len(),
        ));
    }
    Ok(core.get(offset).copied())
}

fn map_quality_error(err: crate::error::ParseError) -> ValidationError {
    match err {
        crate::error::ParseError::InvalidInfoObject { reason, .. } => {
            ValidationError::InvalidQuality { reason, flags: 0 }
        }
        crate::error::ParseError::InvalidAsduHeader { reason, .. } => {
            ValidationError::InvalidQuality { reason, flags: 0 }
        }
        _ => ValidationError::InvalidQuality { reason: "质量校验失败", flags: 0 },
    }
}
