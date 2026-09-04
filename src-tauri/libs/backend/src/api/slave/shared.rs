//! 从站 API 共享模块。
//!
//! 提供从站 API 模块内部共享的类型、常量和辅助函数,包括:
//!
//! - **依赖导入**:统一导入常用类型和模块
//! - **冲突映射**:数据库约束冲突的错误码映射
//! - **全局配置键**:命令不匹配策略、IOA 显示格式、站点策略默认值
//! - **辅助函数**:IOA 显示格式规范化、点表模板构建等

pub(crate) use std::{collections::HashMap, sync::Arc};

pub(crate) use log::{debug, error, info, warn};
pub(crate) use tauri::State;

pub(crate) use crate::{
    api::{
        command_helpers::{
            DbConflictMapping, get_slave_service, load_json_user_setting_or_default, map_db_conflict_response,
            slave_station_not_found,
        },
        mappers::{
            build_capture_export_meta, build_capture_export_response, db_station_to_station_config, infer_db_data_type,
        },
        station_factory::{build_station_config, station_config_to_response},
        types::{
            ApiError, CommandResult, CreateStationRequest, ExportCaptureResponse, LinkParams, PointDef,
            RedundancyGroupMode, SlaveResponse, SlaveStationPolicy, SlaveStationPolicyOverride, StationConfigResponse,
            TriggerSimulationScenarioRequest, UpdateDataPointRequest, UpdateInitializationProfileRequest,
            UpdateSimulationProfileRequest, UpdateSlaveStationConfigBundleRequest,
            UpdateSlaveStationConfigBundleResponse, UpsertSlaveBundleRequest, UpsertSlaveBundleResponse,
        },
    },
    core::{
        iec104_registry::{iec104_type_id, iec104_type_name},
        slave::{SimulationProfile, SlaveCommandMismatchPolicy, SlaveService, StartSelectedPointSimulationRequest},
        types::{BackendSoeEvent, ConnectionInfo, PointSource, SlaveMessageRecord, StationType},
    },
    db::{
        DatabaseService, SlaveDevice as DbSlaveDevice, SlavePoint as DbSlavePoint, Station as DbStation,
        StationType as DbStationType,
    },
    services::StationManager,
};

pub(crate) const SLAVE_GLOBAL_COMMAND_MISMATCH_POLICY_KEY: &str = "slave.global.command_mismatch_policy";
pub(crate) const SLAVE_GLOBAL_IOA_DISPLAY_FORMAT_KEY: &str = "slave.global.ioa_display_format";
pub(crate) const SLAVE_GLOBAL_STATION_POLICY_DEFAULTS_KEY: &str = "slave.global.station_policy_defaults";

pub(crate) const SLAVE_STATION_CONFLICTS: [DbConflictMapping; 2] = [
    ("UNIQUE constraint failed: slave_devices", "CONFLICT_COA_DUPLICATE", "同一连接下公共地址重复"),
    ("UNIQUE constraint failed: slave_points", "CONFLICT_IOA_DUPLICATE", "信息对象地址重复，保存失败"),
];

pub(crate) fn normalize_slave_ioa_display_format(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dec" | "decimal" => Some("dec"),
        "hex" | "hexadecimal" => Some("hex"),
        _ => None,
    }
}

pub(crate) fn sanitize_station_policy_defaults(policy: SlaveStationPolicy) -> SlaveStationPolicy {
    SlaveStationPolicy {
        select_timeout_seconds: policy.select_timeout_seconds.clamp(1, 3600),
        enable_sq1_upload: policy.enable_sq1_upload,
        enable_soe: policy.enable_soe,
        control_execution_mode: policy.control_execution_mode,
    }
}

pub(crate) fn sanitize_station_policy_override(policy: SlaveStationPolicyOverride) -> SlaveStationPolicyOverride {
    SlaveStationPolicyOverride {
        select_timeout_seconds: policy.select_timeout_seconds.map(|value| value.clamp(1, 3600)),
        enable_sq1_upload: policy.enable_sq1_upload,
        enable_soe: policy.enable_soe,
        control_execution_mode: policy.control_execution_mode,
    }
}

pub(crate) fn resolve_effective_station_policy(
    global_defaults: SlaveStationPolicy,
    connection_override: Option<LinkParams>,
    station_override: SlaveStationPolicyOverride,
) -> SlaveStationPolicy {
    let connection_defaults = connection_override
        .map(|params| SlaveStationPolicy {
            select_timeout_seconds: params.select_timeout_seconds.clamp(1, 3600),
            enable_sq1_upload: params.enable_sq1_upload,
            enable_soe: params.enable_soe,
            control_execution_mode: global_defaults.control_execution_mode,
        })
        .unwrap_or(global_defaults);

    SlaveStationPolicy {
        select_timeout_seconds: station_override
            .select_timeout_seconds
            .unwrap_or(connection_defaults.select_timeout_seconds)
            .clamp(1, 3600),
        enable_sq1_upload: station_override.enable_sq1_upload.unwrap_or(connection_defaults.enable_sq1_upload),
        enable_soe: station_override.enable_soe.unwrap_or(connection_defaults.enable_soe),
        control_execution_mode: station_override
            .control_execution_mode
            .unwrap_or(connection_defaults.control_execution_mode),
    }
}

pub(crate) fn load_slave_station_policy_defaults(
    db_service: &Arc<DatabaseService>,
) -> Result<SlaveStationPolicy, String> {
    match db_service.app_settings.get(SLAVE_GLOBAL_STATION_POLICY_DEFAULTS_KEY) {
        Ok(Some(raw)) => serde_json::from_str::<SlaveStationPolicy>(&raw)
            .map(sanitize_station_policy_defaults)
            .map_err(|err| format!("解析从站全局策略默认值失败: {err}")),
        Ok(None) => Ok(SlaveStationPolicy::default()),
        Err(err) => Err(format!("读取从站全局策略默认值失败: {err}")),
    }
}

pub(crate) fn load_optional_station_link_params(
    db_service: &Arc<DatabaseService>,
    station_id: i64,
) -> Result<Option<LinkParams>, String> {
    match db_service.slave_station_profiles.get(station_id) {
        Ok(Some(profile)) => profile.link_params.map_or(Ok(None), |raw| {
            serde_json::from_str::<LinkParams>(&raw)
                .map(|value| if value == LinkParams::default() { None } else { Some(value) })
                .map_err(|err| format!("解析链路参数失败: {err}"))
        }),
        Ok(None) => Ok(None),
        Err(err) => Err(format!("读取链路参数失败: {err}")),
    }
}

pub(crate) fn load_slave_station_policy_override(
    db_service: &Arc<DatabaseService>,
    _station_id: i64,
    slave_id: i64,
) -> Result<SlaveStationPolicyOverride, String> {
    match db_service.slave_devices.get_by_id(slave_id) {
        Ok(Some(row)) => row.station_policy_override.map_or(Ok(SlaveStationPolicyOverride::default()), |raw| {
            serde_json::from_str::<SlaveStationPolicyOverride>(&raw)
                .map(sanitize_station_policy_override)
                .map_err(|err| format!("解析从站策略覆盖失败: {err}"))
        }),
        Ok(None) => Ok(SlaveStationPolicyOverride::default()),
        Err(err) => Err(format!("读取从站策略覆盖失败: {err}")),
    }
}

pub(crate) async fn ensure_slave_service(
    station_manager: &Arc<StationManager>,
    station_id: &str,
    db_service: &Arc<DatabaseService>,
) -> Option<Arc<SlaveService>> {
    if let Some(service) = get_slave_service(station_manager, station_id).await {
        return Some(service);
    }

    let row_id: i64 = station_id.parse().ok()?;
    let persisted = db_service.stations.get_by_id(row_id).ok().flatten()?;
    let default_common_address = db_service
        .slave_devices
        .get_by_station(row_id)
        .ok()?
        .into_iter()
        .min_by_key(|row| row.id.unwrap_or(i64::MAX))
        .map(|row| row.common_address)
        .unwrap_or(1);
    let config = db_station_to_station_config(&persisted, StationType::Slave, default_common_address).ok()?;
    let station_id = station_manager.create_station(config).await.ok()?;

    let service = get_slave_service(station_manager, &station_id).await?;

    load_persisted_slave_settings(&station_id, &service, db_service).await;

    Some(service)
}

pub(crate) async fn load_persisted_slave_settings(
    station_id: &str,
    service: &Arc<SlaveService>,
    db_service: &Arc<DatabaseService>,
) {
    // 0) global station policy defaults
    match load_slave_station_policy_defaults(db_service) {
        Ok(defaults) => service.update_station_policy_defaults(defaults).await,
        Err(err) => warn!("{err} station_id={station_id}"),
    }

    // 0) global command mismatch policy
    if let Ok(Some(raw)) = db_service.app_settings.get(SLAVE_GLOBAL_COMMAND_MISMATCH_POLICY_KEY) {
        match serde_json::from_str::<SlaveCommandMismatchPolicy>(&raw) {
            Ok(policy) => service.set_command_mismatch_policy(policy).await,
            Err(err) => warn!("解析命令错配策略失败 key={} error={}", SLAVE_GLOBAL_COMMAND_MISMATCH_POLICY_KEY, err),
        }
    }

    let station_profile = station_id
        .parse::<i64>()
        .ok()
        .and_then(|row_id| db_service.slave_station_profiles.get(row_id).ok().flatten())
        .unwrap_or_default();

    // 1) link params (t1/t2/k/w...)
    if let Some(raw) = station_profile.link_params.as_deref() {
        match serde_json::from_str::<LinkParams>(&raw) {
            Ok(params) => {
                let use_connection_override = params != LinkParams::default();
                service.update_link_params(params).await;
                service.set_station_policy_connection_override_enabled(use_connection_override).await;
            }
            Err(err) => warn!("解析链路参数失败 station_id={} error={}", station_id, err),
        }
    } else {
        service.set_station_policy_connection_override_enabled(false).await;
    }

    // 2) point defs (DB is the source of truth)
    if let Ok(row_id) = station_id.parse::<i64>() {
        service.clear_station_policy_overrides().await;
        match db_service.slave_devices.get_by_station(row_id) {
            Ok(slave_rows) if !slave_rows.is_empty() => {
                for slave_row in slave_rows {
                    if !slave_row.enabled {
                        continue;
                    }
                    let Some(slave_id) = slave_row.id else {
                        continue;
                    };

                    match load_slave_station_policy_override(db_service, row_id, slave_id) {
                        Ok(policy_override) if !policy_override.is_empty() => {
                            service.upsert_station_policy_override(slave_row.common_address, policy_override).await;
                        }
                        Ok(_) => {}
                        Err(err) => {
                            warn!("读取从站策略覆盖失败 station_id={} slave_id={} error={}", station_id, slave_id, err)
                        }
                    }

                    let defs = match db_service.slave_points.get_by_slave_station(slave_id) {
                        Ok(rows) => db_slave_points_to_point_defs(&rows),
                        Err(err) => {
                            warn!("读取从站点表失败 station_id={} slave_id={} error={}", station_id, slave_id, err);
                            continue;
                        }
                    };

                    if let Err(err) = service.apply_point_defs_for_common_address(slave_row.common_address, &defs).await
                    {
                        warn!(
                            "加载从站点表失败 station_id={} slave_id={} common_address={} error={}",
                            station_id, slave_id, slave_row.common_address, err
                        );
                    }
                }
            }
            Ok(_) => {}
            Err(err) => warn!("读取从站定义失败 station_id={} error={}", station_id, err),
        }
    }

    // 3) initialization profile (used only when no point defs exist)
    if let Some(raw) = station_profile.initialization_profile.as_deref() {
        if let Ok(profile) = serde_json::from_str(&raw) {
            let _ = service.update_initialization_profile(profile).await;
        }
    }

    // 4) simulation profile (may start simulation)
    if let Some(raw) = station_profile.simulation_profile.as_deref() {
        if let Ok(profile) = serde_json::from_str(&raw) {
            let _ = service.update_simulation_profile(profile).await;
        }
    }
}

pub(crate) fn normalize_asdu_alias_map(raw: HashMap<String, String>) -> HashMap<String, String> {
    let mut normalized = HashMap::new();
    for (type_name, alias) in raw {
        let normalized_type_name = String::from(type_name.trim());
        let Some(type_id) = iec104_type_id(&normalized_type_name) else {
            continue;
        };
        let canonical_type_name = iec104_type_name(type_id).to_string();
        let normalized_alias = String::from(alias.trim());
        if normalized_alias.is_empty() {
            continue;
        }
        normalized.insert(canonical_type_name, normalized_alias);
    }
    normalized
}

pub(crate) async fn apply_slave_defs_to_runtime(
    station_manager: &Arc<StationManager>,
    station_id: i64,
    common_address: u16,
    defs: &[PointDef],
    db_service: &Arc<DatabaseService>,
) {
    let station_id = station_id.to_string();
    if let Some(service) = ensure_slave_service(station_manager, &station_id, db_service).await {
        if let Err(err) = service.apply_point_defs_for_common_address(common_address, defs).await {
            warn!("同步从站点表到运行时失败 station_id={} common_address={} error={}", station_id, common_address, err);
        }
    }
}

pub(crate) async fn apply_slave_station_policy_override_to_runtime(
    station_manager: &Arc<StationManager>,
    station_id: i64,
    common_address: u16,
    policy_override: SlaveStationPolicyOverride,
    db_service: &Arc<DatabaseService>,
) {
    let station_id = station_id.to_string();
    if let Some(service) = ensure_slave_service(station_manager, &station_id, db_service).await {
        if policy_override.is_empty() {
            service.remove_station_policy_override(common_address).await;
        } else {
            service.upsert_station_policy_override(common_address, policy_override).await;
        }
    }
}

pub(crate) async fn remove_slave_station_policy_override_from_runtime(
    station_manager: &Arc<StationManager>,
    station_id: i64,
    common_address: u16,
    db_service: &Arc<DatabaseService>,
) {
    let station_id = station_id.to_string();
    if let Some(service) = ensure_slave_service(station_manager, &station_id, db_service).await {
        service.remove_station_policy_override(common_address).await;
    }
}

pub(crate) async fn remove_slave_runtime_points(
    station_manager: &Arc<StationManager>,
    station_id: i64,
    common_address: u16,
    db_service: &Arc<DatabaseService>,
) {
    let station_id = station_id.to_string();
    if let Some(service) = ensure_slave_service(station_manager, &station_id, db_service).await {
        service.remove_common_address_points(common_address).await;
    }
}

pub(crate) fn point_defs_to_slave_db_points(slave_station_id: i64, defs: &[PointDef]) -> Vec<DbSlavePoint> {
    defs.iter()
        .map(|def| DbSlavePoint {
            id: None,
            slave_station_id,
            address: def.address,
            name: def.name.clone(),
            type_id: def.type_id,
            data_type: infer_db_data_type(def.type_id),
            unit: None,
            description: def.description.clone(),
            min_value: None,
            max_value: None,
            default_value: def.default_value.as_ref().and_then(|value| serde_json::to_string(value).ok()),
            value: def.default_value.as_ref().and_then(|value| serde_json::to_string(value).ok()),
            control_ioa: def.control_ioa,
            gi_group: def.gi_group,
            counter_group: def.counter_group,
            is_enabled: def.is_enabled,
            created_at: None,
            updated_at: None,
        })
        .collect()
}

pub(crate) fn db_slave_points_to_point_defs(rows: &[DbSlavePoint]) -> Vec<PointDef> {
    rows.iter()
        .map(|row| PointDef {
            address: row.address,
            name: row.name.clone(),
            type_id: row.type_id,
            data_type: iec104_type_name(row.type_id).to_string(),
            description: row.description.clone(),
            control_ioa: row.control_ioa,
            gi_group: row.gi_group,
            counter_group: row.counter_group,
            default_value: row
                .value
                .as_ref()
                .or(row.default_value.as_ref())
                .and_then(|raw| serde_json::from_str(raw).ok()),
            is_enabled: row.is_enabled,
            source: PointSource::Manual,
        })
        .collect()
}

pub(crate) fn db_slave_to_response(
    row: &DbSlaveDevice,
    station_policy_override: SlaveStationPolicyOverride,
    effective_station_policy: SlaveStationPolicy,
) -> Option<SlaveResponse> {
    let id = row.id?;
    Some(SlaveResponse {
        id,
        station_id: row.station_id,
        name: row.name.clone(),
        common_address: row.common_address,
        enabled: row.enabled,
        station_policy_override,
        effective_station_policy,
    })
}

pub(crate) fn normalize_and_validate_point_defs(point_defs: &mut [PointDef]) -> Result<(), (&'static str, String)> {
    crate::core::shared::point_validation::normalize_and_validate_point_defs(point_defs)
}
