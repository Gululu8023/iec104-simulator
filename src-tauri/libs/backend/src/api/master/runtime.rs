//! 主站运行时控制 API。
//!
//! 提供主站运行时相关的 Tauri 命令,包括:
//!
//! - **生命周期管理**:启动、停止主站
//! - **连接管理**:连接从站、断开连接、查询连接状态
//! - **数据传输**:启动/停止数据传输、总召唤、命令下发
//! - **消息查询**:查询消息记录、SOE 事件、数据点
//! - **消息跟踪**:启用/禁用消息跟踪功能

use super::shared::*;

#[tauri::command]
pub async fn start_master_station(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    info!("启动主站: {}", station_id);

    match station_manager.start_station(&station_id).await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("启动主站失败: {}", e);
            Err(ApiError::app(e.to_string()))
        }
    }
}

/// 停止主站
#[tauri::command]
pub async fn stop_master_station(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    info!("停止主站: {}", station_id);

    match station_manager.stop_station(&station_id).await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("停止主站失败: {}", e);
            Err(ApiError::app(e.to_string()))
        }
    }
}

/// 创建从站连接配置（profile，不会自动连接）
#[tauri::command]
pub async fn connect_to_slave(
    station_id: String,
    request: ConnectRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<String> {
    let mut host = request.host;
    let mut port = request.port;
    let mut link_params: Option<LinkParams> = None;
    let profile_id = request.profile_id;
    let mut auto_start_data_transfer = request.auto_start_data_transfer.unwrap_or(true);
    let mut auto_gi = false;
    let mut auto_reconnect = false;
    let mut retry_count = 0;
    let mut retry_interval_seconds = 5;

    if let Some(pid) = profile_id {
        match db_service.master_links.get_by_id(pid) {
            Ok(Some(profile)) => {
                if profile.station_id.to_string() != station_id {
                    return Err(ApiError::app("连接配置与主站不匹配".to_string()));
                }

                host = profile.connection_params.host.clone();
                port = profile.connection_params.port;
                link_params = Some(connection_params_to_link_params(&profile.connection_params));
                auto_start_data_transfer = request.auto_start_data_transfer.unwrap_or(profile.auto_start_data_transfer);
                auto_gi = profile.auto_gi;
                auto_reconnect = profile.auto_connect;
                retry_count = profile.retry_count;
                retry_interval_seconds = profile.retry_interval;
            }
            Ok(None) => return Err(ApiError::app("连接配置不存在".to_string())),
            Err(err) => return Err(ApiError::app(format!("读取连接配置失败: {err}"))),
        }
    }

    info!("主站 {} 连接到从站: {}:{}", station_id, host, port);

    let slave_addr = match format!("{}:{}", host, port).parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(e) => return Err(ApiError::app(format!("无效的地址: {}", e))),
    };

    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            match service
                .connect_to_slave(
                    slave_addr,
                    profile_id,
                    link_params,
                    auto_start_data_transfer,
                    auto_gi,
                    auto_reconnect,
                    retry_count,
                    retry_interval_seconds,
                )
                .await
            {
                Ok(connection_id) => Ok(connection_id),
                Err(e) => {
                    error!("连接失败: {}", e);
                    let message = e.to_string();
                    if message.contains("重复连接被拒绝") {
                        return Err(ApiError::new("DUPLICATE_CONNECTION", message));
                    }
                    Err(ApiError::app(message))
                }
            }
        }
        None => Err(master_station_not_found()),
    }
}

/// 连接链路配置（基于 link_profile_id 建立一条 TCP 连接）
#[tauri::command]
pub async fn connect_link_profile(
    station_id: String,
    link_profile_id: i64,
    auto_start_data_transfer: Option<bool>,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<String> {
    let profile = match db_service.master_links.get_by_id(link_profile_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(ApiError::new("PROFILE_NOT_FOUND", "链路配置不存在"));
        }
        Err(err) => {
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("读取链路配置失败: {err}")));
        }
    };

    if profile.station_id.to_string() != station_id {
        return Err(ApiError::new("VALIDATION_ERROR", "链路配置与主站不匹配"));
    }

    let request = ConnectRequest {
        host: profile.connection_params.host.clone(),
        port: profile.connection_params.port,
        profile_id: Some(link_profile_id),
        auto_start_data_transfer: Some(auto_start_data_transfer.unwrap_or(profile.auto_start_data_transfer)),
    };

    connect_to_slave(station_id, request, station_manager, db_service).await
}

/// 断开连接
#[tauri::command]
pub async fn disconnect_from_slave(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    info!("主站 {} 断开连接: {}", station_id, connection_id);

    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => match service.disconnect(&connection_id).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("断开连接失败: {}", e);
                Err(ApiError::app(e.to_string()))
            }
        },
        None => Err(master_station_not_found()),
    }
}

/// 手动启动 IEC104 数据传输链路（发送 STARTDT_ACT）
#[tauri::command]
pub async fn start_master_data_transfer(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => match service.start_data_transfer(&connection_id).await {
            Ok(_) => Ok(()),
            Err(err) => Err(ApiError::app(err.to_string())),
        },
        None => Err(master_station_not_found()),
    }
}

/// 手动停止 IEC104 数据传输链路（发送 STOPDT_ACT）
#[tauri::command]
pub async fn stop_master_data_transfer(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => match service.stop_data_transfer(&connection_id).await {
            Ok(_) => Ok(()),
            Err(err) => Err(ApiError::app(err.to_string())),
        },
        None => Err(master_station_not_found()),
    }
}

/// 发送 IEC104 传输层测试帧（TESTFR_ACT）
#[tauri::command]
pub async fn send_test_frame(
    station_id: String,
    connection_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    info!("主站 {} 发送 TESTFR_ACT 到连接: {}", station_id, connection_id);

    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => match service.send_test_frame(&connection_id).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("发送 TESTFR_ACT 失败: {}", e);
                Err(ApiError::app(e.to_string()))
            }
        },
        None => Err(master_station_not_found()),
    }
}

/// 统一IEC104命令入口
#[tauri::command]
pub async fn send_iec104_command(
    station_id: String,
    request: Iec104CommandRequest,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Iec104CommandResult> {
    let connection_id = request.connection_id.trim().to_string();
    if connection_id.is_empty() {
        return Ok(Iec104CommandResult::rejected(Iec104ErrorCode::InvalidValue, "缺少 connection_id"));
    }

    info!("主站 {} 发送统一IEC104命令，connection_id={} slave_id={}", station_id, connection_id, request.slave_id);

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
                                return Ok(Iec104CommandResult::rejected(
                                    Iec104ErrorCode::InvalidCommonAddress,
                                    "从站不属于当前链路连接",
                                ));
                            }
                        }
                        resolved_slave_id = request.slave_id;
                        resolved_common_address = row.common_address;
                    }
                    Ok(None) => {
                        return Ok(Iec104CommandResult::rejected(Iec104ErrorCode::InvalidCommonAddress, "从站不存在"));
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

            match service
                .dispatch_command_with_qualifier_mode(
                    &connection_id,
                    resolved_slave_id,
                    resolved_common_address,
                    request.command,
                    request.qualifier_mode,
                )
                .await
            {
                Ok(receipt) => Ok(Iec104CommandResult::accepted(receipt.trace_id)),
                Err(err) => Ok(Iec104CommandResult {
                    accepted: false,
                    trace_id: err.trace_id,
                    error_code: Some(err.code),
                    error_message: Some(err.message),
                }),
            }
        }
        None => Err(master_station_not_found()),
    }
}

/// 获取主站连接列表
#[tauri::command]
pub async fn get_master_connections(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<Vec<ConnectionInfo>> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            let connections = service.get_connections().await;
            Ok(connections)
        }
        None => Err(master_station_not_found()),
    }
}

/// 增量同步主站数据点
#[tauri::command]
pub async fn sync_master_data_points(
    station_id: String,
    cursor: Option<crate::api::point_sync::PointSyncCursor>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<crate::api::point_sync::PointSyncBatch> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => Ok(crate::api::point_sync::master_batch(service.sync_data_points(cursor).await)),
        None => Err(master_station_not_found()),
    }
}

/// 获取主站通信报文（支持按ID增量拉取）
#[tauri::command]
pub async fn get_master_messages(
    station_id: String,
    since_id: Option<u64>,
    limit: Option<usize>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<Vec<MasterMessageRecord>> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            let messages = service.get_messages(since_id, limit.unwrap_or(200)).await;
            Ok(messages)
        }
        None => Err(master_station_not_found()),
    }
}

/// 设置主站通信报文追踪开关（开启时重置当前追踪会话）
#[tauri::command]
pub async fn set_master_message_tracking(
    station_id: String,
    enabled: bool,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            service.set_message_tracking_enabled(enabled).await;
            Ok(())
        }
        None => Err(master_station_not_found()),
    }
}

/// 获取主站 SOE 事件（支持按ID增量拉取）
#[tauri::command]
pub async fn get_master_soe_events(
    station_id: String,
    since_id: Option<u64>,
    limit: Option<usize>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<Vec<BackendSoeEvent>> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            let events = service.get_soe_events(since_id, limit.unwrap_or(200)).await;
            Ok(events)
        }
        None => Err(master_station_not_found()),
    }
}

/// 清空主站 SOE 事件缓存
#[tauri::command]
pub async fn clear_master_soe_events(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<()> {
    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => {
            service.clear_soe_events().await;
            Ok(())
        }
        None => Err(ApiError::app("主站不存在".to_string())),
    }
}
