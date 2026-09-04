//! 主站 API 模块。
//!
//! 本模块提供主站相关的 Tauri 命令接口,包括:
//!
//! - **站点管理**:创建、删除、更新主站及其链路配置
//! - **运行时控制**:启动、停止主站,连接/断开从站
//! - **数据传输**:总召唤、命令下发、数据点查询
//! - **文件传输**:启动、取消、查询文件传输会话
//! - **抓包导出**:导出 PCAP 格式的网络数据包

pub(crate) mod shared;

pub mod capture;
pub mod file_transfer;
pub mod runtime;
pub mod station;

pub use capture::export_master_capture;
pub use file_transfer::{
    cancel_master_file_transfer, get_master_file_transfer_limits, get_master_file_transfer_session,
    start_master_file_transfer,
};
pub use runtime::{
    clear_master_soe_events, connect_link_profile, connect_to_slave, disconnect_from_slave, get_master_connections,
    get_master_messages, get_master_soe_events, send_iec104_command, send_test_frame, set_master_message_tracking,
    start_master_data_transfer, start_master_station, stop_master_data_transfer, stop_master_station,
    sync_master_data_points,
};
pub use station::{
    create_link_profile, create_link_slave, create_master_station, delete_link_profile, delete_link_slave,
    get_link_slave_asdu_aliases, get_link_slave_point_defs, list_link_profiles, list_link_slaves, update_link_profile,
    update_link_slave, upsert_link_slave_asdu_alias,
};
