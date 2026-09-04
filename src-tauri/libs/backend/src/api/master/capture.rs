//! 主站抓包导出 API。
//!
//! 提供主站网络数据包的导出功能,支持 PCAP 格式。

use super::shared::*;

#[tauri::command]
pub async fn export_master_capture(
    station_id: String,
    format: String,
    connection_id: Option<String>,
    station_manager: State<'_, Arc<StationManager>>,
) -> CommandResult<ExportCaptureResponse> {
    let meta = match build_capture_export_meta("master", &station_id, &format, connection_id.as_deref()) {
        Ok(meta) => meta,
        Err(msg) => return Err(ApiError::app(msg)),
    };

    match get_master_service(station_manager.inner(), &station_id).await {
        Some(service) => match service.export_capture(connection_id.as_deref(), meta.format).await {
            Ok(bytes) => Ok(build_capture_export_response(meta, &bytes)),
            Err(err) => Err(ApiError::app(err.to_string())),
        },
        None => Err(ApiError::app("主站不存在".to_string())),
    }
}
