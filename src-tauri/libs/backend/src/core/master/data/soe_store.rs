//! SOE（Sequence of Events）事件存储
//!
//! 本模块负责管理主站接收到的 SOE 事件，包括事件提取、时标解析和事件存储。
//!
//! # 核心功能
//!
//! - **CP56Time2a 解析**：解析 IEC104 标准的 7 字节时标格式
//! - **SOE 事件提取**：从带时标的 ASDU 中提取 SOE 事件
//! - **事件时标应用**：将 SOE 事件的时标应用到数据点
//!
//! # SOE 事件
//!
//! SOE（Sequence of Events）是 SCADA 系统中的重要概念，用于记录带精确时标的状态变化事件。
//! 在 IEC104 协议中，SOE 事件通过带时标的测量值类型标识传输：
//!
//! - **M_SP_TB_1 (30)**：带时标的单点信息
//! - **M_DP_TB_1 (31)**：带时标的双点信息
//! - **M_ST_TB_1 (32)**：带时标的步位置信息
//! - **M_ME_TD_1 (34)**：带时标的归一化测量值
//! - **M_ME_TE_1 (35)**：带时标的标度化测量值
//! - **M_ME_TF_1 (36)**：带时标的短浮点数测量值
//! - **M_IT_TB_1 (37)**：带时标的累计量
//!
//! # CP56Time2a 格式
//!
//! CP56Time2a 是 IEC104 标准定义的 7 字节时标格式：
//! - Byte 0-1：毫秒（0-59999）
//! - Byte 2：分钟（0-59，bit 0-5）
//! - Byte 3：小时（0-23，bit 0-4）
//! - Byte 4：日（1-31，bit 0-4）
//! - Byte 5：月（1-12，bit 0-3）
//! - Byte 6：年（0-99，表示 2000-2099，bit 0-6）
//!
//! # 传输原因过滤
//!
//! 只有传输原因（Cause of Transmission）为 3（突发/自发）的 ASDU 才会被提取为 SOE 事件。
//! 这确保只记录真正的状态变化事件，而不是周期性上报或总召唤响应。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use log::warn;

use crate::{
    core::{
        protocol_adapter::map_quality_detail,
        types::{BackendSoeEvent, DataPoint, DataPointQualityCommon, is_quality_business_usable},
    },
    network::{DecodedAsdu, DecodedAsduBody, DecodedMeasurementValue},
};

/// 主站 SOE 测量值
///
/// 封装了从 ASDU 中提取的 SOE 事件信息。
#[derive(Debug, Clone)]
pub(crate) struct MasterSoeMeasurement {
    /// SOE 事件详情
    pub(crate) event: BackendSoeEvent,
}

/// 解码 CP56Time2a 时标
///
/// 将 IEC104 标准的 7 字节时标格式解析为 UTC 时间。
///
/// # 参数
///
/// * `raw` - 7 字节的原始时标数据
///
/// # 返回
///
/// - `Some(DateTime<Utc>)` - 解析成功，返回 UTC 时间
/// - `None` - 解析失败（数据长度不足或时间无效）
///
/// # CP56Time2a 格式
///
/// ```text
/// Byte 0-1: 毫秒 (0-59999, 小端序)
///   - ms = raw[0] | (raw[1] << 8)
///   - second = ms / 1000
///   - millisecond = ms % 1000
/// Byte 2: 分钟 (bit 0-5, 0-59)
/// Byte 3: 小时 (bit 0-4, 0-23)
/// Byte 4: 日 (bit 0-4, 1-31)
/// Byte 5: 月 (bit 0-3, 1-12)
/// Byte 6: 年 (bit 0-6, 0-99, 表示 2000-2099)
/// ```
///
/// # 示例
///
/// ```
/// let raw = [0xFA, 0x23, 0x04, 0x0F, 0x0A, 0x03, 0x1A];
/// // 0x23FA = 9210 ms = 9 秒 210 毫秒
/// // 0x04 = 4 分钟
/// // 0x0F = 15 小时
/// // 0x0A = 10 日
/// // 0x03 = 3 月
/// // 0x1A = 26 年 (2026)
/// let timestamp = decode_cp56_time2a(&raw);
/// // 结果: 2026-03-10 15:04:09.210 UTC
/// ```
pub(crate) fn decode_cp56_time2a(raw: &[u8]) -> Option<DateTime<Utc>> {
    let value = iec60870_parser::parser::asdu::timestamp::CP56Time2a::parse(raw).ok()?;
    if value.invalid {
        return None;
    }
    let date = chrono::NaiveDate::from_ymd_opt(i32::from(value.year), u32::from(value.month), u32::from(value.day))?;
    let time = chrono::NaiveTime::from_hms_milli_opt(
        u32::from(value.hours),
        u32::from(value.minutes),
        u32::from(value.milliseconds / 1000),
        u32::from(value.milliseconds % 1000),
    )?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(date.and_time(time), Utc))
}

/// 提取主站 SOE 测量值
///
/// 从接收到的 ASDU 中提取 SOE 事件。
/// 只处理带时标的测量值类型（30-37）且传输原因为 3（突发/自发）的 ASDU。
///
/// # 参数
///
/// * `station_id` - 主站 ID
/// * `connection_id` - 连接 ID
/// * `slave_id` - 从站 ID（可选）
/// * `decoded_asdu` - 已解码的 ASDU
/// * `raw_frame` - 原始帧数据（用于提取时标）
///
/// # 返回
///
/// 返回提取的 SOE 事件列表。如果不满足条件或解析失败，返回空列表。
///
/// # 过滤条件
///
/// 1. **传输原因**：必须为 3（突发/自发）
/// 2. **类型标识**：必须为带时标的测量值类型（30, 31, 32, 34, 35, 36, 37）
/// 3. **ASDU 体**：必须为 Measurements 类型
/// 4. **测量值数量**：至少包含一个测量值
///
/// # 支持的类型标识
///
/// - **30 (M_SP_TB_1)**：带时标的单点信息
/// - **31 (M_DP_TB_1)**：带时标的双点信息
/// - **32 (M_ST_TB_1)**：带时标的步位置信息
/// - **34 (M_ME_TD_1)**：带时标的归一化测量值
/// - **35 (M_ME_TE_1)**：带时标的标度化测量值
/// - **36 (M_ME_TF_1)**：带时标的短浮点数测量值
/// - **37 (M_IT_TB_1)**：带时标的累计量
pub(crate) fn extract_master_soe_measurements(
    station_id: &str,
    connection_id: &str,
    slave_id: Option<i64>,
    decoded_asdu: &DecodedAsdu,
    _raw_frame: &[u8],
) -> Vec<MasterSoeMeasurement> {
    // 检查传输原因（只处理突发/自发事件）
    let cause_low = (decoded_asdu.cause & 0x3F) as u8;
    if cause_low != 3 {
        return Vec::new();
    }

    // 检查类型标识（只处理带时标的测量值）
    if !matches!(decoded_asdu.type_id, 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 30..=37) {
        return Vec::new();
    }

    // 检查 ASDU 体类型
    let DecodedAsduBody::Measurements(items) = &decoded_asdu.body else {
        return Vec::new();
    };
    if items.is_empty() {
        return Vec::new();
    }

    let mut events = Vec::with_capacity(items.len());
    let received_at = Utc::now();
    // 遍历每个测量值
    for item in items {
        let (event_timestamp, timestamp_detail) =
            crate::core::protocol_adapter::resolve_measurement_timestamp(item.timestamp.as_ref(), received_at);
        if timestamp_detail.as_ref().is_none_or(|detail| {
            matches!(
                detail,
                crate::core::types::DataPointTimestampDetail::Cp24 { invalid: true, .. }
                    | crate::core::types::DataPointTimestampDetail::Cp56 { invalid: true, .. }
            )
        }) {
            warn!(
                "[master] soe_timestamp_invalid station_id={} connection_id={} type_id={} ioa={}",
                station_id, connection_id, decoded_asdu.type_id, item.ioa
            );
        }

        // 构建 SOE 事件
        events.push(MasterSoeMeasurement {
            event: BackendSoeEvent {
                id: 0,                 // 数据库 ID，稍后由数据库分配
                logged_at: Utc::now(), // 记录时间
                event_timestamp,       // 事件时标（从 ASDU 中提取）
                connection_id: connection_id.to_string(),
                slave_id,
                point_name: None, // 稍后填充
                common_address: decoded_asdu.common_address,
                ioa: item.ioa,
                type_id: decoded_asdu.type_id,
                cause: decoded_asdu.cause,
                value: decoded_measurement_value_to_f64(&item.value),
                quality: item.quality,
                quality_common: Some(DataPointQualityCommon {
                    iv: item.quality_common.iv,
                    nt: item.quality_common.nt,
                    sb: item.quality_common.sb,
                    bl: item.quality_common.bl,
                }),
                quality_detail: Some(map_quality_detail(&item.quality_detail)),
                timestamp_detail,
                business_usable: is_quality_business_usable(decoded_asdu.type_id, item.quality),
            },
        });
    }
    events
}

/// 应用最新事件时标到数据点
///
/// 将 SOE 事件的时标应用到对应的数据点，更新 latest_event_timestamp 字段。
/// 用于在数据点中记录最后一次状态变化的时间。
///
/// # 参数
///
/// * `points` - 数据点列表（可变引用）
/// * `events` - SOE 事件列表
///
/// # 执行流程
///
/// 1. 如果数据点列表或事件列表为空，直接返回
/// 2. 构建 IOA 到事件时标的映射表
/// 3. 遍历每个数据点，根据 IOA 查找对应的事件时标
/// 4. 如果找到匹配的事件，更新数据点的 latest_event_timestamp 字段
pub(crate) fn apply_latest_event_timestamp_to_points(points: &mut [DataPoint], events: &[MasterSoeMeasurement]) {
    // 空列表直接返回
    if points.is_empty() || events.is_empty() {
        return;
    }

    // 构建 IOA 到事件时标的映射表
    let timestamp_by_ioa: HashMap<u32, DateTime<Utc>> =
        events.iter().map(|item| (item.event.ioa, item.event.event_timestamp)).collect();
    // 遍历数据点，更新事件时标
    for point in points {
        point.latest_event_timestamp = timestamp_by_ioa.get(&point.address).copied();
    }
}

/// 将解码的测量值转换为 f64
///
/// 内部辅助函数，将各种类型的测量值统一转换为 f64 浮点数。
/// 用于在 SOE 事件中存储统一格式的值。
///
/// # 参数
///
/// * `value` - 解码的测量值
///
/// # 返回
///
/// 返回转换后的 f64 值。
///
/// # 转换规则
///
/// - **Single**：布尔值，true → 1.0，false → 0.0
/// - **Double**：双点值，直接转换为 f64
/// - **StepPosition**：步位置值，提取 value 字段
/// - **BitString**：位串值，直接转换为 f64
/// - **Normalized**：归一化值，除以 32768.0 转换为 [-1.0, 1.0] 范围
/// - **Scaled**：标度化值，直接转换为 f64
/// - **Float**：浮点值，直接转换为 f64
/// - **Integrated**：累计量，直接转换为 f64
/// - **PackedStatus**：打包状态，直接转换为 f64
/// - **Raw**：原始值，返回 0.0
fn decoded_measurement_value_to_f64(value: &DecodedMeasurementValue) -> f64 {
    match value {
        DecodedMeasurementValue::Single(v) => {
            // 单点：true → 1.0, false → 0.0
            if *v { 1.0 } else { 0.0 }
        }
        DecodedMeasurementValue::Double(v) => *v as f64,
        DecodedMeasurementValue::StepPosition { value, .. } => *value as f64,
        DecodedMeasurementValue::BitString(v) => *v as f64,
        DecodedMeasurementValue::Normalized(v) => *v as f64 / 32768.0, // 归一化到 [-1.0, 1.0]
        DecodedMeasurementValue::Scaled(v) => *v as f64,
        DecodedMeasurementValue::Float(v) => *v as f64,
        DecodedMeasurementValue::Integrated(v) => *v as f64,
        DecodedMeasurementValue::PackedStatus { status, .. } => *status as f64,
        DecodedMeasurementValue::Raw => 0.0,
    }
}
