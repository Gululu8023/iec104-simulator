//! 主站站点管理 API。
//!
//! 提供主站站点配置相关的 Tauri 命令,包括:
//!
//! - **主站管理**:创建主站
//! - **链路配置**:创建、更新、删除链路配置(Link Profile)
//! - **从站管理**:创建、更新、删除链路从站(Link Slave)
//! - **点表管理**:查询从站点表
//! - **ASDU 别名**:管理从站 ASDU 类型别名

use super::shared::*;

#[tauri::command]
pub async fn create_master_station(
    request: CreateStationRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<String> {
    info!("创建主站: {}", request.name);

    let persisted = match db_service.stations.get_by_type(&DbStationType::Master) {
        Ok(stations) => stations.into_iter().min_by_key(|station| station.id.unwrap_or(i64::MAX)),
        Err(err) => return Err(ApiError::app(format!("读取站点配置失败: {err}"))),
    };

    let config = match persisted {
        Some(saved) => match db_station_to_station_config(&saved, StationType::Master, 0) {
            Ok(config) => config,
            Err(msg) => return Err(ApiError::app(msg)),
        },
        None => {
            let config = match build_station_config(request, StationType::Master) {
                Ok(config) => config,
                Err(msg) => return Err(ApiError::app(msg)),
            };

            let station = DbStation {
                id: None,
                name: config.name.clone(),
                station_type: DbStationType::Master,
                description: None,
                host: config.address.ip().to_string(),
                port: config.address.port(),
                enabled: config.enabled,
                created_at: None,
                updated_at: None,
            };

            let station_id = match db_service.stations.create(&station) {
                Ok(id) => id.to_string(),
                Err(err) => return Err(ApiError::app(format!("保存站点配置失败: {err}"))),
            };

            crate::core::types::StationConfig { id: station_id, ..config }
        }
    };

    let is_new = station_manager.get_master_service(&config.id).await.is_none();

    match station_manager.create_station(config.clone()).await {
        Ok(station_id) => {
            if let Some(service) = get_master_service(station_manager.inner(), &station_id).await {
                if is_new {
                    load_persisted_master_settings(&station_id, &service, db_service.inner()).await;
                }
            }
            Ok(station_id)
        }
        Err(e) => {
            error!("创建主站失败: {}", e);
            Err(ApiError::app(e.to_string()))
        }
    }
}

/// 创建链路配置（不包含从站）
#[tauri::command]
pub async fn create_link_profile(
    station_id: String,
    request: UpsertLinkProfileRequest,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<LinkProfileResponse> {
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

    let params = DbConnectionParams {
        host: request.host,
        port: request.port,
        common_address: 0,
        timeout: request.link_params.t0_seconds as u32,
        k_value: request.link_params.k_value,
        w_value: request.link_params.w_value,
        t0: request.link_params.t0_seconds as u32,
        t1: request.link_params.t1_seconds as u32,
        t2: request.link_params.t2_seconds as u32,
        t3: request.link_params.t3_seconds as u32,
    };

    let profile = DbMasterLink {
        id: None,
        station_id: station_row_id,
        name: request.name,
        protocol_type: DbProtocolType::Iec104,
        connection_params: params,
        is_active: false,
        auto_connect: request.auto_connect,
        auto_start_data_transfer: request.auto_start_data_transfer,
        auto_gi: request.auto_gi,
        retry_count: request.retry_count,
        retry_interval: request.retry_interval,
        created_at: None,
        updated_at: None,
    };

    let id = match db_service.master_links.create(&profile) {
        Ok(value) => value,
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("保存链路配置失败: {err}")));
        }
    };

    match db_service.master_links.get_by_id(id) {
        Ok(Some(saved)) => match db_connection_to_link_profile_response(&saved) {
            Some(resp) => Ok(resp),
            None => Err(ApiError::new("DB_TRANSACTION_FAILED", "链路配置保存后读取失败")),
        },
        Ok(None) => Err(ApiError::new("DB_TRANSACTION_FAILED", "链路配置保存后读取失败")),
        Err(err) => Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}"))),
    }
}

/// 更新链路配置（链路参数修改后需手动重连生效）
#[tauri::command]
pub async fn update_link_profile(
    profile_id: i64,
    request: UpsertLinkProfileRequest,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<LinkProfileResponse> {
    let existing = match db_service.master_links.get_by_id(profile_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("PROFILE_NOT_FOUND", "链路配置不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}")));
        }
    };

    let params = DbConnectionParams {
        host: request.host,
        port: request.port,
        common_address: 0,
        timeout: request.link_params.t0_seconds as u32,
        k_value: request.link_params.k_value,
        w_value: request.link_params.w_value,
        t0: request.link_params.t0_seconds as u32,
        t1: request.link_params.t1_seconds as u32,
        t2: request.link_params.t2_seconds as u32,
        t3: request.link_params.t3_seconds as u32,
    };

    let updated = DbMasterLink {
        id: existing.id,
        station_id: existing.station_id,
        name: request.name,
        protocol_type: existing.protocol_type,
        connection_params: params,
        is_active: existing.is_active,
        auto_connect: request.auto_connect,
        auto_start_data_transfer: request.auto_start_data_transfer,
        auto_gi: request.auto_gi,
        retry_count: request.retry_count,
        retry_interval: request.retry_interval,
        created_at: existing.created_at,
        updated_at: existing.updated_at,
    };

    match db_service.master_links.update(profile_id, &updated) {
        Ok(0) => Err(ApiError::new("PROFILE_NOT_FOUND", "链路配置不存在")),
        Ok(_) => match db_service.master_links.get_by_id(profile_id) {
            Ok(Some(saved)) => match db_connection_to_link_profile_response(&saved) {
                Some(resp) => Ok(resp),
                None => Err(ApiError::new("DB_TRANSACTION_FAILED", "链路配置更新后读取失败")),
            },
            Ok(None) => Err(ApiError::new("DB_TRANSACTION_FAILED", "链路配置更新后读取失败")),
            Err(err) => Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}"))),
        },
        Err(err) => Err(ApiError::new("DB_TRANSACTION_FAILED", format!("更新链路配置失败: {err}"))),
    }
}

#[tauri::command]
pub async fn delete_link_profile(profile_id: i64, db_service: State<'_, Arc<DatabaseService>>) -> CommandResult<()> {
    match db_service.master_links.delete(profile_id) {
        Ok(count) if count > 0 => Ok(()),
        Ok(_) => Err(ApiError::new("PROFILE_NOT_FOUND", "链路配置不存在")),
        Err(err) => Err(ApiError::new("DB_TRANSACTION_FAILED", format!("删除链路配置失败: {err}"))),
    }
}

#[tauri::command]
pub async fn list_link_profiles(
    station_id: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<LinkProfileResponse>> {
    let station_row_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return Err(ApiError::new("VALIDATION_ERROR", "无效的 station_id"));
        }
    };

    let rows = match db_service.master_links.get_by_station(station_row_id) {
        Ok(value) => value,
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}")));
        }
    };

    Ok(rows.iter().filter_map(db_connection_to_link_profile_response).collect())
}

#[tauri::command]
pub async fn create_link_slave(
    connection_id: i64,
    mut request: UpsertLinkSlaveBundleRequest,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<UpsertLinkSlaveBundleResponse> {
    let profile = match db_service.master_links.get_by_id(connection_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return Err(ApiError::new("PROFILE_NOT_FOUND", "链路配置不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}")));
        }
    };

    if let Err((code, message)) = normalize_and_validate_point_defs(&mut request.point_defs) {
        return Err(ApiError::new(code, message));
    }

    let slave = DbMasterSlave {
        id: None,
        connection_id,
        name: request.slave.name,
        common_address: request.slave.common_address,
        enabled: request.slave.enabled,
        created_at: None,
        updated_at: None,
    };

    let points = request
        .point_defs
        .into_iter()
        .map(|def| DbMasterPoint {
            id: None,
            connection_slave_id: 0,
            address: def.address,
            name: def.name,
            type_id: def.type_id,
            data_type: infer_db_data_type(def.type_id),
            description: def.description,
            default_value: def.default_value.and_then(|v| serde_json::to_string(&v).ok()),
            is_enabled: def.is_enabled,
            created_at: None,
            updated_at: None,
        })
        .collect::<Vec<_>>();

    match db_service.master_slaves.create_with_points(connection_id, &slave, &points) {
        Ok((slave_id, inserted)) => {
            refresh_persisted_master_settings(profile.station_id, station_manager.inner(), db_service.inner()).await;
            Ok(UpsertLinkSlaveBundleResponse { slave_id: slave_id, point_defs_updated: inserted })
        }
        Err(err) => Err(map_db_conflict_response(err.to_string(), "保存从站失败", &MASTER_LINK_SLAVE_CONFLICTS)),
    }
}

#[tauri::command]
pub async fn update_link_slave(
    slave_id: i64,
    mut request: UpsertLinkSlaveBundleRequest,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<UpsertLinkSlaveBundleResponse> {
    let existing = match db_service.master_slaves.get_by_id(slave_id) {
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

    let updated = DbMasterSlave {
        id: existing.id,
        connection_id: existing.connection_id,
        name: request.slave.name,
        common_address: request.slave.common_address,
        enabled: request.slave.enabled,
        created_at: existing.created_at,
        updated_at: existing.updated_at,
    };
    let profile = match db_service.master_links.get_by_id(existing.connection_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => return Err(ApiError::new("PROFILE_NOT_FOUND", "链路配置不存在")),
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}")));
        }
    };

    let points = request
        .point_defs
        .into_iter()
        .map(|def| DbMasterPoint {
            id: None,
            connection_slave_id: slave_id,
            address: def.address,
            name: def.name,
            type_id: def.type_id,
            data_type: infer_db_data_type(def.type_id),
            description: def.description,
            default_value: def.default_value.and_then(|v| serde_json::to_string(&v).ok()),
            is_enabled: def.is_enabled,
            created_at: None,
            updated_at: None,
        })
        .collect::<Vec<_>>();

    match db_service.master_slaves.update_with_points(slave_id, &updated, &points) {
        Ok((0, _)) => Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在")),
        Ok((_, inserted)) => {
            refresh_persisted_master_settings(profile.station_id, station_manager.inner(), db_service.inner()).await;
            Ok(UpsertLinkSlaveBundleResponse { slave_id: slave_id, point_defs_updated: inserted })
        }
        Err(err) => Err(map_db_conflict_response(err.to_string(), "更新从站失败", &MASTER_LINK_SLAVE_CONFLICTS)),
    }
}

#[tauri::command]
pub async fn delete_link_slave(
    slave_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    let station_id = db_service
        .master_slaves
        .get_by_id(slave_id)
        .ok()
        .flatten()
        .and_then(|slave| db_service.master_links.get_by_id(slave.connection_id).ok().flatten())
        .map(|profile| profile.station_id);
    match db_service.master_slaves.delete(slave_id) {
        Ok(count) if count > 0 => {
            if let Some(station_id) = station_id {
                if let Some(service) = station_manager.get_master_service(&station_id.to_string()).await {
                    service.clear_point_scope(slave_id).await;
                }
            }
            Ok(())
        }
        Ok(_) => Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在")),
        Err(err) => Err(ApiError::new("DB_TRANSACTION_FAILED", format!("删除从站失败: {err}"))),
    }
}

#[tauri::command]
pub async fn list_link_slaves(
    connection_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<LinkSlaveResponse>> {
    let rows = match db_service.master_slaves.get_by_connection(connection_id) {
        Ok(value) => value,
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };
    Ok(rows.iter().filter_map(db_connection_slave_to_response).collect())
}

#[tauri::command]
pub async fn get_link_slave_point_defs(
    slave_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<PointDef>> {
    let rows = match db_service.master_points.get_by_connection_slave(slave_id) {
        Ok(value) => value,
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取点表失败: {err}")));
        }
    };

    Ok(if rows.is_empty() {
        build_master_built_in_point_defs()
    } else {
        rows.into_iter()
            .map(|row| PointDef {
                address: row.address,
                name: row.name,
                type_id: row.type_id,
                data_type: iec104_type_name(row.type_id).to_string(),
                description: row.description,
                control_ioa: None,
                gi_group: None,
                counter_group: None,
                default_value: row.default_value.and_then(|raw| serde_json::from_str(&raw).ok()),
                is_enabled: row.is_enabled,
                source: PointSource::Imported,
            })
            .collect()
    })
}

#[tauri::command]
pub async fn get_link_slave_asdu_aliases(
    slave_id: i64,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<HashMap<String, String>> {
    let _slave = match db_service.master_slaves.get_by_id(slave_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
        }
    };
    match db_service.master_slave_asdu_aliases.get_all(slave_id) {
        Ok(map) => Ok(normalize_asdu_alias_map(map)),
        Err(err) => Err(ApiError::app(format!("读取 ASDU 别名失败: {err}"))),
    }
}

#[tauri::command]
pub async fn upsert_link_slave_asdu_alias(
    slave_id: i64,
    asdu_type: String,
    alias: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<HashMap<String, String>> {
    let _slave = match db_service.master_slaves.get_by_id(slave_id) {
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
    let mut alias_map = match db_service.master_slave_asdu_aliases.get_all(slave_id) {
        Ok(map) => normalize_asdu_alias_map(map),
        Err(err) => return Err(ApiError::app(format!("读取 ASDU 别名失败: {err}"))),
    };

    if normalized_alias.is_empty() {
        alias_map.remove(&canonical_type_name);
    } else {
        alias_map.insert(canonical_type_name, normalized_alias);
    }

    if let Err(err) = db_service.master_slave_asdu_aliases.replace_all(slave_id, &alias_map) {
        return Err(ApiError::app(format!("保存 ASDU 别名失败: {err}")));
    }

    Ok(alias_map)
}
