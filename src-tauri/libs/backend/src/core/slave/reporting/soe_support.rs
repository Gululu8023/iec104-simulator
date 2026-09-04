//! 从站 SOE 事件支持模块。
//!
//! 本模块提供 SOE(Sequence of Events,事件序列记录)的生成和管理功能,包括:
//!
//! - **类型映射**:将监视点类型映射到对应的带时标 SOE 类型
//!   - M_SP_NA_1(1) → M_SP_TB_1(30) 单点带时标
//!   - M_DP_NA_1(3) → M_DP_TB_1(31) 双点带时标
//!   - M_ME_NA_1(9) → M_ME_TD_1(34) 归一化测量值带时标
//!   - M_ME_NB_1(11) → M_ME_TE_1(35) 标度化测量值带时标
//!   - M_ME_NC_1(13) → M_ME_TF_1(36) 短浮点测量值带时标
//!   - M_IT_NA_1(15) → M_IT_TB_1(37) 累计量带时标
//! - **载荷构建**:生成带 CP56Time2a 时间戳的 ASDU 载荷
//! - **时间偏移**:支持站点级时间偏移配置
//! - **质量处理**:根据类型规范化质量描述符

use iec60870_parser::parser::asdu::encode::encode_measurement_object;

use crate::core::{
    protocol_adapter::{build_clock_sync_payload, encode_cp24_time2a},
    slave::protocol::quality::normalize_quality_raw_for_type,
    types::DataPoint,
};

#[derive(Debug, Clone)]
pub(crate) struct SoeEvent {
    pub(crate) common_address: u16,
    pub(crate) type_id: u8,
    pub(crate) address: u32,
    pub(crate) value: f64,
    pub(crate) quality: u8,
    pub(crate) timestamp: chrono::DateTime<chrono::Utc>,
}

#[inline]
pub(crate) fn map_point_type_to_soe_type(type_id: u8) -> Option<u8> {
    if type_id == 2 {
        return Some(2);
    }
    if matches!(type_id, 1 | 30) {
        return Some(30);
    }
    if type_id == 4 {
        return Some(4);
    }
    if matches!(type_id, 3 | 31) {
        return Some(31);
    }
    if type_id == 6 {
        return Some(6);
    }
    if matches!(type_id, 5 | 32) {
        return Some(32);
    }
    if type_id == 8 {
        return Some(8);
    }
    if matches!(type_id, 7 | 33) {
        return Some(33);
    }
    if type_id == 10 {
        return Some(10);
    }
    if matches!(type_id, 9 | 34) {
        return Some(34);
    }
    if type_id == 12 {
        return Some(12);
    }
    if matches!(type_id, 11 | 35) {
        return Some(35);
    }
    if type_id == 14 {
        return Some(14);
    }
    if matches!(type_id, 13 | 36) {
        return Some(36);
    }
    if type_id == 16 {
        return Some(16);
    }
    if matches!(type_id, 15 | 37) {
        return Some(37);
    }
    None
}

pub(crate) fn build_soe_event_payload(event: &SoeEvent) -> Option<Vec<u8>> {
    let timestamp = if matches!(event.type_id, 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16) {
        encode_cp24_time2a(event.timestamp).to_vec()
    } else {
        build_clock_sync_payload(event.timestamp)
    };
    encode_measurement_object(
        event.type_id,
        event.address,
        event.value,
        normalize_quality_raw_for_type(event.type_id, event.quality),
        false,
        Some(&timestamp),
    )
    .ok()
}

pub(crate) fn build_point_measurement_payload(type_id: u8, point: &DataPoint) -> Option<Vec<u8>> {
    let timestamp = match type_id {
        2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 => Some(encode_cp24_time2a(point.timestamp).to_vec()),
        30..=37 => Some(build_clock_sync_payload(point.timestamp)),
        _ => None,
    };
    let transient = matches!(
        point.quality_detail.as_ref(),
        Some(crate::core::types::DataPointQualityDetail::Qds { transient: Some(true), .. })
    );
    encode_measurement_object(
        type_id,
        point.address,
        point.value,
        normalize_quality_raw_for_type(type_id, point.quality),
        transient,
        timestamp.as_deref(),
    )
    .ok()
}

pub(crate) fn build_point_measurement_payload_with_offset(
    type_id: u8,
    point: &DataPoint,
    station_time_offset_ms: i64,
) -> Option<Vec<u8>> {
    if station_time_offset_ms == 0 || !matches!(type_id, 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 30..=37) {
        return build_point_measurement_payload(type_id, point);
    }

    let mut encoded_point = point.clone();
    encoded_point.timestamp = apply_station_time_offset(point.timestamp, station_time_offset_ms);
    build_point_measurement_payload(type_id, &encoded_point)
}

pub(crate) fn build_counter_measurement_payload(type_id: u8, point: &DataPoint) -> Option<(u8, Vec<u8>)> {
    if let Some(payload) = build_point_measurement_payload(type_id, point) {
        return Some((type_id, payload));
    }
    if type_id != 15 {
        return build_point_measurement_payload(15, point).map(|payload| (15, payload));
    }
    None
}

pub(crate) fn build_counter_measurement_payload_with_offset(
    type_id: u8,
    point: &DataPoint,
    station_time_offset_ms: i64,
) -> Option<(u8, Vec<u8>)> {
    if station_time_offset_ms == 0 {
        return build_counter_measurement_payload(type_id, point);
    }
    if let Some(payload) = build_point_measurement_payload_with_offset(type_id, point, station_time_offset_ms) {
        return Some((type_id, payload));
    }
    if type_id != 15 {
        return build_point_measurement_payload_with_offset(15, point, station_time_offset_ms)
            .map(|payload| (15, payload));
    }
    None
}

#[inline]
pub(crate) fn apply_station_time_offset(
    timestamp: chrono::DateTime<chrono::Utc>,
    offset_ms: i64,
) -> chrono::DateTime<chrono::Utc> {
    timestamp
        .checked_add_signed(chrono::Duration::milliseconds(offset_ms))
        .expect("station time offset must remain in the chrono UTC domain")
}
