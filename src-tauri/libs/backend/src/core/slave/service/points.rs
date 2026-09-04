use std::sync::Arc;

use super::{
    COT_SPONTANEOUS, PointRuntimeStore, PolicyStore, ServiceContext, SimulationState, SlaveService, TelemetryBuffers,
    TransportRuntime,
};
use crate::{
    core::{
        iec104_registry::{PointRole, iec104_default_point_name, iec104_type_name, lookup_capability},
        shared::runtime_hint::emit_slave_runtime_hint_with_cursor,
        slave::{
            SimulationScenario,
            data::message_store::append_tracked_slave_message,
            protocol::quality::{
                apply_quality_metadata, normalize_quality_raw_for_type, quality_common_from_raw,
                quality_detail_from_point,
            },
            reporting::broadcast_runtime::broadcast_points as run_broadcast_points,
            simulation::{
                engine::point_key,
                runtime::{
                    simulate_data_changes_for_common_address as run_simulate_data_changes_for_common_address,
                    start_simulation as run_start_simulation, stop_simulation as run_stop_simulation,
                    trigger_scenario as run_trigger_scenario,
                },
            },
        },
        types::{DataPoint, MessageDirection, PointDef, PointSource, is_quality_business_usable},
    },
    errors::AppResult,
};

impl SlaveService {
    /// 应用数据点定义
    ///
    /// 为默认公共地址应用数据点定义列表。
    ///
    /// # 参数
    ///
    /// * `defs` - 数据点定义列表
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 应用成功
    /// - `Err(AppError)` - 应用失败
    ///
    /// # 使用场景
    ///
    /// 用户批量导入或更新数据点定义。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn apply_point_defs(&self, defs: &[PointDef]) -> AppResult<()> {
        self.apply_point_defs_for_common_address(self.config.common_address, defs).await
    }

    /// 为指定公共地址应用数据点定义
    ///
    /// 批量创建或更新指定公共地址的数据点定义。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    /// * `defs` - 数据点定义列表，每个定义包含：
    ///   - `address` - 信息对象地址（IOA）
    ///   - `name` - 数据点名称
    ///   - `description` - 数据点描述
    ///   - `type_id` - 类型标识
    ///   - `default_value` - 默认值（JSON 格式）
    ///   - `control_ioa` - 控制点地址（可选）
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 应用成功
    /// - `Err(AppError)` - 应用失败
    ///
    /// # 执行流程
    ///
    /// 1. 遍历所有数据点定义
    /// 2. 对于每个定义：
    ///    - 如果数据点不存在，创建新数据点并设置默认值
    ///    - 如果数据点已存在，更新元数据但保留当前值和质量
    /// 3. 删除该公共地址下不在定义列表中的数据点
    /// 4. 发送 "points-changed" 运行时提示
    ///
    /// # 默认值转换
    ///
    /// 支持以下 JSON 类型到 f64 的转换：
    /// - 数字（f64/i64/u64）→ 直接转换
    /// - 布尔值 → true=1.0, false=0.0
    /// - 字符串 → 尝试解析为 f64
    /// - 其他 → 使用协议默认值
    ///
    /// # 使用场景
    ///
    /// 用户从配置文件或数据库批量导入数据点定义。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn apply_point_defs_for_common_address(&self, common_address: u16, defs: &[PointDef]) -> AppResult<()> {
        let keep = self.points.data_points.replace_common_address_definitions(common_address, defs).await;

        self.telemetry.soe_events.write().await.retain(|event| {
            event.common_address != common_address || keep.contains(&point_key(common_address, event.address))
        });
        self.telemetry.soe_records.write().await.retain(|event| {
            event.common_address != common_address || keep.contains(&point_key(common_address, event.ioa))
        });

        let mut simulation = self.simulation.simulation_runtime.lock().await;
        if let Some(session) = simulation.selected_session.as_mut() {
            session.points.retain(|key| key.0 != common_address || keep.contains(key));
            if session.points.is_empty() {
                simulation.selected_session = None;
            }
        }
        simulation.selected_state_by_point.retain(|key, _| key.0 != common_address || keep.contains(key));
        simulation.global_state_by_point.retain(|key, _| key.0 != common_address || keep.contains(key));
        simulation.global_template_by_point.retain(|key, _| key.0 != common_address || keep.contains(key));
        drop(simulation);

        let cursor = self.points.data_points.cursor().await;
        emit_slave_runtime_hint_with_cursor(
            &self.context.runtime_event_handle,
            &self.config.id,
            None,
            "points-changed",
            Some(cursor),
        )
        .await;
        Ok(())
    }

    /// 删除指定公共地址的所有数据点
    ///
    /// 删除指定公共地址下的所有数据点。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    ///
    /// # 使用场景
    ///
    /// 用户清空特定公共地址的数据点，或在重新配置前清理旧数据。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn remove_common_address_points(&self, common_address: u16) {
        self.points.data_points.remove_common_address(common_address).await;
    }

    /// 启动数据模拟
    ///
    /// 启动数据点的自动模拟任务，根据模拟配置定期更新数据点值。
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 启动成功
    /// - `Err(AppError)` - 启动失败
    ///
    /// # 使用场景
    ///
    /// 用户启动数据模拟，测试主站的数据处理能力。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    pub async fn start_simulation(&self) -> AppResult<()> {
        run_start_simulation(&self.simulation_task_runtime(), self.clone_handle()).await
    }

    /// 停止数据模拟
    ///
    /// 停止数据点的自动模拟任务。
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 停止成功
    /// - `Err(AppError)` - 停止失败
    ///
    /// # 使用场景
    ///
    /// 用户停止数据模拟，恢复手动控制数据点值。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    pub async fn stop_simulation(&self) -> AppResult<()> {
        run_stop_simulation(&self.simulation_task_runtime(), self.clone_handle()).await
    }

    /// 触发模拟场景
    ///
    /// 触发预定义的模拟场景，模拟特定的系统行为或故障。
    ///
    /// # 参数
    ///
    /// * `scenario` - 模拟场景，包括：
    ///   - `CommunicationFault` - 通信故障（记录错误消息）
    ///   - `DataBurst` - 数据突发（批量更新数据点）
    ///   - `QualityChange` - 质量变化（修改数据点质量）
    ///   - 其他场景...
    ///
    /// # 返回
    ///
    /// - `Ok(usize)` - 触发成功，返回受影响的数据点数量
    /// - `Err(AppError)` - 触发失败
    ///
    /// # 使用场景
    ///
    /// 用户测试主站对各种异常场景的处理能力。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    pub async fn trigger_scenario(&self, scenario: SimulationScenario) -> AppResult<usize> {
        match scenario {
            SimulationScenario::CommunicationFault { reason } => {
                append_tracked_slave_message(
                    &self.telemetry.message_tracking_guard,
                    &self.telemetry.message_tracking_enabled,
                    &self.telemetry.message_records,
                    &self.telemetry.next_message_id,
                    MessageDirection::Error,
                    "local",
                    reason,
                    None,
                    None,
                    None,
                    None,
                )
                .await;
                Ok(0)
            }
            other => run_trigger_scenario(&self.simulation_task_runtime(), self, other).await,
        }
    }

    /// 更新数据点
    ///
    /// 更新默认公共地址的指定数据点的值和质量，并广播给所有已连接的主站。
    ///
    /// # 参数
    ///
    /// * `address` - 信息对象地址（IOA）
    /// * `value` - 新值
    /// * `quality` - 质量描述符
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 更新成功
    /// - `Err(AppError)` - 更新失败
    ///
    /// # 执行流程
    ///
    /// 1. 更新数据点的值和质量
    /// 2. 以突发/自发（COT=3）传输原因广播给所有主站
    ///
    /// # 使用场景
    ///
    /// 用户手动修改数据点值，模拟现场设备状态变化。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn update_data_point(&self, address: u32, value: f64, quality: u8) -> AppResult<()> {
        self.update_data_point_for_common_address(self.config.common_address, address, value, quality).await
    }

    /// 更新指定公共地址的数据点
    ///
    /// 更新指定公共地址的数据点的值和质量，并广播给所有已连接的主站。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    /// * `address` - 信息对象地址（IOA）
    /// * `value` - 新值
    /// * `quality` - 质量描述符
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 更新成功
    /// - `Err(AppError)` - 更新失败
    ///
    /// # 执行流程
    ///
    /// 1. 校验点表白名单并更新已有数据点
    /// 2. 以突发/自发（COT=3）传输原因广播给所有主站
    ///
    /// # 使用场景
    ///
    /// 用户手动修改特定公共地址的数据点值。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn update_data_point_for_common_address(
        &self,
        common_address: u16,
        address: u32,
        value: f64,
        quality: u8,
    ) -> AppResult<()> {
        let snapshot = {
            let mut data_points = self.points.data_points.transaction().await;
            let Some(point) = data_points.get_mut(&point_key(common_address, address)) else {
                return Err(crate::errors::AppError::Protocol(crate::errors::ProtocolError::InvalidData(format!(
                    "数据点不存在: CA={common_address}, IOA={address}"
                ))));
            };
            point.value = value;
            point.quality = quality;
            point.timestamp = chrono::Utc::now();
            apply_quality_metadata(point, quality);
            point.clone()
        };
        self.broadcast_points(&[snapshot], u16::from(COT_SPONTANEOUS)).await?;
        Ok(())
    }

    /// 批量更新数据点
    ///
    /// 批量更新默认公共地址的多个数据点的值和质量，并广播给所有已连接的主站。
    ///
    /// # 参数
    ///
    /// * `updates` - 更新列表，每个元素为 (address, value, quality) 元组
    ///
    /// # 返回
    ///
    /// - `Ok(usize)` - 更新成功，返回更新的数据点数量
    /// - `Err(AppError)` - 更新失败
    ///
    /// # 执行流程
    ///
    /// 1. 如果更新列表为空，直接返回 0
    /// 2. 遍历更新列表，批量更新或创建数据点
    /// 3. 以突发/自发（COT=3）传输原因广播给所有主站
    ///
    /// # 性能优化
    ///
    /// 本方法使用单次写锁批量更新所有数据点，比多次调用 update_data_point 更高效。
    ///
    /// # 使用场景
    ///
    /// 用户批量修改数据点值，或模拟引擎批量更新数据点。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn bulk_update_data_points(&self, updates: &[(u32, f64, u8)]) -> AppResult<usize> {
        self.bulk_update_data_points_for_common_address(self.config.common_address, updates).await
    }

    /// 批量更新指定公共地址的数据点
    ///
    /// 批量更新指定公共地址的多个数据点的值和质量，并广播给所有已连接的主站。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    /// * `updates` - 更新列表，每个元素为 (address, value, quality) 元组
    ///
    /// # 返回
    ///
    /// - `Ok(usize)` - 更新成功，返回更新的数据点数量
    /// - `Err(AppError)` - 更新失败
    ///
    /// # 执行流程
    ///
    /// 1. 如果更新列表为空，直接返回 0
    /// 2. 获取数据点写锁
    /// 3. 遍历更新列表，批量更新或创建数据点
    /// 4. 释放写锁
    /// 5. 以突发/自发（COT=3）传输原因广播给所有主站
    ///
    /// # 性能优化
    ///
    /// 本方法使用单次写锁批量更新所有数据点，避免多次锁竞争。
    ///
    /// # 使用场景
    ///
    /// 用户批量修改特定公共地址的数据点值。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn bulk_update_data_points_for_common_address(
        &self,
        common_address: u16,
        updates: &[(u32, f64, u8)],
    ) -> AppResult<usize> {
        if updates.is_empty() {
            return Ok(0);
        }

        let snapshots = {
            let mut snapshots = Vec::with_capacity(updates.len());
            let mut data_points = self.points.data_points.transaction().await;
            if let Some((address, ..)) =
                updates.iter().find(|(address, ..)| !data_points.contains_key(&point_key(common_address, *address)))
            {
                return Err(crate::errors::AppError::Protocol(crate::errors::ProtocolError::InvalidData(format!(
                    "数据点不存在: CA={common_address}, IOA={address}"
                ))));
            }
            for (address, value, quality) in updates {
                let point = data_points
                    .get_mut(&point_key(common_address, *address))
                    .expect("point existence checked before bulk update");
                point.value = *value;
                point.quality = *quality;
                point.timestamp = chrono::Utc::now();
                apply_quality_metadata(point, *quality);
                snapshots.push(point.clone());
            }
            snapshots
        };

        self.broadcast_points(&snapshots, u16::from(COT_SPONTANEOUS)).await?;
        Ok(snapshots.len())
    }

    /// 获取数据点
    ///
    /// 获取默认公共地址的指定数据点。
    ///
    /// # 参数
    ///
    /// * `address` - 信息对象地址（IOA）
    ///
    /// # 返回
    ///
    /// - `Some(DataPoint)` - 找到数据点，返回数据点副本
    /// - `None` - 未找到数据点
    ///
    /// # 使用场景
    ///
    /// 查询特定数据点的当前状态，用于前端显示或业务逻辑判断。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_data_point(&self, address: u32) -> Option<DataPoint> {
        self.get_data_point_for_common_address(self.config.common_address, address).await
    }

    /// 获取指定公共地址的数据点
    ///
    /// 从数据点缓存中获取指定公共地址和 IOA 的数据点。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    /// * `address` - 信息对象地址（IOA）
    ///
    /// # 返回
    ///
    /// - `Some(DataPoint)` - 找到数据点，返回数据点副本
    /// - `None` - 未找到数据点
    ///
    /// # 查询逻辑
    ///
    /// 1. 获取数据点缓存的读锁
    /// 2. 根据 (common_address, address) 构建数据点键
    /// 3. 从缓存中查找数据点
    /// 4. 如果找到，克隆并返回数据点
    ///
    /// # 使用场景
    ///
    /// 查询特定公共地址下的数据点状态，支持多公共地址场景。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_data_point_for_common_address(&self, common_address: u16, address: u32) -> Option<DataPoint> {
        self.points.data_points.read().await.get(&point_key(common_address, address)).cloned()
    }

    /// 模拟数据变化
    ///
    /// 触发默认公共地址的数据点模拟变化。
    /// 这是一个包装方法，调用 simulate_data_changes_for_common_address 并使用默认公共地址。
    ///
    /// # 执行流程
    ///
    /// 1. 调用 simulate_data_changes_for_common_address
    /// 2. 传入默认公共地址（self.config.common_address）
    ///
    /// # 使用场景
    ///
    /// 手动触发一次数据模拟变化，用于测试或演示。
    ///
    /// # 线程安全
    ///
    /// 本方法通过内部调用的方法保证线程安全。
    pub async fn simulate_data_changes(&self) {
        self.simulate_data_changes_for_common_address(self.config.common_address).await;
    }

    /// 模拟指定公共地址的数据变化
    ///
    /// 触发指定公共地址的数据点模拟变化，根据模拟配置随机更新数据点值。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    ///
    /// # 执行流程
    ///
    /// 1. 创建模拟任务运行时上下文
    /// 2. 调用 run_simulate_data_changes_for_common_address
    /// 3. 根据模拟配置（变化概率、随机种子等）更新数据点
    /// 4. 将变化的数据点广播给所有已连接的主站
    ///
    /// # 模拟逻辑
    ///
    /// - 遍历指定公共地址的所有数据点
    /// - 根据 change_probability 决定是否更新每个数据点
    /// - 使用随机数生成器生成新值
    /// - 更新数据点的值、质量和时标
    /// - 以突发/自发（COT=3）传输原因广播变化
    ///
    /// # 使用场景
    ///
    /// 手动触发特定公共地址的数据模拟，用于测试多公共地址场景。
    ///
    /// # 线程安全
    ///
    /// 本方法通过内部调用的方法保证线程安全。
    pub async fn simulate_data_changes_for_common_address(&self, common_address: u16) {
        run_simulate_data_changes_for_common_address(&self.simulation_task_runtime(), self, common_address).await;
    }

    /// 强制上送指定公共地址下所有监视点的当前值。
    pub async fn force_upload_current_points(&self, common_address: u16) -> AppResult<usize> {
        let snapshots = self
            .points
            .data_points
            .read()
            .await
            .values()
            .filter(|point| point.common_address.unwrap_or(self.config.common_address) == common_address)
            .filter(|point| {
                lookup_capability(point.type_id)
                    .is_some_and(|capability| capability.point_role == PointRole::Monitor && capability.slave_upload)
            })
            .cloned()
            .collect::<Vec<_>>();

        self.broadcast_points(&snapshots, u16::from(COT_SPONTANEOUS)).await?;
        Ok(snapshots.len())
    }

    /// 克隆服务句柄
    ///
    /// 创建一个新的 SlaveService 实例，共享所有内部状态。
    /// 这是一个内部方法，用于在异步任务中传递服务实例。
    ///
    /// # 返回
    ///
    /// 返回新的 SlaveService 实例，所有 Arc 字段都指向相同的共享状态。
    ///
    /// # 实现细节
    ///
    /// - 克隆配置（StationConfig 实现了 Clone）
    /// - 对所有 Arc 字段调用 Arc::clone，增加引用计数但不复制数据
    /// - 新实例与原实例共享所有可变状态（通过 Arc<RwLock<T>> 或 Arc<Mutex<T>>）
    ///
    /// # 使用场景
    ///
    /// - 在模拟任务中传递服务实例
    /// - 在广播任务中传递服务实例
    /// - 在异步闭包中捕获服务实例
    ///
    /// # 性能考虑
    ///
    /// 本方法非常轻量，只增加引用计数，不复制实际数据。
    ///
    /// # 线程安全
    ///
    /// Arc::clone 是线程安全的，可以在多线程环境中安全调用。
    pub(crate) fn clone_handle(&self) -> Self {
        Self {
            config: self.config.clone(),
            transport: TransportRuntime {
                status: Arc::clone(&self.transport.status),
                connections: Arc::clone(&self.transport.connections),
                peer_local_addrs: Arc::clone(&self.transport.peer_local_addrs),
                server: Arc::clone(&self.transport.server),
                protocols: Arc::clone(&self.transport.protocols),
                link_params: Arc::clone(&self.transport.link_params),
                server_task: Arc::clone(&self.transport.server_task),
                station_time_offset_ms: Arc::clone(&self.transport.station_time_offset_ms),
            },
            points: PointRuntimeStore {
                data_points: Arc::clone(&self.points.data_points),
                initialization_profile: Arc::clone(&self.points.initialization_profile),
            },
            telemetry: TelemetryBuffers {
                message_records: Arc::clone(&self.telemetry.message_records),
                capture_packets: Arc::clone(&self.telemetry.capture_packets),
                message_tracking_enabled: Arc::clone(&self.telemetry.message_tracking_enabled),
                message_tracking_guard: Arc::clone(&self.telemetry.message_tracking_guard),
                next_message_id: Arc::clone(&self.telemetry.next_message_id),
                spontaneous_coalescer: Arc::clone(&self.telemetry.spontaneous_coalescer),
                soe_events: Arc::clone(&self.telemetry.soe_events),
                soe_records: Arc::clone(&self.telemetry.soe_records),
                next_soe_id: Arc::clone(&self.telemetry.next_soe_id),
            },
            policy: PolicyStore {
                station_policy_defaults: Arc::clone(&self.policy.station_policy_defaults),
                station_policy_overrides: Arc::clone(&self.policy.station_policy_overrides),
                station_policy_connection_override_enabled: Arc::clone(
                    &self.policy.station_policy_connection_override_enabled,
                ),
                command_mismatch_policy: Arc::clone(&self.policy.command_mismatch_policy),
            },
            simulation: SimulationState {
                simulation_task: Arc::clone(&self.simulation.simulation_task),
                simulation_profile: Arc::clone(&self.simulation.simulation_profile),
                simulation_seed: Arc::clone(&self.simulation.simulation_seed),
                simulation_runtime: Arc::clone(&self.simulation.simulation_runtime),
            },
            context: ServiceContext {
                db_service: Arc::clone(&self.context.db_service),
                runtime_event_handle: Arc::clone(&self.context.runtime_event_handle),
            },
        }
    }

    /// 初始化数据点
    ///
    /// 根据初始化配置批量创建默认数据点。
    /// 这是一个私有方法，在从站启动时自动调用（如果数据点列表为空）。
    ///
    /// # 执行流程
    ///
    /// 1. 读取初始化配置（InitializationProfile）
    /// 2. 获取数据点缓存的写锁
    /// 3. 删除默认公共地址下的所有现有数据点
    /// 4. 根据配置批量创建新数据点：
    ///    - 数量：profile.point_count
    ///    - 起始地址：profile.start_address
    ///    - 值计算：base_value + (index * step)
    ///    - 类型：固定为 13（M_ME_NC_1，短浮点数测量值）
    ///
    /// # 数据点属性
    ///
    /// 创建的数据点具有以下属性：
    /// - `connection_id`：\"local\"（本地创建）
    /// - `type_id`：13（M_ME_NC_1）
    /// - `name`：根据类型和地址生成默认名称
    /// - `value`：base_value + (index * step)
    /// - `quality`：根据配置规范化的质量描述符
    /// - `timestamp`：当前时间
    /// - `point_source`：Manual（手动创建）
    ///
    /// # 使用场景
    ///
    /// - 从站首次启动时自动创建默认数据点
    /// - 用户更新初始化配置后重新初始化数据点
    ///
    /// # 注意事项
    ///
    /// - 本方法会删除默认公共地址下的所有现有数据点
    /// - 只创建类型 13（短浮点数）的数据点
    /// - 如果需要其他类型的数据点，应使用 apply_point_defs 方法
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，确保数据一致性。
    pub(super) async fn initialize_data_points(&self) {
        let profile = self.points.initialization_profile.read().await.clone();
        let mut data_points = self.points.data_points.transaction().await;
        data_points.remove_where(|(ca, _), _| *ca == self.config.common_address);

        for idx in 0..profile.point_count {
            let address = profile.start_address.saturating_add(idx);
            let value = profile.base_value + (idx as f64 * profile.step);
            let raw_quality = normalize_quality_raw_for_type(13, profile.quality);
            data_points.insert(point_key(self.config.common_address, address), DataPoint {
                connection_id: "local".to_string(),
                link_profile_id: None,
                slave_id: None,
                common_address: Some(self.config.common_address),
                address,
                name: iec104_default_point_name(13, address),
                description: None,
                control_ioa: None,
                gi_group: None,
                counter_group: None,
                type_id: 13,
                data_type: iec104_type_name(13).to_string(),
                value,
                quality: raw_quality,
                quality_common: Some(quality_common_from_raw(13, raw_quality)),
                quality_detail: Some(quality_detail_from_point(13, raw_quality, value)),
                business_usable: is_quality_business_usable(13, raw_quality),
                timestamp: chrono::Utc::now(),
                latest_event_timestamp: None,
                timestamp_detail: None,
                report_count: None,
                latest_cause: None,
                point_source: Some(PointSource::Manual),
                control_status_snapshot: None,
            });
        }
    }

    /// 广播数据点
    ///
    /// 将数据点变化广播给所有已连接的主站。
    /// 这是一个内部方法，由其他公共方法调用以实现数据点的主动上报。
    ///
    /// # 参数
    ///
    /// * `points` - 要广播的数据点列表
    /// * `cause` - 传输原因（Cause of Transmission）
    ///   - 3：突发/自发（COT_SPONTANEOUS）- 数据点主动变化
    ///   - 20：总召唤响应（COT_INTERROGATION）- 响应总召唤命令
    ///   - 其他：根据具体场景确定
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 广播成功
    /// - `Err(AppError)` - 广播失败
    ///
    /// # 执行流程
    ///
    /// 1. 创建广播运行时上下文（包含所有必要的共享状态）
    /// 2. 调用 run_broadcast_points 执行实际的广播逻辑：
    ///    - 遍历所有已连接的主站
    ///    - 检查主站的数据传输状态（必须为 Started）
    ///    - 根据数据点类型构建 ASDU
    ///    - 将 ASDU 封装为 I 帧并发送
    ///    - 记录发送日志和 PCAP 抓包
    ///    - 更新连接统计信息
    ///
    /// # 广播策略
    ///
    /// - **按类型分组**：相同类型的数据点会被打包到同一个 ASDU 中
    /// - **按公共地址分组**：相同公共地址的数据点会被打包到同一个 ASDU 中
    /// - **流量控制**：遵守 IEC104 的 k/w 参数限制，避免发送窗口溢出
    /// - **突发合并**：支持突发上报合并，减少网络流量
    ///
    /// # 使用场景
    ///
    /// - 数据点值变化时主动上报（COT=3）
    /// - 响应总召唤命令（COT=20）
    /// - 模拟引擎批量更新数据点后广播
    /// - 用户手动更新数据点后广播
    ///
    /// # 线程安全
    ///
    /// 本方法通过内部调用的方法保证线程安全，所有共享状态都通过 Arc<RwLock<T>> 保护。
    pub(crate) async fn broadcast_points(&self, points: &[DataPoint], cause: u16) -> AppResult<()> {
        run_broadcast_points(&self.broadcast_runtime(), points, cause).await
    }
}
