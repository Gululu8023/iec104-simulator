//! 通用 API 命令模块
//!
//! 提供与站点类型无关的通用 Tauri 命令接口，包括站点管理、状态查询、文件导出等功能。

use std::{fs, path::PathBuf, sync::Arc};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use log::{error, info, warn};
use tauri::State;

use crate::{
    api::types::{ApiError, CommandResult, StationSummary},
    core::{iec104_registry::Iec104Capability, types::StationStatus},
    db::{DatabaseService, StationType as DbStationType},
    services::{StationManager, station_manager::StationStats},
};

#[tauri::command]
pub async fn get_iec104_capabilities() -> CommandResult<Vec<Iec104Capability>> {
    Ok(crate::core::iec104_registry::all_capabilities())
}

const UI_LANGUAGE_SETTING_KEY: &str = "ui.language";
const UI_SETTING_CATEGORY: &str = "ui";
const DEFAULT_UI_LANGUAGE: &str = "zh-CN";
const SUPPORTED_UI_LANGUAGES: &[&str] = &["zh-CN", "en-US"];

fn normalize_ui_language(locale: &str) -> Option<&'static str> {
    let trimmed = locale.trim();
    SUPPORTED_UI_LANGUAGES.iter().copied().find(|supported| *supported == trimmed)
}

fn read_ui_language(db_service: &Arc<DatabaseService>) -> CommandResult<String> {
    match db_service.app_settings.get(UI_LANGUAGE_SETTING_KEY) {
        Ok(Some(raw)) => match serde_json::from_str::<String>(&raw) {
            Ok(locale) => match normalize_ui_language(&locale) {
                Some(locale) => Ok(locale.to_string()),
                None => {
                    warn!("ui.language 包含不支持的语言值，使用默认值: {}", locale);
                    Ok(DEFAULT_UI_LANGUAGE.to_string())
                }
            },
            Err(err) => {
                warn!("ui.language JSON 解析失败，使用默认值: {}", err);
                Ok(DEFAULT_UI_LANGUAGE.to_string())
            }
        },
        Ok(None) => Ok(DEFAULT_UI_LANGUAGE.to_string()),
        Err(err) => Err(ApiError::new("DB_READ_FAILED", format!("读取界面语言失败: {err}"))),
    }
}

fn write_ui_language(db_service: &Arc<DatabaseService>, locale: &str) -> CommandResult<()> {
    let Some(locale) = normalize_ui_language(locale) else {
        return Err(ApiError::with_params(
            "INVALID_LOCALE",
            format!("不支持的界面语言: {locale}"),
            std::collections::HashMap::from([("locale".to_string(), serde_json::Value::String(locale.to_string()))]),
        ));
    };

    let value = match serde_json::to_string(locale) {
        Ok(value) => value,
        Err(err) => return Err(ApiError::new("INTERNAL_ERROR", format!("序列化界面语言失败: {err}"))),
    };

    match db_service.app_settings.set(UI_LANGUAGE_SETTING_KEY, &value, UI_SETTING_CATEGORY) {
        Ok(()) => Ok(()),
        Err(err) => Err(ApiError::new("DB_WRITE_FAILED", format!("保存界面语言失败: {err}"))),
    }
}

/// 解码 Base64 字符串为字节数组
fn decode_base64_bytes(input: &str) -> Result<Vec<u8>, String> {
    let compact: Vec<u8> = input.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect();
    STANDARD.decode(compact).map_err(|error| format!("Base64 数据不合法: {error}"))
}

/// 获取当前界面语言
#[tauri::command]
pub async fn get_ui_language(db_service: State<'_, Arc<DatabaseService>>) -> CommandResult<String> {
    read_ui_language(db_service.inner())
}

/// 更新当前界面语言
#[tauri::command]
pub async fn update_ui_language(locale: String, db_service: State<'_, Arc<DatabaseService>>) -> CommandResult<()> {
    write_ui_language(db_service.inner(), &locale)
}

/// 获取所有站点列表（运行时）
///
/// 查询当前运行时加载的所有站点
/// ID，按主站和从站分类返回。注意：仅返回运行时加载的站点，不包括数据库中未加载的站点。
#[tauri::command]
pub async fn get_all_stations(station_manager: State<'_, Arc<StationManager>>) -> CommandResult<AllStationsResponse> {
    let master_stations = station_manager.get_master_stations().await;
    let slave_stations = station_manager.get_slave_stations().await;

    let response = AllStationsResponse { master_stations, slave_stations };

    Ok(response)
}

/// 列出站点配置（数据库 + 运行状态）
///
/// 从数据库查询站点配置，并补充运行时状态信息。支持按站点类型过滤（"master"/"slave"），
/// 未加载的站点状态默认为 Stopped。
#[tauri::command]
pub async fn list_stations(
    station_type: Option<String>,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<Vec<StationSummary>> {
    let station_type = station_type.map(|v| v.trim().to_ascii_lowercase());

    let rows = match station_type.as_deref() {
        Some("master") => db_service.stations.get_by_type(&crate::db::StationType::Master),
        Some("slave") => db_service.stations.get_by_type(&crate::db::StationType::Slave),
        Some(other) if !other.is_empty() => {
            return Err(ApiError::new("VALIDATION_ERROR", format!("无效的 station_type: {other}")));
        }
        _ => db_service.stations.get_all(),
    };

    let rows = match rows {
        Ok(rows) => rows,
        Err(err) => return Err(ApiError::new("DB_READ_FAILED", format!("读取站点列表失败: {err}"))),
    };

    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(id) = row.id else { continue };
        let station_id = id.to_string();
        let status = match station_manager.get_station_status(&station_id).await {
            Ok(status) => status,
            Err(_) => StationStatus::Stopped,
        };

        let common_address = if row.station_type == DbStationType::Slave {
            db_service
                .slave_devices
                .get_by_station(id)
                .ok()
                .and_then(|rows| rows.into_iter().min_by_key(|item| item.id.unwrap_or(i64::MAX)))
                .map(|device| device.common_address)
                .unwrap_or(1)
        } else {
            0
        };

        summaries.push(StationSummary {
            station_id,
            name: row.name,
            station_type: row.station_type.to_string(),
            host: row.host,
            port: row.port,
            common_address,
            enabled: row.enabled,
            status,
        });
    }

    Ok(summaries)
}

/// 删除站点及其关联配置
///
/// 从数据库删除站点配置，并清理关联的用户设置和运行时实例。关联配置清理和运行时实例移除采用
/// Best-Effort 策略，即使失败也不影响主流程。
#[tauri::command]
pub async fn delete_station(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
    db_service: State<'_, Arc<DatabaseService>>,
) -> CommandResult<()> {
    info!("删除站点: {}", station_id);

    let row_id = match station_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return Err(ApiError::new("VALIDATION_ERROR", "无效的 station_id")),
    };

    // 先删 DB，避免“站点未加载就删不掉配置”
    let deleted = match db_service.stations.delete(row_id) {
        Ok(count) => count,
        Err(err) => {
            error!("删除站点配置失败 station_id={} error={}", station_id, err);
            return Err(ApiError::new("DB_TRANSACTION_FAILED", format!("删除站点配置失败: {err}")));
        }
    };
    if deleted == 0 {
        return Err(ApiError::new("STATION_NOT_FOUND", "站点不存在"));
    }

    // 再尝试停止并移除运行实例（best-effort）
    if let Err(err) = station_manager.remove_station(&station_id).await {
        info!("站点运行实例未加载或移除失败 station_id={} error={}", station_id, err);
    }

    Ok(())
}

/// 获取站点运行状态
///
/// 查询指定站点的当前运行状态（Stopped/Starting/Running/Stopping）。
#[tauri::command]
pub async fn get_station_status(
    station_id: String,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<StationStatus> {
    match station_manager.get_station_status(&station_id).await {
        Ok(status) => Ok(status),
        Err(e) => {
            error!("获取站点状态失败: {}", e);
            Err(ApiError::new("STATION_STATUS_FAILED", e.to_string()))
        }
    }
}

/// 获取站点统计信息
///
/// 查询所有站点的统计信息，包括总数、运行数、主站数、从站数等。
#[tauri::command]
pub async fn get_station_stats(station_manager: State<'_, Arc<StationManager>>) -> CommandResult<StationStats> {
    let stats = station_manager.get_station_stats().await;
    Ok(stats)
}

/// 将导出内容保存到本地文件
///
/// 将 Base64 编码的导出内容解码并保存到指定的本地文件路径。自动创建父目录，会覆盖已存在的同名文件。
#[tauri::command]
pub async fn save_export_file(file_path: String, data_base64: String) -> CommandResult<()> {
    let trimmed_path = file_path.trim();
    if trimmed_path.is_empty() {
        return Err(ApiError::new("INVALID_EXPORT_PATH", "保存路径不能为空"));
    }

    let target_path = PathBuf::from(trimmed_path);
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = fs::create_dir_all(parent) {
                error!("创建导出目录失败 path={} error={}", parent.display(), err);
                return Err(ApiError::new("EXPORT_DIRECTORY_FAILED", format!("创建导出目录失败: {err}")));
            }
        }
    }

    let bytes = match decode_base64_bytes(&data_base64) {
        Ok(bytes) => bytes,
        Err(err) => return Err(ApiError::new("INVALID_EXPORT_DATA", format!("导出内容解码失败: {err}"))),
    };

    match fs::write(&target_path, bytes) {
        Ok(()) => {
            info!("导出文件已保存 path={}", target_path.display());
            Ok(())
        }
        Err(err) => {
            error!("写入导出文件失败 path={} error={}", target_path.display(), err);
            Err(ApiError::new("EXPORT_WRITE_FAILED", format!("写入导出文件失败: {err}")))
        }
    }
}

/// 停止所有站点
///
/// 批量停止所有正在运行的站点（包括主站和从站）。此操作不会删除站点配置，仅停止运行。
#[tauri::command]
pub async fn stop_all_stations(station_manager: State<'_, Arc<StationManager>>) -> CommandResult<()> {
    info!("停止所有站点");

    match station_manager.stop_all_stations().await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("停止所有站点失败: {}", e);
            Err(ApiError::new("STATION_STOP_FAILED", e.to_string()))
        }
    }
}

/// 获取应用信息
///
/// 查询应用的基本信息，包括名称、版本（从 Cargo.toml 读取）、描述等。
#[tauri::command]
pub async fn get_app_info() -> CommandResult<AppInfo> {
    let repository_url = env!("CARGO_PKG_REPOSITORY").to_string();
    let info = AppInfo {
        name: "IEC104协议模拟器".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "专业的IEC104协议模拟器".to_string(),
        license: env!("CARGO_PKG_LICENSE").to_string(),
        issues_url: format!("{repository_url}/issues"),
        docs_url: Some(format!("{repository_url}/tree/main/docs")),
        repository_url,
    };

    Ok(info)
}

/// 所有站点响应结构
///
/// 用于 `get_all_stations` 命令的响应，按类型分类返回站点 ID 列表。
#[derive(serde::Serialize)]
pub struct AllStationsResponse {
    /// 主站 ID 列表
    pub master_stations: Vec<String>,
    /// 从站 ID 列表
    pub slave_stations: Vec<String>,
}

/// 应用信息结构
///
/// 包含应用的基本元数据信息。
#[derive(serde::Serialize)]
pub struct AppInfo {
    /// 应用名称
    pub name: String,
    /// 应用版本号
    pub version: String,
    /// 应用描述
    pub description: String,
    /// SPDX 许可证标识
    pub license: String,
    /// 公开源码仓库
    pub repository_url: String,
    /// 问题反馈地址
    pub issues_url: String,
    /// 文档链接（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
}
