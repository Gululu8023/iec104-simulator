//! 从站模拟功能 API。
//!
//! 提供从站数据模拟相关的 Tauri 命令,包括:
//!
//! - **手动模拟**:触发单次数据变化
//! - **持续模拟**:启动/停止周期性随机变化
//! - **选点波形**:对指定点应用 15 种波形类型
//! - **场景注入**:雪崩场景、波形场景、通信故障场景
//! - **配置管理**:查询、更新初始化配置和模拟配置

use super::shared::*;

/// 模拟从站数据变化
#[tauri::command]
pub async fn simulate_slave_data_changes(
    station_id: String,
    common_address: Option<u16>,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    info!("模拟从站 {} 数据变化: common_address={:?}", station_id, common_address);

    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            if let Some(target_common_address) = common_address {
                service.simulate_data_changes_for_common_address(target_common_address).await;
            } else {
                service.simulate_data_changes().await;
            }
            Ok(())
        }
        None => Err(slave_station_not_found()),
    }
}

/// 强制上送从站当前数据，不修改数据点值。
#[tauri::command]
pub async fn force_upload_slave_data(
    station_id: String,
    common_address: u16,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<usize> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            service.force_upload_current_points(common_address).await.map_err(|error| ApiError::app(error.to_string()))
        }
        None => Err(slave_station_not_found()),
    }
}

/// 启动从站持续模拟
#[tauri::command]
pub async fn start_slave_simulation(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.start_simulation().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}

/// 停止从站持续模拟
#[tauri::command]
pub async fn stop_slave_simulation(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.stop_simulation().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}

/// 启动从站选点模拟
#[tauri::command]
pub async fn start_slave_selected_point_simulation(
    station_id: String,
    request: StartSelectedPointSimulationRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.start_selected_point_simulation(request).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}

/// 停止从站选点模拟
#[tauri::command]
pub async fn stop_slave_selected_point_simulation(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.stop_selected_point_simulation().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}

/// 获取模拟配置
#[tauri::command]
pub async fn get_slave_simulation_profile(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<SimulationProfile> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => Ok(service.get_simulation_profile().await),
        None => Err(slave_station_not_found()),
    }
}

/// 更新模拟配置
#[tauri::command]
pub async fn update_slave_simulation_profile(
    station_id: String,
    request: UpdateSimulationProfileRequest,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.update_simulation_profile(request.profile.clone()).await {
            Ok(_) => {
                let station_row_id = station_id.parse::<i64>().map_err(|_| "无效的 station_id".to_string());
                match serde_json::to_string(&request.profile).map_err(|e| e.to_string()).and_then(|json| {
                    station_row_id.and_then(|id| {
                        db_service
                            .slave_station_profiles
                            .set_simulation_profile(id, Some(&json))
                            .map_err(|e| e.to_string())
                    })
                }) {
                    Ok(()) => {}
                    Err(err) => {
                        error!("[slave] persist simulation_profile failed station_id={} error={}", station_id, err)
                    }
                }
                Ok(())
            }
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}

/// 更新初始化模板
#[tauri::command]
pub async fn update_slave_initialization_profile(
    station_id: String,
    request: UpdateInitializationProfileRequest,
    db_service: State<'_, Arc<DatabaseService>>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.update_initialization_profile(request.profile.clone()).await {
            Ok(_) => {
                let station_row_id = station_id.parse::<i64>().map_err(|_| "无效的 station_id".to_string());
                match serde_json::to_string(&request.profile).map_err(|e| e.to_string()).and_then(|json| {
                    station_row_id.and_then(|id| {
                        db_service
                            .slave_station_profiles
                            .set_initialization_profile(id, Some(&json))
                            .map_err(|e| e.to_string())
                    })
                }) {
                    Ok(()) => {}
                    Err(err) => {
                        error!("[slave] persist initialization_profile failed station_id={} error={}", station_id, err)
                    }
                }
                Ok(())
            }
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}

/// 触发模拟场景
#[tauri::command]
pub async fn trigger_slave_scenario(
    station_id: String,
    request: TriggerSimulationScenarioRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<usize> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => match service.trigger_scenario(request.scenario).await {
            Ok(changed) => Ok(changed),
            Err(e) => Err(ApiError::app(e.to_string())),
        },
        None => Err(slave_station_not_found()),
    }
}
