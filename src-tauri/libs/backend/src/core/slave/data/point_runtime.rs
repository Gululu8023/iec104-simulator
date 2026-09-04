//! 从站数据点运行时管理模块。
//!
//! 本模块负责数据点运行时状态的更新和持久化，包括：
//!
//! - **运行时值持久化**：将数据点的当前值保存到数据库
//! - **控制点状态管理**：跟踪控制命令的执行状态（选择、执行、终止）
//! - **质量描述符处理**：规范化和应用质量标志位
//! - **点值更新**：更新内存中的数据点映射表

use std::sync::Arc;

use log::warn;

use crate::{
    core::{
        iec104_registry::{iec104_default_point_name, iec104_type_name},
        shared::versioned_point_store::PointStoreTransaction,
        slave::{
            protocol::{
                control_target::ResolvedControlTarget,
                quality::{
                    apply_quality_metadata, normalize_quality_raw_for_type, quality_common_from_raw,
                    quality_detail_from_point,
                },
            },
            simulation::engine::{PointKey, point_key},
        },
        types::{ControlStatusKind, ControlStatusSnapshot, DataPoint, PointSource, is_quality_business_usable},
    },
    db::DatabaseService,
    utils::logger::iec104_log_kv,
};

/// 编码数据点运行时值为 JSON 字符串
fn encode_runtime_point_value(point: &DataPoint) -> Option<String> {
    serde_json::to_string(&point.value).ok()
}

/// 持久化数据点运行时值到数据库
///
/// 将数据点的当前值保存到逻辑从站点表。
pub(crate) async fn persist_runtime_point_values(
    station_id: &str,
    default_common_address: u16,
    db_service: &Arc<DatabaseService>,
    points: &[DataPoint],
) {
    if points.is_empty() {
        return;
    }

    let station_row_id = match station_id.parse::<i64>() {
        Ok(id) => id,
        Err(_) => {
            warn!("[slave] skip runtime value persistence: invalid station_id={}", station_id);
            return;
        }
    };

    let mut updates = Vec::with_capacity(points.len());
    for point in points {
        let common_address = point.common_address.unwrap_or(default_common_address);
        let Some(encoded) = encode_runtime_point_value(point) else {
            warn!(
                "[slave] encode runtime value failed station_id={} ca={} ioa={}",
                station_row_id, common_address, point.address
            );
            continue;
        };
        updates.push((common_address, point.address, encoded));
    }

    if updates.is_empty() {
        return;
    }

    let expected = updates.len();
    let db_service = Arc::clone(db_service);
    match tokio::task::spawn_blocking(move || db_service.slave_points.update_runtime_values(station_row_id, &updates))
        .await
    {
        Ok(Ok(updated)) if updated < expected => warn!(
            "[slave] runtime value persistence skipped missing points station_id={} expected={} updated={}",
            station_row_id, expected, updated
        ),
        Ok(Ok(_)) => {}
        Ok(Err(err)) => warn!("[slave] persist runtime values failed station_id={} error={}", station_row_id, err),
        Err(err) => warn!("[slave] runtime value persistence task failed station_id={} error={}", station_row_id, err),
    }
}

/// 更新控制目标数据点
///
/// 根据控制命令更新目标数据点的值和质量，仅在点已存在时更新。
pub(crate) fn update_control_target_point(
    map: &mut PointStoreTransaction<'_, PointKey, DataPoint>,
    connection_id: &str,
    common_address: u16,
    target: ResolvedControlTarget,
    value: f64,
    quality: u8,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> bool {
    let Some(mapped_target) = target.mapped_monitor_target else {
        return true;
    };
    let key = point_key(common_address, mapped_target.address);
    if !map.contains_key(&key) {
        return false;
    }

    upsert_slave_point(
        map,
        connection_id,
        common_address,
        mapped_target.address,
        mapped_target.type_id,
        value,
        quality,
        timestamp,
    );
    true
}

/// 更新或插入从站控制点状态快照
///
/// 记录控制命令的执行状态（选择、激活确认、终止），用于跟踪控制流程。
pub(crate) fn upsert_slave_control_point_snapshot(
    map: &mut PointStoreTransaction<'_, PointKey, DataPoint>,
    connection_id: &str,
    common_address: u16,
    address: u32,
    type_id: u8,
    state: ControlStatusKind,
    cause: u16,
    command_value: Option<f64>,
    select_execute: Option<bool>,
    command_cp56: Option<&str>,
) {
    let updated_at = chrono::Utc::now();
    let key = point_key(common_address, address);
    if let Some(point) = map.get_mut(&key) {
        let mut snapshot = point.control_status_snapshot.clone().unwrap_or(ControlStatusSnapshot {
            state,
            updated_at,
            latest_cause: None,
            actcon_negative: None,
            actcon_at: None,
            actterm_at: None,
            timeout_type: None,
            select_execute,
            command_cp56: None,
        });
        snapshot.state = state;
        snapshot.updated_at = updated_at;
        snapshot.latest_cause = Some(cause);
        snapshot.timeout_type = None;
        snapshot.select_execute = select_execute;
        snapshot.command_cp56 = command_cp56.map(str::to_owned);
        match cause & 0x3F {
            7 | 9 => {
                snapshot.actcon_negative = Some((cause & 0x40) != 0);
                snapshot.actcon_at = Some(updated_at);
                snapshot.actterm_at = None;
            }
            10 => {
                snapshot.actcon_negative = Some(false);
                snapshot.actterm_at = Some(updated_at);
            }
            44..=47 => {
                snapshot.actcon_negative = Some(true);
                snapshot.actcon_at = Some(updated_at);
                snapshot.actterm_at = None;
            }
            _ => {}
        }
        point.connection_id = connection_id.to_string();
        point.common_address = Some(common_address);
        point.type_id = type_id;
        point.data_type = iec104_type_name(type_id).to_string();
        if let Some(value) = command_value {
            point.value = value;
        }
        point.timestamp = updated_at;
        point.control_status_snapshot = Some(snapshot);
        return;
    }

    let mut snapshot = ControlStatusSnapshot {
        state,
        updated_at,
        latest_cause: Some(cause),
        actcon_negative: None,
        actcon_at: None,
        actterm_at: None,
        timeout_type: None,
        select_execute,
        command_cp56: command_cp56.map(str::to_owned),
    };
    match cause & 0x3F {
        7 | 9 => {
            snapshot.actcon_negative = Some((cause & 0x40) != 0);
            snapshot.actcon_at = Some(updated_at);
        }
        10 => {
            snapshot.actcon_negative = Some(false);
            snapshot.actterm_at = Some(updated_at);
        }
        44..=47 => {
            snapshot.actcon_negative = Some(true);
            snapshot.actcon_at = Some(updated_at);
        }
        _ => {}
    }

    map.insert(key, DataPoint {
        connection_id: connection_id.to_string(),
        link_profile_id: None,
        slave_id: None,
        common_address: Some(common_address),
        address,
        name: iec104_default_point_name(type_id, address),
        description: None,
        control_ioa: None,
        gi_group: None,
        counter_group: None,
        type_id,
        data_type: iec104_type_name(type_id).to_string(),
        value: command_value.unwrap_or_default(),
        quality: 0,
        quality_common: Some(quality_common_from_raw(type_id, 0)),
        quality_detail: Some(quality_detail_from_point(type_id, 0, command_value.unwrap_or_default())),
        business_usable: true,
        timestamp: updated_at,
        latest_event_timestamp: None,
        timestamp_detail: None,
        report_count: None,
        latest_cause: Some(cause),
        point_source: Some(PointSource::Manual),
        control_status_snapshot: Some(snapshot),
    });
}

/// 记录允许的命令类型不匹配警告
///
/// 当控制命令类型与目标点类型不匹配但仍被允许执行时记录警告日志。
pub(crate) fn log_allowed_type_mismatch(
    station_id: &str,
    peer_addr: &str,
    common_address: u16,
    command_type_id: u8,
    target_type_id: u8,
    command_ioa: u32,
) {
    warn!(
        "[slave] command_type_mismatch_allowed {} command_type_id={} target_type_id={} ca={}",
        iec104_log_kv(
            station_id,
            Some(peer_addr),
            Some(command_type_id),
            Some(7),
            Some(command_ioa),
            Some("COMMAND_TYPE_MISMATCH"),
            None,
        ),
        command_type_id,
        target_type_id,
        common_address
    );
}

/// 更新或插入从站数据点
///
/// 更新现有点或创建新点，自动规范化质量描述符并同步质量元数据。
pub(crate) fn upsert_slave_point(
    map: &mut PointStoreTransaction<'_, PointKey, DataPoint>,
    connection_id: &str,
    common_address: u16,
    address: u32,
    type_id: u8,
    value: f64,
    quality: u8,
    timestamp: chrono::DateTime<chrono::Utc>,
) {
    let key = point_key(common_address, address);
    if let Some(point) = map.get_mut(&key) {
        point.connection_id = connection_id.to_string();
        point.common_address = Some(common_address);
        point.type_id = type_id;
        point.data_type = iec104_type_name(type_id).to_string();
        point.value = value;
        apply_quality_metadata(point, quality);
        point.timestamp = timestamp;
        return;
    }

    let raw_quality = normalize_quality_raw_for_type(type_id, quality);
    map.insert(key, DataPoint {
        connection_id: connection_id.to_string(),
        link_profile_id: None,
        slave_id: None,
        common_address: Some(common_address),
        address,
        name: iec104_default_point_name(type_id, address),
        description: None,
        control_ioa: None,
        gi_group: None,
        counter_group: None,
        type_id,
        data_type: iec104_type_name(type_id).to_string(),
        value,
        quality: raw_quality,
        quality_common: Some(quality_common_from_raw(type_id, raw_quality)),
        quality_detail: Some(quality_detail_from_point(type_id, raw_quality, value)),
        business_usable: is_quality_business_usable(type_id, raw_quality),
        timestamp,
        latest_event_timestamp: None,
        timestamp_detail: None,
        report_count: None,
        latest_cause: None,
        point_source: Some(PointSource::Manual),
        control_status_snapshot: None,
    });
}
