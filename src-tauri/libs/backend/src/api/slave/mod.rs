//! 从站 API 模块。
//!
//! 本模块提供从站相关的 Tauri 命令接口,包括:
//!
//! - **站点管理**:创建、更新从站及其子从站配置
//! - **运行时控制**:启动、停止从站,查询连接状态
//! - **数据点管理**:查询、更新数据点值
//! - **策略配置**:链路参数、冗余模式、命令不匹配策略
//! - **模拟功能**:持续模拟、选点波形、场景注入
//! - **抓包导出**:导出 PCAP 格式的网络数据包

pub(crate) mod shared;

pub mod capture;
pub mod policy;
pub mod runtime;
pub mod simulation;
pub mod station;

pub use capture::export_slave_capture;
pub use policy::{
    get_slave_command_mismatch_policy, get_slave_ioa_display_format, get_slave_link_params, get_slave_redundancy_mode,
    get_slave_station_policy_defaults, update_slave_command_mismatch_policy, update_slave_ioa_display_format,
    update_slave_link_params, update_slave_redundancy_mode, update_slave_station_policy_defaults,
};
pub use runtime::{
    bulk_update_slave_data_points, clear_slave_soe_events, get_slave_connections, get_slave_messages,
    get_slave_soe_events, get_slave_station_time_offset, set_slave_message_tracking, start_slave_station,
    stop_slave_station, sync_slave_data_points, update_slave_data_point,
};
pub use simulation::{
    force_upload_slave_data, get_slave_simulation_profile, simulate_slave_data_changes,
    start_slave_selected_point_simulation, start_slave_simulation, stop_slave_selected_point_simulation,
    stop_slave_simulation, trigger_slave_scenario, update_slave_initialization_profile,
    update_slave_simulation_profile,
};
pub use station::{
    create_slave_station, create_station_slave, delete_station_slave, get_slave_station_config,
    get_station_slave_asdu_aliases, get_station_slave_point_defs, list_station_slaves,
    replace_station_slave_point_defs, update_slave_station_config, update_station_slave,
    upsert_station_slave_asdu_alias,
};
