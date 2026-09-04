//! 点表导入 API 命令模块
//!
//! 提供点表导入功能的 Tauri 命令接口，支持从 XML/CSV/JSON 文件导入点表定义。

use std::{
    collections::{BTreeSet, HashMap},
    future::Future,
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use log::warn;
use tauri::State;

use crate::{
    api::{
        master::shared as master_shared,
        slave::shared as slave_shared,
        types::{
            ApiError, CommandResult, PointTableImportApplyResult, PointTableImportIssue, PointTableImportMode,
            PointTableImportNormalizedPayload, PointTableImportPreview, PointTableImportSummary,
            PointTableImportTarget, PointTableTemplateExportResponse,
        },
    },
    core::{
        iec104_registry::iec104_type_name,
        shared::point_table_import::{build_point_table_csv_template, decode_point_table_import_file},
        types::{PointDef, PointSource},
    },
    db::{DatabaseService, MasterPoint as DbMasterPoint},
    services::StationManager,
};

#[tauri::command]
pub async fn generate_point_table_csv_template(locale: String) -> CommandResult<PointTableTemplateExportResponse> {
    let (file_name, bytes) = build_point_table_csv_template(&locale);
    Ok(PointTableTemplateExportResponse {
        file_name,
        mime_type: "text/csv;charset=utf-8".to_string(),
        data_base64: STANDARD.encode(&bytes),
    })
}

/// 预览点表导入
#[tauri::command]
pub async fn preview_point_table_import(
    file_path: String,
    target: PointTableImportTarget,
    mode: PointTableImportMode,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<PointTableImportPreview> {
    // 验证导入目标是否支持

    // 解析点表文件
    let decoded = match decode_point_table_import_file(&file_path) {
        Ok(value) => value,
        Err(err) => return Err(ApiError::new("POINT_TABLE_IMPORT_FAILED", err)),
    };

    // 查询目标的现有点表数量
    let existing_points_count = match resolve_existing_point_count(&target, db_service.inner()) {
        Ok(value) => value,
        Err((code, message)) => return Err(ApiError::new(code, message)),
    };

    // 构建警告列表（包含解析警告和目标兼容性警告）
    let mut errors = decoded.errors;
    let mut warnings = decoded.warnings;
    errors.extend(validate_source_common_addresses(&target, &decoded.payload, db_service.inner()));
    if mode == PointTableImportMode::AppendOnly {
        let existing_addresses = match resolve_existing_point_addresses(&target, db_service.inner()) {
            Ok(value) => value,
            Err((code, message)) => return Err(ApiError::new(code, message)),
        };
        for def in &decoded.payload.point_defs {
            if existing_addresses.contains(&def.address) {
                errors.push(PointTableImportIssue {
                    code: "CONFLICT_IOA_EXISTING".to_string(),
                    message: format!("IOA={} 已存在，严格追加模式不允许覆盖", def.address),
                    location: Some(format!("IOA={}", def.address)),
                });
            }
        }
        let existing_aliases = match resolve_existing_aliases(&target, db_service.inner()) {
            Ok(value) => value,
            Err((code, message)) => return Err(ApiError::new(code, message)),
        };
        for (type_name, alias) in &decoded.payload.asdu_aliases {
            if existing_aliases.get(type_name).is_some_and(|existing| existing != alias) {
                errors.push(PointTableImportIssue {
                    code: "CONFLICT_ASDU_ALIAS".to_string(),
                    message: format!("ASDU 别名 {type_name} 已存在且值不同"),
                    location: Some(type_name.clone()),
                });
            }
        }
    }
    warnings.extend(build_target_capability_warnings(&target, &decoded.payload));

    // 构建统计摘要
    let summary = PointTableImportSummary {
        total_points: decoded.total_points,
        normalized_points: decoded.payload.point_defs.len(),
        preview_points: decoded.points_preview.len(),
        duplicate_address_count: decoded.duplicate_address_count,
        control_mapping_count: decoded.payload.point_defs.iter().filter(|def| def.control_ioa.is_some()).count(),
        asdu_alias_count: decoded.payload.asdu_aliases.len(),
        existing_points_count,
        error_count: errors.len(),
        warning_count: warnings.len(),
    };

    // 返回预览结果
    Ok(PointTableImportPreview {
        target,
        file_path: decoded.file_path,
        file_name: decoded.file_name,
        file_size: decoded.file_size,
        format: decoded.format,
        encoding: decoded.encoding,
        can_apply: errors.is_empty(),
        summary,
        errors,
        warnings,
        points_preview: decoded.points_preview,
        asdu_aliases_preview: decoded.payload.asdu_aliases.clone(),
        normalized_payload: decoded.payload,
    })
}

/// 应用点表导入
#[tauri::command]
pub async fn apply_point_table_import(
    target: PointTableImportTarget,
    payload: PointTableImportNormalizedPayload,
    mode: PointTableImportMode,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<PointTableImportApplyResult> {
    // 验证导入目标是否支持

    // 根据目标类型清理不支持的特性
    let mut sanitized_payload = sanitize_payload_for_target(&target, payload);
    let incoming_points_count = sanitized_payload.point_defs.len();
    let incoming_alias_count = sanitized_payload.asdu_aliases.len();
    if sanitized_payload.point_defs.is_empty() {
        return Err(ApiError::new("EMPTY_POINT_TABLE", "未解析到可导入的点表定义"));
    }

    if let Some(issue) =
        validate_source_common_addresses(&target, &sanitized_payload, db_service.inner()).into_iter().next()
    {
        return Err(ApiError::new(issue.code, issue.message));
    }

    if mode == PointTableImportMode::AppendOnly {
        let existing_defs = match resolve_existing_point_defs(&target, db_service.inner()) {
            Ok(value) => value,
            Err((code, message)) => return Err(ApiError::new(code, message)),
        };
        let existing_addresses = existing_defs.iter().map(|def| def.address).collect::<BTreeSet<_>>();
        if let Some(conflict) =
            sanitized_payload.point_defs.iter().find(|def| existing_addresses.contains(&def.address))
        {
            return Err(ApiError::new(
                "CONFLICT_IOA_EXISTING",
                format!("IOA={} 已存在，严格追加导入已整体拒绝", conflict.address),
            ));
        }
        let existing_aliases = match resolve_existing_aliases(&target, db_service.inner()) {
            Ok(value) => value,
            Err((code, message)) => return Err(ApiError::new(code, message)),
        };
        for (type_name, alias) in &sanitized_payload.asdu_aliases {
            if existing_aliases.get(type_name).is_some_and(|existing| existing != alias) {
                return Err(ApiError::new(
                    "CONFLICT_ASDU_ALIAS",
                    format!("ASDU 别名 {type_name} 已存在且值不同，严格追加导入已整体拒绝"),
                ));
            }
        }
        let mut merged_defs = existing_defs;
        merged_defs.append(&mut sanitized_payload.point_defs);
        merged_defs.sort_by_key(|def| def.address);
        sanitized_payload.point_defs = merged_defs;
        let mut merged_aliases = existing_aliases;
        merged_aliases.extend(sanitized_payload.asdu_aliases);
        sanitized_payload.asdu_aliases = merged_aliases;
    }

    // 验证点表定义的合法性
    if let Err((code, message)) = normalize_and_validate_import_point_defs(&target, &mut sanitized_payload.point_defs) {
        return Err(ApiError::new(code, message));
    }

    // 根据目标类型执行对应的导入逻辑
    let result = match &target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => {
            apply_master_link_slave_import(*slave_id, &sanitized_payload, db_service.inner()).await
        }
        PointTableImportTarget::SlaveStationSlave { slave_id } => {
            apply_slave_station_slave_import(*slave_id, &sanitized_payload, db_service.inner(), station_manager.inner())
                .await
        }
    };

    // 处理导入结果
    match result {
        Ok((existing_points, imported_points, asdu_aliases_updated, runtime_applied)) => {
            let runtime_applied = if matches!(&target, PointTableImportTarget::MasterLinkSlave { .. }) {
                refresh_master_import_runtime(&target, db_service.inner(), station_manager.inner()).await
            } else {
                runtime_applied
            };
            let applied_point_cursor = if runtime_applied {
                resolve_applied_point_cursor(&target, db_service.inner(), station_manager.inner()).await
            } else {
                None
            };
            Ok(PointTableImportApplyResult {
                target,
                imported_points: if mode == PointTableImportMode::AppendOnly {
                    incoming_points_count
                } else {
                    imported_points
                },
                existing_points,
                asdu_aliases_updated: if mode == PointTableImportMode::AppendOnly {
                    incoming_alias_count
                } else {
                    asdu_aliases_updated
                },
                runtime_applied,
                applied_point_cursor,
            })
        }
        Err((code, message)) => Err(ApiError::new(code, message)),
    }
}

async fn resolve_applied_point_cursor(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
    station_manager: &Arc<StationManager>,
) -> Option<crate::core::types::PointSyncCursor> {
    match target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => {
            let slave = db_service.master_slaves.get_by_id(*slave_id).ok().flatten()?;
            let link = db_service.master_links.get_by_id(slave.connection_id).ok().flatten()?;
            Some(station_manager.get_master_service(&link.station_id.to_string()).await?.point_cursor().await)
        }
        PointTableImportTarget::SlaveStationSlave { slave_id } => {
            let slave = db_service.slave_devices.get_by_id(*slave_id).ok().flatten()?;
            Some(station_manager.get_slave_service(&slave.station_id.to_string()).await?.point_cursor().await)
        }
    }
}

async fn refresh_master_import_runtime(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
    station_manager: &Arc<StationManager>,
) -> bool {
    let station_id = match target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => db_service
            .master_slaves
            .get_by_id(*slave_id)
            .ok()
            .flatten()
            .and_then(|slave| db_service.master_links.get_by_id(slave.connection_id).ok().flatten())
            .map(|profile| profile.station_id),
        _ => None,
    };
    let Some(station_id) = station_id else {
        return false;
    };
    master_shared::refresh_persisted_master_settings(station_id, station_manager, db_service).await
}

/// 查询导入目标的现有点表数量
fn resolve_existing_point_count(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
) -> Result<usize, (&'static str, String)> {
    match target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => {
            let slave = db_service
                .master_slaves
                .get_by_id(*slave_id)
                .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取主站从站失败: {err}")))?;
            let Some(_) = slave else {
                return Err(("SLAVE_NOT_FOUND", "从站不存在".to_string()));
            };
            db_service
                .master_points
                .get_by_connection_slave(*slave_id)
                .map(|rows| rows.len())
                .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取主站点表失败: {err}")))
        }
        PointTableImportTarget::SlaveStationSlave { slave_id } => {
            let slave = db_service
                .slave_devices
                .get_by_id(*slave_id)
                .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")))?;
            let Some(_) = slave else {
                return Err(("SLAVE_NOT_FOUND", "从站不存在".to_string()));
            };
            db_service
                .slave_points
                .get_by_slave_station(*slave_id)
                .map(|rows| rows.len())
                .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取从站点表失败: {err}")))
        }
    }
}

fn resolve_existing_point_addresses(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
) -> Result<BTreeSet<u32>, (&'static str, String)> {
    resolve_existing_point_defs(target, db_service).map(|defs| defs.into_iter().map(|def| def.address).collect())
}

fn resolve_existing_point_defs(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
) -> Result<Vec<PointDef>, (&'static str, String)> {
    let master_def = |address: u32,
                      name: String,
                      type_id: u8,
                      description: Option<String>,
                      default_value: Option<String>,
                      is_enabled: bool| PointDef {
        address,
        name,
        type_id,
        data_type: iec104_type_name(type_id).to_string(),
        description,
        control_ioa: None,
        gi_group: None,
        counter_group: None,
        default_value: default_value.and_then(|raw| serde_json::from_str(&raw).ok()),
        is_enabled,
        source: PointSource::Manual,
    };
    match target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => db_service
            .master_points
            .get_by_connection_slave(*slave_id)
            .map(|rows| {
                rows.into_iter()
                    .map(|row| {
                        master_def(
                            row.address,
                            row.name,
                            row.type_id,
                            row.description,
                            row.default_value,
                            row.is_enabled,
                        )
                    })
                    .collect()
            })
            .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取主站点表失败: {err}"))),
        PointTableImportTarget::SlaveStationSlave { slave_id } => db_service
            .slave_points
            .get_by_slave_station(*slave_id)
            .map(|rows| slave_shared::db_slave_points_to_point_defs(&rows))
            .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取从站点表失败: {err}"))),
    }
}

fn resolve_target_common_address(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
) -> Result<Option<u16>, (&'static str, String)> {
    match target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => db_service
            .master_slaves
            .get_by_id(*slave_id)
            .map(|row| row.map(|slave| slave.common_address))
            .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取主站从站失败: {err}"))),
        PointTableImportTarget::SlaveStationSlave { slave_id } => db_service
            .slave_devices
            .get_by_id(*slave_id)
            .map(|row| row.map(|slave| slave.common_address))
            .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}"))),
    }
}

fn validate_source_common_addresses(
    target: &PointTableImportTarget,
    payload: &PointTableImportNormalizedPayload,
    db_service: &Arc<DatabaseService>,
) -> Vec<PointTableImportIssue> {
    if payload.source_common_addresses.is_empty() {
        return Vec::new();
    }
    let target_common_address = match resolve_target_common_address(target, db_service) {
        Ok(value) => value,
        Err((code, message)) => {
            return vec![PointTableImportIssue { code: code.to_string(), message, location: None }];
        }
    };
    payload
        .source_common_addresses
        .iter()
        .filter(|entry| Some(entry.common_address) != target_common_address)
        .map(|entry| PointTableImportIssue {
            code: "COMMON_ADDRESS_MISMATCH".to_string(),
            message: format!(
                "文件 common_address={} 与导入目标 CA={} 不一致",
                entry.common_address,
                target_common_address.map_or_else(|| "未指定".to_string(), |value| value.to_string())
            ),
            location: Some(entry.location.clone()),
        })
        .collect()
}

fn resolve_existing_aliases(
    target: &PointTableImportTarget,
    db_service: &Arc<DatabaseService>,
) -> Result<HashMap<String, String>, (&'static str, String)> {
    match target {
        PointTableImportTarget::MasterLinkSlave { slave_id } => db_service
            .master_slave_asdu_aliases
            .get_all(*slave_id)
            .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取 ASDU 别名失败: {err}"))),
        PointTableImportTarget::SlaveStationSlave { slave_id } => db_service
            .slave_device_asdu_aliases
            .get_all(*slave_id)
            .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取 ASDU 别名失败: {err}"))),
    }
}

/// 构建目标兼容性警告列表
fn build_target_capability_warnings(
    target: &PointTableImportTarget,
    payload: &PointTableImportNormalizedPayload,
) -> Vec<PointTableImportIssue> {
    let mut warnings = Vec::new();
    let control_mapping_count = payload.point_defs.iter().filter(|def| def.control_ioa.is_some()).count();
    if control_mapping_count > 0 && !target_supports_control_mapping(target) {
        warnings.push(PointTableImportIssue {
            code: "TARGET_UNSUPPORTED_CONTROL_MAPPING".to_string(),
            message: format!("当前导入目标不支持保存 control_ioa 映射，提交时将忽略 {control_mapping_count} 项"),
            location: None,
        });
    }
    warnings
}

fn target_supports_control_mapping(target: &PointTableImportTarget) -> bool {
    matches!(target, PointTableImportTarget::SlaveStationSlave { .. })
}

fn sanitize_payload_for_target(
    target: &PointTableImportTarget,
    payload: PointTableImportNormalizedPayload,
) -> PointTableImportNormalizedPayload {
    let PointTableImportNormalizedPayload { point_defs, asdu_aliases, source_common_addresses } = payload;
    let point_defs = point_defs
        .into_iter()
        .map(|mut def| {
            if !target_supports_control_mapping(target) {
                def.control_ioa = None;
            }
            def
        })
        .collect::<Vec<_>>();

    let asdu_aliases = match target {
        PointTableImportTarget::MasterLinkSlave { .. } => master_shared::normalize_asdu_alias_map(asdu_aliases),
        PointTableImportTarget::SlaveStationSlave { .. } => slave_shared::normalize_asdu_alias_map(asdu_aliases),
    };

    PointTableImportNormalizedPayload { point_defs, asdu_aliases, source_common_addresses }
}

fn normalize_and_validate_import_point_defs(
    target: &PointTableImportTarget,
    point_defs: &mut [crate::api::types::PointDef],
) -> Result<(), (&'static str, String)> {
    match target {
        PointTableImportTarget::MasterLinkSlave { .. } => master_shared::normalize_and_validate_point_defs(point_defs),
        PointTableImportTarget::SlaveStationSlave { .. } => slave_shared::normalize_and_validate_point_defs(point_defs),
    }
}

async fn apply_master_link_slave_import(
    slave_id: i64,
    payload: &PointTableImportNormalizedPayload,
    db_service: &Arc<DatabaseService>,
) -> Result<(usize, usize, usize, bool), (&'static str, String)> {
    let existing = db_service
        .master_slaves
        .get_by_id(slave_id)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取主站从站失败: {err}")))?;
    let Some(_existing) = existing else {
        return Err(("SLAVE_NOT_FOUND", "从站不存在".to_string()));
    };
    let previous_points = db_service
        .master_points
        .get_by_connection_slave(slave_id)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取主站点表失败: {err}")))?;
    let previous_aliases = db_service
        .master_slave_asdu_aliases
        .get_all(slave_id)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取 ASDU 别名失败: {err}")))?;

    let next_points = point_defs_to_connection_slave_points(slave_id, &payload.point_defs);
    let updated = db_service
        .master_points
        .replace_by_connection_slave(slave_id, &next_points)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("保存主站点表失败: {err}")))?;

    if let Err(err) = db_service.master_slave_asdu_aliases.replace_all(slave_id, &payload.asdu_aliases) {
        let _ = db_service.master_points.replace_by_connection_slave(slave_id, &previous_points);
        let _ = db_service.master_slave_asdu_aliases.replace_all(slave_id, &previous_aliases);
        return Err(("DB_TRANSACTION_FAILED", format!("保存 ASDU 别名失败: {err}")));
    }

    Ok((previous_points.len(), updated, payload.asdu_aliases.len(), false))
}

async fn apply_slave_station_slave_import(
    slave_id: i64,
    payload: &PointTableImportNormalizedPayload,
    db_service: &Arc<DatabaseService>,
    station_manager: &Arc<StationManager>,
) -> Result<(usize, usize, usize, bool), (&'static str, String)> {
    let station_slave = db_service
        .slave_devices
        .get_by_id(slave_id)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")))?;
    let Some(station_slave) = station_slave else {
        return Err(("SLAVE_NOT_FOUND", "从站不存在".to_string()));
    };
    let updated = apply_slave_station_slave_import_with_runtime(
        slave_id,
        station_slave.common_address,
        &payload.point_defs,
        &payload.asdu_aliases,
        db_service,
        |defs| async move {
            let station_id = station_slave.station_id.to_string();
            let service = slave_shared::ensure_slave_service(station_manager, &station_id, db_service)
                .await
                .ok_or_else(|| "从站运行态服务不存在".to_string())?;
            service
                .apply_point_defs_for_common_address(station_slave.common_address, &defs)
                .await
                .map_err(|err| format!("同步子从站点表到运行态失败: {err}"))?;
            Ok(())
        },
    )
    .await?;

    Ok((updated.0, updated.1, updated.2, true))
}

async fn apply_slave_station_slave_import_with_runtime<F, Fut>(
    slave_id: i64,
    common_address: u16,
    defs: &[crate::api::types::PointDef],
    asdu_aliases: &HashMap<String, String>,
    db_service: &Arc<DatabaseService>,
    runtime_apply: F,
) -> Result<(usize, usize, usize), (&'static str, String)>
where
    F: Fn(Vec<crate::api::types::PointDef>) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let previous_rows = db_service
        .slave_points
        .get_by_slave_station(slave_id)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取子从站点表失败: {err}")))?;
    let previous_defs = slave_shared::db_slave_points_to_point_defs(&previous_rows);
    let previous_aliases = db_service
        .slave_device_asdu_aliases
        .get_all(slave_id)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("读取 ASDU 别名失败: {err}")))?;
    let next_points = slave_shared::point_defs_to_slave_db_points(slave_id, defs);

    let updated = db_service
        .slave_points
        .replace_by_slave_station(slave_id, &next_points)
        .map_err(|err| ("DB_TRANSACTION_FAILED", format!("保存子从站点表失败: {err}")))?;

    if let Err(err) = db_service.slave_device_asdu_aliases.replace_all(slave_id, asdu_aliases) {
        let _ = db_service.slave_points.replace_by_slave_station(slave_id, &previous_rows);
        let _ = db_service.slave_device_asdu_aliases.replace_all(slave_id, &previous_aliases);
        return Err(("DB_TRANSACTION_FAILED", format!("保存 ASDU 别名失败: {err}")));
    }

    if let Err(runtime_err) = runtime_apply(defs.to_vec()).await {
        let rollback_points = db_service.slave_points.replace_by_slave_station(slave_id, &previous_rows);
        if let Err(rollback_err) = rollback_points {
            return Err((
                "DB_TRANSACTION_FAILED",
                format!("子从站点表导入运行态同步失败且回滚持久化失败: {runtime_err}; rollback={rollback_err}"),
            ));
        }
        let _ = db_service.slave_device_asdu_aliases.replace_all(slave_id, &previous_aliases);
        if let Err(restore_err) = runtime_apply(previous_defs).await {
            warn!(
                "[point-table-import] slave common-address runtime rollback failed ca={} error={}",
                common_address, restore_err
            );
        }
        return Err(("DB_TRANSACTION_FAILED", format!("子从站点表导入失败，持久化已回滚: {runtime_err}")));
    }

    Ok((previous_rows.len(), updated, asdu_aliases.len()))
}

fn point_defs_to_connection_slave_points(slave_id: i64, defs: &[crate::api::types::PointDef]) -> Vec<DbMasterPoint> {
    defs.iter()
        .map(|def| DbMasterPoint {
            id: None,
            connection_slave_id: slave_id,
            address: def.address,
            name: def.name.clone(),
            type_id: def.type_id,
            data_type: master_shared::infer_db_data_type(def.type_id),
            description: def.description.clone(),
            default_value: def.default_value.as_ref().and_then(|value| serde_json::to_string(value).ok()),
            is_enabled: def.is_enabled,
            created_at: None,
            updated_at: None,
        })
        .collect()
}
