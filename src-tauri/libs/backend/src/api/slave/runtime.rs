//! 从站运行时控制 API。
//!
//! 提供从站运行时相关的 Tauri 命令,包括:
//!
//! - **生命周期管理**:启动、停止从站
//! - **连接查询**:查询主站连接状态
//! - **数据点管理**:查询、更新、批量更新数据点值
//! - **消息查询**:查询消息记录、SOE 事件
//! - **消息跟踪**:启用/禁用消息跟踪功能

use super::shared::*;

/// 启动从站
#[tauri::command]
pub async fn start_slave_station(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    info!("启动从站: {}", station_id);

    if ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await.is_none() {
        return Err(slave_station_not_found());
    }

    match station_manager.start_station(&station_id).await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("启动从站失败: {}", e);
            Err(ApiError::app(e.to_string()))
        }
    }
}

/// 停止从站
#[tauri::command]
pub async fn stop_slave_station(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    info!("停止从站: {}", station_id);

    if ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await.is_none() {
        return Err(slave_station_not_found());
    }

    match station_manager.stop_station(&station_id).await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("停止从站失败: {}", e);
            Err(ApiError::app(e.to_string()))
        }
    }
}

/// 获取从站连接列表
#[tauri::command]
pub async fn get_slave_connections(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<ConnectionInfo>> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            let connections = service.get_connections().await;
            Ok(connections)
        }
        None => Err(slave_station_not_found()),
    }
}

#[tauri::command]
pub async fn get_slave_station_time_offset(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<i64> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => Ok(service.get_station_time_offset_ms().await),
        None => Err(slave_station_not_found()),
    }
}

/// 增量同步从站数据点
#[tauri::command]
pub async fn sync_slave_data_points(
    station_id: String,
    cursor: Option<crate::api::point_sync::PointSyncCursor>,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<crate::api::point_sync::PointSyncBatch> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => Ok(crate::api::point_sync::slave_batch(service.sync_data_points(cursor).await)),
        None => Err(slave_station_not_found()),
    }
}

/// 更新从站数据点
#[tauri::command]
pub async fn update_slave_data_point(
    station_id: String,
    request: UpdateDataPointRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    info!(
        "更新从站 {} 数据点: ca={:?} 地址={} 值={}",
        station_id, request.common_address, request.address, request.value
    );

    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            let result = if let Some(common_address) = request.common_address {
                service
                    .update_data_point_for_common_address(
                        common_address,
                        request.address,
                        request.value,
                        request.quality,
                    )
                    .await
            } else {
                service.update_data_point(request.address, request.value, request.quality).await
            };

            match result {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("更新数据点失败: {}", e);
                    Err(ApiError::app(e.to_string()))
                }
            }
        }
        None => Err(slave_station_not_found()),
    }
}

/// 批量更新从站数据点
#[tauri::command]
pub async fn bulk_update_slave_data_points(
    station_id: String,
    requests: Vec<UpdateDataPointRequest>,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<usize> {
    info!("批量更新从站 {} 数据点: count={}", station_id, requests.len());

    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            let mut default_updates: Vec<(u32, f64, u8)> = Vec::new();
            let mut updates_by_common_address: HashMap<u16, Vec<(u32, f64, u8)>> = HashMap::new();

            for request in requests {
                if let Some(common_address) = request.common_address {
                    updates_by_common_address.entry(common_address).or_default().push((
                        request.address,
                        request.value,
                        request.quality,
                    ));
                } else {
                    default_updates.push((request.address, request.value, request.quality));
                }
            }

            let mut total_updated: usize = 0;

            if !default_updates.is_empty() {
                match service.bulk_update_data_points(&default_updates).await {
                    Ok(updated) => total_updated += updated,
                    Err(err) => {
                        error!("批量更新默认 CA 数据点失败: {}", err);
                        return Err(ApiError::app(err.to_string()));
                    }
                }
            }

            for (common_address, updates) in updates_by_common_address {
                match service.bulk_update_data_points_for_common_address(common_address, &updates).await {
                    Ok(updated) => total_updated += updated,
                    Err(err) => {
                        error!("批量更新 CA={} 数据点失败: {}", common_address, err);
                        return Err(ApiError::app(err.to_string()));
                    }
                }
            }

            Ok(total_updated)
        }
        None => Err(slave_station_not_found()),
    }
}

/// 模拟从站数据变化
/// 获取从站通信报文（支持按ID增量拉取）
#[tauri::command]
pub async fn get_slave_messages(
    station_id: String,
    since_id: Option<u64>,
    limit: Option<usize>,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<SlaveMessageRecord>> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            let messages = service.get_messages(since_id, limit.unwrap_or(200)).await;
            Ok(messages)
        }
        None => Err(slave_station_not_found()),
    }
}

/// 设置从站通信报文追踪开关（开启时重置当前追踪会话）
#[tauri::command]
pub async fn set_slave_message_tracking(
    station_id: String,
    enabled: bool,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    match ensure_slave_service(station_manager.inner(), &station_id, db_service.inner()).await {
        Some(service) => {
            service.set_message_tracking_enabled(enabled).await;
            Ok(())
        }
        None => Err(slave_station_not_found()),
    }
}

/// 获取从站 SOE 事件（支持按ID增量拉取）
#[tauri::command]
pub async fn get_slave_soe_events(
    station_id: String,
    since_id: Option<u64>,
    limit: Option<usize>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<Vec<BackendSoeEvent>> {
    match get_slave_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            let events = service.get_soe_events(since_id, limit.unwrap_or(200)).await;
            Ok(events)
        }
        None => Err(slave_station_not_found()),
    }
}

/// 清空从站 SOE 事件缓存
#[tauri::command]
pub async fn clear_slave_soe_events(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match get_slave_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            service.clear_soe_events().await;
            Ok(())
        }
        None => Err(slave_station_not_found()),
    }
}
