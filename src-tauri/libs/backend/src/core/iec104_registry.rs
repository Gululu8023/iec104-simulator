use iec60870_parser::parser::asdu::types::{
    InformationFamily, QualityKind, TYPE_DESCRIPTOR_TABLE, TimestampKind, TypeDescriptor, TypeId, ValueKind,
    lookup_type_descriptor,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TypeFamily {
    SinglePoint,
    DoublePoint,
    StepPosition,
    Bitstring,
    Normalized,
    Scaled,
    ShortFloat,
    IntegratedTotal,
    Protection,
    PackedStatus,
    Control,
    Initialization,
    Security,
    SystemCommand,
    Parameter,
    FileTransfer,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueModel {
    None,
    Boolean,
    DoublePoint,
    StepPosition,
    Bitstring32,
    Normalized,
    ScaledI16,
    Float32,
    Counter,
    Structured,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointRole {
    Monitor,
    Control,
    NonPoint,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationInputKind {
    Select,
    Integer,
    Float,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SimulationCapability {
    pub allowed_waveforms: &'static [&'static str],
    pub default_waveform: &'static str,
    pub input_kind: SimulationInputKind,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub precision: u8,
}

impl ValueModel {
    pub(crate) fn accepts(self, value: f64) -> bool {
        use iec60870_parser::parser::asdu::encode::{
            encode_bitstring32, encode_counter, encode_double_point, encode_normalized, encode_scaled_i16,
            encode_short_float, encode_single_point, encode_step_position,
        };

        match self {
            Self::Boolean => encode_single_point(value).is_ok(),
            Self::DoublePoint => encode_double_point(value).is_ok(),
            Self::StepPosition => encode_step_position(value).is_ok(),
            Self::Bitstring32 => encode_bitstring32(value).is_ok(),
            Self::Normalized => encode_normalized(value).is_ok(),
            Self::ScaledI16 => encode_scaled_i16(value).is_ok(),
            Self::Float32 => encode_short_float(value).is_ok(),
            Self::Counter => encode_counter(value).is_ok(),
            Self::None | Self::Structured => true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityModel {
    None,
    Siq,
    Diq,
    Qds,
    Bcr,
    Qdp,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampModel {
    None,
    Cp24,
    Cp56,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CotLifecycle {
    Report,
    SelectExecute,
    ActivationTermination,
    ActivationConfirmation,
    RequestResponse,
    FileSession,
    ParserOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct Iec104Capability {
    pub type_id: u8,
    pub name: &'static str,
    pub ioa_length: usize,
    pub family: TypeFamily,
    pub point_role: PointRole,
    pub master_send: bool,
    pub master_receive: bool,
    pub slave_receive: bool,
    pub slave_upload: bool,
    pub point_configurable: bool,
    pub parser_only: bool,
    pub value_model: ValueModel,
    pub quality_model: QualityModel,
    pub timestamp_model: TimestampModel,
    pub cot_lifecycle: CotLifecycle,
    pub simulation: Option<SimulationCapability>,
}

pub fn lookup_capability(type_id: u8) -> Option<Iec104Capability> {
    lookup_type_descriptor(TypeId::new(type_id)).map(capability_from_descriptor)
}

pub fn all_capabilities() -> Vec<Iec104Capability> {
    TYPE_DESCRIPTOR_TABLE.iter().flatten().map(capability_from_descriptor).collect()
}

pub fn same_type_family(left: u8, right: u8) -> bool {
    lookup_capability(left).zip(lookup_capability(right)).is_some_and(|(left, right)| left.family == right.family)
}

pub fn iec104_type_name(type_id: u8) -> &'static str {
    lookup_type_descriptor(TypeId::new(type_id)).map(|descriptor| descriptor.name).unwrap_or("UNKNOWN")
}

pub fn iec104_type_id(type_name: &str) -> Option<u8> {
    let normalized = type_name.trim().to_ascii_uppercase();
    TYPE_DESCRIPTOR_TABLE
        .iter()
        .flatten()
        .find(|descriptor| descriptor.name == normalized)
        .map(|descriptor| descriptor.type_id.raw())
}

pub fn iec104_protocol_default_value(type_id: u8) -> f64 {
    match lookup_capability(type_id).map(|capability| capability.value_model) {
        Some(ValueModel::DoublePoint) => 1.0,
        _ => 0.0,
    }
}

pub fn iec104_default_point_name_prefix(type_id: u8) -> &'static str {
    match type_id {
        1 | 2 | 30 => "单点",
        3 | 4 | 31 => "双点",
        5 | 6 | 32 => "步位",
        7 | 8 | 33 => "位串",
        9 | 10 | 21 | 34 => "归一化测量",
        11 | 12 | 35 => "标度化测量",
        13 | 14 | 36 => "短浮点测量",
        15 | 16 | 37 => "累计量",
        17 | 38 => "保护事件",
        18 | 39 => "保护启动",
        19 | 40 => "保护输出",
        20 => "成组单点",
        45 | 58 => "单命令",
        46 | 59 => "双命令",
        47 | 60 => "升降命令",
        48 | 61 => "归一化设点",
        49 | 62 => "标度化设点",
        50 | 63 => "短浮点设点",
        51 | 64 => "位串命令",
        110 => "归一化参数",
        111 => "标度化参数",
        112 => "短浮点参数",
        113 => "参数激活",
        _ => "数据点",
    }
}

pub fn iec104_default_point_name(type_id: u8, address: u32) -> String {
    format!("{}_{}", iec104_default_point_name_prefix(type_id), address)
}

fn capability_from_descriptor(descriptor: &TypeDescriptor) -> Iec104Capability {
    let type_id = descriptor.type_id.raw();
    let family = match descriptor.family {
        InformationFamily::SinglePoint => TypeFamily::SinglePoint,
        InformationFamily::DoublePoint => TypeFamily::DoublePoint,
        InformationFamily::StepPosition => TypeFamily::StepPosition,
        InformationFamily::BitString => TypeFamily::Bitstring,
        InformationFamily::Normalized => TypeFamily::Normalized,
        InformationFamily::Scaled => TypeFamily::Scaled,
        InformationFamily::ShortFloat => TypeFamily::ShortFloat,
        InformationFamily::IntegratedTotal => TypeFamily::IntegratedTotal,
        InformationFamily::Protection => TypeFamily::Protection,
        InformationFamily::PackedStatus => TypeFamily::PackedStatus,
        InformationFamily::Control => TypeFamily::Control,
        InformationFamily::Initialization => TypeFamily::Initialization,
        InformationFamily::Security => TypeFamily::Security,
        InformationFamily::SystemCommand => TypeFamily::SystemCommand,
        InformationFamily::Parameter => TypeFamily::Parameter,
        InformationFamily::FileTransfer => TypeFamily::FileTransfer,
    };
    let measurement_supported = matches!(type_id, 1..=16 | 30..=37);
    let master_measurement_receive = measurement_supported || matches!(type_id, 20 | 21);
    let control = matches!(type_id, 45..=51 | 58..=64);
    let system_supported = matches!(type_id, 100..=105 | 107);
    let file = matches!(type_id, 120..=127);
    let master_send = matches!(type_id, 45..=51 | 58..=64 | 100..=105 | 120..=127);
    let master_receive = master_measurement_receive || control || system_supported || file;
    let slave_receive = control || system_supported || file;
    let slave_upload = measurement_supported || control || system_supported || file;
    let parser_only = !(master_send || master_receive || slave_receive || slave_upload);
    let value_model = match descriptor.value_kind {
        ValueKind::None => ValueModel::None,
        ValueKind::Boolean => ValueModel::Boolean,
        ValueKind::DoublePoint => ValueModel::DoublePoint,
        ValueKind::StepPosition => ValueModel::StepPosition,
        ValueKind::BitString32 => ValueModel::Bitstring32,
        ValueKind::Normalized => ValueModel::Normalized,
        ValueKind::ScaledI16 => ValueModel::ScaledI16,
        ValueKind::Float32 => ValueModel::Float32,
        ValueKind::Counter => ValueModel::Counter,
        ValueKind::Structured => ValueModel::Structured,
    };

    Iec104Capability {
        type_id,
        name: descriptor.name,
        ioa_length: descriptor.ioa_len,
        family,
        point_role: point_role_for_type(type_id),
        master_send,
        master_receive,
        slave_receive,
        slave_upload,
        point_configurable: measurement_supported || control,
        parser_only,
        value_model,
        quality_model: match descriptor.quality_kind {
            QualityKind::None => QualityModel::None,
            QualityKind::Siq => QualityModel::Siq,
            QualityKind::Diq => QualityModel::Diq,
            QualityKind::Qds => QualityModel::Qds,
            QualityKind::Bcr => QualityModel::Bcr,
            QualityKind::Qdp => QualityModel::Qdp,
        },
        timestamp_model: match descriptor.timestamp_kind {
            TimestampKind::None => TimestampModel::None,
            TimestampKind::Cp24 => TimestampModel::Cp24,
            TimestampKind::Cp56 => TimestampModel::Cp56,
        },
        cot_lifecycle: lifecycle_for_type(type_id, parser_only),
        simulation: measurement_supported.then(|| simulation_capability(value_model)).flatten(),
    }
}

fn point_role_for_type(type_id: u8) -> PointRole {
    match type_id {
        1..=21 | 30..=40 => PointRole::Monitor,
        45..=51 | 58..=64 => PointRole::Control,
        _ => PointRole::NonPoint,
    }
}

const DISCRETE_WAVEFORMS: &[&str] = &["fixed", "step", "pulse", "square"];
const STEP_POSITION_WAVEFORMS: &[&str] = &["fixed", "step", "saw-up", "saw-down", "triangle", "stair", "random-walk"];
const CONTINUOUS_WAVEFORMS: &[&str] = &[
    "fixed",
    "step",
    "pulse",
    "square",
    "saw-up",
    "saw-down",
    "triangle",
    "sine",
    "ramp-hold-fall",
    "stair",
    "exp-approach",
    "damped-oscillation",
    "random",
    "random-walk",
];
const COUNTER_WAVEFORMS: &[&str] = &["fixed", "counter"];

fn simulation_capability(value_model: ValueModel) -> Option<SimulationCapability> {
    let capability = match value_model {
        ValueModel::Boolean => SimulationCapability {
            allowed_waveforms: DISCRETE_WAVEFORMS,
            default_waveform: "step",
            input_kind: SimulationInputKind::Select,
            min: 0.0,
            max: 1.0,
            step: 1.0,
            precision: 0,
        },
        ValueModel::DoublePoint => SimulationCapability {
            allowed_waveforms: DISCRETE_WAVEFORMS,
            default_waveform: "step",
            input_kind: SimulationInputKind::Select,
            min: 0.0,
            max: 3.0,
            step: 1.0,
            precision: 0,
        },
        ValueModel::StepPosition => SimulationCapability {
            allowed_waveforms: STEP_POSITION_WAVEFORMS,
            default_waveform: "stair",
            input_kind: SimulationInputKind::Integer,
            min: -64.0,
            max: 63.0,
            step: 1.0,
            precision: 0,
        },
        ValueModel::Bitstring32 => SimulationCapability {
            allowed_waveforms: COUNTER_WAVEFORMS,
            default_waveform: "fixed",
            input_kind: SimulationInputKind::Integer,
            min: 0.0,
            max: u32::MAX as f64,
            step: 1.0,
            precision: 0,
        },
        ValueModel::Normalized => SimulationCapability {
            allowed_waveforms: CONTINUOUS_WAVEFORMS,
            default_waveform: "sine",
            input_kind: SimulationInputKind::Float,
            min: -1.0,
            max: 0.9999,
            step: 0.0001,
            precision: 4,
        },
        ValueModel::ScaledI16 => SimulationCapability {
            allowed_waveforms: CONTINUOUS_WAVEFORMS,
            default_waveform: "sine",
            input_kind: SimulationInputKind::Integer,
            min: i16::MIN as f64,
            max: i16::MAX as f64,
            step: 1.0,
            precision: 0,
        },
        ValueModel::Float32 => SimulationCapability {
            allowed_waveforms: CONTINUOUS_WAVEFORMS,
            default_waveform: "sine",
            input_kind: SimulationInputKind::Float,
            min: f32::MIN as f64,
            max: f32::MAX as f64,
            step: 0.1,
            precision: 4,
        },
        ValueModel::Counter => SimulationCapability {
            allowed_waveforms: COUNTER_WAVEFORMS,
            default_waveform: "counter",
            input_kind: SimulationInputKind::Integer,
            min: i32::MIN as f64,
            max: i32::MAX as f64,
            step: 1.0,
            precision: 0,
        },
        ValueModel::None | ValueModel::Structured => return None,
    };
    Some(capability)
}

fn lifecycle_for_type(type_id: u8, parser_only: bool) -> CotLifecycle {
    if parser_only {
        CotLifecycle::ParserOnly
    } else if matches!(type_id, 45..=50 | 58..=63) {
        CotLifecycle::SelectExecute
    } else if matches!(type_id, 51 | 64 | 100 | 101 | 103) {
        CotLifecycle::ActivationTermination
    } else if matches!(type_id, 104 | 105 | 107) {
        CotLifecycle::ActivationConfirmation
    } else if type_id == 102 {
        CotLifecycle::RequestResponse
    } else if matches!(type_id, 120..=127) {
        CotLifecycle::FileSession
    } else {
        CotLifecycle::Report
    }
}
