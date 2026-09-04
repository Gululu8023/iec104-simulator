//! 数据库 CRUD 操作模块。
//!
//! 本模块提供所有数据库表的增删改查操作,采用 Repository 模式封装 SQL 逻辑,包括:
//!
//! - **StationOperations**:站点配置的 CRUD
//! - **MasterLinkOperations**:链路连接的 CRUD
//! - **MasterSlaveOperations**:链路从站的 CRUD
//! - **MasterPointOperations**:链路从站点表的 CRUD
//! - **SlaveDeviceOperations**:从站子从站的 CRUD
//! - **SlavePointOperations**:从站子从站点表的 CRUD
//! - **AppSettingOperations**:用户设置的 CRUD
//!
//! # 设计特点
//!
//! - **事务支持**:批量操作使用事务保证原子性
//! - **级联删除**:利用外键约束自动清理关联数据
//! - **时间戳自动更新**:created_at/updated_at 自动维护
//! - **错误处理**:返回 rusqlite::Result 便于错误传播

use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::{Connection, Result as SqlResult, params};

use super::models::*;

/// 站点操作
pub struct StationOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl StationOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// 创建站点
    pub fn create(&self, station: &Station) -> SqlResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO stations (name, station_type, description, host, port, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                station.name,
                station.station_type.to_string(),
                station.description,
                station.host,
                station.port as i64,
                station.enabled,
                now,
                now
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn create_slave_with_default_device(
        &self,
        station: &Station,
        device: &SlaveDevice,
        points: &[SlavePoint],
    ) -> SqlResult<(i64, i64)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO stations (name, station_type, description, host, port, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                station.name,
                station.station_type.to_string(),
                station.description,
                station.host,
                station.port as i64,
                station.enabled,
                now,
                now
            ],
        )?;
        let station_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO slave_devices (station_id, name, common_address, station_policy_override, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![station_id, device.name, device.common_address as i64, device.station_policy_override, device.enabled, now, now],
        )?;
        let device_id = tx.last_insert_rowid();
        for point in points {
            tx.execute(
                "INSERT INTO slave_points (slave_station_id, address, name, type_id, data_type, unit,
                 description, min_value, max_value, default_value, value, control_ioa, gi_group, counter_group,
                 is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    device_id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.unit,
                    point.description,
                    point.min_value,
                    point.max_value,
                    point.default_value,
                    point.value,
                    point.control_ioa.map(|value| value as i64),
                    point.gi_group.map(i64::from),
                    point.counter_group.map(i64::from),
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok((station_id, device_id))
    }

    /// 获取所有站点
    pub fn get_all(&self) -> SqlResult<Vec<Station>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, station_type, description, host, port, enabled, created_at, updated_at
             FROM stations ORDER BY created_at DESC",
        )?;

        let station_iter = stmt.query_map([], |row| Station::from_row(row))?;

        let mut stations = Vec::new();
        for station in station_iter {
            stations.push(station?);
        }

        Ok(stations)
    }

    /// 根据ID获取站点
    pub fn get_by_id(&self, id: i64) -> SqlResult<Option<Station>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, station_type, description, host, port, enabled, created_at, updated_at
             FROM stations WHERE id = ?1",
        )?;

        let mut station_iter = stmt.query_map([id], |row| Station::from_row(row))?;

        match station_iter.next() {
            Some(station) => Ok(Some(station?)),
            None => Ok(None),
        }
    }

    /// 根据名称获取站点
    pub fn get_by_name(&self, name: &str) -> SqlResult<Option<Station>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, station_type, description, host, port, enabled, created_at, updated_at
             FROM stations WHERE name = ?1",
        )?;

        let mut station_iter = stmt.query_map([name], |row| Station::from_row(row))?;

        match station_iter.next() {
            Some(station) => Ok(Some(station?)),
            None => Ok(None),
        }
    }

    /// 更新站点
    pub fn update(&self, id: i64, station: &Station) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE stations SET name = ?1, station_type = ?2, description = ?3, host = ?4,
             port = ?5, enabled = ?6, updated_at = ?7 WHERE id = ?8",
            params![
                station.name,
                station.station_type.to_string(),
                station.description,
                station.host,
                station.port as i64,
                station.enabled,
                now,
                id
            ],
        )
    }

    /// 删除站点
    pub fn delete(&self, id: i64) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM stations WHERE id = ?1", [id])
    }

    /// 根据类型获取站点
    pub fn get_by_type(&self, station_type: &StationType) -> SqlResult<Vec<Station>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, station_type, description, host, port, enabled, created_at, updated_at
             FROM stations WHERE station_type = ?1 ORDER BY created_at DESC",
        )?;

        let station_iter = stmt.query_map([station_type.to_string()], |row| Station::from_row(row))?;

        let mut stations = Vec::new();
        for station in station_iter {
            stations.push(station?);
        }

        Ok(stations)
    }
}

/// 连接操作
pub struct MasterLinkOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl MasterLinkOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// 创建连接
    pub fn create(&self, connection: &super::models::MasterLink) -> SqlResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let params_json = serde_json::to_string(&connection.connection_params)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        conn.execute(
            "INSERT INTO master_links (station_id, name, protocol_type, connection_params, is_active,
             auto_connect, auto_start_data_transfer, auto_gi, retry_count, retry_interval, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                connection.station_id,
                connection.name,
                connection.protocol_type.to_string(),
                params_json,
                connection.is_active,
                connection.auto_connect,
                connection.auto_start_data_transfer,
                connection.auto_gi,
                connection.retry_count as i64,
                connection.retry_interval as i64,
                now,
                now
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// 获取站点的所有连接
    pub fn get_by_station(&self, station_id: i64) -> SqlResult<Vec<super::models::MasterLink>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, station_id, name, protocol_type, connection_params, is_active, 
             auto_connect, auto_start_data_transfer, auto_gi, retry_count, retry_interval, created_at, updated_at
             FROM master_links WHERE station_id = ?1 ORDER BY created_at DESC",
        )?;

        let connection_iter = stmt.query_map([station_id], |row| super::models::MasterLink::from_row(row))?;

        let mut master_links = Vec::new();
        for connection in connection_iter {
            master_links.push(connection?);
        }

        Ok(master_links)
    }

    /// 根据ID获取连接
    pub fn get_by_id(&self, id: i64) -> SqlResult<Option<super::models::MasterLink>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, station_id, name, protocol_type, connection_params, is_active, 
             auto_connect, auto_start_data_transfer, auto_gi, retry_count, retry_interval, created_at, updated_at
             FROM master_links WHERE id = ?1",
        )?;

        let mut connection_iter = stmt.query_map([id], |row| super::models::MasterLink::from_row(row))?;

        match connection_iter.next() {
            Some(connection) => Ok(Some(connection?)),
            None => Ok(None),
        }
    }

    /// 更新连接
    pub fn update(&self, id: i64, connection: &super::models::MasterLink) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let params_json = serde_json::to_string(&connection.connection_params)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        conn.execute(
            "UPDATE master_links SET name = ?1, protocol_type = ?2, connection_params = ?3,
             is_active = ?4, auto_connect = ?5, auto_start_data_transfer = ?6, auto_gi = ?7, retry_count = ?8, retry_interval = ?9, updated_at = ?10 
             WHERE id = ?11",
            params![
                connection.name,
                connection.protocol_type.to_string(),
                params_json,
                connection.is_active,
                connection.auto_connect,
                connection.auto_start_data_transfer,
                connection.auto_gi,
                connection.retry_count as i64,
                connection.retry_interval as i64,
                now,
                id
            ]
        )
    }

    /// 删除连接
    pub fn delete(&self, id: i64) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM master_links WHERE id = ?1", [id])
    }

    /// 设置连接活动状态
    pub fn set_active(&self, id: i64, is_active: bool) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute("UPDATE master_links SET is_active = ?1, updated_at = ?2 WHERE id = ?3", params![
            is_active, now, id
        ])
    }

    /// 获取活动连接
    pub fn get_active(&self) -> SqlResult<Vec<super::models::MasterLink>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, station_id, name, protocol_type, connection_params, is_active, 
             auto_connect, auto_start_data_transfer, auto_gi, retry_count, retry_interval, created_at, updated_at
             FROM master_links WHERE is_active = 1 ORDER BY created_at DESC",
        )?;

        let connection_iter = stmt.query_map([], |row| super::models::MasterLink::from_row(row))?;

        let mut master_links = Vec::new();
        for connection in connection_iter {
            master_links.push(connection?);
        }

        Ok(master_links)
    }
}

/// 用户配置操作
pub struct AppSettingOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl AppSettingOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// 设置配置值
    pub fn set(&self, key: &str, value: &str, category: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value, category, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![key, value, category, now],
        )?;

        Ok(())
    }

    /// 获取配置值
    pub fn get(&self, key: &str) -> SqlResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;

        let mut value_iter = stmt.query_map([key], |row| row.get::<_, String>(0))?;

        match value_iter.next() {
            Some(value) => Ok(Some(value?)),
            None => Ok(None),
        }
    }

    /// 获取分类下的所有配置
    pub fn get_by_category(&self, category: &str) -> SqlResult<Vec<AppSetting>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, key, value, category, description, created_at, updated_at
             FROM app_settings WHERE category = ?1 ORDER BY key",
        )?;

        let setting_iter = stmt.query_map([category], |row| AppSetting::from_row(row))?;

        let mut settings = Vec::new();
        for setting in setting_iter {
            settings.push(setting?);
        }

        Ok(settings)
    }

    /// 删除配置
    pub fn delete(&self, key: &str) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM app_settings WHERE key = ?1", [key])
    }
}

/// 主站从站操作（connection -> coa）
pub struct MasterSlaveOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl MasterSlaveOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn create(&self, slave: &MasterSlave) -> SqlResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO master_slaves (connection_id, name, common_address, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![slave.connection_id, slave.name, slave.common_address as i64, slave.enabled, now, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn create_with_points(
        &self,
        connection_id: i64,
        slave: &MasterSlave,
        points: &[MasterPoint],
    ) -> SqlResult<(i64, usize)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO master_slaves (connection_id, name, common_address, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![connection_id, slave.name, slave.common_address as i64, slave.enabled, now, now],
        )?;
        let slave_id = tx.last_insert_rowid();
        let mut inserted = 0usize;
        for point in points {
            tx.execute(
                "INSERT INTO master_points (connection_slave_id, address, name, type_id, data_type, description,
                 default_value, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    slave_id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.description,
                    point.default_value,
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok((slave_id, inserted))
    }

    pub fn get_by_connection(&self, connection_id: i64) -> SqlResult<Vec<MasterSlave>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, connection_id, name, common_address, enabled, created_at, updated_at
             FROM master_slaves WHERE connection_id = ?1 ORDER BY common_address ASC",
        )?;
        let iter = stmt.query_map([connection_id], MasterSlave::from_row)?;
        let mut rows = Vec::new();
        for item in iter {
            rows.push(item?);
        }
        Ok(rows)
    }

    pub fn get_by_id(&self, id: i64) -> SqlResult<Option<MasterSlave>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, connection_id, name, common_address, enabled, created_at, updated_at
             FROM master_slaves WHERE id = ?1",
        )?;
        let mut iter = stmt.query_map([id], MasterSlave::from_row)?;
        match iter.next() {
            Some(item) => Ok(Some(item?)),
            None => Ok(None),
        }
    }

    pub fn update(&self, id: i64, slave: &MasterSlave) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE master_slaves SET name = ?1, common_address = ?2, enabled = ?3, updated_at = ?4
             WHERE id = ?5",
            params![slave.name, slave.common_address as i64, slave.enabled, now, id],
        )
    }

    pub fn update_with_points(
        &self,
        id: i64,
        slave: &MasterSlave,
        points: &[MasterPoint],
    ) -> SqlResult<(usize, usize)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let updated = tx.execute(
            "UPDATE master_slaves SET name = ?1, common_address = ?2, enabled = ?3, updated_at = ?4
             WHERE id = ?5",
            params![slave.name, slave.common_address as i64, slave.enabled, now, id],
        )?;
        if updated == 0 {
            return Ok((0, 0));
        }
        tx.execute("DELETE FROM master_points WHERE connection_slave_id = ?1", params![id])?;
        let mut inserted = 0usize;
        for point in points {
            tx.execute(
                "INSERT INTO master_points (connection_slave_id, address, name, type_id, data_type, description,
                 default_value, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.description,
                    point.default_value,
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok((updated, inserted))
    }

    pub fn delete(&self, id: i64) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM master_slaves WHERE id = ?1", [id])
    }
}

/// 主站从站点表操作
pub struct MasterPointOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl MasterPointOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_by_connection_slave(&self, connection_slave_id: i64) -> SqlResult<Vec<MasterPoint>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, connection_slave_id, address, name, type_id, data_type, description,
                    default_value, is_enabled, created_at, updated_at
             FROM master_points WHERE connection_slave_id = ?1 ORDER BY address ASC",
        )?;
        let iter = stmt.query_map([connection_slave_id], MasterPoint::from_row)?;
        let mut rows = Vec::new();
        for item in iter {
            rows.push(item?);
        }
        Ok(rows)
    }

    pub fn replace_by_connection_slave(&self, connection_slave_id: i64, points: &[MasterPoint]) -> SqlResult<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM master_points WHERE connection_slave_id = ?1", params![connection_slave_id])?;
        let now = Utc::now().to_rfc3339();
        let mut inserted = 0usize;
        for point in points {
            tx.execute(
                "INSERT INTO master_points (connection_slave_id, address, name, type_id, data_type, description,
                 default_value, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    connection_slave_id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.description,
                    point.default_value,
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn upsert(&self, point: &MasterPoint) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO master_points (connection_slave_id, address, name, type_id, data_type, description,
             default_value, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(connection_slave_id, address) DO UPDATE SET
               name = excluded.name,
               type_id = excluded.type_id,
               data_type = excluded.data_type,
               description = excluded.description,
               default_value = excluded.default_value,
               is_enabled = excluded.is_enabled,
               updated_at = excluded.updated_at",
            params![
                point.connection_slave_id,
                point.address as i64,
                point.name,
                point.type_id as i64,
                point.data_type.to_string(),
                point.description,
                point.default_value,
                point.is_enabled,
                now,
                now
            ],
        )
    }
}

/// 从站子站操作（station/connection -> coa）
pub struct SlaveDeviceOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl SlaveDeviceOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn create(&self, station: &SlaveDevice) -> SqlResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO slave_devices (station_id, name, common_address, station_policy_override, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![station.station_id, station.name, station.common_address as i64, station.station_policy_override, station.enabled, now, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn create_with_points(
        &self,
        station_id: i64,
        slave_station: &SlaveDevice,
        points: &[SlavePoint],
    ) -> SqlResult<(i64, usize)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO slave_devices (station_id, name, common_address, station_policy_override, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                station_id,
                slave_station.name,
                slave_station.common_address as i64,
                slave_station.station_policy_override,
                slave_station.enabled,
                now,
                now
            ],
        )?;
        let slave_station_id = tx.last_insert_rowid();
        let mut inserted = 0usize;
        for point in points {
            tx.execute(
                "INSERT INTO slave_points (slave_station_id, address, name, type_id, data_type, unit,
                 description, min_value, max_value, default_value, value, control_ioa, gi_group, counter_group, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    slave_station_id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.unit,
                    point.description,
                    point.min_value,
                    point.max_value,
                    point.default_value,
                    point.value,
                    point.control_ioa.map(|v| v as i64),
                    point.gi_group.map(i64::from),
                    point.counter_group.map(i64::from),
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok((slave_station_id, inserted))
    }

    pub fn get_by_station(&self, station_id: i64) -> SqlResult<Vec<SlaveDevice>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, station_id, name, common_address, station_policy_override, enabled, created_at, updated_at
             FROM slave_devices WHERE station_id = ?1 ORDER BY common_address ASC",
        )?;
        let iter = stmt.query_map([station_id], SlaveDevice::from_row)?;
        let mut rows = Vec::new();
        for item in iter {
            rows.push(item?);
        }
        Ok(rows)
    }

    pub fn get_by_id(&self, id: i64) -> SqlResult<Option<SlaveDevice>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, station_id, name, common_address, station_policy_override, enabled, created_at, updated_at
             FROM slave_devices WHERE id = ?1",
        )?;
        let mut iter = stmt.query_map([id], SlaveDevice::from_row)?;
        match iter.next() {
            Some(item) => Ok(Some(item?)),
            None => Ok(None),
        }
    }

    pub fn update(&self, id: i64, slave_station: &SlaveDevice) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE slave_devices SET name = ?1, common_address = ?2, station_policy_override = ?3, enabled = ?4, updated_at = ?5
             WHERE id = ?6",
            params![slave_station.name, slave_station.common_address as i64, slave_station.station_policy_override, slave_station.enabled, now, id],
        )
    }

    pub fn update_with_points(
        &self,
        id: i64,
        slave_station: &SlaveDevice,
        points: &[SlavePoint],
    ) -> SqlResult<(usize, usize)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let updated = tx.execute(
            "UPDATE slave_devices SET name = ?1, common_address = ?2, station_policy_override = ?3, enabled = ?4, updated_at = ?5
             WHERE id = ?6",
            params![slave_station.name, slave_station.common_address as i64, slave_station.station_policy_override, slave_station.enabled, now, id],
        )?;
        if updated == 0 {
            return Ok((0, 0));
        }
        tx.execute("DELETE FROM slave_points WHERE slave_station_id = ?1", params![id])?;
        let mut inserted = 0usize;
        for point in points {
            tx.execute(
                "INSERT INTO slave_points (slave_station_id, address, name, type_id, data_type, unit,
                 description, min_value, max_value, default_value, value, control_ioa, gi_group, counter_group, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.unit,
                    point.description,
                    point.min_value,
                    point.max_value,
                    point.default_value,
                    point.value,
                    point.control_ioa.map(|v| v as i64),
                    point.gi_group.map(i64::from),
                    point.counter_group.map(i64::from),
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok((updated, inserted))
    }

    pub fn delete(&self, id: i64) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM slave_devices WHERE id = ?1", [id])
    }
}

/// 从站子站点表操作
pub struct SlavePointOperations {
    /// 数据库连接
    conn: Arc<Mutex<Connection>>,
}

impl SlavePointOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_by_slave_station(&self, slave_station_id: i64) -> SqlResult<Vec<SlavePoint>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, slave_station_id, address, name, type_id, data_type, unit, description,
                    min_value, max_value, default_value, value, control_ioa, gi_group, counter_group, is_enabled, created_at, updated_at
             FROM slave_points WHERE slave_station_id = ?1 ORDER BY address ASC",
        )?;
        let iter = stmt.query_map([slave_station_id], SlavePoint::from_row)?;
        let mut rows = Vec::new();
        for item in iter {
            rows.push(item?);
        }
        Ok(rows)
    }

    /// 批量更新从站运行时点值，并返回实际更新的记录数。
    pub fn update_runtime_values(&self, station_id: i64, updates: &[(u16, u32, String)]) -> SqlResult<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let mut updated = 0usize;
        {
            let mut stmt = tx.prepare(
                "UPDATE slave_points
                 SET value = ?1, updated_at = ?2
                 WHERE address = ?3
                   AND slave_station_id = (
                       SELECT id FROM slave_devices
                       WHERE station_id = ?4 AND common_address = ?5
                   )",
            )?;
            for (common_address, address, value) in updates {
                updated +=
                    stmt.execute(params![value, now, i64::from(*address), station_id, i64::from(*common_address)])?;
            }
        }
        tx.commit()?;
        Ok(updated)
    }

    pub fn replace_by_slave_station(&self, slave_station_id: i64, points: &[SlavePoint]) -> SqlResult<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM slave_points WHERE slave_station_id = ?1", params![slave_station_id])?;
        let now = Utc::now().to_rfc3339();
        let mut inserted = 0usize;
        for point in points {
            tx.execute(
                "INSERT INTO slave_points (slave_station_id, address, name, type_id, data_type, unit,
                 description, min_value, max_value, default_value, value, control_ioa, gi_group, counter_group, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    slave_station_id,
                    point.address as i64,
                    point.name,
                    point.type_id as i64,
                    point.data_type.to_string(),
                    point.unit,
                    point.description,
                    point.min_value,
                    point.max_value,
                    point.default_value,
                    point.value,
                    point.control_ioa.map(|v| v as i64),
                    point.gi_group.map(i64::from),
                    point.counter_group.map(i64::from),
                    point.is_enabled,
                    now,
                    now
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn upsert(&self, point: &SlavePoint) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO slave_points (slave_station_id, address, name, type_id, data_type, unit,
             description, min_value, max_value, default_value, value, control_ioa, gi_group, counter_group, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(slave_station_id, address) DO UPDATE SET
               name = excluded.name,
               type_id = excluded.type_id,
               data_type = excluded.data_type,
               unit = excluded.unit,
               description = excluded.description,
               min_value = excluded.min_value,
               max_value = excluded.max_value,
               default_value = excluded.default_value,
               value = excluded.value,
               control_ioa = excluded.control_ioa,
               gi_group = excluded.gi_group,
               counter_group = excluded.counter_group,
               is_enabled = excluded.is_enabled,
               updated_at = excluded.updated_at",
            params![
                point.slave_station_id,
                point.address as i64,
                point.name,
                point.type_id as i64,
                point.data_type.to_string(),
                point.unit,
                point.description,
                point.min_value,
                point.max_value,
                point.default_value,
                point.value,
                point.control_ioa.map(|v| v as i64),
                point.gi_group.map(i64::from),
                point.counter_group.map(i64::from),
                point.is_enabled,
                now,
                now
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_runtime_value_update_commits_known_points_and_counts_missing_points() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE slave_devices (
                    id INTEGER PRIMARY KEY,
                    station_id INTEGER NOT NULL,
                    common_address INTEGER NOT NULL
                 );
                 CREATE TABLE slave_points (
                    id INTEGER PRIMARY KEY,
                    slave_station_id INTEGER NOT NULL,
                    address INTEGER NOT NULL,
                    value TEXT,
                    updated_at TEXT
                 );
                 INSERT INTO slave_devices (id, station_id, common_address) VALUES (10, 1, 1), (20, 1, 2);
                 INSERT INTO slave_points (id, slave_station_id, address, value) VALUES
                    (100, 10, 1001, '0'),
                    (200, 20, 2001, '0');",
            )
            .expect("schema and fixtures");
        let operations = SlavePointOperations::new(Arc::new(Mutex::new(connection)));

        let updated = operations
            .update_runtime_values(1, &[
                (1, 1001, "11".to_string()),
                (2, 2001, "22".to_string()),
                (2, 9999, "99".to_string()),
            ])
            .expect("batch update");

        assert_eq!(updated, 2);
        let connection = operations.conn.lock().expect("connection lock");
        let values = connection
            .prepare("SELECT value FROM slave_points ORDER BY id")
            .expect("query")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("rows")
            .collect::<SqlResult<Vec<_>>>()
            .expect("values");
        assert_eq!(values, vec!["11", "22"]);
    }
}
