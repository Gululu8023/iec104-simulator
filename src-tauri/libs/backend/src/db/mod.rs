//! 数据库持久化层模块。
//!
//! 基于 SQLite 的数据持久化功能，包括连接管理、模型定义、CRUD 操作和 v1 表结构初始化。
//!
//! # 数据表
//!
//! - **stations**: 站点配置（主站/从站）
//! - **master_links**: 主站链路连接配置
//! - **master_slaves**: 链路从站配置
//! - **master_points**: 主站链路从站点表
//! - **slave_devices**: 从站逻辑设备配置
//! - **slave_points**: 从站逻辑设备点表
//! - **slave_station_profiles**: 从站运行配置
//! - **app_settings**: 应用配置（键值对）

pub mod connection;
pub mod models;
pub mod operations;
pub mod structured_settings;

// 重新导出主要类型
use std::sync::{Arc, Mutex};

pub use connection::DatabaseManager;
pub use models::*;
pub use operations::*;
pub use structured_settings::*;

/// 数据库服务 - 提供统一的数据库操作接口
pub struct DatabaseService {
    /// 站点操作
    pub stations: StationOperations,
    /// 连接操作
    pub master_links: MasterLinkOperations,
    /// 用户设置操作
    pub app_settings: AppSettingOperations,
    pub slave_station_profiles: SlaveStationProfileOperations,
    pub master_slave_asdu_aliases: MasterSlaveAsduAliasOperations,
    pub slave_device_asdu_aliases: SlaveDeviceAsduAliasOperations,
    /// 连接从站操作
    pub master_slaves: MasterSlaveOperations,
    /// 连接从站数据点操作
    pub master_points: MasterPointOperations,
    /// 从站子从站操作
    pub slave_devices: SlaveDeviceOperations,
    /// 从站子从站数据点操作
    pub slave_points: SlavePointOperations,
}

impl DatabaseService {
    /// 创建新的数据库服务
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self {
            stations: StationOperations::new(Arc::clone(&conn)),
            master_links: MasterLinkOperations::new(Arc::clone(&conn)),
            app_settings: AppSettingOperations::new(Arc::clone(&conn)),
            slave_station_profiles: SlaveStationProfileOperations::new(Arc::clone(&conn)),
            master_slave_asdu_aliases: MasterSlaveAsduAliasOperations::new(Arc::clone(&conn)),
            slave_device_asdu_aliases: SlaveDeviceAsduAliasOperations::new(Arc::clone(&conn)),
            master_slaves: MasterSlaveOperations::new(Arc::clone(&conn)),
            master_points: MasterPointOperations::new(Arc::clone(&conn)),
            slave_devices: SlaveDeviceOperations::new(Arc::clone(&conn)),
            slave_points: SlavePointOperations::new(Arc::clone(&conn)),
        }
    }
}
