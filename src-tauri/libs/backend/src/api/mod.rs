//! API 层模块
//!
//! 提供 Tauri 命令接口，是前端与后端通信的桥梁层。
//!
//! # 模块
//!
//! - **types**: API 请求和响应类型
//! - **command_helpers**: 命令处理辅助函数
//! - **common_commands**: 通用命令接口
//! - **mappers**: 数据类型映射器
//! - **master**: 主站命令接口
//! - **slave**: 从站命令接口
//! - **message_parser**: IEC104 报文解析器
//! - **point_table_import**: 点表导入功能
//! - **station_factory**: 站点工厂

pub mod command_helpers;
pub mod common_commands;
pub mod mappers;
pub mod master;
pub mod message_parser;
pub mod point_sync;
pub mod point_table_import;
pub mod slave;
pub mod station_factory;
pub mod types;

pub use types::*;

pub fn master_invoke_handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        master::station::create_master_station,
        master::runtime::start_master_station,
        master::runtime::stop_master_station,
        master::station::create_link_profile,
        master::station::update_link_profile,
        master::station::delete_link_profile,
        master::station::list_link_profiles,
        master::station::create_link_slave,
        master::station::update_link_slave,
        master::station::delete_link_slave,
        master::station::list_link_slaves,
        master::station::get_link_slave_point_defs,
        master::station::get_link_slave_asdu_aliases,
        master::station::upsert_link_slave_asdu_alias,
        master::runtime::connect_link_profile,
        master::runtime::connect_to_slave,
        master::runtime::disconnect_from_slave,
        master::runtime::start_master_data_transfer,
        master::runtime::stop_master_data_transfer,
        master::file_transfer::start_master_file_transfer,
        master::file_transfer::get_master_file_transfer_limits,
        master::file_transfer::get_master_file_transfer_session,
        master::file_transfer::cancel_master_file_transfer,
        master::runtime::send_test_frame,
        master::runtime::send_iec104_command,
        master::runtime::get_master_connections,
        master::runtime::sync_master_data_points,
        master::runtime::get_master_messages,
        master::runtime::set_master_message_tracking,
        master::runtime::get_master_soe_events,
        master::runtime::clear_master_soe_events,
        master::capture::export_master_capture,
        point_table_import::preview_point_table_import,
        point_table_import::apply_point_table_import,
        point_table_import::generate_point_table_csv_template,
        message_parser::parse_message_frame,
        common_commands::list_stations,
        common_commands::delete_station,
        common_commands::get_station_stats,
        common_commands::stop_all_stations,
        common_commands::get_app_info,
        common_commands::get_iec104_capabilities,
        common_commands::get_ui_language,
        common_commands::update_ui_language,
        common_commands::save_export_file,
    ]
}

pub fn slave_invoke_handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        slave::station::create_slave_station,
        slave::station::get_slave_station_config,
        slave::station::update_slave_station_config,
        slave::runtime::start_slave_station,
        slave::runtime::stop_slave_station,
        slave::runtime::get_slave_connections,
        slave::runtime::get_slave_station_time_offset,
        slave::runtime::sync_slave_data_points,
        slave::runtime::update_slave_data_point,
        slave::runtime::bulk_update_slave_data_points,
        slave::runtime::get_slave_soe_events,
        slave::runtime::clear_slave_soe_events,
        slave::station::create_station_slave,
        slave::station::update_station_slave,
        slave::station::delete_station_slave,
        slave::station::list_station_slaves,
        slave::station::get_station_slave_point_defs,
        slave::station::replace_station_slave_point_defs,
        slave::station::get_station_slave_asdu_aliases,
        slave::station::upsert_station_slave_asdu_alias,
        slave::policy::get_slave_link_params,
        slave::policy::get_slave_redundancy_mode,
        slave::policy::get_slave_command_mismatch_policy,
        slave::policy::get_slave_ioa_display_format,
        slave::policy::get_slave_station_policy_defaults,
        slave::policy::update_slave_link_params,
        slave::policy::update_slave_redundancy_mode,
        slave::policy::update_slave_command_mismatch_policy,
        slave::policy::update_slave_ioa_display_format,
        slave::policy::update_slave_station_policy_defaults,
        slave::simulation::simulate_slave_data_changes,
        slave::simulation::force_upload_slave_data,
        slave::simulation::start_slave_simulation,
        slave::simulation::stop_slave_simulation,
        slave::simulation::start_slave_selected_point_simulation,
        slave::simulation::stop_slave_selected_point_simulation,
        slave::simulation::get_slave_simulation_profile,
        slave::simulation::update_slave_simulation_profile,
        slave::simulation::update_slave_initialization_profile,
        slave::simulation::trigger_slave_scenario,
        slave::runtime::get_slave_messages,
        slave::runtime::set_slave_message_tracking,
        slave::capture::export_slave_capture,
        point_table_import::preview_point_table_import,
        point_table_import::apply_point_table_import,
        point_table_import::generate_point_table_csv_template,
        message_parser::parse_message_frame,
        common_commands::list_stations,
        common_commands::delete_station,
        common_commands::get_station_stats,
        common_commands::stop_all_stations,
        common_commands::get_app_info,
        common_commands::get_iec104_capabilities,
        common_commands::get_ui_language,
        common_commands::update_ui_language,
        common_commands::save_export_file,
    ]
}
