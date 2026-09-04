use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::Mutex;

use super::{
    SimulationProfile, StartSelectedPointSimulationRequest, WaveformSimulationConfig,
    math::{
        WaveformRuntimeState, compute_waveform_simulation_value, default_waveform_config_for_point, next_lcg_signed,
        normalize_selected_simulation_points, normalize_simulated_value_by_type, normalize_waveform_simulation_config,
    },
};
use crate::{
    core::{
        iec104_registry::{ValueModel, lookup_capability},
        slave::{
            data::point_store::SlavePointStore,
            protocol::{
                point_type::{
                    is_double_point_type, is_integrated_total_type, is_simulatable_monitor_type, is_single_point_type,
                },
                quality::apply_quality_metadata,
            },
        },
        types::DataPoint,
    },
    errors::{AppError, AppResult, ConfigError},
};

/// 数据点键：(公共地址, 信息对象地址)
pub(crate) type PointKey = (u16, u32);
/// 数据点映射表
pub(crate) type PointMap = HashMap<PointKey, DataPoint>;

/// 选点模拟会话
#[derive(Debug, Clone)]
pub(crate) struct SelectedPointSimulationSession {
    /// 波形配置
    pub(crate) config: WaveformSimulationConfig,
    /// 参与模拟的数据点集合
    pub(crate) points: HashSet<PointKey>,
}

/// 模拟运行时状态
#[derive(Debug, Default)]
pub(crate) struct SimulationRuntime {
    /// 选点模拟会话（优先级高于全局模拟）
    pub(crate) selected_session: Option<SelectedPointSimulationSession>,
    /// 选点模拟的运行时状态
    pub(crate) selected_state_by_point: HashMap<PointKey, WaveformRuntimeState>,
    /// 全局模拟的运行时状态
    pub(crate) global_state_by_point: HashMap<PointKey, WaveformRuntimeState>,
    /// 全局模拟的波形模板
    pub(crate) global_template_by_point: HashMap<PointKey, WaveformSimulationConfig>,
    /// 全局时钟滴答计数
    pub(crate) tick: u64,
}

/// 构造数据点键
#[inline]
pub(crate) fn point_key(common_address: u16, address: u32) -> PointKey {
    (common_address, address)
}

/// 配置选点模拟会话
pub(crate) fn configure_selected_point_simulation(
    default_common_address: u16,
    data_points: &PointMap,
    runtime: &mut SimulationRuntime,
    request: StartSelectedPointSimulationRequest,
) -> AppResult<()> {
    // 规范化选点列表，过滤无效地址
    let normalized_points = normalize_selected_simulation_points(default_common_address, &request.points);
    if normalized_points.is_empty() {
        return Err(AppError::Config(ConfigError::Other("未提供可模拟的数据点".to_string())));
    }

    let mut selected_value_model: Option<ValueModel> = None;
    for key in &normalized_points {
        let point = data_points.get(key).ok_or_else(|| {
            AppError::Config(ConfigError::Other(format!("模拟数据点不存在: CA={}, IOA={}", key.0, key.1)))
        })?;
        if !is_simulatable_monitor_type(point.type_id) {
            return Err(AppError::Config(ConfigError::Other(format!("TypeID {} 不支持数据模拟", point.type_id))));
        }
        let (value_model, simulation) = lookup_capability(point.type_id)
            .and_then(|capability| capability.simulation.map(|simulation| (capability.value_model, simulation)))
            .ok_or_else(|| {
                AppError::Config(ConfigError::Other(format!("TypeID {} 缺少模拟能力定义", point.type_id)))
            })?;
        if selected_value_model.is_some_and(|selected| selected != value_model) {
            return Err(AppError::Config(ConfigError::Other("所选数据点使用不同的协议值模型".to_string())));
        }
        if !simulation.allowed_waveforms.contains(&request.config.waveform_type.as_str()) {
            return Err(AppError::Config(ConfigError::Other(format!(
                "TypeID {} 不支持波形 {}",
                point.type_id,
                request.config.waveform_type.as_str()
            ))));
        }
        selected_value_model = Some(value_model);
    }

    // 创建新会话并清空旧状态
    runtime.selected_session = Some(SelectedPointSimulationSession {
        config: normalize_waveform_simulation_config(request.config),
        points: normalized_points,
    });
    runtime.selected_state_by_point.clear();
    Ok(())
}

/// 清除选点模拟会话
pub(crate) fn clear_selected_point_simulation(runtime: &mut SimulationRuntime) {
    runtime.selected_session = None;
    runtime.selected_state_by_point.clear();
}

/// 执行统一模拟步进（支持选点模拟和全局模拟）
pub(crate) async fn apply_unified_simulation_step(
    data_points: &Arc<SlavePointStore>,
    profile: &SimulationProfile,
    seed: &Arc<Mutex<u64>>,
    simulation_runtime: &Arc<Mutex<SimulationRuntime>>,
) -> Vec<DataPoint> {
    let seed_value = *seed.lock().await;
    let mut changed = Vec::new();
    let mut points = data_points.transaction().await;
    let mut runtime = simulation_runtime.lock().await;
    let selected_session = runtime.selected_session.clone();
    let selected_points = selected_session.as_ref().map(|session| session.points.clone()).unwrap_or_default();

    // 清理全局模拟状态：移除被选点模拟覆盖的点
    runtime.global_state_by_point.retain(|key, _| profile.enabled && !selected_points.contains(key));
    runtime.global_template_by_point.retain(|key, _| profile.enabled && !selected_points.contains(key));
    if selected_session.is_none() {
        runtime.selected_state_by_point.clear();
    }

    // 递增全局时钟
    runtime.tick = runtime.tick.wrapping_add(1);
    let tick = runtime.tick;

    points.for_each_mut(|key, point| {
        // 跳过不可模拟的点（如控制点）
        if !is_simulatable_monitor_type(point.type_id) {
            return false;
        }
        // 计算下一个值：选点模拟优先于全局模拟
        let next_value = if let Some(session) = selected_session.as_ref().filter(|session| session.points.contains(key))
        {
            // 使用选点模拟配置
            let runtime_state = runtime.selected_state_by_point.entry(*key).or_default();
            Some(compute_waveform_simulation_value(&session.config, point, tick, seed_value, *key, runtime_state))
        } else {
            // 清理选点状态
            runtime.selected_state_by_point.remove(key);
            if !profile.enabled {
                // 全局模拟未启用，清理状态
                runtime.global_state_by_point.remove(key);
                runtime.global_template_by_point.remove(key);
                None
            } else {
                // 使用全局模拟配置（首次为点生成默认模板）
                let template = runtime
                    .global_template_by_point
                    .entry(*key)
                    .or_insert_with(|| default_waveform_config_for_point(point))
                    .clone();
                let runtime_state = runtime.global_state_by_point.entry(*key).or_default();
                Some(compute_waveform_simulation_value(&template, point, tick, seed_value, *key, runtime_state))
            }
        };

        let Some(next_value) = next_value else {
            return false;
        };
        // 跳过值未变化的点
        if (point.value - next_value).abs() < f64::EPSILON {
            return false;
        }

        // 更新数据点
        point.value = next_value;
        point.timestamp = chrono::Utc::now();
        apply_quality_metadata(point, point.quality);
        changed.push(point.clone());
        true
    });

    changed
}

/// 执行简单模拟步进（基于随机噪声的简化模拟）
pub(crate) async fn apply_simulation_step(
    data_points: &Arc<SlavePointStore>,
    profile: &SimulationProfile,
    seed: &Arc<Mutex<u64>>,
    common_address: Option<u16>,
) -> Vec<DataPoint> {
    let mut changed = Vec::new();
    let mut points = data_points.transaction().await;
    let mut guard = seed.lock().await;

    points.for_each_mut(|_, point| {
        // 过滤公共地址
        if let Some(target_common_address) = common_address {
            if point.common_address != Some(target_common_address) {
                return false;
            }
        }
        // 跳过不可模拟的点
        if !is_simulatable_monitor_type(point.type_id) {
            return false;
        }
        let previous_value = point.value;

        // 生成随机噪声 [-1.0, 1.0]
        let noise = next_lcg_signed(&mut guard);

        // 根据点类型应用不同的模拟逻辑
        if is_single_point_type(point.type_id) {
            // 单点信息：0/1 翻转
            let mut next = if point.value > 0.5 { 1.0 } else { 0.0 };
            if noise.abs() > 0.65 {
                next = if next > 0.5 { 0.0 } else { 1.0 };
            }
            point.value = next;
        } else if is_double_point_type(point.type_id) {
            // 双点信息：1/2 切换
            let current = point.value.round().clamp(0.0, 3.0) as i64;
            let next = if noise.abs() > 0.65 {
                match current {
                    1 => 2.0,
                    2 => 1.0,
                    3 => 2.0,
                    _ => 1.0,
                }
            } else {
                current as f64
            };
            point.value = next;
        } else if is_integrated_total_type(point.type_id) {
            // 累计量：步进增减
            let step = ((profile.delta_ratio.abs().max(0.01) * 100.0).round() as i64).clamp(1, 100);
            let current = point.value.round() as i64;
            let next = if noise > 0.55 {
                current.saturating_add(step)
            } else if noise < -0.55 {
                current.saturating_sub(step)
            } else {
                current
            };
            point.value = (next as f64).clamp(profile.min_value, profile.max_value).round();
        } else {
            // 测量值：基于当前值的比例变化
            let base = point.value.abs().max(1.0);
            let delta = base * profile.delta_ratio * noise;
            point.value = (point.value + delta).clamp(profile.min_value, profile.max_value);
        }

        point.value = normalize_simulated_value_by_type(point.type_id, point.value);
        if (point.value - previous_value).abs() < f64::EPSILON {
            return false;
        }
        point.timestamp = chrono::Utc::now();
        changed.push(point.clone());
        true
    });
    changed
}
