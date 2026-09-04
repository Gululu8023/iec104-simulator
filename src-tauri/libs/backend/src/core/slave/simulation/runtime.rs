//! 模拟任务运行时管理
//!
//! 管理从站模拟任务的生命周期，包括启动、停止、配置更新和场景触发。

use std::sync::Arc;

use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time::{Duration, sleep},
};

use super::{
    SimulationProfile, SimulationScenario, StartSelectedPointSimulationRequest, WaveformType,
    engine::{
        SimulationRuntime, apply_simulation_step, apply_unified_simulation_step, clear_selected_point_simulation,
        configure_selected_point_simulation, point_key,
    },
    math::{next_lcg_signed, normalize_simulated_value_by_type, normalize_waveform_cycle},
};
use crate::{
    core::{
        shared::runtime_hint::emit_slave_runtime_hint_with_cursor,
        slave::{
            data::point_store::SlavePointStore,
            protocol::{
                point_type::{is_double_point_type, is_single_point_type},
                quality::apply_quality_metadata,
            },
            service::SlaveService,
        },
        types::{DataPoint, StationStatus},
    },
    errors::AppResult,
};

/// 自发传输原因（COT=3）
const COT_SPONTANEOUS: u16 = 3;

/// 从站模拟任务运行时
///
/// 持有模拟任务所需的所有共享状态和资源。
#[derive(Clone)]
pub(crate) struct SlaveSimulationTaskRuntime {
    /// 从站标识符
    pub(crate) station_id: String,
    /// 默认公共地址
    pub(crate) default_common_address: u16,
    /// 数据点映射表
    pub(crate) data_points: Arc<SlavePointStore>,
    /// 模拟任务句柄
    pub(crate) simulation_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// 从站状态，用于将模拟步进与停止状态切换串行化。
    pub(crate) status: Arc<RwLock<StationStatus>>,
    /// 模拟配置
    pub(crate) simulation_profile: Arc<RwLock<SimulationProfile>>,
    /// 随机数种子
    pub(crate) simulation_seed: Arc<Mutex<u64>>,
    /// 模拟运行时状态
    pub(crate) simulation_runtime: Arc<Mutex<SimulationRuntime>>,
    /// Tauri 应用句柄
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
}

/// 更新模拟配置
///
/// 更新模拟配置并重新协调模拟任务状态。
pub(crate) async fn update_simulation_profile(
    runtime: &SlaveSimulationTaskRuntime,
    service: SlaveService,
    profile: SimulationProfile,
) -> AppResult<()> {
    *runtime.simulation_seed.lock().await = profile.seed;
    *runtime.simulation_profile.write().await = profile;
    reconcile_simulation_task(runtime, service).await
}

/// 启动选点模拟
///
/// 配置选点模拟会话并启动模拟任务。
pub(crate) async fn start_selected_point_simulation(
    runtime: &SlaveSimulationTaskRuntime,
    service: SlaveService,
    request: StartSelectedPointSimulationRequest,
) -> AppResult<()> {
    let data_points = runtime.data_points.read().await;
    let mut simulation_runtime = runtime.simulation_runtime.lock().await;
    configure_selected_point_simulation(
        runtime.default_common_address,
        &data_points,
        &mut simulation_runtime,
        request,
    )?;
    drop(data_points);
    drop(simulation_runtime);
    reconcile_simulation_task(runtime, service).await
}

/// 停止选点模拟
///
/// 清除选点模拟会话并重新协调模拟任务状态。
pub(crate) async fn stop_selected_point_simulation(
    runtime: &SlaveSimulationTaskRuntime,
    service: SlaveService,
) -> AppResult<()> {
    let mut simulation_runtime = runtime.simulation_runtime.lock().await;
    clear_selected_point_simulation(&mut simulation_runtime);
    drop(simulation_runtime);
    reconcile_simulation_task(runtime, service).await
}

/// 启动全局模拟
///
/// 启用全局模拟配置并启动模拟任务。
pub(crate) async fn start_simulation(runtime: &SlaveSimulationTaskRuntime, service: SlaveService) -> AppResult<()> {
    runtime.simulation_profile.write().await.enabled = true;
    reconcile_simulation_task(runtime, service).await
}

/// 停止全局模拟
///
/// 禁用全局模拟配置并重新协调模拟任务状态。
pub(crate) async fn stop_simulation(runtime: &SlaveSimulationTaskRuntime, service: SlaveService) -> AppResult<()> {
    runtime.simulation_profile.write().await.enabled = false;
    reconcile_simulation_task(runtime, service).await
}

/// 恢复模拟任务
///
/// 重新协调模拟任务状态，用于从站启动时恢复模拟。
pub(crate) async fn resume_simulation_task(
    runtime: &SlaveSimulationTaskRuntime,
    service: SlaveService,
) -> AppResult<()> {
    reconcile_simulation_task(runtime, service).await
}

pub(crate) async fn suspend_simulation_task(runtime: &SlaveSimulationTaskRuntime) {
    if let Some(handle) = runtime.simulation_task.lock().await.take() {
        handle.abort();
        let _ = handle.await;
    }
}

/// 为指定公共地址模拟数据变化
///
/// 使用简单模拟算法为指定公共地址的数据点生成变化，并自动广播。
pub(crate) async fn simulate_data_changes_for_common_address(
    runtime: &SlaveSimulationTaskRuntime,
    service: &SlaveService,
    common_address: u16,
) {
    let profile = runtime.simulation_profile.read().await.clone();
    let changed =
        apply_simulation_step(&runtime.data_points, &profile, &runtime.simulation_seed, Some(common_address)).await;
    if !changed.is_empty() && profile.auto_broadcast {
        let _ = service.broadcast_points(&changed, COT_SPONTANEOUS).await;
    }
}

/// 触发模拟场景
///
/// 执行特定的模拟场景（雪崩、波形、通信故障），返回变化的数据点数量。
pub(crate) async fn trigger_scenario(
    runtime: &SlaveSimulationTaskRuntime,
    service: &SlaveService,
    scenario: SimulationScenario,
) -> AppResult<usize> {
    let changed = match scenario {
        SimulationScenario::Avalanche { count, common_address } => {
            apply_avalanche(runtime, count, common_address).await
        }
        SimulationScenario::Waveform {
            address,
            common_address,
            waveform_type,
            amplitude,
            base,
            period_seconds,
            phase,
            fixed_value,
        } => {
            let target_common_address = common_address.unwrap_or(runtime.default_common_address);
            apply_waveform(
                runtime,
                target_common_address,
                address,
                waveform_type,
                amplitude,
                base,
                period_seconds,
                phase,
                fixed_value,
            )
            .await
        }
        SimulationScenario::CommunicationFault { .. } => Vec::new(),
    };

    if !changed.is_empty() {
        service.broadcast_points(&changed, COT_SPONTANEOUS).await?;
    }
    Ok(changed.len())
}

/// 协调模拟任务状态
///
/// 根据全局模拟和选点模拟的启用状态，决定是否启动或停止模拟任务。
async fn reconcile_simulation_task(runtime: &SlaveSimulationTaskRuntime, service: SlaveService) -> AppResult<()> {
    // 检查是否有任何模拟启用
    let has_global = runtime.simulation_profile.read().await.enabled;
    let has_selected = runtime.simulation_runtime.lock().await.selected_session.is_some();

    if has_global || has_selected {
        // 有模拟启用，确保任务运行
        start_simulation_task_if_needed(runtime, service).await
    } else {
        // 无模拟启用，停止任务
        if let Some(handle) = runtime.simulation_task.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }
        Ok(())
    }
}

/// 按需启动模拟任务
///
/// 检查任务是否已运行，如果未运行则启动新的模拟任务。
async fn start_simulation_task_if_needed(runtime: &SlaveSimulationTaskRuntime, service: SlaveService) -> AppResult<()> {
    {
        // 检查现有任务状态
        let mut guard = runtime.simulation_task.lock().await;
        if let Some(handle) = guard.as_ref() {
            if !handle.is_finished() {
                // 任务仍在运行，无需启动
                return Ok(());
            }
        }
        *guard = None;
    }

    // 克隆共享状态用于异步任务
    let data_points = Arc::clone(&runtime.data_points);
    let simulation_profile = Arc::clone(&runtime.simulation_profile);
    let simulation_seed = Arc::clone(&runtime.simulation_seed);
    let simulation_runtime = Arc::clone(&runtime.simulation_runtime);
    let runtime_event_handle = Arc::clone(&runtime.runtime_event_handle);
    let status = Arc::clone(&runtime.status);
    let station_id = runtime.station_id.clone();

    // 启动模拟循环任务
    let handle = tokio::spawn(async move {
        loop {
            // 读取配置并等待间隔
            let profile = simulation_profile.read().await.clone();
            let interval_ms = profile.interval_ms.max(50);
            sleep(Duration::from_millis(interval_ms)).await;

            let status_guard = status.read().await;
            if *status_guard != StationStatus::Running {
                break;
            }

            // 执行统一模拟步进
            let changed =
                apply_unified_simulation_step(&data_points, &profile, &simulation_seed, &simulation_runtime).await;
            drop(status_guard);

            // 检查是否应该继续运行
            let has_selected = simulation_runtime.lock().await.selected_session.is_some();
            if !profile.enabled && !has_selected {
                break;
            }

            // 自动广播变化的数据点
            if !changed.is_empty() && profile.auto_broadcast && *status.read().await == StationStatus::Running {
                let _ = service.broadcast_points(&changed, COT_SPONTANEOUS).await;
            }
            if !changed.is_empty() {
                let status_guard = status.read().await;
                if *status_guard != StationStatus::Running {
                    break;
                }
                let cursor = data_points.cursor().await;
                emit_slave_runtime_hint_with_cursor(
                    &runtime_event_handle,
                    &station_id,
                    None,
                    "simulation-step",
                    Some(cursor),
                )
                .await;
            }
        }
    });
    *runtime.simulation_task.lock().await = Some(handle);
    Ok(())
}

/// 应用雪崩场景
///
/// 批量修改数据点值，模拟大量数据点同时变化的场景。
async fn apply_avalanche(
    runtime: &SlaveSimulationTaskRuntime,
    count: Option<u32>,
    common_address: Option<u16>,
) -> Vec<DataPoint> {
    let mut changed = Vec::new();
    let mut points = runtime.data_points.transaction().await;
    let mut touched: usize = 0;
    points.for_each_mut(|_, point| {
        if let Some(target_common_address) = common_address {
            if point.common_address != Some(target_common_address) {
                return false;
            }
        }
        if !is_single_point_type(point.type_id) && !is_double_point_type(point.type_id) {
            return false;
        }
        if count.is_some_and(|limit| touched >= limit as usize) {
            return false;
        }
        point.value = if is_single_point_type(point.type_id) {
            if point.value > 0.5 { 0.0 } else { 1.0 }
        } else if point.value.round() == 1.0 {
            2.0
        } else {
            1.0
        };
        point.timestamp = chrono::Utc::now();
        apply_quality_metadata(point, point.quality);
        changed.push(point.clone());
        touched = touched.saturating_add(1);
        true
    });
    changed
}

/// 应用波形场景
///
/// 为指定数据点应用波形计算，支持多种波形类型（固定值、线性、正弦、三角、方波、随机）。
async fn apply_waveform(
    runtime: &SlaveSimulationTaskRuntime,
    common_address: u16,
    address: u32,
    waveform_type: WaveformType,
    amplitude: f64,
    base: f64,
    period_seconds: f64,
    phase: f64,
    fixed_value: f64,
) -> Vec<DataPoint> {
    let mut points = runtime.data_points.transaction().await;
    if let Some(point) = points.get_mut(&point_key(common_address, address)) {
        let computed_value = match waveform_type {
            WaveformType::Fixed => fixed_value,
            WaveformType::Linear => {
                let t = normalize_waveform_cycle(phase, period_seconds);
                base + amplitude * t
            }
            WaveformType::Sine => {
                let t = normalize_waveform_cycle(phase, period_seconds);
                base + amplitude * (std::f64::consts::TAU * t).sin()
            }
            WaveformType::Triangle => {
                let t = normalize_waveform_cycle(phase, period_seconds);
                let triangle = if t < 0.25 {
                    t * 4.0
                } else if t < 0.75 {
                    2.0 - t * 4.0
                } else {
                    t * 4.0 - 4.0
                };
                base + amplitude * triangle
            }
            WaveformType::Step => {
                let t = normalize_waveform_cycle(phase, period_seconds);
                if t < 0.5 { base } else { base + amplitude }
            }
            WaveformType::Random => {
                let noise = {
                    let mut seed = runtime.simulation_seed.lock().await;
                    next_lcg_signed(&mut seed)
                };
                base + amplitude.abs() * noise
            }
        };

        point.value = normalize_simulated_value_by_type(point.type_id, computed_value);
        point.timestamp = chrono::Utc::now();
        return vec![point.clone()];
    }
    Vec::new()
}
