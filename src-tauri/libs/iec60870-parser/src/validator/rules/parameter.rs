//! 参数类 ASDU 校验规则

use crate::{
    error::ValidationError,
    parser::asdu::{
        parameters::{self, ParameterValue},
        spec,
    },
    types::FrameView,
    validator::{FrameRule, semantics},
};

/// 校验参数类 ASDU（TI 110~113）的 QPM/QPA 语义。
#[derive(Debug)]
pub struct ParameterQualifierRule;

impl FrameRule for ParameterQualifierRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let asdu = match frame.asdu() {
            Some(asdu) => asdu,
            None => return Ok(()),
        };
        // 使用枚举分类方法，替代魔法数字范围判断
        if !asdu.header.type_id.kind().is_parameter() {
            return Ok(());
        }
        let set = parameters::parse_parameters(asdu.header, asdu.payload)
            .map_err(|_| ValidationError::InvalidQualifier { reason: "参数 ASDU 解析失败", value: 0 })?;
        for item in set.items {
            match item.value {
                ParameterValue::Normalized { qualifier, .. }
                | ParameterValue::Scaled { qualifier, .. }
                | ParameterValue::ShortFloat { qualifier, .. } => {
                    let view = semantics::qpm_view(qualifier);
                    if !semantics::is_valid_qpm_kind(view.kind) {
                        return Err(ValidationError::InvalidQualifier {
                            reason: "QPM 类型未定义", value: qualifier
                        });
                    }
                }
                ParameterValue::Activation(qual) => {
                    if !spec::is_valid_qpa(qual) {
                        return Err(ValidationError::InvalidQualifier {
                            reason: "参数激活限定词非法", value: qual
                        });
                    }
                }
                ParameterValue::Raw(_) => {}
            }
        }
        Ok(())
    }
}
