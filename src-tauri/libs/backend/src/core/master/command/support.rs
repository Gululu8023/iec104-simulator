//! 主站命令辅助逻辑
//!
//! 提供命令元数据提取等辅助功能。

use crate::core::{
    master::{Iec104Command, SelectExecute},
    protocol_adapter::encode_cp56_time2a,
};

/// 提取命令的主要信息对象地址（IOA）
///
/// 不同类型的命令有不同的 IOA 语义：
/// - 总召、计数器召唤：IOA 为 0（广播地址）
/// - 时钟同步：无 IOA
/// - 其他命令：使用命令中指定的 IOA
///
/// # 参数
///
/// * `command` - IEC104 命令
///
/// # 返回
///
/// - `Some(u32)` - 命令的主要 IOA
/// - `None` - 命令无 IOA（如时钟同步）
pub(crate) fn command_primary_ioa(command: &Iec104Command) -> Option<u32> {
    match command {
        Iec104Command::GeneralInterrogation { .. } => Some(0),
        Iec104Command::CounterInterrogation { .. } => Some(0),
        Iec104Command::ClockSynchronization { .. } => None,
        Iec104Command::TestCommand { ioa, .. } => Some(*ioa),
        Iec104Command::ResetProcessCommand { ioa, .. } => Some(*ioa),
        Iec104Command::SingleCommand { ioa, .. } => Some(*ioa),
        Iec104Command::DoubleCommand { ioa, .. } => Some(*ioa),
        Iec104Command::RegulatingStepCommand { ioa, .. } => Some(*ioa),
        Iec104Command::SetPointNormalized { ioa, .. } => Some(*ioa),
        Iec104Command::SetPointScaled { ioa, .. } => Some(*ioa),
        Iec104Command::SetPointShortFloat { ioa, .. } => Some(*ioa),
        Iec104Command::BitStringCommand { ioa, .. } => Some(*ioa),
        Iec104Command::SingleCommandTimed { ioa, .. }
        | Iec104Command::DoubleCommandTimed { ioa, .. }
        | Iec104Command::RegulatingStepCommandTimed { ioa, .. }
        | Iec104Command::SetPointNormalizedTimed { ioa, .. }
        | Iec104Command::SetPointScaledTimed { ioa, .. }
        | Iec104Command::SetPointShortFloatTimed { ioa, .. }
        | Iec104Command::BitStringCommandTimed { ioa, .. } => Some(*ioa),
        Iec104Command::ReadCommand { ioa } => Some(*ioa),
    }
}

pub(crate) fn command_qualifier_identity(command: &Iec104Command) -> Option<u8> {
    match command {
        Iec104Command::GeneralInterrogation { qoi } => Some(qoi.as_u8()),
        Iec104Command::CounterInterrogation { qcc } => Some(qcc.as_u8()),
        Iec104Command::SingleCommand { qu, .. }
        | Iec104Command::DoubleCommand { qu, .. }
        | Iec104Command::RegulatingStepCommand { qu, .. }
        | Iec104Command::SingleCommandTimed { qu, .. }
        | Iec104Command::DoubleCommandTimed { qu, .. }
        | Iec104Command::RegulatingStepCommandTimed { qu, .. } => Some(*qu),
        Iec104Command::SetPointNormalized { qos, .. }
        | Iec104Command::SetPointScaled { qos, .. }
        | Iec104Command::SetPointShortFloat { qos, .. }
        | Iec104Command::SetPointNormalizedTimed { qos, .. }
        | Iec104Command::SetPointScaledTimed { qos, .. }
        | Iec104Command::SetPointShortFloatTimed { qos, .. } => Some(qos.ql),
        _ => None,
    }
}

pub(crate) fn command_cp56_identity(command: &Iec104Command) -> Option<[u8; 7]> {
    match command {
        Iec104Command::SingleCommandTimed { timestamp, .. }
        | Iec104Command::DoubleCommandTimed { timestamp, .. }
        | Iec104Command::RegulatingStepCommandTimed { timestamp, .. }
        | Iec104Command::SetPointNormalizedTimed { timestamp, .. }
        | Iec104Command::SetPointScaledTimed { timestamp, .. }
        | Iec104Command::SetPointShortFloatTimed { timestamp, .. }
        | Iec104Command::BitStringCommandTimed { timestamp, .. } => Some(encode_cp56_time2a(*timestamp)),
        _ => None,
    }
}

/// 提取命令的选择/执行标志
///
/// 用于区分命令是选择（Select）还是执行（Execute）操作。
/// 只有控制命令和设定值命令才有此标志。
///
/// # 参数
///
/// * `command` - IEC104 命令
///
/// # 返回
///
/// - `Some(SelectExecute)` - 命令的选择/执行标志
/// - `None` - 命令不支持选择/执行（如总召、时钟同步等）
pub(crate) fn command_select_execute(command: &Iec104Command) -> Option<SelectExecute> {
    match command {
        Iec104Command::SingleCommand { se, .. }
        | Iec104Command::DoubleCommand { se, .. }
        | Iec104Command::RegulatingStepCommand { se, .. }
        | Iec104Command::SingleCommandTimed { se, .. }
        | Iec104Command::DoubleCommandTimed { se, .. }
        | Iec104Command::RegulatingStepCommandTimed { se, .. } => Some(*se),
        Iec104Command::SetPointNormalized { qos, .. }
        | Iec104Command::SetPointScaled { qos, .. }
        | Iec104Command::SetPointShortFloat { qos, .. }
        | Iec104Command::SetPointNormalizedTimed { qos, .. }
        | Iec104Command::SetPointScaledTimed { qos, .. }
        | Iec104Command::SetPointShortFloatTimed { qos, .. } => Some(qos.se),
        _ => None,
    }
}

/// 提取命令的目标值
///
/// 将命令中的控制值或设定值统一转换为 f64 类型,便于存储和展示。
///
/// # 参数
///
/// * `command` - IEC104 命令
///
/// # 返回
///
/// - `Some(f64)` - 命令的目标值
/// - `None` - 命令无目标值（如总召、时钟同步等）
pub(crate) fn command_target_value(command: &Iec104Command) -> Option<f64> {
    match command {
        Iec104Command::SingleCommand { value, .. } | Iec104Command::SingleCommandTimed { value, .. } => {
            Some(if *value { 1.0 } else { 0.0 })
        }
        Iec104Command::DoubleCommand { value, .. } | Iec104Command::DoubleCommandTimed { value, .. } => {
            Some(f64::from(*value))
        }
        Iec104Command::RegulatingStepCommand { step, .. } | Iec104Command::RegulatingStepCommandTimed { step, .. } => {
            Some(f64::from(*step))
        }
        Iec104Command::SetPointNormalized { value, .. } | Iec104Command::SetPointNormalizedTimed { value, .. } => {
            Some(*value)
        }
        Iec104Command::SetPointScaled { value, .. } | Iec104Command::SetPointScaledTimed { value, .. } => {
            Some(f64::from(*value))
        }
        Iec104Command::SetPointShortFloat { value, .. } | Iec104Command::SetPointShortFloatTimed { value, .. } => {
            Some(f64::from(*value))
        }
        Iec104Command::BitStringCommand { value, .. } | Iec104Command::BitStringCommandTimed { value, .. } => {
            Some(f64::from(*value))
        }
        _ => None,
    }
}

/// 提取时钟同步命令的时间戳文本
///
/// 将 CP56Time2a 格式的时间戳格式化为可读字符串。
///
/// # 参数
///
/// * `command` - IEC104 命令
///
/// # 返回
///
/// - `Some(String)` - 格式化的时间戳字符串（格式: "YYYY-MM-DD HH:MM:SS.mmm UTC"）
/// - `None` - 命令不是时钟同步或无时间戳
pub(crate) fn command_cp56_text(command: &Iec104Command) -> Option<String> {
    match command {
        Iec104Command::ClockSynchronization { timestamp } => {
            timestamp.map(|value| value.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string())
        }
        Iec104Command::SingleCommandTimed { timestamp, .. }
        | Iec104Command::DoubleCommandTimed { timestamp, .. }
        | Iec104Command::RegulatingStepCommandTimed { timestamp, .. }
        | Iec104Command::SetPointNormalizedTimed { timestamp, .. }
        | Iec104Command::SetPointScaledTimed { timestamp, .. }
        | Iec104Command::SetPointShortFloatTimed { timestamp, .. }
        | Iec104Command::BitStringCommandTimed { timestamp, .. } => {
            Some(timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string())
        }
        _ => None,
    }
}
