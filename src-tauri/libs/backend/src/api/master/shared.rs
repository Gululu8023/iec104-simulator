//! 主站 API 共享模块。
//!
//! 提供主站 API 模块内部共享的类型、常量和辅助函数,包括:
//!
//! - **依赖导入**:统一导入常用类型和模块
//! - **冲突映射**:数据库约束冲突的错误码映射
//! - **辅助函数**:ASDU 别名规范化、参数转换等

pub(crate) use std::{collections::HashMap, net::SocketAddr, sync::Arc};

pub(crate) use chrono::{DateTime, Utc};
pub(crate) use log::{error, info, warn};
pub(crate) use tauri::State;

pub(crate) use crate::{
    api::{
        command_helpers::{DbConflictMapping, get_master_service, map_db_conflict_response, master_station_not_found},
        mappers::{
            build_capture_export_meta, build_capture_export_response, db_station_to_station_config, infer_db_data_type,
        },
        station_factory::build_station_config,
        types::{
            ApiError, CommandResult, ConnectRequest, CreateStationRequest, ExportCaptureResponse, Iec104CommandRequest,
            Iec104CommandResult, LinkParams, LinkProfileResponse, LinkSlaveResponse, MasterFileTransferLimits,
            MasterFileTransferRequest, MasterFileTransferSession, PointDef, UpsertLinkProfileRequest,
            UpsertLinkSlaveBundleRequest, UpsertLinkSlaveBundleResponse,
        },
    },
    core::{
        iec104_registry::{iec104_default_point_name, iec104_type_id, iec104_type_name},
        master::MasterService,
        protocol_adapter::encode_cp56_time2a,
        types::{BackendSoeEvent, ConnectionInfo, MasterMessageRecord, PointSource, StationType},
    },
    db::{
        ConnectionParams as DbConnectionParams, DatabaseService, MasterLink as DbMasterLink,
        MasterPoint as DbMasterPoint, MasterSlave as DbMasterSlave, ProtocolType as DbProtocolType,
        Station as DbStation, StationType as DbStationType,
    },
    errors::{AppError, Iec104ErrorCode, NetworkError, ProtocolError},
    services::StationManager,
};

pub(crate) const MASTER_LINK_SLAVE_CONFLICTS: [DbConflictMapping; 2] = [
    ("UNIQUE constraint failed: master_slaves", "CONFLICT_COA_DUPLICATE", "同一链路下公共地址重复"),
    ("UNIQUE constraint failed: master_points", "CONFLICT_IOA_DUPLICATE", "信息对象地址重复，保存失败"),
];

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

const MASTER_BUILT_IN_POINT_TEMPLATES: [(u8, u32, u32); 11] = [
    (1, 0x0001, 100),
    (3, 0x1001, 100),
    (5, 0x2001, 20),
    (7, 0x3001, 20),
    (13, 0x4001, 100),
    (15, 0x6401, 20),
    (45, 0x6001, 100),
    (46, 0x6081, 100),
    (47, 0x6101, 20),
    (50, 0x6201, 100),
    (51, 0x6281, 100),
];

pub(crate) fn build_master_built_in_point_defs() -> Vec<PointDef> {
    let mut defs = Vec::new();
    for (type_id, start, count) in MASTER_BUILT_IN_POINT_TEMPLATES {
        for offset in 0..count {
            let address = start + offset;
            defs.push(PointDef {
                address,
                name: iec104_default_point_name(type_id, address),
                type_id,
                data_type: iec104_type_name(type_id).to_string(),
                description: None,
                control_ioa: None,
                gi_group: None,
                counter_group: None,
                default_value: (type_id == 51).then(|| serde_json::Value::from(0)),
                is_enabled: true,
                source: PointSource::BuiltIn,
            });
        }
    }
    defs
}

fn parse_query_time(raw: Option<&String>, field_name: &str) -> Result<Option<DateTime<Utc>>, String> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed =
        DateTime::parse_from_rfc3339(trimmed).map_err(|err| format!("{field_name} 不是合法的 RFC3339 时间: {err}"))?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

pub(crate) fn parse_query_time_to_cp56(raw: Option<&String>, field_name: &str) -> Result<[u8; 7], String> {
    Ok(parse_query_time(raw, field_name)?.map(encode_cp56_time2a).unwrap_or([0u8; 7]))
}

pub(crate) fn validate_query_time_range(start: Option<&String>, end: Option<&String>) -> Result<(), String> {
    let start_time = parse_query_time(start, "query_start_time")?;
    let end_time = parse_query_time(end, "query_end_time")?;
    if let (Some(start_time), Some(end_time)) = (start_time, end_time) {
        if start_time > end_time {
            return Err("query_start_time 不能晚于 query_end_time".to_string());
        }
    }
    Ok(())
}

pub(crate) fn map_master_file_transfer_error(err: AppError) -> ApiError {
    match err {
        AppError::Network(NetworkError::NotConnected) => ApiError::new("NOT_CONNECTED", "TCP 未建立，无法进行文件传输"),
        AppError::Network(NetworkError::Protocol(message)) => {
            let code = if message.contains("当前连接已有运行中的文件传输会话") {
                "FILE_TRANSFER_BUSY"
            } else {
                "PROTOCOL_STATE"
            };
            ApiError::new(code, message)
        }
        AppError::Protocol(ProtocolError::InvalidData(message)) => {
            let code = if message.contains("读取上传文件失败") {
                "UPLOAD_FILE_MISSING"
            } else if message.contains("超过上限") {
                "FILE_TOO_LARGE"
            } else {
                "VALIDATION_ERROR"
            };
            ApiError::new(code, message)
        }
        other => ApiError::new("INTERNAL_ERROR", other.to_string()),
    }
}

/// 主站启动时恢复持久化设置：预加载点表，不主动建立 TCP 连接
pub(crate) async fn load_persisted_master_settings(
    station_id: &str,
    service: &Arc<MasterService>,
    db_service: &Arc<DatabaseService>,
) {
    let row_id = match station_id.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return,
    };

    let profiles = match db_service.master_links.get_by_station(row_id) {
        Ok(rows) => rows,
        Err(err) => {
            warn!("读取主站连接配置失败 station_id={} error={}", station_id, err);
            return;
        }
    };

    for profile in profiles {
        let profile_id = match profile.id {
            Some(id) => id,
            None => continue,
        };

        // 加载从站点表定义并预填充到内存
        let slaves = db_service.master_slaves.get_by_connection(profile_id).unwrap_or_default();

        for slave in slaves {
            let slave_id = match slave.id {
                Some(value) => value,
                None => continue,
            };
            let point_defs = db_service.master_points.get_by_connection_slave(slave_id).unwrap_or_default();

            let defs: Vec<PointDef> = if point_defs.is_empty() {
                build_master_built_in_point_defs()
            } else {
                point_defs
                    .iter()
                    .map(|row| PointDef {
                        address: row.address,
                        name: row.name.clone(),
                        type_id: row.type_id,
                        data_type: iec104_type_name(row.type_id).to_string(),
                        description: row.description.clone(),
                        control_ioa: None,
                        gi_group: None,
                        counter_group: None,
                        default_value: row.default_value.as_ref().and_then(|raw| serde_json::from_str(raw).ok()),
                        is_enabled: row.is_enabled,
                        source: PointSource::Imported,
                    })
                    .collect()
            };

            service
                .preload_point_defs(slave_id, Some(profile_id), Some(slave.common_address), &defs, slave.enabled)
                .await;
        }
    }
}

pub(crate) async fn refresh_persisted_master_settings(
    station_id: i64,
    station_manager: &Arc<StationManager>,
    db_service: &Arc<DatabaseService>,
) -> bool {
    let station_id = station_id.to_string();
    let Some(service) = get_master_service(station_manager, &station_id).await else {
        return false;
    };
    load_persisted_master_settings(&station_id, &service, db_service).await;
    true
}

pub(crate) fn connection_params_to_link_params(params: &DbConnectionParams) -> LinkParams {
    LinkParams {
        k_value: params.k_value,
        w_value: params.w_value,
        t0_seconds: u64::from(params.t0),
        t1_seconds: u64::from(params.t1),
        t2_seconds: u64::from(params.t2),
        t3_seconds: u64::from(params.t3),
        max_asdu_bytes: LinkParams::default().max_asdu_bytes,
        select_timeout_seconds: LinkParams::default().select_timeout_seconds,
        enable_sq1_upload: LinkParams::default().enable_sq1_upload,
        enable_soe: LinkParams::default().enable_soe,
        unknown_typeid_negative_ack: LinkParams::default().unknown_typeid_negative_ack,
        outbound_queue_capacity: LinkParams::default().outbound_queue_capacity,
        spontaneous_flush_ms: LinkParams::default().spontaneous_flush_ms,
        simulation_latest_only: LinkParams::default().simulation_latest_only,
    }
}

pub(crate) fn db_connection_to_link_profile_response(conn: &DbMasterLink) -> Option<LinkProfileResponse> {
    let id = conn.id?;
    Some(LinkProfileResponse {
        id,
        station_id: conn.station_id.to_string(),
        name: conn.name.clone(),
        host: conn.connection_params.host.clone(),
        port: conn.connection_params.port,
        link_params: connection_params_to_link_params(&conn.connection_params),
        auto_connect: conn.auto_connect,
        auto_start_data_transfer: conn.auto_start_data_transfer,
        auto_gi: conn.auto_gi,
        retry_count: conn.retry_count,
        retry_interval: conn.retry_interval,
    })
}

pub(crate) fn db_connection_slave_to_response(row: &DbMasterSlave) -> Option<LinkSlaveResponse> {
    let id = row.id?;
    Some(LinkSlaveResponse {
        id,
        connection_id: row.connection_id,
        name: row.name.clone(),
        common_address: row.common_address,
        enabled: row.enabled,
    })
}

pub(crate) fn normalize_and_validate_point_defs(point_defs: &mut [PointDef]) -> Result<(), (&'static str, String)> {
    crate::core::shared::point_validation::normalize_and_validate_point_defs(point_defs)
}
