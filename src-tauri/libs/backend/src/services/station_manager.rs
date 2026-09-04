//! 站点管理服务。
//!
//! 统一管理主站和从站的生命周期。

use std::{collections::HashMap, sync::Arc};

use log::{debug, error, info, warn};
use tokio::sync::RwLock;

use crate::{
    core::{
        master::MasterService,
        slave::SlaveService,
        types::{StationConfig, StationStatus, StationType},
    },
    db::DatabaseService,
    errors::AppResult,
    services::RuntimeEventSink,
};

/// 站点管理器
pub struct StationManager {
    database: Arc<DatabaseService>,
    events: Arc<RuntimeEventSink>,
    /// 主站服务列表
    master_services: Arc<RwLock<HashMap<String, Arc<MasterService>>>>,
    /// 从站服务列表
    slave_services: Arc<RwLock<HashMap<String, Arc<SlaveService>>>>,
}

impl StationManager {
    /// 创建新的站点管理器
    pub fn new(database: Arc<DatabaseService>, events: Arc<RuntimeEventSink>) -> Self {
        Self {
            database,
            events,
            master_services: Arc::new(RwLock::new(HashMap::new())),
            slave_services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建站点。
    ///
    /// 根据配置类型创建主站或从站服务。如果站点已存在则直接返回。
    pub async fn create_station(&self, config: StationConfig) -> AppResult<String> {
        let station_id = config.id.clone();

        match config.station_type {
            StationType::Master => {
                // 幂等性检查：如果主站已存在则直接返回
                if self.master_services.read().await.contains_key(&station_id) {
                    return Ok(station_id);
                }
                let service =
                    Arc::new(MasterService::new(config, Arc::clone(&self.database), Arc::clone(&self.events)));
                self.master_services.write().await.insert(station_id.clone(), service);
                info!("创建主站: {}", station_id);
            }
            StationType::Slave => {
                // 幂等性检查：如果从站已存在则直接返回
                if self.slave_services.read().await.contains_key(&station_id) {
                    return Ok(station_id);
                }
                let service = Arc::new(SlaveService::new(config, Arc::clone(&self.database), Arc::clone(&self.events)));
                self.slave_services.write().await.insert(station_id.clone(), service);
                info!("创建从站: {}", station_id);
            }
        }

        Ok(station_id)
    }

    /// 获取站点配置
    pub async fn get_station_config(&self, station_id: &str) -> AppResult<StationConfig> {
        if let Some(service) = self.master_services.read().await.get(station_id) {
            return Ok(service.get_config().clone());
        }

        if let Some(service) = self.slave_services.read().await.get(station_id) {
            return Ok(service.get_config().clone());
        }

        Err(crate::errors::AppError::Config(crate::errors::ConfigError::StationNotFound(station_id.to_string())))
    }

    /// 更新站点配置（保持 station_id 不变）。
    pub async fn update_station_config(&self, station_id: &str, mut config: StationConfig) -> AppResult<()> {
        // 强制保持 station_id 不变
        config.id = station_id.to_string();

        // === 主站更新分支 ===
        let existing_master = {
            let guard = self.master_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(existing) = existing_master {
            debug!("update_station_config master branch station_id={}", station_id);

            // 类型检查：主站不可更新为从站
            if config.station_type != StationType::Master {
                return Err(crate::errors::AppError::Config(crate::errors::ConfigError::Other(
                    "站点类型不匹配：主站不可更新为从站".to_string(),
                )));
            }

            // 记录运行状态，用于后续恢复
            let was_running = existing.get_status().await == StationStatus::Running;

            // 停止旧服务（释放端口、断开连接等）
            existing.stop().await?;
            debug!("update_station_config master stop done station_id={} was_running={}", station_id, was_running);

            // 创建新服务实例
            let new_service =
                Arc::new(MasterService::new(config, Arc::clone(&self.database), Arc::clone(&self.events)));

            // 如果之前在运行，则启动新服务
            if was_running {
                new_service.start().await?;
            }

            // 替换服务实例
            self.master_services.write().await.insert(station_id.to_string(), new_service);
            debug!("update_station_config master replace service done station_id={}", station_id);
            return Ok(());
        }

        // === 从站更新分支 ===
        let existing_slave = {
            let guard = self.slave_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(existing) = existing_slave {
            debug!("update_station_config slave branch station_id={}", station_id);

            // 类型检查：从站不可更新为主站
            if config.station_type != StationType::Slave {
                return Err(crate::errors::AppError::Config(crate::errors::ConfigError::Other(
                    "站点类型不匹配：从站不可更新为主站".to_string(),
                )));
            }

            // 记录运行状态，用于后续恢复
            let was_running = existing.get_status().await == StationStatus::Running;

            // 停止旧服务（释放端口、断开连接等）
            existing.stop().await?;
            debug!("update_station_config slave stop done station_id={} was_running={}", station_id, was_running);

            // 创建新服务实例
            let new_service = Arc::new(SlaveService::new(config, Arc::clone(&self.database), Arc::clone(&self.events)));

            // 如果之前在运行，则启动新服务
            if was_running {
                new_service.start().await?;
            }

            // 替换服务实例
            self.slave_services.write().await.insert(station_id.to_string(), new_service);
            debug!("update_station_config slave replace service done station_id={}", station_id);
            return Ok(());
        }

        Err(crate::errors::AppError::Config(crate::errors::ConfigError::StationNotFound(station_id.to_string())))
    }

    /// 删除站点
    pub async fn remove_station(&self, station_id: &str) -> AppResult<()> {
        // 先尝试停止站点
        if let Err(e) = self.stop_station(station_id).await {
            warn!("停止站点 {} 时出错: {}", station_id, e);
        }

        // 从主站列表中移除
        if self.master_services.write().await.remove(station_id).is_some() {
            info!("删除主站: {}", station_id);
            return Ok(());
        }

        // 从从站列表中移除
        if self.slave_services.write().await.remove(station_id).is_some() {
            info!("删除从站: {}", station_id);
            return Ok(());
        }

        Err(crate::errors::AppError::Config(crate::errors::ConfigError::StationNotFound(station_id.to_string())))
    }

    /// 启动站点
    pub async fn start_station(&self, station_id: &str) -> AppResult<()> {
        // 尝试启动主站
        let master_service = {
            let guard = self.master_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(service) = master_service {
            return service.start().await;
        }

        // 尝试启动从站
        let slave_service = {
            let guard = self.slave_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(service) = slave_service {
            return service.start().await;
        }

        Err(crate::errors::AppError::Config(crate::errors::ConfigError::StationNotFound(station_id.to_string())))
    }

    /// 停止站点
    pub async fn stop_station(&self, station_id: &str) -> AppResult<()> {
        // 尝试停止主站
        let master_service = {
            let guard = self.master_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(service) = master_service {
            return service.stop().await;
        }

        // 尝试停止从站
        let slave_service = {
            let guard = self.slave_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(service) = slave_service {
            return service.stop().await;
        }

        Err(crate::errors::AppError::Config(crate::errors::ConfigError::StationNotFound(station_id.to_string())))
    }

    /// 获取站点状态
    pub async fn get_station_status(&self, station_id: &str) -> AppResult<StationStatus> {
        // 尝试获取主站状态
        let master_service = {
            let guard = self.master_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(service) = master_service {
            return Ok(service.get_status().await);
        }

        // 尝试获取从站状态
        let slave_service = {
            let guard = self.slave_services.read().await;
            guard.get(station_id).cloned()
        };
        if let Some(service) = slave_service {
            return Ok(service.get_status().await);
        }

        Err(crate::errors::AppError::Config(crate::errors::ConfigError::StationNotFound(station_id.to_string())))
    }

    /// 获取所有主站
    pub async fn get_master_stations(&self) -> Vec<String> {
        self.master_services.read().await.keys().cloned().collect()
    }

    /// 获取所有从站
    pub async fn get_slave_stations(&self) -> Vec<String> {
        self.slave_services.read().await.keys().cloned().collect()
    }

    /// 获取主站服务
    pub async fn get_master_service(&self, station_id: &str) -> Option<Arc<MasterService>> {
        self.master_services.read().await.get(station_id).cloned()
    }

    /// 获取从站服务
    pub async fn get_slave_service(&self, station_id: &str) -> Option<Arc<SlaveService>> {
        self.slave_services.read().await.get(station_id).cloned()
    }

    /// 停止所有站点
    pub async fn stop_all_stations(&self) -> AppResult<()> {
        info!("停止所有站点");

        // 停止所有主站
        let master_ids: Vec<String> = self.master_services.read().await.keys().cloned().collect();
        for station_id in master_ids {
            if let Err(e) = self.stop_station(&station_id).await {
                error!("停止主站 {} 失败: {}", station_id, e);
            }
        }

        // 停止所有从站
        let slave_ids: Vec<String> = self.slave_services.read().await.keys().cloned().collect();
        for station_id in slave_ids {
            if let Err(e) = self.stop_station(&station_id).await {
                error!("停止从站 {} 失败: {}", station_id, e);
            }
        }

        Ok(())
    }

    /// 获取站点统计信息
    pub async fn get_station_stats(&self) -> StationStats {
        let master_services: Vec<Arc<MasterService>> = {
            let guard = self.master_services.read().await;
            guard.values().cloned().collect()
        };
        let slave_services: Vec<Arc<SlaveService>> = {
            let guard = self.slave_services.read().await;
            guard.values().cloned().collect()
        };

        let mut running_masters = 0;
        let mut running_slaves = 0;

        for service in &master_services {
            if service.get_status().await == StationStatus::Running {
                running_masters += 1;
            }
        }

        for service in &slave_services {
            if service.get_status().await == StationStatus::Running {
                running_slaves += 1;
            }
        }

        StationStats {
            total_masters: master_services.len(),
            total_slaves: slave_services.len(),
            running_masters,
            running_slaves,
        }
    }
}

/// 站点统计信息。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StationStats {
    /// 主站总数
    pub total_masters: usize,
    /// 从站总数
    pub total_slaves: usize,
    /// 运行中的主站数量
    pub running_masters: usize,
    /// 运行中的从站数量
    pub running_slaves: usize,
}
