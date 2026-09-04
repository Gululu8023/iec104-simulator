//! API 层数据映射工具模块
//!
//! 提供数据类型转换和映射功能，包括类型推断、站点配置转换、抓包元数据构建等。

use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::{
    api::types::ExportCaptureResponse,
    core::{
        iec104_registry::{ValueModel, lookup_capability},
        types::{ProtocolType, StationConfig, StationType},
    },
    db::{DataType as DbDataType, Station as DbStation, StationType as DbStationType},
    utils::pcap::CaptureFormat,
};

/// 根据 IEC104 类型 ID 推断数据库数据类型
pub fn infer_db_data_type(type_id: u8) -> DbDataType {
    let capability = lookup_capability(type_id).expect("validated point Type must exist in IEC104 registry");
    match capability.value_model {
        ValueModel::Boolean => DbDataType::SinglePoint,
        ValueModel::DoublePoint => DbDataType::DoublePoint,
        ValueModel::StepPosition => DbDataType::StepPosition,
        ValueModel::Bitstring32 => DbDataType::Bitstring32,
        ValueModel::Normalized => DbDataType::Normalized,
        ValueModel::ScaledI16 => DbDataType::Scaled,
        ValueModel::Float32 => DbDataType::Float,
        ValueModel::Counter => DbDataType::IntegratedTotal,
        ValueModel::None | ValueModel::Structured => {
            panic!("Type {type_id} has no database value model")
        }
    }
}

/// 将数据库站点记录转换为运行时站点配置
pub fn db_station_to_station_config(
    station: &DbStation,
    expected: StationType,
    common_address: u16,
) -> Result<StationConfig, String> {
    let Some(id) = station.id else {
        return Err("站点ID缺失".to_string());
    };

    let actual = match station.station_type {
        DbStationType::Master => StationType::Master,
        DbStationType::Slave => StationType::Slave,
    };

    if actual != expected {
        return Err(format!("站点类型不匹配: expected={:?}, actual={:?}", expected, actual));
    }

    let address = format!("{}:{}", station.host, station.port)
        .parse::<std::net::SocketAddr>()
        .map_err(|e| format!("无效的地址: {}", e))?;

    Ok(StationConfig {
        id: id.to_string(),
        name: station.name.clone(),
        protocol: ProtocolType::Iec104,
        station_type: actual,
        address,
        common_address,
        enabled: station.enabled,
    })
}

/// 抓包导出元数据
#[derive(Debug, Clone)]
pub struct CaptureExportMeta {
    /// 抓包文件格式
    pub format: CaptureFormat,
    /// 生成的文件名
    pub file_name: String,
    /// MIME 类型
    pub mime_type: &'static str,
}

/// 构建抓包导出元数据
pub fn build_capture_export_meta(
    station_role: &str,
    station_id: &str,
    format_raw: &str,
    connection_id: Option<&str>,
) -> Result<CaptureExportMeta, String> {
    let format = match format_raw.trim().to_ascii_lowercase().as_str() {
        "pcap" => CaptureFormat::Pcap,
        "pcapng" => CaptureFormat::Pcapng,
        _ => return Err("不支持的格式（仅支持 pcap/pcapng）".to_string()),
    };

    let role = match station_role {
        "master" | "slave" => station_role,
        _ => return Err("不支持的站点角色".to_string()),
    };

    let ext = match format {
        CaptureFormat::Pcap => "pcap",
        CaptureFormat::Pcapng => "pcapng",
    };
    let mime_type = match format {
        CaptureFormat::Pcap => "application/vnd.tcpdump.pcap",
        CaptureFormat::Pcapng => "application/x-pcapng",
    };
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let file_name = match connection_id {
        Some(conn) if !conn.is_empty() => {
            format!("iec104-{role}-{station_id}-{conn}-{ts}.{ext}")
        }
        _ => format!("iec104-{role}-{station_id}-{ts}.{ext}"),
    };

    Ok(CaptureExportMeta { format, file_name, mime_type })
}

pub fn build_capture_export_response(meta: CaptureExportMeta, bytes: &[u8]) -> ExportCaptureResponse {
    ExportCaptureResponse {
        file_name: meta.file_name,
        mime_type: meta.mime_type.to_string(),
        data_base64: STANDARD.encode(bytes),
    }
}
