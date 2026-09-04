//! COT (传送原因) 校验规则

use crate::{error::ValidationError, types::FrameView, validator::FrameRule};

/// 校验 COT（原因码非 0）。
#[derive(Debug)]
pub struct CauseRule;

impl FrameRule for CauseRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let header = match frame.asdu_header() {
            Some(header) => header,
            None => return Ok(()),
        };
        let cause = header.cot.cause_of_transmission();
        if cause == 0 {
            return Err(ValidationError::InvalidCause {
                reason: "COT 原因码为 0，非法", cot: header.cot.as_u16()
            });
        }
        Ok(())
    }
}
