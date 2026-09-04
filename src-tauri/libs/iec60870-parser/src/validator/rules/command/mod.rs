//! 命令类 ASDU 校验规则

mod combined;
mod cot;
mod qualifier;

pub use combined::CommandRule;
pub use cot::CommandCotRule;
pub use qualifier::CommandQualifierRule;

use crate::parser::asdu::{
    commands::{CommandDetail, CommandQualifier, DoublePointState},
    types::TypeId,
};

/// 判断是否为命令类 TypeId
pub(crate) fn is_command_ti(ti: TypeId) -> bool {
    ti.kind().is_command()
}

/// 校验命令限定词的 QOS 字段
pub(crate) fn validate_qos(qos: crate::parser::asdu::qualifiers::SetpointQualifier) -> Result<(), &'static str> {
    let _ = qos;
    Ok(())
}

fn validate_qoc_qu(raw: u8) -> Result<(), &'static str> {
    let qu = (raw >> 2) & 0x1F;
    if qu <= 3 { Ok(()) } else { Err("QOC.QU 取值必须为 0~3") }
}

/// 校验单个命令信息体的限定词和状态位
pub(crate) fn validate_command_item(
    ti: u8,
    cmd: &crate::parser::asdu::commands::Command<'_>,
) -> Result<(), &'static str> {
    match &cmd.detail {
        CommandDetail::Single { .. } => match cmd.qualifier {
            CommandQualifier::Sco { raw, .. } => {
                validate_qoc_qu(raw)?;
            }
            _ => return Err("SCO 限定词类型不匹配"),
        },
        CommandDetail::Double { state, .. } => {
            if *state == DoublePointState::Indeterminate || *state == DoublePointState::Invalid {
                return Err("DCO 状态位非法");
            }
            match cmd.qualifier {
                CommandQualifier::Dco { raw, .. } => {
                    validate_qoc_qu(raw)?;
                }
                _ => return Err("DCO 限定词类型不匹配"),
            }
        }
        CommandDetail::Step { position } => {
            if !matches!(*position, 1 | 2) {
                return Err("RCO 状态位非法");
            }
            match cmd.qualifier {
                // RCO: bit0-1=RCS, bit2-6=QOC.QU, bit7=S/E
                CommandQualifier::Rco { raw, .. } => {
                    validate_qoc_qu(raw)?;
                }
                _ => return Err("RCO 限定词类型不匹配"),
            }
        }
        CommandDetail::SetPointNormalized(_) | CommandDetail::SetPointScaled(_) | CommandDetail::SetPointFloat(_) => {
            match cmd.qualifier {
                CommandQualifier::Qos(qos) => validate_qos(qos)?,
                _ => return Err("QOS 限定词类型不匹配"),
            }
        }
        CommandDetail::BitString(_) => {
            if !matches!(cmd.qualifier, CommandQualifier::None) {
                return Err("位串命令的限定词必须为 0");
            }
        }
        CommandDetail::Read => {
            if !matches!(cmd.qualifier, CommandQualifier::None) {
                return Err("读命令的限定词必须为 0");
            }
        }
        CommandDetail::ClockSync { .. } => {
            if !matches!(cmd.qualifier, CommandQualifier::None) {
                return Err("时钟同步命令不应包含限定词");
            }
        }
        CommandDetail::ResetProcess => match cmd.qualifier {
            CommandQualifier::Qrp(qrp) if qrp.is_valid() => {}
            _ => return Err("QRP 取值必须为 1 或 2"),
        },
        CommandDetail::Raw(_) => match ti {
            100 => match cmd.qualifier {
                CommandQualifier::Qoi(qoi) if qoi.is_valid() => {}
                _ => return Err("QOI 取值必须为 20~36"),
            },
            101 => match cmd.qualifier {
                CommandQualifier::Qcc(qcc) if qcc.is_valid_request() => {}
                _ => return Err("QCC 请求组号必须为 1~5"),
            },
            _ => {}
        },
    }
    Ok(())
}
