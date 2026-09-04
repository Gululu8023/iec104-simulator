use std::{path::PathBuf, sync::Arc};

use log::{info, warn};
use tauri::Manager;

use crate::{
    db::{DatabaseManager, DatabaseService},
    errors::AppResult,
    services::{RuntimeEventSink, StationManager},
    utils::logger::{LogLevel, LoggerConfig, LoggerGuard, init_logger, set_log_level},
};

const DATABASE_FILE_NAME: &str = "iec104_simulator.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppRole {
    Master,
    Slave,
}

impl AppRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Master => "master",
            Self::Slave => "slave",
        }
    }
}

#[derive(Debug)]
struct RuntimePaths {
    database: PathBuf,
    log_dir: PathBuf,
    storage: &'static str,
}

impl RuntimePaths {
    fn executable_relative(role: AppRole) -> Option<Self> {
        let executable_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        Some(Self {
            database: executable_dir.join("data").join(role.as_str()).join(DATABASE_FILE_NAME),
            log_dir: executable_dir.join("logs").join(role.as_str()),
            storage: "executable-relative",
        })
    }

    fn tauri(app: &tauri::App, role: AppRole) -> tauri::Result<Self> {
        Ok(Self {
            database: app.path().app_local_data_dir()?.join("data").join(role.as_str()).join(DATABASE_FILE_NAME),
            log_dir: app.path().app_log_dir()?.join(role.as_str()),
            storage: "tauri",
        })
    }

    fn prepare(&self) -> std::io::Result<()> {
        let database_dir = self.database.parent().expect("database path must have a parent directory");
        std::fs::create_dir_all(database_dir)?;
        std::fs::create_dir_all(&self.log_dir)
    }
}

fn initialize_storage(paths: RuntimePaths) -> AppResult<(RuntimePaths, DatabaseManager, LoggerGuard)> {
    paths.prepare()?;
    let database = DatabaseManager::new(&paths.database)?;
    let logger = init_logger(&LoggerConfig::with_file(LogLevel::Info, &paths.log_dir))?;
    Ok((paths, database, logger))
}

fn read_log_level(database: &DatabaseService) -> LogLevel {
    match database.app_settings.get("system.log_level") {
        Ok(Some(raw)) => {
            let value = serde_json::from_str::<String>(&raw).unwrap_or_else(|_| raw.trim().to_string());
            LogLevel::parse(&value)
        }
        Ok(None) => {
            if let Err(error) = database.app_settings.set("system.log_level", "\"info\"", "system") {
                warn!("写入默认日志级别失败: {error}");
            }
            LogLevel::Info
        }
        Err(error) => {
            warn!("读取日志级别失败，使用info: {error}");
            LogLevel::Info
        }
    }
}

pub fn setup_app(app: &mut tauri::App, role: AppRole) -> Result<(), Box<dyn std::error::Error>> {
    let initialized = RuntimePaths::executable_relative(role).map(initialize_storage).transpose();

    let (paths, database_manager, logger_guard) = match initialized {
        Ok(Some(initialized)) => initialized,
        Ok(None) => initialize_storage(RuntimePaths::tauri(app, role)?)?,
        Err(error) => {
            eprintln!("初始化exe同级数据目录失败，回退到系统目录: {error}");
            initialize_storage(RuntimePaths::tauri(app, role)?)?
        }
    };

    database_manager.migrate()?;
    let database = Arc::new(DatabaseService::new(database_manager.get_connection()));
    set_log_level(read_log_level(&database))?;

    let events = Arc::new(RuntimeEventSink::new(app.handle().clone()));
    let station_manager = Arc::new(StationManager::new(Arc::clone(&database), events));

    app.manage(logger_guard);
    app.manage(Arc::clone(&database));
    app.manage(station_manager);

    info!("IEC104 {}模拟器启动", role.as_str());
    info!("存储模式: {}", paths.storage);
    info!("数据库路径: {}", paths.database.display());
    info!("日志目录: {}", paths.log_dir.display());
    info!("Tauri应用初始化完成");
    Ok(())
}
