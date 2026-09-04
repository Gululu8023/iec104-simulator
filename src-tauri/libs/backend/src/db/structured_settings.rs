use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use rusqlite::{Connection, OptionalExtension, Result as SqlResult, params};

#[derive(Debug, Clone, Default)]
pub struct SlaveStationProfile {
    pub link_params: Option<String>,
    pub initialization_profile: Option<String>,
    pub simulation_profile: Option<String>,
    pub redundancy_mode: Option<String>,
}

pub struct SlaveStationProfileOperations {
    conn: Arc<Mutex<Connection>>,
}

impl SlaveStationProfileOperations {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get(&self, station_id: i64) -> SqlResult<Option<SlaveStationProfile>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT link_params, initialization_profile, simulation_profile, redundancy_mode
             FROM slave_station_profiles WHERE station_id = ?1",
            [station_id],
            |row| {
                Ok(SlaveStationProfile {
                    link_params: row.get(0)?,
                    initialization_profile: row.get(1)?,
                    simulation_profile: row.get(2)?,
                    redundancy_mode: row.get(3)?,
                })
            },
        )
        .optional()
    }

    fn set_field(&self, station_id: i64, column: &str, value: Option<&str>) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let sql = match column {
            "link_params" => {
                "INSERT INTO slave_station_profiles (station_id, link_params, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP) ON CONFLICT(station_id) DO UPDATE SET link_params = excluded.link_params, updated_at = excluded.updated_at"
            }
            "initialization_profile" => {
                "INSERT INTO slave_station_profiles (station_id, initialization_profile, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP) ON CONFLICT(station_id) DO UPDATE SET initialization_profile = excluded.initialization_profile, updated_at = excluded.updated_at"
            }
            "simulation_profile" => {
                "INSERT INTO slave_station_profiles (station_id, simulation_profile, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP) ON CONFLICT(station_id) DO UPDATE SET simulation_profile = excluded.simulation_profile, updated_at = excluded.updated_at"
            }
            "redundancy_mode" => {
                "INSERT INTO slave_station_profiles (station_id, redundancy_mode, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP) ON CONFLICT(station_id) DO UPDATE SET redundancy_mode = excluded.redundancy_mode, updated_at = excluded.updated_at"
            }
            _ => unreachable!("profile column is fixed by repository methods"),
        };
        conn.execute(sql, params![station_id, value])?;
        Ok(())
    }

    pub fn set_link_params(&self, station_id: i64, value: Option<&str>) -> SqlResult<()> {
        self.set_field(station_id, "link_params", value)
    }

    pub fn set_initialization_profile(&self, station_id: i64, value: Option<&str>) -> SqlResult<()> {
        self.set_field(station_id, "initialization_profile", value)
    }

    pub fn set_simulation_profile(&self, station_id: i64, value: Option<&str>) -> SqlResult<()> {
        self.set_field(station_id, "simulation_profile", value)
    }

    pub fn set_redundancy_mode(&self, station_id: i64, value: Option<&str>) -> SqlResult<()> {
        self.set_field(station_id, "redundancy_mode", value)
    }
}

struct AsduAliasOperations {
    conn: Arc<Mutex<Connection>>,
    table: &'static str,
}

impl AsduAliasOperations {
    fn get_all(&self, slave_id: i64) -> SqlResult<HashMap<String, String>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("SELECT type_name, alias FROM {} WHERE slave_id = ?1", self.table);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([slave_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        rows.collect()
    }

    fn replace_all(&self, slave_id: i64, aliases: &HashMap<String, String>) -> SqlResult<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(&format!("DELETE FROM {} WHERE slave_id = ?1", self.table), [slave_id])?;
        let sql = format!("INSERT INTO {} (slave_id, type_name, alias) VALUES (?1, ?2, ?3)", self.table);
        for (type_name, alias) in aliases {
            tx.execute(&sql, params![slave_id, type_name, alias])?;
        }
        tx.commit()?;
        Ok(aliases.len())
    }
}

macro_rules! alias_repository {
    ($name:ident, $table:literal) => {
        pub struct $name(AsduAliasOperations);
        impl $name {
            pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
                Self(AsduAliasOperations { conn, table: $table })
            }
            pub fn get_all(&self, slave_id: i64) -> SqlResult<HashMap<String, String>> {
                self.0.get_all(slave_id)
            }
            pub fn replace_all(&self, slave_id: i64, aliases: &HashMap<String, String>) -> SqlResult<usize> {
                self.0.replace_all(slave_id, aliases)
            }
        }
    };
}

alias_repository!(MasterSlaveAsduAliasOperations, "master_slave_asdu_aliases");
alias_repository!(SlaveDeviceAsduAliasOperations, "slave_device_asdu_aliases");
