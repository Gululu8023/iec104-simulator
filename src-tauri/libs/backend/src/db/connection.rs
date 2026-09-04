use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use rusqlite::Connection;

use crate::errors::{DatabaseError, DatabaseResult};

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_SQL: &str = include_str!("schema.sql");
const REQUIRED_TABLES: &[&str] = &[
    "stations",
    "master_links",
    "app_settings",
    "slave_station_profiles",
    "master_slaves",
    "master_slave_asdu_aliases",
    "master_points",
    "slave_devices",
    "slave_device_asdu_aliases",
    "slave_points",
];

pub struct DatabaseManager {
    connection: Arc<Mutex<Connection>>,
}

impl DatabaseManager {
    pub fn new(db_path: &Path) -> DatabaseResult<Self> {
        log::info!("初始化数据库管理器: path={}", db_path.display());
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        if let Err(err) = conn.pragma_update(None, "journal_mode", "WAL") {
            log::warn!("设置WAL模式失败: {err}");
        }
        if let Err(err) = conn.pragma_update(None, "synchronous", "NORMAL") {
            log::warn!("设置同步模式失败: {err}");
        }
        if let Err(err) = conn.pragma_update(None, "cache_size", -10000) {
            log::warn!("设置数据库缓存失败: {err}");
        }
        Ok(Self { connection: Arc::new(Mutex::new(conn)) })
    }

    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.connection)
    }

    pub fn migrate(&self) -> DatabaseResult<()> {
        let mut conn = self.connection.lock().expect("database connection mutex poisoned");
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let table_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;

        if version == 0 && table_count == 0 {
            let transaction = conn.transaction()?;
            transaction.execute_batch(SCHEMA_SQL)?;
            transaction.commit()?;
            log::info!("已初始化v1数据库结构");
            return Ok(());
        }

        if version != SCHEMA_VERSION {
            return Err(DatabaseError::IncompatibleSchema(format!(
                "expected user_version={SCHEMA_VERSION}, found {version}; pre-v1 databases are not migrated"
            )));
        }

        for table in REQUIRED_TABLES {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(DatabaseError::IncompatibleSchema(format!(
                    "v1 database is missing required table {table}"
                )));
            }
        }

        Ok(())
    }
}
