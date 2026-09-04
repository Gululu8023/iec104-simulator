//! 从站站点管理 API。
//!
//! 提供从站站点配置相关的 Tauri 命令,包括:
//!
//! - **从站管理**:创建从站、更新从站配置
//! - **子从站管理**:创建、更新、删除子从站(Station Slave)
//! - **点表管理**:查询、替换从站点表和子从站点表
//! - **ASDU 别名**:管理子从站 ASDU 类型别名
//! - **点表模板**:支持使用模板批量生成数据点

use super::shared::*;

/// 创建从站
#[tauri::command]
pub async fn create_slave_station(
    request: CreateStationRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<String> {
    info!("创建从站: {}", request.name);

    let config = match build_station_config(request, StationType::Slave) {
        Ok(config) => config,
        Err(msg) => return Err(ApiError::app(msg)),
    };

    let station = DbStation {
        id: None,
        name: config.name.clone(),
        station_type: DbStationType::Slave,
        description: None,
        host: config.address.ip().to_string(),
        port: config.address.port(),
        enabled: config.enabled,
        created_at: None,
        updated_at: None,
    };

    let station_row_id = match db_service.stations.create(&station) {
        Ok(station_id) => station_id,
        Err(err) => return Err(ApiError::app(format!("保存站点配置失败: {err}"))),
    };
    let station_id = station_row_id.to_string();
    let config = crate::core::types::StationConfig { id: station_id, ..config };

    match station_manager.create_station(config.clone()).await {
        Ok(station_id) => {
            if let Some(service) = get_slave_service(station_manager.inner(), &station_id).await {
                load_persisted_slave_settings(&station_id, &service, db_service.inner()).await;
            }
            Ok(station_id)
        }
        Err(e) => {
            error!("创建从站失败: {}", e);
            // 运行态创建失败时清理持久化，避免出现“站点已创建但无法使用”的脏状态。
            let _ = db_service.stations.delete(station_row_id);
            Err(ApiError::app(e.to_string()))
        }
    }
}

/// 获取从站配置
#[tauri::command]
pub async fn get_slave_station_config(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<StationConfigResponse> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => Ok(station_config_to_response(service.get_config())),
        None => Err(slave_station_not_found()),
    }
}

/// 更新从站配置（保持站点ID不变）
#[tauri::command]
pub async fn update_slave_station_config(
    station_id: String,
    request: UpdateSlaveStationConfigBundleRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<UpdateSlaveStationConfigBundleResponse> {
    debug!("[slave-settings] update listener config request station_id={}", station_id);
    let mut config = match build_station_config(request.config, StationType::Slave) {
        Ok(config) => config,
        Err(msg) => return Err(ApiError::new("VALIDATION_ERROR", msg)),
    };
    config.id = station_id.clone();

    let station_row_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return Err(ApiError::new("VALIDATION_ERROR", "无效的 station_id"));
        }
    };

    let previous_station = match db_service.stations.get_by_id(station_row_id) {
        Ok(Some(station)) => station,
        Ok(None) => {
            return Err(ApiError::new("STATION_NOT_FOUND", "站点不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取站点配置失败: {err}")));
        }
    };
    let default_device = match db_service.slave_devices.get_by_station(station_row_id) {
        Ok(rows) => rows.into_iter().min_by_key(|row| row.id.unwrap_or(i64::MAX)),
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取默认逻辑从站失败: {err}")));
        }
    };
    let Some(default_device) = default_device else {
        return Err(ApiError::new("SLAVE_NOT_FOUND", "监听端点缺少默认逻辑从站"));
    };
    config.common_address = default_device.common_address;

    let station = DbStation {
        id: Some(station_row_id),
        name: config.name.clone(),
        station_type: DbStationType::Slave,
        description: None,
        host: config.address.ip().to_string(),
        port: config.address.port(),
        enabled: config.enabled,
        created_at: None,
        updated_at: None,
    };

    let updated = match db_service.stations.update(station_row_id, &station) {
        Ok(result) => result,
        Err(err) => {
            error!("保存从站配置失败 station_id={} error={}", station_id, err);
            let message = err.to_string();
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("保存从站配置失败: {message}")));
        }
    };
    if updated == 0 {
        return Err(ApiError::new("STATION_NOT_FOUND", "站点不存在"));
    }

    // 运行态同步：若失败则回滚持久化，避免“半成功”状态。
    let runtime_sync = async {
        station_manager.create_station(config.clone()).await.map_err(|err| format!("预注册从站服务失败: {err}"))?;
        station_manager
            .update_station_config(&station_id, config.clone())
            .await
            .map_err(|err| format!("更新从站运行配置失败: {err}"))?;
        let service = get_slave_service(station_manager.inner(), &station_id)
            .await
            .ok_or_else(|| "更新从站配置失败: 运行态服务不存在".to_string())?;
        load_persisted_slave_settings(&station_id, &service, db_service.inner()).await;
        Ok::<(), String>(())
    }
    .await;
    if let Err(runtime_err) = runtime_sync {
        warn!("[slave-settings] runtime sync failed, start rollback station_id={} error={}", station_id, runtime_err);
        let rollback_result = db_service.stations.update(station_row_id, &previous_station);
        if let Err(rollback_err) = rollback_result {
            error!(
                "[slave-settings] rollback failed station_id={} runtime_error={} rollback_error={}",
                station_id, runtime_err, rollback_err
            );
            return Err(ApiError::new(
                "DB_TRANSACTION_FAILED",
                format!("更新从站运行配置失败且回滚持久化失败: {runtime_err}; rollback={rollback_err}"),
            ));
        }

        if let Ok(previous_config) =
            db_station_to_station_config(&previous_station, StationType::Slave, default_device.common_address)
        {
            if let Err(err) = station_manager.create_station(previous_config.clone()).await {
                warn!(
                    "[slave-settings] runtime rollback create_station failed station_id={} error={}",
                    station_id, err
                );
            } else if let Err(err) = station_manager.update_station_config(&station_id, previous_config.clone()).await {
                warn!(
                    "[slave-settings] runtime rollback update_station_config failed station_id={} error={}",
                    station_id, err
                );
            } else if let Some(service) = get_slave_service(station_manager.inner(), &station_id).await {
                load_persisted_slave_settings(&station_id, &service, db_service.inner()).await;
            }
        }

        return Err(ApiError::new(
            "DB_TRANSACTION_FAILED",
            format!("更新从站运行配置失败，持久化已回滚: {runtime_err}"),
        ));
    }

    Ok(UpdateSlaveStationConfigBundleResponse { station: station_config_to_response(&config), point_defs_updated: 0 })
}

#[tauri::command]
pub async fn create_station_slave(
    station_id: String,
    mut request: UpsertSlaveBundleRequest,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<UpsertSlaveBundleResponse> {
    let station_row_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return Err(ApiError::new("VALIDATION_ERROR", "无效的 station_id"));
        }
    };

    match db_service.stations.get_by_id(station_row_id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(ApiError::new("STATION_NOT_FOUND", "站点不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取站点失败: {err}")));
        }
    }

    if let Err((code, message)) = normalize_and_validate_point_defs(&mut request.point_defs) {
        return Err(ApiError::new(code, message));
    }

    let UpsertSlaveBundleRequest { slave, point_defs } = request;
    let station_policy_override = sanitize_station_policy_override(slave.station_policy_override);
    let station_policy_override_json = if station_policy_override.is_empty() {
        None
    } else {
        match serde_json::to_string(&station_policy_override) {
            Ok(value) => Some(value),
            Err(err) => return Err(ApiError::app(format!("序列化从站策略覆盖失败: {err}"))),
        }
    };
    let station_slave = DbSlaveDevice {
        id: None,
        station_id: station_row_id,
        name: slave.name,
        common_address: slave.common_address,
        station_policy_override: station_policy_override_json,
        enabled: slave.enabled,
        created_at: None,
        updated_at: None,
    };

    let points = point_defs_to_slave_db_points(0, &point_defs);

    match db_service.slave_devices.create_with_points(station_row_id, &station_slave, &points) {
        Ok((slave_id, inserted)) => {
            apply_slave_defs_to_runtime(
                station_manager.inner(),
                station_row_id,
                station_slave.common_address,
                &point_defs,
                db_service.inner(),
            )
            .await;
            apply_slave_station_policy_override_to_runtime(
                station_manager.inner(),
                station_row_id,
                station_slave.common_address,
                station_policy_override,
                db_service.inner(),
            )
            .await;
            Ok(UpsertSlaveBundleResponse { slave_id: slave_id, point_defs_updated: inserted })
        }
        Err(err) => Err(map_db_conflict_response(err.to_string(), "保存从站失败", &SLAVE_STATION_CONFLICTS)),
    }
}

#[tauri::command]
pub async fn update_station_slave(
    slave_id: i64,
    mut request: UpsertSlaveBundleRequest,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<UpsertSlaveBundleResponse> {
    let existing = match db_service.slave_devices.get_by_id(slave_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };

    if let Err((code, message)) = normalize_and_validate_point_defs(&mut request.point_defs) {
        return Err(ApiError::new(code, message));
    }

    let UpsertSlaveBundleRequest { slave, point_defs } = request;
    let station_policy_override = sanitize_station_policy_override(slave.station_policy_override);
    let station_policy_override_json = if station_policy_override.is_empty() {
        None
    } else {
        match serde_json::to_string(&station_policy_override) {
            Ok(value) => Some(value),
            Err(err) => return Err(ApiError::app(format!("序列化从站策略覆盖失败: {err}"))),
        }
    };
    let previous_common_address = existing.common_address;

    let updated = DbSlaveDevice {
        id: existing.id,
        station_id: existing.station_id,
        name: slave.name,
        common_address: slave.common_address,
        station_policy_override: station_policy_override_json,
        enabled: slave.enabled,
        created_at: existing.created_at,
        updated_at: existing.updated_at,
    };

    let points = point_defs_to_slave_db_points(slave_id, &point_defs);
    match db_service.slave_devices.update_with_points(slave_id, &updated, &points) {
        Ok((0, _)) => Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在")),
        Ok((_, inserted)) => {
            if previous_common_address != updated.common_address {
                remove_slave_runtime_points(
                    station_manager.inner(),
                    updated.station_id,
                    previous_common_address,
                    db_service.inner(),
                )
                .await;
                remove_slave_station_policy_override_from_runtime(
                    station_manager.inner(),
                    updated.station_id,
                    previous_common_address,
                    db_service.inner(),
                )
                .await;
            }

            apply_slave_defs_to_runtime(
                station_manager.inner(),
                updated.station_id,
                updated.common_address,
                &point_defs,
                db_service.inner(),
            )
            .await;
            apply_slave_station_policy_override_to_runtime(
                station_manager.inner(),
                updated.station_id,
                updated.common_address,
                station_policy_override,
                db_service.inner(),
            )
            .await;

            Ok(UpsertSlaveBundleResponse { slave_id, point_defs_updated: inserted })
        }
        Err(err) => Err(map_db_conflict_response(err.to_string(), "更新从站失败", &SLAVE_STATION_CONFLICTS)),
    }
}

#[tauri::command]
pub async fn delete_station_slave(
    slave_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    let existing = match db_service.slave_devices.get_by_id(slave_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };

    match db_service.slave_devices.delete(slave_id) {
        Ok(count) if count > 0 => {
            remove_slave_runtime_points(
                station_manager.inner(),
                existing.station_id,
                existing.common_address,
                db_service.inner(),
            )
            .await;
            remove_slave_station_policy_override_from_runtime(
                station_manager.inner(),
                existing.station_id,
                existing.common_address,
                db_service.inner(),
            )
            .await;
            Ok(())
        }
        Ok(_) => Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在")),
        Err(err) => Err(ApiError::new("DB_TRANSACTION_FAILED", format!("删除从站失败: {err}"))),
    }
}

#[tauri::command]
pub async fn list_station_slaves(
    station_id: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<SlaveResponse>> {
    let station_row_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return Err(ApiError::new("VALIDATION_ERROR", "无效的 station_id"));
        }
    };
    let rows = match db_service.slave_devices.get_by_station(station_row_id) {
        Ok(value) => value,
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };

    let global_defaults = match load_slave_station_policy_defaults(db_service.inner()) {
        Ok(value) => value,
        Err(err) => return Err(ApiError::app(err)),
    };
    let connection_override = match load_optional_station_link_params(db_service.inner(), station_row_id) {
        Ok(value) => value,
        Err(err) => return Err(ApiError::app(err)),
    };

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        let Some(slave_id) = row.id else {
            continue;
        };
        let station_policy_override =
            match load_slave_station_policy_override(db_service.inner(), station_row_id, slave_id) {
                Ok(value) => value,
                Err(err) => return Err(ApiError::app(err)),
            };
        let effective_station_policy =
            resolve_effective_station_policy(global_defaults, connection_override, station_policy_override);
        if let Some(item) = db_slave_to_response(row, station_policy_override, effective_station_policy) {
            items.push(item);
        }
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_station_slave_point_defs(
    slave_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<PointDef>> {
    let rows = match db_service.slave_points.get_by_slave_station(slave_id) {
        Ok(value) => value,
        Err(err) => return Err(ApiError::app(format!("读取点表失败: {err}"))),
    };
    Ok(db_slave_points_to_point_defs(&rows))
}

#[tauri::command]
pub async fn replace_station_slave_point_defs(
    slave_id: i64,
    mut defs: Vec<PointDef>,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<usize> {
    let station_slave = match db_service.slave_devices.get_by_id(slave_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };

    if let Err((code, message)) = normalize_and_validate_point_defs(&mut defs) {
        return Err(ApiError::new(code, message));
    }

    let points = point_defs_to_slave_db_points(slave_id, &defs);
    match db_service.slave_points.replace_by_slave_station(slave_id, &points) {
        Ok(updated) => {
            apply_slave_defs_to_runtime(
                station_manager.inner(),
                station_slave.station_id,
                station_slave.common_address,
                &defs,
                db_service.inner(),
            )
            .await;
            Ok(updated)
        }
        Err(err) => Err(ApiError::app(format!("保存点表失败: {err}"))),
    }
}

#[tauri::command]
pub async fn get_station_slave_asdu_aliases(
    slave_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<HashMap<String, String>> {
    let _station_slave = match db_service.slave_devices.get_by_id(slave_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };

    match db_service.slave_device_asdu_aliases.get_all(slave_id) {
        Ok(map) => Ok(normalize_asdu_alias_map(map)),
        Err(err) => Err(ApiError::app(format!("读取 ASDU 别名失败: {err}"))),
    }
}

#[tauri::command]
pub async fn upsert_station_slave_asdu_alias(
    slave_id: i64,
    asdu_type: String,
    alias: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<HashMap<String, String>> {
    let _station_slave = match db_service.slave_devices.get_by_id(slave_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };

    let normalized_type_name = String::from(asdu_type.trim());
    if normalized_type_name.is_empty() {
        return Err(ApiError::new("INVALID_ASDU_TYPE", "ASDU 类型不能为空"));
    }
    let Some(type_id) = iec104_type_id(&normalized_type_name) else {
        return Err(ApiError::new("INVALID_ASDU_TYPE", "ASDU 类型无效"));
    };
    let canonical_type_name = iec104_type_name(type_id).to_string();
    let normalized_alias = String::from(alias.trim());
    let mut alias_map = match db_service.slave_device_asdu_aliases.get_all(slave_id) {
        Ok(map) => normalize_asdu_alias_map(map),
        Err(err) => return Err(ApiError::app(format!("读取 ASDU 别名失败: {err}"))),
    };

    if normalized_alias.is_empty() {
        alias_map.remove(&canonical_type_name);
    } else {
        alias_map.insert(canonical_type_name, normalized_alias);
    }

    if let Err(err) = db_service.slave_device_asdu_aliases.replace_all(slave_id, &alias_map) {
        return Err(ApiError::app(format!("保存 ASDU 别名失败: {err}")));
    }

    Ok(alias_map)
}
