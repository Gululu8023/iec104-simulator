//! API 命令层共享辅助逻辑
//!
//! 提供 Tauri 命令处理的通用辅助函数，包括错误处理、配置管理、服务获取等。

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::{
    api::types::{ApiError, CommandResult},
    core::{master::MasterService, slave::SlaveService},
    db::DatabaseService,
    services::StationManager,
};

/// 数据库冲突映射元组
///
/// 用于定义数据库约束冲突的映射规则，包含三个元素：
/// - 约束名称模式（用于匹配错误消息）
/// - 错误码（返回给前端的错误码）
/// - 用户友好的错误消息
///
/// # 示例
///
/// ```rust
/// let conflicts: &[DbConflictMapping] = &[
///     ("UNIQUE constraint failed: stations.name", "STATION_NAME_CONFLICT", "站点名称已存在"),
///     ("FOREIGN KEY constraint failed", "FOREIGN_KEY_VIOLATION", "关联数据不存在"),
/// ];
/// ```
pub(crate) type DbConflictMapping = (&'static str, &'static str, &'static str);

/// 映射数据库冲突错误为用户友好的响应
///
/// 将数据库约束冲突错误转换为结构化的 API
/// 错误响应。遍历冲突映射规则，匹配则返回对应错误码和消息，否则返回通用 `DB_TRANSACTION_FAILED`
/// 错误。
pub(crate) fn map_db_conflict_response(
    message: String,
    fallback_prefix: &str,
    conflicts: &[DbConflictMapping],
) -> ApiError {
    for (constraint, code, display_message) in conflicts {
        if message.contains(constraint) {
            return ApiError::new(*code, *display_message);
        }
    }

    ApiError::new("DB_TRANSACTION_FAILED", format!("{fallback_prefix}: {message}"))
}

/// 加载 JSON 格式的用户设置或返回默认值
///
/// 从数据库加载用户设置并反序列化为指定类型，如果不存在或解析失败则返回默认值。
pub(crate) fn load_json_user_setting_or_default<T>(
    db_service: &Arc<DatabaseService>,
    key: &str,
    default_value: T,
    parse_error_prefix: &str,
    read_error_prefix: &str,
) -> CommandResult<T>
where
    T: DeserializeOwned,
{
    match db_service.app_settings.get(key) {
        Ok(Some(raw)) => match serde_json::from_str::<T>(&raw) {
            Ok(value) => Ok(value),
            Err(err) => Err(ApiError::new("USER_SETTING_PARSE_FAILED", format!("{parse_error_prefix}: {err}"))),
        },
        Ok(None) => Ok(default_value),
        Err(err) => Err(ApiError::new("DB_READ_FAILED", format!("{read_error_prefix}: {err}"))),
    }
}

/// 获取主站服务。
pub(crate) async fn get_master_service(
    station_manager: &Arc<StationManager>,
    station_id: &str,
) -> Option<Arc<MasterService>> {
    station_manager.get_master_service(station_id).await
}

/// 获取从站服务。
pub(crate) async fn get_slave_service(
    station_manager: &Arc<StationManager>,
    station_id: &str,
) -> Option<Arc<SlaveService>> {
    station_manager.get_slave_service(station_id).await
}

/// 创建"主站不存在"错误响应
pub(crate) fn master_station_not_found() -> ApiError {
    ApiError::new("STATION_NOT_FOUND", "主站不存在")
}

/// 创建"从站不存在"错误响应
pub(crate) fn slave_station_not_found() -> ApiError {
    ApiError::new("STATION_NOT_FOUND", "从站不存在")
}
