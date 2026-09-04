//! 安全认证 ASDU 校验规则

use crate::{
    error::ValidationError,
    parser::asdu::security::{self, SecurityFrame},
    types::FrameView,
    validator::FrameRule,
};

/// 校验安全认证 ASDU：禁止空分段。
#[derive(Debug)]
pub struct SecuritySegmentRule;

impl FrameRule for SecuritySegmentRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let asdu = match frame.asdu() {
            Some(asdu) => asdu,
            None => return Ok(()),
        };
        // 使用枚举分类方法，替代魔法数字范围判断
        if !asdu.header.type_id.kind().is_security() {
            return Ok(());
        }
        let parsed = security::parse_security(asdu.header, asdu.payload)
            .map_err(|_| ValidationError::InvalidSecurity { reason: "安全 ASDU 解析失败" })?;
        if let SecurityFrame::Segment(seg) = parsed {
            if seg.fragment.is_empty() {
                return Err(ValidationError::InvalidSecurity { reason: "安全分段数据为空" });
            }
        }
        Ok(())
    }
}
