//! 主站命令处理模块。
//!
//! 本模块定义了主站发送的所有 IEC104 命令类型、错误处理和命令构建逻辑。
//!
//! # 主要功能
//!
//! - **命令定义**：定义所有 IEC104 命令类型（总召唤、控制、设点等）
//! - **命令验证**：验证命令参数的合法性（地址范围、限定词、值范围等）
//! - **ASDU 构建**：将高层命令转换为 ASDU 字节流
//! - **错误处理**：统一的错误码和错误信息
//!
//! # 命令类型
//!
//! - **系统命令**：总召唤、电能召唤、时钟同步、复位进程
//! - **控制命令**：单点控制、双点控制、升降命令
//! - **设点命令**：归一化设点、标度化设点、短浮点设点
//! - **其他命令**：比特串命令、测试命令

use chrono::{DateTime, Utc};
use iec60870_parser::parser::asdu::{
    encode::{append_cp56_time2a, encode_normalized, encode_read_command},
    spec::DLT_TEST_FBP,
};
use log::info;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        protocol_adapter::{
            build_bitstring_command_payload, build_clock_sync_command_payload, build_counter_interrogation_payload,
            build_double_control_payload, build_general_interrogation_payload, build_regulating_step_payload,
            build_reset_process_payload, build_setpoint_float_payload, build_setpoint_normalized_payload,
            build_setpoint_scaled_payload, build_single_control_payload, build_test_command_payload,
            encode_cp56_time2a,
        },
        types::{AsduInfo, ControlQualifierMode},
    },
    errors::{Iec104ErrorCode, Iec104ValidationError},
};

pub type QualifierMode = ControlQualifierMode;

/// IEC104 信息对象地址（IOA）最大值（24 位）
const IEC104_IOA_MAX: u32 = 0x00FF_FFFF;
/// 传送原因：激活（COT=6）
const COT_ACTIVATION: u8 = 6;
/// 传送原因：撤销（COT=8）
const COT_DEACTIVATION: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectExecute {
    Select,
    Execute,
    Cancel,
}

impl SelectExecute {
    pub const fn is_select(self) -> bool {
        matches!(self, Self::Select | Self::Cancel)
    }

    pub const fn cause(self) -> u8 {
        if matches!(self, Self::Cancel) { COT_DEACTIVATION } else { COT_ACTIVATION }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InterrogationQualifier {
    Station,
    Group { group: u8 },
}

impl InterrogationQualifier {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Station => 20,
            Self::Group { group } => 20u8.saturating_add(*group),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CounterFreeze {
    Read,
    Freeze,
    FreezeAndReset,
    Reset,
}

impl CounterFreeze {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Read => 0,
            Self::Freeze => 1,
            Self::FreezeAndReset => 2,
            Self::Reset => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterInterrogationQualifier {
    pub request: u8,
    pub freeze: CounterFreeze,
}

impl CounterInterrogationQualifier {
    pub const fn as_u8(self) -> u8 {
        (self.freeze.as_u8() << 6) | (self.request & 0x3F)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResetProcessQualifier {
    GeneralReset,
    ClearEventBuffer,
}

impl ResetProcessQualifier {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::GeneralReset => 1,
            Self::ClearEventBuffer => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetpointQos {
    pub ql: u8,
    pub se: SelectExecute,
}

impl SetpointQos {
    pub const fn as_u8(self) -> u8 {
        let se_bit = if self.se.is_select() { 0x80 } else { 0x00 };
        se_bit | (self.ql & 0x7F)
    }
}

/// IEC104 主站统一命令模型（强类型限定词）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Iec104Command {
    GeneralInterrogation {
        qoi: InterrogationQualifier,
    },
    CounterInterrogation {
        qcc: CounterInterrogationQualifier,
    },
    ClockSynchronization {
        #[serde(default)]
        timestamp: Option<DateTime<Utc>>,
    },
    TestCommand {
        #[serde(default)]
        ioa: u32,
        #[serde(default = "default_test_pattern")]
        pattern: u16,
    },
    ResetProcessCommand {
        #[serde(default)]
        ioa: u32,
        qrp: ResetProcessQualifier,
    },
    SingleCommand {
        ioa: u32,
        value: bool,
        se: SelectExecute,
        qu: u8,
    },
    DoubleCommand {
        ioa: u32,
        value: u8,
        se: SelectExecute,
        qu: u8,
    },
    RegulatingStepCommand {
        ioa: u32,
        step: u8,
        se: SelectExecute,
        qu: u8,
    },
    SetPointNormalized {
        ioa: u32,
        value: f64,
        qos: SetpointQos,
    },
    SetPointScaled {
        ioa: u32,
        value: i16,
        qos: SetpointQos,
    },
    SetPointShortFloat {
        ioa: u32,
        value: f32,
        qos: SetpointQos,
    },
    BitStringCommand {
        ioa: u32,
        value: u32,
    },
    SingleCommandTimed {
        ioa: u32,
        value: bool,
        se: SelectExecute,
        qu: u8,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    DoubleCommandTimed {
        ioa: u32,
        value: u8,
        se: SelectExecute,
        qu: u8,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    RegulatingStepCommandTimed {
        ioa: u32,
        step: u8,
        se: SelectExecute,
        qu: u8,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    SetPointNormalizedTimed {
        ioa: u32,
        value: f64,
        qos: SetpointQos,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    SetPointScaledTimed {
        ioa: u32,
        value: i16,
        qos: SetpointQos,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    SetPointShortFloatTimed {
        ioa: u32,
        value: f32,
        qos: SetpointQos,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    BitStringCommandTimed {
        ioa: u32,
        value: u32,
        #[serde(default = "default_command_timestamp")]
        timestamp: DateTime<Utc>,
    },
    ReadCommand {
        ioa: u32,
    },
}

fn default_test_pattern() -> u16 {
    DLT_TEST_FBP
}

fn default_command_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDispatchReceipt {
    pub trace_id: String,
    pub type_id: u8,
    pub common_address: u16,
}

/// 主站命令处理器
pub struct MasterCommands;

impl MasterCommands {
    /// 统一命令构建入口。
    pub fn build_asdu(common_address: u16, command: &Iec104Command) -> Result<AsduInfo, Iec104ValidationError> {
        Self::build_asdu_with_qualifier_mode(common_address, command, QualifierMode::Standard)
    }

    pub fn build_asdu_with_qualifier_mode(
        common_address: u16,
        command: &Iec104Command,
        qualifier_mode: QualifierMode,
    ) -> Result<AsduInfo, Iec104ValidationError> {
        validate_common_address(common_address)?;
        let cause = command_cause(command);

        let now = Utc::now();
        let (type_id, data) = match command {
            Iec104Command::GeneralInterrogation { qoi } => {
                validate_general_interrogation_qualifier(qoi)?;
                (100, build_general_interrogation_payload(qoi.as_u8()))
            }
            Iec104Command::CounterInterrogation { qcc } => {
                validate_counter_interrogation_qualifier(*qcc)?;
                (101, build_counter_interrogation_payload(qcc.as_u8()))
            }
            Iec104Command::ClockSynchronization { timestamp } => {
                let ts = timestamp.unwrap_or(now);
                (103, build_clock_sync_command_payload(0, ts))
            }
            Iec104Command::TestCommand { ioa, pattern } => {
                validate_ioa(*ioa)?;
                if *pattern != DLT_TEST_FBP {
                    return Err(Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        format!("test command pattern must be 0x{DLT_TEST_FBP:04X}"),
                    ));
                }
                (104, build_test_command_payload(*ioa, *pattern))
            }
            Iec104Command::ResetProcessCommand { ioa, qrp } => {
                validate_ioa(*ioa)?;
                (105, build_reset_process_payload(*ioa, qrp.as_u8()))
            }
            Iec104Command::SingleCommand { ioa, value, se, qu } => {
                validate_ioa(*ioa)?;
                validate_command_qualifier(*qu, qualifier_mode)?;
                (45, build_single_control_payload(*ioa, *value, se.is_select(), *qu))
            }
            Iec104Command::DoubleCommand { ioa, value, se, qu } => {
                validate_ioa(*ioa)?;
                validate_double_command_value(*value)?;
                validate_command_qualifier(*qu, qualifier_mode)?;
                (46, build_double_control_payload(*ioa, *value, se.is_select(), *qu))
            }
            Iec104Command::RegulatingStepCommand { ioa, step, se, qu } => {
                validate_ioa(*ioa)?;
                validate_regulating_step(*step)?;
                validate_command_qualifier(*qu, qualifier_mode)?;
                (47, build_regulating_step_payload(*ioa, *step, se.is_select(), *qu))
            }
            Iec104Command::SetPointNormalized { ioa, value, qos } => {
                validate_ioa(*ioa)?;
                validate_setpoint_qos(*qos)?;
                let raw = encode_normalized(*value).map_err(|_| {
                    Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        "normalized setpoint must be finite and in [-1.0, 1.0)",
                    )
                })?;
                (48, build_setpoint_normalized_payload(*ioa, raw, qos.as_u8()))
            }
            Iec104Command::SetPointScaled { ioa, value, qos } => {
                validate_ioa(*ioa)?;
                validate_setpoint_qos(*qos)?;
                (49, build_setpoint_scaled_payload(*ioa, *value, qos.as_u8()))
            }
            Iec104Command::SetPointShortFloat { ioa, value, qos } => {
                validate_ioa(*ioa)?;
                if !value.is_finite() {
                    return Err(Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        "setpoint short float must be finite",
                    ));
                }
                validate_setpoint_qos(*qos)?;
                (50, build_setpoint_float_payload(*ioa, *value, qos.as_u8()))
            }
            Iec104Command::BitStringCommand { ioa, value } => {
                validate_ioa(*ioa)?;
                (51, build_bitstring_command_payload(*ioa, *value))
            }
            Iec104Command::SingleCommandTimed { ioa, value, se, qu, timestamp } => {
                validate_ioa(*ioa)?;
                validate_command_qualifier(*qu, qualifier_mode)?;
                (58, append_cp56(build_single_control_payload(*ioa, *value, se.is_select(), *qu), *timestamp))
            }
            Iec104Command::DoubleCommandTimed { ioa, value, se, qu, timestamp } => {
                validate_ioa(*ioa)?;
                validate_double_command_value(*value)?;
                validate_command_qualifier(*qu, qualifier_mode)?;
                (59, append_cp56(build_double_control_payload(*ioa, *value, se.is_select(), *qu), *timestamp))
            }
            Iec104Command::RegulatingStepCommandTimed { ioa, step, se, qu, timestamp } => {
                validate_ioa(*ioa)?;
                validate_regulating_step(*step)?;
                validate_command_qualifier(*qu, qualifier_mode)?;
                (60, append_cp56(build_regulating_step_payload(*ioa, *step, se.is_select(), *qu), *timestamp))
            }
            Iec104Command::SetPointNormalizedTimed { ioa, value, qos, timestamp } => {
                validate_ioa(*ioa)?;
                validate_setpoint_qos(*qos)?;
                let raw = encode_normalized(*value).map_err(|_| {
                    Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        "normalized setpoint must be finite and in [-1.0, 1.0)",
                    )
                })?;
                (61, append_cp56(build_setpoint_normalized_payload(*ioa, raw, qos.as_u8()), *timestamp))
            }
            Iec104Command::SetPointScaledTimed { ioa, value, qos, timestamp } => {
                validate_ioa(*ioa)?;
                validate_setpoint_qos(*qos)?;
                (62, append_cp56(build_setpoint_scaled_payload(*ioa, *value, qos.as_u8()), *timestamp))
            }
            Iec104Command::SetPointShortFloatTimed { ioa, value, qos, timestamp } => {
                validate_ioa(*ioa)?;
                if !value.is_finite() {
                    return Err(Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        "setpoint short float must be finite",
                    ));
                }
                validate_setpoint_qos(*qos)?;
                (63, append_cp56(build_setpoint_float_payload(*ioa, *value, qos.as_u8()), *timestamp))
            }
            Iec104Command::BitStringCommandTimed { ioa, value, timestamp } => {
                validate_ioa(*ioa)?;
                (64, append_cp56(build_bitstring_command_payload(*ioa, *value), *timestamp))
            }
            Iec104Command::ReadCommand { ioa } => {
                validate_ioa(*ioa)?;
                (102, encode_read_command(*ioa))
            }
        };

        info!("构建IEC104命令 type_id={} common_address={}", type_id, common_address);
        Ok(AsduInfo { type_id, cause, common_address, data, timestamp: now })
    }
}

fn append_cp56(payload: Vec<u8>, timestamp: DateTime<Utc>) -> Vec<u8> {
    append_cp56_time2a(payload, encode_cp56_time2a(timestamp))
}

fn validate_common_address(common_address: u16) -> Result<(), Iec104ValidationError> {
    if common_address == 0 {
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidCommonAddress,
            "common address must be in 1..=65535",
        ));
    }
    Ok(())
}

fn command_cause(command: &Iec104Command) -> u16 {
    match command {
        Iec104Command::SingleCommand { se, .. }
        | Iec104Command::DoubleCommand { se, .. }
        | Iec104Command::RegulatingStepCommand { se, .. }
        | Iec104Command::SingleCommandTimed { se, .. }
        | Iec104Command::DoubleCommandTimed { se, .. }
        | Iec104Command::RegulatingStepCommandTimed { se, .. } => u16::from(se.cause()),
        Iec104Command::SetPointNormalized { qos, .. }
        | Iec104Command::SetPointScaled { qos, .. }
        | Iec104Command::SetPointShortFloat { qos, .. }
        | Iec104Command::SetPointNormalizedTimed { qos, .. }
        | Iec104Command::SetPointScaledTimed { qos, .. }
        | Iec104Command::SetPointShortFloatTimed { qos, .. } => u16::from(qos.se.cause()),
        _ => u16::from(COT_ACTIVATION),
    }
}

fn validate_ioa(ioa: u32) -> Result<(), Iec104ValidationError> {
    if ioa > IEC104_IOA_MAX {
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidObjectAddress,
            format!("IOA {ioa} exceeds 24-bit range"),
        ));
    }
    Ok(())
}

fn validate_general_interrogation_qualifier(qoi: &InterrogationQualifier) -> Result<(), Iec104ValidationError> {
    match qoi {
        InterrogationQualifier::Station => Ok(()),
        InterrogationQualifier::Group { group } if (1..=16).contains(group) => Ok(()),
        InterrogationQualifier::Group { group } => Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("QOI group {group} is invalid, expected 1..=16"),
        )),
    }
}

fn validate_counter_interrogation_qualifier(qcc: CounterInterrogationQualifier) -> Result<(), Iec104ValidationError> {
    if (1..=5).contains(&qcc.request) {
        Ok(())
    } else {
        Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("QCC request group {} is invalid, expected 1..=5", qcc.request),
        ))
    }
}

fn validate_double_command_value(value: u8) -> Result<(), Iec104ValidationError> {
    if matches!(value, 1 | 2) {
        Ok(())
    } else {
        Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidValue,
            format!("double command value {value} is invalid, expected 1(off) or 2(on)"),
        ))
    }
}

fn validate_regulating_step(step: u8) -> Result<(), Iec104ValidationError> {
    if matches!(step, 1 | 2) {
        Ok(())
    } else {
        Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidValue,
            format!("regulating step {step} is invalid, expected 1(decrease) or 2(increase)"),
        ))
    }
}

fn validate_command_qualifier(qu: u8, mode: QualifierMode) -> Result<(), Iec104ValidationError> {
    if qu <= 3 || (mode == QualifierMode::RawTest && qu <= 31) {
        Ok(())
    } else {
        Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!(
                "QOC QU {qu} is invalid, expected {}",
                if mode == QualifierMode::RawTest { "0..=31" } else { "0..=3" }
            ),
        ))
    }
}

fn validate_setpoint_qos(qos: SetpointQos) -> Result<(), Iec104ValidationError> {
    if qos.ql <= 0x7F {
        Ok(())
    } else {
        Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("QOS QL {} is invalid, expected 0..=127", qos.ql),
        ))
    }
}
