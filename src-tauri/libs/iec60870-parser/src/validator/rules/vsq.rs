//! VSQ (信息体数量) 校验规则

use crate::{error::ValidationError, types::FrameView, validator::FrameRule};

/// 校验 VSQ（数量 >0）。
#[derive(Debug)]
pub struct VsqRule;

impl FrameRule for VsqRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let header = match frame.asdu_header() {
            Some(header) => header,
            None => return Ok(()),
        };
        let vsq = header.vsq;
        let count = vsq & 0x7F;
        if count == 0 {
            return Err(ValidationError::InvalidVsq { reason: "信息体数量为0", count, sq: (vsq & 0x80) != 0 });
        }
        Ok(())
    }
}
