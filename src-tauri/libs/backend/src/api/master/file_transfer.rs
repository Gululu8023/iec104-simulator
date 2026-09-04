//! 主站文件传输 API。
//!
//! 提供主站文件传输相关的 Tauri 命令,包括启动、查询和取消文件传输会话。

use super::shared::*;

fn resolve_file_transfer_max_asdu_bytes(profile_id: Option<i64>, db_service: &DatabaseService) -> u16 {
    profile_id
        .and_then(|profile_id| db_service.master_links.get_by_id(profile_id).ok().flatten())
        .map(|conn| connection_params_to_link_params(&conn.connection_params).max_asdu_bytes)
        .unwrap_or(LinkParams::default().max_asdu_bytes)
}

#[tauri::command]
pub async fn start_master_file_transfer(
    station_id: String,
    request: MasterFileTransferRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<MasterFileTransferSession> {
    let connection_id = request.connection_id.trim().to_string();
    if connection_id.is_empty() {
        return Err(ApiError::new("VALIDATION_ERROR", "缺少 connection_id"));
    }

    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            let connections = service.get_connections().await;
            let runtime_connection = connections.iter().find(|item| item.id == connection_id);

            let mut resolved_slave_id = 0i64;
            let mut resolved_common_address = service.common_address();

            if request.slave_id > 0 {
                match db_service.master_slaves.get_by_id(request.slave_id) {
                    Ok(Some(row)) => {
                        if let Some(runtime_conn) = runtime_connection {
                            if runtime_conn.profile_id != Some(row.connection_id) {
                                return Err(ApiError::new("VALIDATION_ERROR", "从站不属于当前链路连接"));
                            }
                        }
                        resolved_slave_id = request.slave_id;
                        resolved_common_address = row.common_address;
                    }
                    Ok(None) => {
                        return Err(ApiError::new("SLAVE_NOT_FOUND", "从站不存在"));
                    }
                    Err(err) => {
                        return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
                    }
                }
            } else if let Some(runtime_conn) = runtime_connection {
                if let Some(profile_id) = runtime_conn.profile_id {
                    match db_service.master_slaves.get_by_connection(profile_id) {
                        Ok(mut rows) => {
                            rows.retain(|item| item.enabled);
                            rows.sort_by_key(|item| item.common_address);
                            if let Some(default_slave) = rows.into_iter().next() {
                                resolved_slave_id = default_slave.id.unwrap_or(0);
                                resolved_common_address = default_slave.common_address;
                            }
                        }
                        Err(err) => {
                            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取从站失败: {err}")));
                        }
                    }
                }
            }

            let max_asdu_bytes = resolve_file_transfer_max_asdu_bytes(
                runtime_connection.and_then(|conn| conn.profile_id),
                db_service.inner(),
            );
            if let Err(err) =
                validate_query_time_range(request.query_start_time.as_ref(), request.query_end_time.as_ref())
            {
                return Err(ApiError::new("VALIDATION_ERROR", err));
            }
            let query_range_start_time =
                match parse_query_time_to_cp56(request.query_start_time.as_ref(), "query_start_time") {
                    Ok(value) => value,
                    Err(err) => return Err(ApiError::new("VALIDATION_ERROR", err)),
                };
            let query_range_end_time = match parse_query_time_to_cp56(request.query_end_time.as_ref(), "query_end_time")
            {
                Ok(value) => value,
                Err(err) => return Err(ApiError::new("VALIDATION_ERROR", err)),
            };

            let core_request = crate::core::master::service::MasterFileTransferRequest {
                connection_id: connection_id.clone(),
                slave_id: resolved_slave_id,
                common_address: resolved_common_address,
                mode: request.mode,
                ioa: request.ioa.unwrap_or(0),
                nof: request.nof,
                local_path: request.local_path.clone(),
                query_range_start_time,
                query_range_end_time,
                max_asdu_bytes,
            };

            match service.start_file_transfer(core_request).await {
                Ok(session) => Ok(session),
                Err(err) => Err(map_master_file_transfer_error(err)),
            }
        }
        None => Err(ApiError::new("STATION_NOT_FOUND", "主站不存在")),
    }
}

#[tauri::command]
pub async fn get_master_file_transfer_limits(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<MasterFileTransferLimits> {
    let Some(service) = get_master_service(station_manager.inner(), &station_id).await else {
        return Err(master_station_not_found());
    };
    let runtime_connection =
        service.get_connections().await.into_iter().find(|connection| connection.id == connection_id);
    let Some(runtime_connection) = runtime_connection else {
        return Err(ApiError::new("CONNECTION_NOT_FOUND", "连接不存在"));
    };
    let max_asdu_bytes = resolve_file_transfer_max_asdu_bytes(runtime_connection.profile_id, db_service.inner());
    let Some(segment_payload_max) = crate::core::protocol_adapter::file_transfer_segment_payload_max(max_asdu_bytes)
    else {
        return Err(ApiError::new("VALIDATION_ERROR", "max_asdu_bytes 过小，无法承载文件段"));
    };
    let single_section_max_file_size =
        crate::core::protocol_adapter::file_transfer_single_section_max_file_size(max_asdu_bytes)
            .expect("segment payload capacity was validated");
    Ok(MasterFileTransferLimits { segment_payload_max: segment_payload_max as u16, single_section_max_file_size })
}

#[tauri::command]
pub async fn get_master_file_transfer_session(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<Option<MasterFileTransferSession>> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => Ok(service.get_file_transfer_session(&connection_id).await),
        None => Err(master_station_not_found()),
    }
}

#[tauri::command]
pub async fn cancel_master_file_transfer(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<MasterFileTransferSession> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => match service.cancel_file_transfer(&connection_id).await {
            Ok(session) => Ok(session),
            Err(err) => Err(ApiError::app(err.to_string())),
        },
        None => Err(master_station_not_found()),
    }
}
