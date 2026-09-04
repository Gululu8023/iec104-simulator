//! 命令 COT 语义校验规则

use crate::{
    error::ValidationError,
    types::FrameView,
    validator::{
        FrameRule,
        semantics::{self, SemanticError},
    },
};

/// 校验命令 ASDU 的 COT 语义（可选规则）。
#[derive(Debug)]
pub struct CommandCotRule;

impl FrameRule for CommandCotRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let header = match frame.asdu_header() {
            Some(header) => header,
            None => return Ok(()),
        };
        match semantics::validate_command_cot(&header) {
            Ok(()) => Ok(()),
            Err(SemanticError::InvalidCot(msg)) => {
                Err(ValidationError::InvalidCause { reason: msg, cot: header.cot.as_u16() })
            }
        }
    }
}
