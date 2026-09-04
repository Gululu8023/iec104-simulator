//! IEC104 wire 模型与模拟器领域模型之间的适配层。
//!
//! 本模块为主站和从站服务提供统一的 ASDU（应用服务数据单元）编解码功能，
//! 包括命令构建、数据点解析、时间戳编码等核心功能。
//!
//! # 主要功能
//!
//! - **ASDU 编码**：将高层命令转换为 IEC104 协议字节流
//! - **ASDU 解码**：将协议字节流解析为运行时数据结构
//! - **命令验证**：验证命令参数的合法性（IOA、限定词、值范围等）
//! - **数据点映射**：将测量值映射为数据点缓存
//! - **时间戳处理**：CP56Time2a 格式的编解码
//! - **类型转换**：TypeID 与类型名称的双向映射

use std::collections::HashMap;

use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc, Weekday};
use iec60870_parser::parser::asdu::{
    commands::CommandQualifier,
    encode::{
        encode_ack_section, encode_asdu as encode_wire_asdu, encode_bitstring_command, encode_clock_sync_command,
        encode_directory, encode_double_command, encode_file_ready, encode_ioa_qualifier, encode_last_section,
        encode_query_log, encode_regulating_step_command, encode_section_ready, encode_segment, encode_select_call,
        encode_setpoint_f32, encode_setpoint_i16, encode_single_command, encode_test_command,
        encode_timed_test_command, file_transfer_segment_payload_max as wire_file_transfer_segment_payload_max,
        file_transfer_single_section_max_file_size as wire_file_transfer_single_section_max_file_size,
    },
    qualifiers::{
        CounterInterrogationQualifier, InterrogationQualifier, ResetProcessQualifier, SetpointQualifier, parse_qcc,
        parse_qoi,
    },
    timestamp::{CP24Time2a, CP56Time2a},
};

use crate::{
    core::{
        iec104_registry::{iec104_default_point_name, iec104_type_name},
        types::{
            AsduInfo, DataPoint, DataPointQualityCommon, DataPointQualityDetail, DataPointTimestampDetail,
            is_quality_business_usable, sync_data_point_business_usable,
        },
    },
    errors::{Iec104ErrorCode, Iec104ValidationError},
    network::{
        DecodedAsdu, DecodedAsduBody, DecodedCommand, DecodedCommandDetail, DecodedFileTransferRecord,
        DecodedMeasurementQualityDetail, DecodedMeasurementTimestamp, DecodedMeasurementValue,
    },
};

/// IEC104 信息对象地址（IOA）最大值（24 位，0xFFFFFF）
const IEC104_IOA_MAX: u32 = 0x00FF_FFFF;
pub fn file_transfer_segment_payload_max(max_asdu_bytes: u16) -> Option<usize> {
    wire_file_transfer_segment_payload_max(max_asdu_bytes)
}

pub fn file_transfer_single_section_max_file_size(max_asdu_bytes: u16) -> Option<u32> {
    wire_file_transfer_single_section_max_file_size(max_asdu_bytes)
}

/// 设点命令值类型。
///
/// 支持三种设点命令格式：归一化、标度化、短浮点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SetPointCommandValue {
    /// 归一化值（-32768 到 +32767，映射到 -1.0 到 +1.0）
    Normalized(i16),
    /// 标度化值（整数测量值）
    Scaled(i16),
    /// 短浮点值（IEEE 754 单精度）
    Float(f32),
}

impl SetPointCommandValue {
    /// 将设点值转换为 f64 浮点数。
    ///
    /// Normalized 值除以 32768.0 映射到 [-1.0, 1.0] 范围，Scaled 和 Float 直接转换。
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Normalized(v) => v as f64 / 32768.0,
            Self::Scaled(v) => v as f64,
            Self::Float(v) => v as f64,
        }
    }
}

/// 命令传送原因（COT）动作类型。
///
/// 区分命令的激活和撤销操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCotAction {
    /// 激活（COT=6）：执行命令
    Activation,
    /// 撤销（COT=8）：取消命令（用于选择-执行模式的撤销）
    Deactivation,
}

/// 从站侧统一命令事件。
///
/// 从解析后的 ASDU 中提取的命令事件，用于从站侧处理。
/// 包含所有 IEC104 支持的命令类型。
#[derive(Debug, Clone)]
pub enum SlaveCommandEvent {
    /// 总召唤命令（C_IC_NA_1, TI=100）
    GeneralInterrogation {
        /// 信息对象地址（通常为 0）
        ioa: u32,
        /// 召唤限定词（QOI）
        qualifier: InterrogationQualifier,
    },
    /// 电能召唤命令（C_CI_NA_1, TI=101）
    CounterInterrogation {
        /// 信息对象地址（通常为 0）
        ioa: u32,
        /// 计数器召唤限定词（QCC）
        qualifier: CounterInterrogationQualifier,
    },
    /// 时钟同步命令（C_CS_NA_1, TI=103）
    ClockSynchronization {
        /// 信息对象地址（通常为 0）
        ioa: u32,
        /// 时间戳（CP56Time2a 格式）
        timestamp: DateTime<Utc>,
    },
    /// 测试命令（C_TS_NA_1/C_TS_TA_1, TI=104/107）
    TestCommand {
        /// 类型标识（104 或 107）
        type_id: u8,
        /// 信息对象地址
        ioa: u32,
        /// 测试模式字（2 字节）
        pattern: u16,
        /// Type 107 请求携带的原始 CP56Time2a。
        timestamp: Option<[u8; 7]>,
    },
    /// 复位进程命令（C_RP_NA_1, TI=105）
    ResetProcess {
        /// 信息对象地址
        ioa: u32,
        /// 复位进程限定词（QRP）
        qualifier: ResetProcessQualifier,
    },
    /// 单点控制命令（C_SC_NA_1/C_SC_TA_1, TI=45/58）
    SingleControl {
        /// 类型标识（45 或 58）
        type_id: u8,
        /// 信息对象地址
        ioa: u32,
        /// 控制值（true=合/ON, false=分/OFF）
        value: bool,
        /// 选择标志（true=选择, false=执行）
        select: bool,
        /// 命令动作（激活/撤销）
        action: CommandCotAction,
        /// 命令限定词（SCO）
        qualifier: CommandQualifier,
        timestamp: Option<[u8; 7]>,
    },
    /// 双点控制命令（C_DC_NA_1/C_DC_TA_1, TI=46/59）
    DoubleControl {
        /// 类型标识（46 或 59）
        type_id: u8,
        /// 信息对象地址
        ioa: u32,
        /// 控制值（1=分/OFF, 2=合/ON）
        value: u8,
        /// 选择标志（true=选择, false=执行）
        select: bool,
        /// 命令动作（激活/撤销）
        action: CommandCotAction,
        /// 命令限定词（DCO）
        qualifier: CommandQualifier,
        timestamp: Option<[u8; 7]>,
    },
    /// 升降命令（C_RC_NA_1/C_RC_TA_1, TI=47/60）
    RegulatingStep {
        /// 类型标识（47 或 60）
        type_id: u8,
        /// 信息对象地址
        ioa: u32,
        /// 步进值（1=降低, 2=升高）
        step: u8,
        /// 选择标志（true=选择, false=执行）
        select: bool,
        /// 命令动作（激活/撤销）
        action: CommandCotAction,
        /// 命令限定词（RCO）
        qualifier: CommandQualifier,
        timestamp: Option<[u8; 7]>,
    },
    /// 设点命令（C_SE_NA_1/C_SE_NB_1/C_SE_NC_1/C_SE_TA_1/C_SE_TB_1/C_SE_TC_1, TI=48-50/61-63）
    SetPoint {
        /// 类型标识（48-50 或 61-63）
        type_id: u8,
        /// 信息对象地址
        ioa: u32,
        /// 设点值（归一化/标度化/短浮点）
        value: SetPointCommandValue,
        /// 命令动作（激活/撤销）
        action: CommandCotAction,
        /// 设点限定词（QOS）
        qualifier: SetpointQualifier,
        timestamp: Option<[u8; 7]>,
    },
    /// 比特串命令（C_BO_NA_1/C_BO_TA_1, TI=51/64）
    BitStringCommand {
        /// 类型标识（51 或 64）
        type_id: u8,
        /// 信息对象地址
        ioa: u32,
        /// 比特串值（32 位）
        value: u32,
        timestamp: Option<[u8; 7]>,
    },
    /// 读命令（C_RD_NA_1, TI=102）
    ReadCommand {
        /// 信息对象地址
        ioa: u32,
    },
}

/// 将 ASDU 信息编码为协议字节流。
///
/// 编码后的字节流包含 TypeID、VSQ、COT、CA 和数据域。
pub fn encode_asdu(asdu: &AsduInfo) -> Vec<u8> {
    encode_asdu_with_vsq(asdu, 1, false)
}

/// 将 ASDU 信息编码为协议字节流（指定信息对象数量）。
pub fn encode_asdu_with_count(asdu: &AsduInfo, object_count: u8) -> Vec<u8> {
    encode_asdu_with_vsq(asdu, object_count, false)
}

/// 将 ASDU 信息编码为协议字节流（完整 VSQ 参数）。
///
/// 编码格式：TypeID(1B) + VSQ(1B) + COT(2B) + CA(2B) + 数据域。
/// VSQ 字段：bit0-6 为信息对象数量（0-127），bit7 为顺序标志。
pub fn encode_asdu_with_vsq(asdu: &AsduInfo, object_count: u8, sequence: bool) -> Vec<u8> {
    encode_wire_asdu(asdu.type_id, asdu.cause, asdu.common_address, &asdu.data, object_count, sequence)
}

/// 构建总召唤命令载荷（IOA=0 + QOI）。
pub fn build_general_interrogation_payload(qualifier: u8) -> Vec<u8> {
    build_ioa_qualifier_payload(0, qualifier)
}

/// 构建电能召唤命令载荷（IOA=0 + QCC）。
pub fn build_counter_interrogation_payload(qualifier: u8) -> Vec<u8> {
    build_ioa_qualifier_payload(0, qualifier)
}

/// 构建单点控制命令载荷（IOA + SCO）。
///
/// SCO 字段：bit0=控制值，bit2-6=QOC.QU，bit7=S/E。
pub fn build_single_control_payload(ioa: u32, value: bool, select: bool, qu: u8) -> Vec<u8> {
    encode_single_command(ioa, value, select, qu)
}

/// 构建双点控制命令载荷（IOA + DCO）。
///
/// DCO 字段：bit0-1=控制值，bit2-6=QOC.QU，bit7=S/E。
pub fn build_double_control_payload(ioa: u32, value: u8, select: bool, qu: u8) -> Vec<u8> {
    encode_double_command(ioa, value, select, qu)
}

/// 构建升降命令载荷（IOA + RCO）。
///
/// RCO 字段：bit0-1=步进值，bit2-6=QOC.QU，bit7=S/E。
pub fn build_regulating_step_payload(ioa: u32, step: u8, select: bool, qu: u8) -> Vec<u8> {
    encode_regulating_step_command(ioa, step, select, qu)
}

/// 构建归一化设点命令载荷（IOA + i16 + QOS）。
pub fn build_setpoint_normalized_payload(ioa: u32, value: i16, qualifier: u8) -> Vec<u8> {
    encode_setpoint_i16(ioa, value, qualifier)
}

/// 构建标度化设点命令载荷（IOA + i16 + QOS）。
pub fn build_setpoint_scaled_payload(ioa: u32, value: i16, qualifier: u8) -> Vec<u8> {
    encode_setpoint_i16(ioa, value, qualifier)
}

/// 构建短浮点设点命令载荷（IOA + f32 + QOS）。
pub fn build_setpoint_float_payload(ioa: u32, value: f32, qualifier: u8) -> Vec<u8> {
    encode_setpoint_f32(ioa, value, qualifier)
}

/// 构建时钟同步命令载荷（CP56Time2a）。
pub fn build_clock_sync_payload(timestamp: DateTime<Utc>) -> Vec<u8> {
    encode_cp56_time2a(timestamp).to_vec()
}

/// 构建时钟同步命令信息对象（IOA + CP56Time2a）。
pub fn build_clock_sync_command_payload(ioa: u32, timestamp: DateTime<Utc>) -> Vec<u8> {
    encode_clock_sync_command(ioa, encode_cp56_time2a(timestamp))
}

/// 构建测试命令载荷 TI=104（IOA + 测试模式字）。
pub fn build_test_command_payload(ioa: u32, pattern: u16) -> Vec<u8> {
    encode_test_command(ioa, pattern)
}

/// 构建带时标测试命令载荷 TI=107（IOA + 测试模式字 + CP56Time2a）。
pub fn build_test_command_ta_payload(ioa: u32, pattern: u16, timestamp: DateTime<Utc>) -> Vec<u8> {
    encode_timed_test_command(ioa, pattern, encode_cp56_time2a(timestamp))
}

/// 构建复位进程命令载荷（IOA + QRP）。
pub fn build_reset_process_payload(ioa: u32, qualifier: u8) -> Vec<u8> {
    build_ioa_qualifier_payload(ioa, qualifier)
}

/// 构建 IOA + 限定词载荷（通用格式）。
pub fn build_ioa_qualifier_payload(ioa: u32, qualifier: u8) -> Vec<u8> {
    encode_ioa_qualifier(ioa, qualifier)
}

/// 构建比特串命令载荷（IOA + BSI）。
pub fn build_bitstring_command_payload(ioa: u32, value: u32) -> Vec<u8> {
    encode_bitstring_command(ioa, value)
}

/// 构建文件就绪载荷 TI=120（IOA + NOF + FRQ + LOF）。
pub fn build_file_ready_payload(ioa: u32, file_name: u16, qualifier: u8, length: u32) -> Vec<u8> {
    encode_file_ready(ioa, file_name, qualifier, length)
}

/// 构建节就绪载荷 TI=121（IOA + NOF + SRQ + NOS + LOF）。
pub fn build_section_ready_payload(ioa: u32, file_name: u16, qualifier: u8, section: u8, length: u32) -> Vec<u8> {
    encode_section_ready(ioa, file_name, qualifier, section, length)
}

/// 构建选择调用载荷 TI=122（IOA + NOF + SCQ + NOS）。
pub fn build_select_call_payload(ioa: u32, file_name: u16, qualifier: u8, section: u8) -> Vec<u8> {
    encode_select_call(ioa, file_name, qualifier, section)
}

/// 构建最后节载荷 TI=123（IOA + NOF + LSQ + 最后节号 + 最后段号）。
pub fn build_last_section_payload(
    ioa: u32,
    file_name: u16,
    qualifier: u8,
    last_section_number: u8,
    last_segment_number: u8,
) -> Vec<u8> {
    encode_last_section(ioa, file_name, qualifier, last_section_number, last_segment_number)
}

/// 构建节确认载荷 TI=124（IOA + NOF + AFQ + NOS）。
pub fn build_ack_section_payload(ioa: u32, file_name: u16, qualifier: u8, section: u8) -> Vec<u8> {
    encode_ack_section(ioa, file_name, qualifier, section)
}

/// 构建段载荷 TI=125（IOA + NOF + NOS + LOS + 数据）。
pub fn build_segment_payload(ioa: u32, file_name: u16, section: u8, data: &[u8]) -> Vec<u8> {
    encode_segment(ioa, file_name, section, data)
}

/// 构建目录载荷 TI=126（IOA + NOF + LOF + SOF + CP56Time2a）。
pub fn build_directory_payload(ioa: u32, file_name: u16, length: u32, status: u8, timestamp: DateTime<Utc>) -> Vec<u8> {
    encode_directory(ioa, file_name, length, status, encode_cp56_time2a(timestamp))
}

/// 构建日志查询载荷 TI=127（IOA + NOF + 起始时间 + 结束时间）。
pub fn build_query_log_payload(
    ioa: u32,
    file_name: u16,
    range_start_time: &[u8; 7],
    range_end_time: &[u8; 7],
) -> Vec<u8> {
    encode_query_log(ioa, file_name, range_start_time, range_end_time)
}

/// 从解码的 ASDU 中提取首个信息对象地址。
pub fn first_ioa_from_decoded_asdu(decoded_asdu: &DecodedAsdu) -> Option<u32> {
    match &decoded_asdu.body {
        DecodedAsduBody::Measurements(items) => items.first().map(|x| x.ioa),
        DecodedAsduBody::Commands(items) => items.first().map(|x| x.ioa),
        DecodedAsduBody::Interrogations(items) => items.first().map(|x| x.ioa),
        DecodedAsduBody::FileTransfers(items) => items.first().map(file_transfer_record_ioa),
        DecodedAsduBody::Raw => None,
    }
}

fn file_transfer_record_ioa(record: &DecodedFileTransferRecord) -> u32 {
    match record {
        DecodedFileTransferRecord::FileReady { ioa, .. }
        | DecodedFileTransferRecord::SectionReady { ioa, .. }
        | DecodedFileTransferRecord::SelectCall { ioa, .. }
        | DecodedFileTransferRecord::LastSection { ioa, .. }
        | DecodedFileTransferRecord::AckSection { ioa, .. }
        | DecodedFileTransferRecord::Segment { ioa, .. }
        | DecodedFileTransferRecord::Directory { ioa, .. }
        | DecodedFileTransferRecord::QueryLog { ioa, .. } => *ioa,
        DecodedFileTransferRecord::Raw(_) => 0,
    }
}

/// 将解析器输出的测量值解码为主站侧数据点缓存。
pub fn decode_data_points(
    connection_id: &str,
    link_profile_id: Option<i64>,
    slave_id: Option<i64>,
    common_address: Option<u16>,
    decoded_asdu: &DecodedAsdu,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Vec<DataPoint> {
    let DecodedAsduBody::Measurements(items) = &decoded_asdu.body else {
        return Vec::new();
    };

    let mut points = Vec::with_capacity(items.len());
    for item in items {
        let (point_timestamp, timestamp_detail) = resolve_measurement_timestamp(item.timestamp.as_ref(), timestamp);
        let (name, value) = match item.value {
            DecodedMeasurementValue::Single(v) => (format!("SP_{}", item.ioa), if v { 1.0 } else { 0.0 }),
            DecodedMeasurementValue::Double(v) => (format!("DP_{}", item.ioa), v as f64),
            DecodedMeasurementValue::StepPosition { value, .. } => (format!("ST_{}", item.ioa), value as f64),
            DecodedMeasurementValue::BitString(v) => (format!("BS_{}", item.ioa), v as f64),
            DecodedMeasurementValue::Normalized(v) => (format!("NV_{}", item.ioa), v as f64 / 32768.0),
            DecodedMeasurementValue::Scaled(v) => (format!("SV_{}", item.ioa), v as f64),
            DecodedMeasurementValue::Float(v) => (format!("SF_{}", item.ioa), v as f64),
            DecodedMeasurementValue::Integrated(v) => (format!("IT_{}", item.ioa), v as f64),
            DecodedMeasurementValue::PackedStatus { status, .. } => (format!("PS_{}", item.ioa), status as f64),
            DecodedMeasurementValue::Raw => continue,
        };

        points.push(DataPoint {
            connection_id: connection_id.to_string(),
            link_profile_id,
            slave_id,
            common_address,
            address: item.ioa,
            name,
            description: None,
            control_ioa: None,
            gi_group: None,
            counter_group: None,
            type_id: decoded_asdu.type_id,
            data_type: iec104_type_name(decoded_asdu.type_id).to_string(),
            value,
            quality: item.quality,
            quality_common: Some(DataPointQualityCommon {
                iv: item.quality_common.iv,
                nt: item.quality_common.nt,
                sb: item.quality_common.sb,
                bl: item.quality_common.bl,
            }),
            quality_detail: Some(map_quality_detail(&item.quality_detail)),
            business_usable: is_quality_business_usable(decoded_asdu.type_id, item.quality),
            timestamp: point_timestamp,
            latest_event_timestamp: None,
            timestamp_detail,
            report_count: None,
            latest_cause: None,
            point_source: None,
            control_status_snapshot: None,
        });
    }

    points
}

pub(crate) fn resolve_measurement_timestamp(
    detail: Option<&DecodedMeasurementTimestamp>,
    received_at: DateTime<Utc>,
) -> (DateTime<Utc>, Option<DataPointTimestampDetail>) {
    match detail {
        Some(DecodedMeasurementTimestamp::Cp56 {
            raw,
            milliseconds,
            minutes,
            hours,
            day,
            month,
            year,
            weekday,
            summer_time,
            invalid,
        }) => {
            let resolved = if *invalid {
                None
            } else {
                NaiveDate::from_ymd_opt(i32::from(*year), u32::from(*month), u32::from(*day))
                    .and_then(|date| {
                        date.and_hms_milli_opt(
                            u32::from(*hours),
                            u32::from(*minutes),
                            u32::from(*milliseconds / 1000),
                            u32::from(*milliseconds % 1000),
                        )
                    })
                    .map(|value| value.and_utc())
            };
            (
                resolved.unwrap_or(received_at),
                Some(DataPointTimestampDetail::Cp56 {
                    raw: raw.clone(),
                    invalid: *invalid,
                    summer_time: *summer_time,
                    weekday: *weekday,
                    resolved,
                }),
            )
        }
        Some(DecodedMeasurementTimestamp::Cp24 { raw, milliseconds, minutes, invalid }) => {
            let resolved = if *invalid {
                None
            } else {
                received_at
                    .date_naive()
                    .and_hms_milli_opt(
                        received_at.hour(),
                        u32::from(*minutes),
                        u32::from(*milliseconds / 1000),
                        u32::from(*milliseconds % 1000),
                    )
                    .map(|base| {
                        let base = base.and_utc();
                        [base - chrono::Duration::hours(1), base, base + chrono::Duration::hours(1)]
                            .into_iter()
                            .min_by_key(|candidate| {
                                (candidate.timestamp_millis() - received_at.timestamp_millis()).abs()
                            })
                            .expect("CP24 candidate set is non-empty")
                    })
            };
            (
                resolved.unwrap_or(received_at),
                Some(DataPointTimestampDetail::Cp24 { raw: raw.clone(), invalid: *invalid, resolved }),
            )
        }
        None => (received_at, None),
    }
}

pub(crate) fn map_quality_detail(detail: &DecodedMeasurementQualityDetail) -> DataPointQualityDetail {
    match detail {
        DecodedMeasurementQualityDetail::Siq { raw, spi, iv, nt, sb, bl } => {
            DataPointQualityDetail::Siq { raw: *raw, spi: *spi, iv: *iv, nt: *nt, sb: *sb, bl: *bl }
        }
        DecodedMeasurementQualityDetail::Diq { raw, dpi, iv, nt, sb, bl } => {
            DataPointQualityDetail::Diq { raw: *raw, dpi: *dpi, iv: *iv, nt: *nt, sb: *sb, bl: *bl }
        }
        DecodedMeasurementQualityDetail::Qds { raw, ov, iv, nt, sb, bl, transient, scd_status, scd_change } => {
            DataPointQualityDetail::Qds {
                raw: *raw,
                ov: *ov,
                iv: *iv,
                nt: *nt,
                sb: *sb,
                bl: *bl,
                transient: *transient,
                scd_status: *scd_status,
                scd_change: *scd_change,
            }
        }
        DecodedMeasurementQualityDetail::Bcr { raw, sq, cy, ca, iv } => {
            DataPointQualityDetail::Bcr { raw: *raw, sq: *sq, cy: *cy, ca: *ca, iv: *iv }
        }
        DecodedMeasurementQualityDetail::None => DataPointQualityDetail::None,
    }
}

/// 从解析的 ASDU 中解码主站命令意图。
pub fn decode_slave_command(decoded_asdu: &DecodedAsdu) -> Result<Vec<SlaveCommandEvent>, Iec104ValidationError> {
    let cause_parts = parse_cause(decoded_asdu.cause)?;
    let action = command_action_from_cot(cause_parts.cot);

    // If P/N bit is set (negative confirmation), don't process the command.
    if cause_parts.negative {
        return Ok(Vec::new());
    }

    match &decoded_asdu.body {
        DecodedAsduBody::Interrogations(items) => decode_interrogations(decoded_asdu.type_id, items),
        DecodedAsduBody::Commands(items) => decode_commands(decoded_asdu.type_id, items, action),
        _ => Err(Iec104ValidationError::new(
            Iec104ErrorCode::UnsupportedCommand,
            format!("unsupported command type_id={}", decoded_asdu.type_id),
        )),
    }
}

/// 使用统一规则在 map 中插入或更新数据点。
pub fn upsert_data_point(
    map: &mut HashMap<u32, DataPoint>,
    connection_id: &str,
    address: u32,
    type_id: u8,
    value: f64,
    quality: u8,
    timestamp: chrono::DateTime<chrono::Utc>,
) {
    if let Some(point) = map.get_mut(&address) {
        point.type_id = type_id;
        point.data_type = iec104_type_name(type_id).to_string();
        point.value = value;
        point.quality = quality;
        sync_data_point_business_usable(point);
        point.timestamp = timestamp;
        return;
    }

    map.insert(address, DataPoint {
        connection_id: connection_id.to_string(),
        link_profile_id: None,
        slave_id: None,
        common_address: None,
        address,
        name: iec104_default_point_name(type_id, address),
        description: None,
        control_ioa: None,
        gi_group: None,
        counter_group: None,
        type_id,
        data_type: iec104_type_name(type_id).to_string(),
        value,
        quality,
        quality_common: Some(DataPointQualityCommon::default()),
        quality_detail: Some(DataPointQualityDetail::None),
        business_usable: is_quality_business_usable(type_id, quality),
        timestamp,
        latest_event_timestamp: None,
        timestamp_detail: None,
        report_count: None,
        latest_cause: None,
        point_source: None,
        control_status_snapshot: None,
    });
}

/// Parsed parts of a COT (Cause of Transmission) u16 field.
///
/// Layout (as returned by `Cot::as_u16()`):
///   low byte  = reason(bit0-5) | P/N(bit6) | T(bit7)
///   high byte = origin address
#[derive(Debug, Clone, Copy)]
pub struct CauseParts {
    /// 6-bit cause of transmission reason (0-63)
    pub cot: u8,
    /// P/N bit: true = negative confirm (command rejected)
    pub negative: bool,
    /// T bit: true = test mode
    pub test: bool,
    /// Originator address
    pub origin: u8,
}

/// Parse the raw COT u16 into structured parts, validating that the
/// cause is a valid command-direction COT (activation=6 or deactivation=8).
fn parse_cause(cause: u16) -> Result<CauseParts, Iec104ValidationError> {
    let low = (cause & 0xFF) as u8;
    let cot = low & 0x3F;
    let negative = (low & 0x40) != 0;
    let test = (low & 0x80) != 0;
    let origin = ((cause >> 8) & 0xFF) as u8;

    if matches!(cot, 6 | 8) {
        return Ok(CauseParts { cot, negative, test, origin });
    }

    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidValue,
        format!("unsupported command cause of transmission: {cot}"),
    ))
}

fn decode_interrogations(
    type_id: u8,
    items: &[crate::network::DecodedInterrogation],
) -> Result<Vec<SlaveCommandEvent>, Iec104ValidationError> {
    let mut events = Vec::with_capacity(items.len());
    for item in items {
        validate_ioa(item.ioa)?;
        match type_id {
            100 => {
                let qualifier = parse_qoi(item.qualifier);
                validate_general_interrogation_qualifier(qualifier)?;
                events.push(SlaveCommandEvent::GeneralInterrogation { ioa: item.ioa, qualifier });
            }
            101 => {
                let qualifier = parse_qcc(item.qualifier);
                validate_counter_interrogation_qualifier(qualifier)?;
                events.push(SlaveCommandEvent::CounterInterrogation { ioa: item.ioa, qualifier });
            }
            _ => {
                return Err(Iec104ValidationError::new(
                    Iec104ErrorCode::UnsupportedCommand,
                    format!("unsupported interrogation type_id={type_id}"),
                ));
            }
        }
    }
    Ok(events)
}

fn decode_commands(
    type_id: u8,
    items: &[DecodedCommand],
    action: CommandCotAction,
) -> Result<Vec<SlaveCommandEvent>, Iec104ValidationError> {
    let mut events = Vec::with_capacity(items.len());

    for cmd in items {
        validate_ioa(cmd.ioa)?;
        let timestamp = decode_control_timestamp(type_id, cmd.timestamp.as_deref())?;
        let event = match &cmd.detail {
            DecodedCommandDetail::Single { value, select } => {
                validate_single_qualifier(cmd.qualifier)?;
                SlaveCommandEvent::SingleControl {
                    type_id,
                    ioa: cmd.ioa,
                    value: *value,
                    select: *select,
                    action,
                    qualifier: cmd.qualifier,
                    timestamp,
                }
            }
            DecodedCommandDetail::Double { value, select } => {
                validate_double_value(*value)?;
                validate_double_qualifier(cmd.qualifier)?;
                SlaveCommandEvent::DoubleControl {
                    type_id,
                    ioa: cmd.ioa,
                    value: *value,
                    select: *select,
                    action,
                    qualifier: cmd.qualifier,
                    timestamp,
                }
            }
            DecodedCommandDetail::Step { position, select } => {
                validate_step_value(*position)?;
                validate_step_qualifier(cmd.qualifier)?;
                SlaveCommandEvent::RegulatingStep {
                    type_id,
                    ioa: cmd.ioa,
                    step: *position,
                    select: *select,
                    action,
                    qualifier: cmd.qualifier,
                    timestamp,
                }
            }
            DecodedCommandDetail::SetPointFloat(v) => {
                let qualifier = validate_setpoint_qualifier(cmd.qualifier)?;
                if !v.is_finite() {
                    return Err(Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        "setpoint float must be finite",
                    ));
                }
                SlaveCommandEvent::SetPoint {
                    type_id,
                    ioa: cmd.ioa,
                    value: SetPointCommandValue::Float(*v),
                    action,
                    qualifier,
                    timestamp,
                }
            }
            DecodedCommandDetail::SetPointNormalized(v) => {
                let qualifier = validate_setpoint_qualifier(cmd.qualifier)?;
                SlaveCommandEvent::SetPoint {
                    type_id,
                    ioa: cmd.ioa,
                    value: SetPointCommandValue::Normalized(*v),
                    action,
                    qualifier,
                    timestamp,
                }
            }
            DecodedCommandDetail::SetPointScaled(v) => {
                let qualifier = validate_setpoint_qualifier(cmd.qualifier)?;
                SlaveCommandEvent::SetPoint {
                    type_id,
                    ioa: cmd.ioa,
                    value: SetPointCommandValue::Scaled(*v),
                    action,
                    qualifier,
                    timestamp,
                }
            }
            DecodedCommandDetail::ClockSynchronization(timestamp) => SlaveCommandEvent::ClockSynchronization {
                ioa: cmd.ioa,
                timestamp: cp56_time2a_to_datetime(*timestamp)?,
            },
            DecodedCommandDetail::ResetProcess => {
                let qualifier = validate_reset_process_qualifier(cmd.qualifier)?;
                SlaveCommandEvent::ResetProcess { ioa: cmd.ioa, qualifier }
            }
            DecodedCommandDetail::TestPattern(pattern) => {
                if *pattern != iec60870_parser::parser::asdu::spec::DLT_TEST_FBP {
                    return Err(Iec104ValidationError::new(
                        Iec104ErrorCode::InvalidValue,
                        format!(
                            "test command pattern must be 0x{:04X}",
                            iec60870_parser::parser::asdu::spec::DLT_TEST_FBP
                        ),
                    ));
                }
                let timestamp = if type_id == 107 {
                    let raw = cmd.timestamp.as_deref().ok_or_else(|| {
                        Iec104ValidationError::new(Iec104ErrorCode::InvalidValue, "type 107 requires CP56Time2a")
                    })?;
                    let bytes: [u8; 7] = raw.try_into().map_err(|_| {
                        Iec104ValidationError::new(
                            Iec104ErrorCode::InvalidValue,
                            format!("type 107 CP56Time2a length must be 7, got {}", raw.len()),
                        )
                    })?;
                    decode_cp56_time2a(&bytes)?;
                    Some(bytes)
                } else {
                    None
                };
                SlaveCommandEvent::TestCommand { type_id, ioa: cmd.ioa, pattern: *pattern, timestamp }
            }
            DecodedCommandDetail::Raw(_) => {
                return Err(Iec104ValidationError::new(
                    Iec104ErrorCode::UnsupportedCommand,
                    format!("unsupported command type_id={type_id}"),
                ));
            }
            DecodedCommandDetail::BitString(val) => {
                SlaveCommandEvent::BitStringCommand { type_id, ioa: cmd.ioa, value: *val, timestamp }
            }
            DecodedCommandDetail::Read => SlaveCommandEvent::ReadCommand { ioa: cmd.ioa },
        };
        events.push(event);
    }

    Ok(events)
}

fn decode_control_timestamp(type_id: u8, raw: Option<&[u8]>) -> Result<Option<[u8; 7]>, Iec104ValidationError> {
    if matches!(type_id, 58..=64) {
        let raw = raw.ok_or_else(|| {
            Iec104ValidationError::new(Iec104ErrorCode::InvalidValue, format!("type {type_id} requires CP56Time2a"))
        })?;
        let bytes: [u8; 7] = raw.try_into().map_err(|_| {
            Iec104ValidationError::new(
                Iec104ErrorCode::InvalidValue,
                format!("type {type_id} CP56Time2a length must be 7, got {}", raw.len()),
            )
        })?;
        let parsed = iec60870_parser::parser::asdu::timestamp::CP56Time2a::parse(&bytes).map_err(|error| {
            Iec104ValidationError::new(
                Iec104ErrorCode::InvalidValue,
                format!("invalid type {type_id} CP56Time2a: {error}"),
            )
        })?;
        if parsed.invalid {
            return Err(Iec104ValidationError::new(
                Iec104ErrorCode::InvalidValue,
                format!("type {type_id} CP56Time2a is marked invalid"),
            ));
        }
        return Ok(Some(bytes));
    }
    if matches!(type_id, 45..=51) && raw.is_some() {
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidValue,
            format!("type {type_id} must not contain CP56Time2a"),
        ));
    }
    Ok(None)
}

fn command_action_from_cot(cot: u8) -> CommandCotAction {
    if cot == 8 { CommandCotAction::Deactivation } else { CommandCotAction::Activation }
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

fn validate_general_interrogation_qualifier(qualifier: InterrogationQualifier) -> Result<(), Iec104ValidationError> {
    if qualifier.is_valid() {
        return Ok(());
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("QOI {} is invalid, expected 20..=36", qualifier.as_u8()),
    ))
}

fn validate_counter_interrogation_qualifier(
    qualifier: CounterInterrogationQualifier,
) -> Result<(), Iec104ValidationError> {
    if qualifier.is_valid_request() {
        return Ok(());
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("QCC request group {} is invalid, expected 1..=5", qualifier.request),
    ))
}

fn validate_single_qualifier(qualifier: CommandQualifier) -> Result<(), Iec104ValidationError> {
    if let CommandQualifier::Sco { raw, .. } = qualifier {
        let qu = (raw >> 2) & 0x1F;
        if qu <= 3 {
            return Ok(());
        }
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("SCO QOC.QU {qu} is invalid, expected 0..=3"),
        ));
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("SCO qualifier type mismatch, got {:#010b}", qualifier.as_u8()),
    ))
}

fn validate_double_value(value: u8) -> Result<(), Iec104ValidationError> {
    if matches!(value, 1 | 2) {
        return Ok(());
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidValue,
        format!("DCO state {value} is invalid, expected 1(off) or 2(on)"),
    ))
}

fn validate_double_qualifier(qualifier: CommandQualifier) -> Result<(), Iec104ValidationError> {
    if let CommandQualifier::Dco { raw, .. } = qualifier {
        let qu = (raw >> 2) & 0x1F;
        if qu <= 3 {
            return Ok(());
        }
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("DCO QOC.QU {qu} is invalid, expected 0..=3"),
        ));
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("DCO qualifier type mismatch, got {:#010b}", qualifier.as_u8()),
    ))
}

fn validate_step_qualifier(qualifier: CommandQualifier) -> Result<(), Iec104ValidationError> {
    // RCO: bit0-1=RCS, bit2-6=QOC.QU, bit7=S/E
    if let CommandQualifier::Rco { raw, .. } = qualifier {
        let qu = (raw >> 2) & 0x1F;
        if qu <= 3 {
            return Ok(());
        }
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("RCO QOC.QU {qu} is invalid, expected 0..=3"),
        ));
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("RCO qualifier type mismatch, got {:#010b}", qualifier.as_u8()),
    ))
}

fn validate_step_value(value: u8) -> Result<(), Iec104ValidationError> {
    if matches!(value, 1 | 2) {
        return Ok(());
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidValue,
        format!("RCO state {value} is invalid, expected 1(decrease) or 2(increase)"),
    ))
}

fn validate_setpoint_qualifier(qualifier: CommandQualifier) -> Result<SetpointQualifier, Iec104ValidationError> {
    if let CommandQualifier::Qos(qos) = qualifier {
        return Ok(qos);
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("QOS qualifier type mismatch, got {:#010b}", qualifier.as_u8()),
    ))
}

fn validate_reset_process_qualifier(
    qualifier: CommandQualifier,
) -> Result<ResetProcessQualifier, Iec104ValidationError> {
    if let CommandQualifier::Qrp(qrp) = qualifier {
        if qrp.is_valid() {
            return Ok(qrp);
        }
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidQualifier,
            format!("QRP {} is invalid, expected 1 or 2", qrp.as_u8()),
        ));
    }
    Err(Iec104ValidationError::new(
        Iec104ErrorCode::InvalidQualifier,
        format!("QRP qualifier type mismatch, got {:#010b}", qualifier.as_u8()),
    ))
}

pub fn encode_cp56_time2a(timestamp: DateTime<Utc>) -> [u8; 7] {
    assert!((2000..=2099).contains(&timestamp.year()), "CP56Time2a year must be in 2000..=2099");
    CP56Time2a {
        milliseconds: (timestamp.second() * 1000 + timestamp.timestamp_subsec_millis()) as u16,
        minutes: timestamp.minute() as u8,
        hours: timestamp.hour() as u8,
        day: timestamp.day() as u8,
        month: timestamp.month() as u8,
        year: timestamp.year() as u16,
        weekday: weekday_to_u8(timestamp.weekday()),
        summer_time: false,
        invalid: false,
    }
    .to_bytes()
}

pub(crate) fn encode_cp24_time2a(timestamp: DateTime<Utc>) -> [u8; 3] {
    CP24Time2a {
        milliseconds: (timestamp.second() * 1000 + timestamp.timestamp_subsec_millis()) as u16,
        minutes: timestamp.minute() as u8,
        invalid: false,
    }
    .to_bytes()
}

fn decode_cp56_time2a(raw: &[u8]) -> Result<DateTime<Utc>, Iec104ValidationError> {
    if raw.len() != 7 {
        return Err(Iec104ValidationError::new(
            Iec104ErrorCode::InvalidValue,
            format!("CP56Time2a length must be 7, got {}", raw.len()),
        ));
    }

    let parsed = CP56Time2a::parse(raw)
        .map_err(|err| Iec104ValidationError::new(Iec104ErrorCode::InvalidValue, err.to_string()))?;
    cp56_time2a_to_datetime(parsed)
}

fn cp56_time2a_to_datetime(parsed: CP56Time2a) -> Result<DateTime<Utc>, Iec104ValidationError> {
    let second = u32::from(parsed.milliseconds / 1000);
    let millisecond = u32::from(parsed.milliseconds % 1000);
    let minute = u32::from(parsed.minutes);
    let hour = u32::from(parsed.hours);
    let day = u32::from(parsed.day);
    let month = u32::from(parsed.month);
    let year = i32::from(parsed.year);

    let date = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
        Iec104ValidationError::new(
            Iec104ErrorCode::InvalidValue,
            format!("invalid CP56 date: year={year} month={month} day={day}"),
        )
    })?;

    let naive = date.and_hms_milli_opt(hour, minute, second, millisecond).ok_or_else(|| {
        Iec104ValidationError::new(
            Iec104ErrorCode::InvalidValue,
            format!("invalid CP56 time: hour={hour} minute={minute} second={second} ms={millisecond}"),
        )
    })?;

    Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

fn weekday_to_u8(weekday: Weekday) -> u8 {
    match weekday {
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
        Weekday::Sun => 7,
    }
}
