use super::SlaveService;
use crate::{
    core::{
        slave::{
            InitializationProfile, SimulationProfile, SlaveCommandMismatchPolicy, StartSelectedPointSimulationRequest,
            reporting::policy_store::{sanitize_station_policy_defaults, sanitize_station_policy_override},
            simulation::runtime::{
                start_selected_point_simulation as run_start_selected_point_simulation,
                stop_selected_point_simulation as run_stop_selected_point_simulation,
                update_simulation_profile as run_update_simulation_profile,
            },
        },
        types::{LinkParams, SlaveStationPolicy, SlaveStationPolicyOverride},
    },
    errors::AppResult,
};

impl SlaveService {
    /// 更新初始化配置
    ///
    /// 更新从站的数据点初始化配置，并在数据点列表为空时自动初始化数据点。
    ///
    /// # 参数
    ///
    /// * `profile` - 初始化配置，包含：
    ///   - `point_count` - 数据点数量
    ///   - `start_address` - 起始地址
    ///   - `base_value` - 基础值
    ///   - `step` - 步长
    ///   - `quality` - 质量描述符
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 更新成功
    /// - `Err(AppError)` - 更新失败
    ///
    /// # 执行流程
    ///
    /// 1. 更新初始化配置
    /// 2. 如果数据点列表为空，调用 initialize_data_points 初始化数据点
    ///
    /// # 使用场景
    ///
    /// 用户修改初始化配置，或在从站启动前设置数据点初始化参数。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn update_initialization_profile(&self, profile: InitializationProfile) -> AppResult<()> {
        *self.points.initialization_profile.write().await = profile;
        if self.points.data_points.read().await.is_empty() {
            self.initialize_data_points().await;
        }
        Ok(())
    }

    /// 更新模拟配置
    ///
    /// 更新从站的数据模拟配置，并重启模拟任务。
    ///
    /// # 参数
    ///
    /// * `profile` - 模拟配置，包含：
    ///   - `enabled` - 是否启用模拟
    ///   - `interval_ms` - 模拟间隔（毫秒）
    ///   - `change_probability` - 变化概率（0.0-1.0）
    ///   - `seed` - 随机种子
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 更新成功
    /// - `Err(AppError)` - 更新失败
    ///
    /// # 执行流程
    ///
    /// 1. 更新模拟配置
    /// 2. 如果模拟任务正在运行，重启模拟任务以应用新配置
    ///
    /// # 使用场景
    ///
    /// 用户修改模拟配置，调整数据点变化频率和随机性。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn update_simulation_profile(&self, profile: SimulationProfile) -> AppResult<()> {
        run_update_simulation_profile(&self.simulation_task_runtime(), self.clone_handle(), profile).await
    }

    /// 获取模拟配置
    ///
    /// 返回当前的数据模拟配置。
    ///
    /// # 返回
    ///
    /// 返回 SimulationProfile 实例，包含所有模拟参数。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_simulation_profile(&self) -> SimulationProfile {
        self.simulation.simulation_profile.read().await.clone()
    }

    /// 启动选定数据点模拟
    ///
    /// 启动对指定数据点的模拟，覆盖全局模拟配置。
    ///
    /// # 参数
    ///
    /// * `request` - 选定数据点模拟请求，包含：
    ///   - `common_address` - 公共地址
    ///   - `addresses` - 数据点地址列表
    ///   - `interval_ms` - 模拟间隔（毫秒）
    ///   - `change_probability` - 变化概率（0.0-1.0）
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 启动成功
    /// - `Err(AppError)` - 启动失败
    ///
    /// # 使用场景
    ///
    /// 用户需要对特定数据点进行独立的模拟控制。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    pub async fn start_selected_point_simulation(&self, request: StartSelectedPointSimulationRequest) -> AppResult<()> {
        run_start_selected_point_simulation(&self.simulation_task_runtime(), self.clone_handle(), request).await
    }

    /// 停止选定数据点模拟
    ///
    /// 停止对选定数据点的模拟，恢复全局模拟配置。
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 停止成功
    /// - `Err(AppError)` - 停止失败
    ///
    /// # 使用场景
    ///
    /// 用户停止对特定数据点的独立模拟控制。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    pub async fn stop_selected_point_simulation(&self) -> AppResult<()> {
        run_stop_selected_point_simulation(&self.simulation_task_runtime(), self.clone_handle()).await
    }

    /// 更新链路参数
    ///
    /// 更新 IEC104 链路参数，并应用到所有已存在的协议实例。
    ///
    /// # 参数
    ///
    /// * `params` - 链路参数，包含：
    ///   - `k_value` - 最大未确认 I 帧数量（发送窗口大小）
    ///   - `w_value` - 最大未确认接收 I 帧数量（接收窗口大小）
    ///   - `t0_seconds` - 连接建立超时（秒）
    ///   - `t1_seconds` - 发送或测试 APDU 超时（秒）
    ///   - `t2_seconds` - 接收方确认超时（秒）
    ///   - `t3_seconds` - 空闲超时（秒）
    ///   - `max_asdu_bytes` - 最大 ASDU 长度（字节）
    ///
    /// # 执行流程
    ///
    /// 1. 更新链路参数配置
    /// 2. 遍历所有已存在的协议实例
    /// 3. 应用新的 k_value 和 max_asdu_bytes 到每个协议实例
    ///
    /// # 注意事项
    ///
    /// - 本方法采用"尽力而为"策略，即使部分协议实例更新失败也不会返回错误
    /// - 新建立的连接会自动使用新的链路参数
    ///
    /// # 使用场景
    ///
    /// 用户调整链路参数以优化性能或适应不同的网络环境。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 和 Mutex 保护共享状态，支持并发调用。
    pub async fn update_link_params(&self, params: LinkParams) {
        *self.transport.link_params.write().await = params;

        // best-effort: apply K to existing protocol instances
        let protocols = self.transport.protocols.read().await.values().cloned().collect::<Vec<_>>();
        for protocol in protocols {
            let mut protocol = protocol.lock().await;
            protocol.set_max_unconfirmed(params.k_value);
            protocol.set_max_asdu_len(params.max_asdu_bytes as usize);
        }
    }

    /// 更新从站策略默认值
    ///
    /// 更新从站策略的默认值，应用到所有未设置覆盖策略的公共地址。
    ///
    /// # 参数
    ///
    /// * `defaults` - 从站策略默认值，包含：
    ///   - `general_interrogation_enabled` - 是否启用总召唤
    ///   - `clock_sync_enabled` - 是否启用时钟同步
    ///   - `test_command_enabled` - 是否启用测试命令
    ///   - `control_enabled` - 是否启用控制命令
    ///
    /// # 执行流程
    ///
    /// 1. 对输入的策略进行规范化处理（sanitize）
    /// 2. 更新策略默认值
    ///
    /// # 使用场景
    ///
    /// 用户设置从站的全局策略，控制从站对各类 ASDU 的响应行为。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn update_station_policy_defaults(&self, defaults: SlaveStationPolicy) {
        *self.policy.station_policy_defaults.write().await = sanitize_station_policy_defaults(defaults);
    }

    /// 插入或更新从站策略覆盖
    ///
    /// 为指定公共地址设置策略覆盖，优先级高于默认策略。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    /// * `policy_override` - 策略覆盖，包含可选的策略字段
    ///
    /// # 执行流程
    ///
    /// 1. 对输入的策略覆盖进行规范化处理
    /// 2. 如果规范化后的策略为空，删除该公共地址的覆盖
    /// 3. 否则，插入或更新该公共地址的策略覆盖
    ///
    /// # 使用场景
    ///
    /// 用户为特定公共地址设置独立的策略，实现细粒度控制。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn upsert_station_policy_override(
        &self,
        common_address: u16,
        policy_override: SlaveStationPolicyOverride,
    ) {
        let normalized = sanitize_station_policy_override(policy_override);
        let mut overrides = self.policy.station_policy_overrides.write().await;
        if normalized.is_empty() {
            overrides.remove(&common_address);
        } else {
            overrides.insert(common_address, normalized);
        }
    }

    /// 删除从站策略覆盖
    ///
    /// 删除指定公共地址的策略覆盖，恢复使用默认策略。
    ///
    /// # 参数
    ///
    /// * `common_address` - 公共地址
    ///
    /// # 使用场景
    ///
    /// 用户取消特定公共地址的独立策略，恢复使用全局默认策略。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn remove_station_policy_override(&self, common_address: u16) {
        self.policy.station_policy_overrides.write().await.remove(&common_address);
    }

    /// 清空所有从站策略覆盖
    ///
    /// 清空所有公共地址的策略覆盖，所有公共地址恢复使用默认策略。
    ///
    /// # 使用场景
    ///
    /// 用户重置所有策略覆盖，统一使用全局默认策略。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn clear_station_policy_overrides(&self) {
        self.policy.station_policy_overrides.write().await.clear();
    }

    /// 设置从站策略连接级覆盖开关
    ///
    /// 启用或禁用连接级策略覆盖功能。
    ///
    /// # 参数
    ///
    /// * `enabled` - true 启用连接级策略覆盖，false 禁用
    ///
    /// # 使用场景
    ///
    /// 用户控制是否允许按连接设置独立的策略覆盖。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn set_station_policy_connection_override_enabled(&self, enabled: bool) {
        *self.policy.station_policy_connection_override_enabled.write().await = enabled;
    }

    /// 设置命令不匹配策略
    ///
    /// 设置当接收到的控制命令与数据点类型不匹配时的处理策略。
    ///
    /// # 参数
    ///
    /// * `policy` - 命令不匹配策略：
    ///   - `Strict` - 严格模式，拒绝类型不匹配的命令
    ///   - `Compatible` - 兼容模式，尝试类型转换
    ///   - `Debug` - 调试模式，记录日志但不拒绝
    ///
    /// # 使用场景
    ///
    /// 用户根据实际需求调整从站对类型不匹配命令的容错策略。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn set_command_mismatch_policy(&self, policy: SlaveCommandMismatchPolicy) {
        *self.policy.command_mismatch_policy.write().await = policy;
    }

    /// 获取命令不匹配策略
    ///
    /// 返回当前的命令不匹配处理策略。
    ///
    /// # 返回
    ///
    /// 返回 SlaveCommandMismatchPolicy 枚举值。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_command_mismatch_policy(&self) -> SlaveCommandMismatchPolicy {
        *self.policy.command_mismatch_policy.read().await
    }
}
