//! 命令类组合校验规则（限定词）。

use super::{is_command_ti, validate_command_item};
use crate::{error::ValidationError, parser::asdu::commands, types::FrameView, validator::FrameRule};

/// 组合校验命令类 ASDU，避免重复解析同一 payload。
#[derive(Debug)]
pub struct CommandRule;

impl FrameRule for CommandRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        let asdu = match frame.asdu() {
            Some(asdu) => asdu,
            None => return Ok(()),
        };
        if !is_command_ti(asdu.header.type_id) {
            return Ok(());
        }

        let set = commands::parse_commands(asdu.header, asdu.payload)
            .map_err(|_| ValidationError::InvalidQualifier { reason: "命令 ASDU 解析失败", value: 0 })?;

        for cmd in &set.items {
            if let Err(msg) = validate_command_item(asdu.header.type_id.raw(), cmd) {
                return Err(ValidationError::InvalidQualifier { reason: msg, value: cmd.qualifier.as_u8() });
            }
        }

        Ok(())
    }
}
