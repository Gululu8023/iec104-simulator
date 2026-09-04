//! 从站策略配置 API。
//!
//! 提供从站策略相关的 Tauri 命令,包括:
//!
//! - **链路参数**:T1/T2/T3/K/W 等 IEC 104 协议参数
//! - **冗余模式**:单机/主备/双主冗余组配置
//! - **命令策略**:命令不匹配时的处理策略
//! - **显示格式**:IOA 地址显示格式(十进制/十六进制)
//! - **默认策略**:从站全局默认策略配置

use super::shared::*;

/// 获取从站链路参数（t1/t2/k/w 等）
#[tauri::command]
pub async fn get_slave_link_params(
    station_id: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<LinkParams> {
    let station_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return Err(ApiError::app("无效的 station_id".to_string())),
    };
    match db_service.slave_station_profiles.get(station_id) {
        Ok(Some(profile)) => match profile.link_params {
            Some(raw) => match serde_json::from_str(&raw) {
                Ok(value) => Ok(value),
                Err(err) => Err(ApiError::app(format!("解析链路参数失败: {err}"))),
            },
            None => Ok(LinkParams::default()),
        },
        Ok(None) => Ok(LinkParams::default()),
        Err(err) => Err(ApiError::app(format!("读取链路参数失败: {err}"))),
    }
}

/// 获取从站冗余组模式（仅配置，不直接影响运行时行为）
#[tauri::command]
pub async fn get_slave_redundancy_mode(
    station_id: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<RedundancyGroupMode> {
    let station_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return Err(ApiError::app("无效的 station_id".to_string())),
    };
    match db_service.slave_station_profiles.get(station_id) {
        Ok(Some(profile)) => match profile.redundancy_mode {
            Some(raw) => match serde_json::from_str(&raw) {
                Ok(value) => Ok(value),
                Err(err) => Err(ApiError::app(format!("解析冗余组模式失败: {err}"))),
            },
            None => Ok(RedundancyGroupMode::Single),
        },
        Ok(None) => Ok(RedundancyGroupMode::Single),
        Err(err) => Err(ApiError::app(format!("读取冗余组模式失败: {err}"))),
    }
}

/// 更新从站冗余组模式（仅落库，不直接影响运行时行为）
#[tauri::command]
pub async fn update_slave_redundancy_mode(
    station_id: String,
    mode: RedundancyGroupMode,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    let station_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return Err(ApiError::app("无效的 station_id".to_string())),
    };
    let raw = match serde_json::to_string(&mode) {
        Ok(raw) => raw,
        Err(err) => return Err(ApiError::app(format!("序列化冗余组模式失败: {err}"))),
    };

    if let Err(err) = db_service.slave_station_profiles.set_redundancy_mode(station_id, Some(&raw)) {
        return Err(ApiError::app(format!("保存冗余组模式失败: {err}")));
    }

    Ok(())
}

/// 获取从站全局命令错配策略
#[tauri::command]
pub async fn get_slave_command_mismatch_policy(
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<SlaveCommandMismatchPolicy> {
    load_json_user_setting_or_default(
        db_service.inner(),
        SLAVE_GLOBAL_COMMAND_MISMATCH_POLICY_KEY,
        SlaveCommandMismatchPolicy::Strict,
        "解析命令错配策略失败",
        "读取命令错配策略失败",
    )
}

/// 获取从站全局 IOA 地址显示格式
#[tauri::command]
pub async fn get_slave_ioa_display_format(db_service: State<'_, Arc<DatabaseService>>) -> CommandResult<String> {
    match db_service.app_settings.get(SLAVE_GLOBAL_IOA_DISPLAY_FORMAT_KEY) {
        Ok(Some(raw)) => match serde_json::from_str::<String>(&raw) {
            Ok(value) => match normalize_slave_ioa_display_format(&value) {
                Some(normalized) => Ok(normalized.to_string()),
                None => Err(ApiError::app(format!("IOA 地址显示格式无效: {value}"))),
            },
            Err(err) => Err(ApiError::app(format!("解析 IOA 地址显示格式失败: {err}"))),
        },
        Ok(None) => Ok("dec".to_string()),
        Err(err) => Err(ApiError::app(format!("读取 IOA 地址显示格式失败: {err}"))),
    }
}

/// 获取从站全局站点策略默认值（SelectTimeout/SQ1/SOE）
#[tauri::command]
pub async fn get_slave_station_policy_defaults(
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<SlaveStationPolicy> {
    match load_slave_station_policy_defaults(db_service.inner()) {
        Ok(value) => Ok(value),
        Err(err) => Err(ApiError::app(err)),
    }
}

/// 更新从站全局站点策略默认值（落库并热更新所有已加载从站服务）
#[tauri::command]
pub async fn update_slave_station_policy_defaults(
    defaults: SlaveStationPolicy,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    let normalized = sanitize_station_policy_defaults(defaults);
    let raw = match serde_json::to_string(&normalized) {
        Ok(raw) => raw,
        Err(err) => {
            return Err(ApiError::app(format!("序列化从站策略默认值失败: {err}")));
        }
    };

    if let Err(err) = db_service.app_settings.set(SLAVE_GLOBAL_STATION_POLICY_DEFAULTS_KEY, &raw, "slave") {
        return Err(ApiError::app(format!("保存从站策略默认值失败: {err}")));
    }

    let station_ids = station_manager.get_slave_stations().await;
    for station_id in station_ids {
        if let Some(service) = station_manager.get_slave_service(&station_id).await {
            service.update_station_policy_defaults(normalized).await;
        }
    }

    Ok(())
}

/// 更新从站全局命令错配策略（落库并热更新所有已加载从站服务）
#[tauri::command]
pub async fn update_slave_command_mismatch_policy(
    policy: SlaveCommandMismatchPolicy,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    let raw = match serde_json::to_string(&policy) {
        Ok(raw) => raw,
        Err(err) => return Err(ApiError::app(format!("序列化命令错配策略失败: {err}"))),
    };

    if let Err(err) = db_service.app_settings.set(SLAVE_GLOBAL_COMMAND_MISMATCH_POLICY_KEY, &raw, "slave") {
        return Err(ApiError::app(format!("保存命令错配策略失败: {err}")));
    }

    let station_ids = station_manager.get_slave_stations().await;
    for station_id in station_ids {
        if let Some(service) = station_manager.get_slave_service(&station_id).await {
            service.set_command_mismatch_policy(policy).await;
        }
    }

    Ok(())
}

/// 更新从站全局 IOA 地址显示格式
#[tauri::command]
pub async fn update_slave_ioa_display_format(
    format: String,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    let Some(normalized) = normalize_slave_ioa_display_format(&format) else {
        return Err(ApiError::app(format!("不支持的 IOA 地址显示格式: {format}")));
    };

    let raw = match serde_json::to_string(normalized) {
        Ok(raw) => raw,
        Err(err) => {
            return Err(ApiError::app(format!("序列化 IOA 地址显示格式失败: {err}")));
        }
    };

    if let Err(err) = db_service.app_settings.set(SLAVE_GLOBAL_IOA_DISPLAY_FORMAT_KEY, &raw, "slave") {
        return Err(ApiError::app(format!("保存 IOA 地址显示格式失败: {err}")));
    }

    Ok(())
}

/// 更新从站链路参数（t1/t2/k/w 等）
#[tauri::command]
pub async fn update_slave_link_params(
    station_id: String,
    params: LinkParams,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    let station_row_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return Err(ApiError::app("无效的 station_id".to_string())),
    };
    let use_connection_override = params != LinkParams::default();
    let raw = if use_connection_override {
        let raw = match serde_json::to_string(&params) {
            Ok(raw) => raw,
            Err(err) => return Err(ApiError::app(format!("序列化链路参数失败: {err}"))),
        };
        Some(raw)
    } else {
        None
    };
    if let Err(err) = db_service.slave_station_profiles.set_link_params(station_row_id, raw.as_deref()) {
        return Err(ApiError::app(format!("保存链路参数失败: {err}")));
    }

    if let Some(service) = ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        service.update_link_params(params).await;
        service.set_station_policy_connection_override_enabled(use_connection_override).await;
    }

    Ok(())
}
