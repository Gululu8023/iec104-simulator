//! 站点配置转换工厂模块
//!
//! 提供站点配置在不同层次之间的转换功能，包括 API 请求、运行时配置、持久化配置之间的相互转换。

use std::net::SocketAddr;

use crate::{
    api::types::{CreateStationRequest, StationConfigResponse, StationProtocol},
    core::types::{ProtocolType, StationConfig, StationType},
};

/// 从 API 请求构建运行时站点配置
pub fn build_station_config(request: CreateStationRequest, station_type: StationType) -> Result<StationConfig, String> {
    // 验证协议类型
    if request.protocol != StationProtocol::Iec104 {
        return Err("仅支持iec104".to_string());
    }

    // 确定期望的站点类型字符串
    let expected_station_type = match station_type {
        StationType::Master => "master",
        StationType::Slave => "slave",
    };

    // 验证站点类型是否匹配
    if !request.station_type.trim().eq_ignore_ascii_case(expected_station_type) {
        return Err(format!("station_type 必须为 {}", expected_station_type));
    }

    // 解析网络地址
    let address = parse_socket_addr(&request.host, request.port)?;

    // 构建运行时配置
    Ok(StationConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: request.name,
        protocol: ProtocolType::Iec104,
        station_type,
        address,
        common_address: request.common_address,
        enabled: request.enabled,
    })
}

/// 将运行时站点配置转换为 API 响应
pub fn station_config_to_response(config: &StationConfig) -> StationConfigResponse {
    StationConfigResponse {
        station_id: config.id.clone(),
        name: config.name.clone(),
        protocol: StationProtocol::Iec104,
        station_type: match config.station_type {
            StationType::Master => "master".to_string(),
            StationType::Slave => "slave".to_string(),
        },
        host: config.address.ip().to_string(),
        port: config.address.port(),
        common_address: config.common_address,
        enabled: config.enabled,
    }
}

/// 解析网络地址字符串为 SocketAddr
fn parse_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    format!("{host}:{port}").parse::<SocketAddr>().map_err(|e| format!("无效的地址: {}", e))
}
