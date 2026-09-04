//! 波形模拟数学计算

use std::collections::HashSet;

use super::{SelectedSimulationPoint, WaveformSimulationConfig, WaveformSimulationType};
use crate::core::{
    iec104_registry::{ValueModel, lookup_capability},
    types::DataPoint,
};

/// 波形运行时状态（用于有状态波形如 RandomWalk、Counter）
#[derive(Debug, Clone, Default)]
pub(crate) struct WaveformRuntimeState {
    /// 上次更新的时间桶
    pub(crate) last_bucket: Option<i64>,
    /// 当前累积值
    pub(crate) current_value: Option<f64>,
}

/// 生成有符号随机数 [-1.0, 1.0]（线性同余生成器）
pub(crate) fn next_lcg_signed(seed: &mut u64) -> f64 {
    // LCG 算法：seed = seed * a + c
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let raw = (*seed >> 33) as u32;
    let unit = raw as f64 / u32::MAX as f64; // [0.0, 1.0]
    unit * 2.0 - 1.0 // 转换为 [-1.0, 1.0]
}

/// 规范化波形周期相位 [0.0, 1.0]
pub(crate) fn normalize_waveform_cycle(phase: f64, period_seconds: f64) -> f64 {
    let safe_period = if period_seconds.is_finite() && period_seconds > 0.0 { period_seconds } else { 1.0 };
    (phase / safe_period).rem_euclid(1.0)
}

/// 规范化选点列表，过滤无效地址
pub(crate) fn normalize_selected_simulation_points(
    default_common_address: u16,
    points: &[SelectedSimulationPoint],
) -> HashSet<(u16, u32)> {
    points
        .iter()
        .filter_map(|point| {
            let address = point.address;
            // 过滤地址为 0 的点
            if address == 0 {
                return None;
            }
            Some((point.common_address.unwrap_or(default_common_address), address))
        })
        .collect()
}

/// 规范化波形配置参数（校验并修正无效值）
pub(crate) fn normalize_waveform_simulation_config(raw: WaveformSimulationConfig) -> WaveformSimulationConfig {
    // 规范化时间参数（单位：tick）
    let waveform_period = normalize_tick_count(raw.waveform_period, 4.0, 1.0, 300.0);
    let waveform_pulse_width = normalize_tick_count(raw.waveform_pulse_width, 1.0, 1.0, waveform_period);
    let waveform_rise_time = normalize_tick_count(raw.waveform_rise_time, 2.0, 1.0, 300.0);
    let waveform_hold_time = normalize_tick_count(raw.waveform_hold_time, 2.0, 1.0, 300.0);
    let waveform_fall_time = normalize_tick_count(raw.waveform_fall_time, 2.0, 1.0, 300.0);

    // 规范化数值参数
    let waveform_step_count = raw.waveform_step_count.clamp(2, 64);
    let waveform_amplitude = raw.waveform_amplitude.abs();
    let waveform_step_size = raw.waveform_step_size.abs();
    let waveform_duty_cycle = raw.waveform_duty_cycle.clamp(1.0, 99.0);
    let waveform_damping = raw.waveform_damping.clamp(0.05, 10.0);

    // 规范化基准值和目标值
    let waveform_base = if raw.waveform_base.is_finite() { raw.waveform_base } else { 0.0 };
    let waveform_target_value = if raw.waveform_target_value.is_finite() {
        raw.waveform_target_value
    } else {
        waveform_base + waveform_amplitude
    };
    let fixed_value = if raw.fixed_value.is_finite() { raw.fixed_value } else { waveform_base };

    // 计算并规范化值域范围
    let min_value = if raw.waveform_min_value.is_finite() {
        raw.waveform_min_value
    } else {
        waveform_base - waveform_amplitude.max(5.0) * 3.0
    };
    let max_value = if raw.waveform_max_value.is_finite() {
        raw.waveform_max_value
    } else {
        waveform_base + waveform_amplitude.max(5.0) * 3.0
    };
    let waveform_min_value = min_value.min(max_value);
    let waveform_max_value = min_value.max(max_value);

    // 规范化复位值
    let waveform_reset_value = if raw.waveform_reset_value.is_finite() {
        raw.waveform_reset_value.clamp(waveform_min_value, waveform_max_value)
    } else {
        waveform_base.clamp(waveform_min_value, waveform_max_value)
    };

    WaveformSimulationConfig {
        fixed_value,
        waveform_type: raw.waveform_type,
        waveform_base,
        waveform_amplitude,
        waveform_period,
        waveform_duty_cycle,
        waveform_pulse_width,
        waveform_rise_time,
        waveform_hold_time,
        waveform_fall_time,
        waveform_step_size,
        waveform_step_count,
        waveform_target_value,
        waveform_damping,
        waveform_min_value,
        waveform_max_value,
        waveform_reset_value,
    }
}

/// 计算波形模拟值
pub(crate) fn compute_waveform_simulation_value(
    config: &WaveformSimulationConfig,
    point: &DataPoint,
    tick: u64,
    seed: u64,
    key: (u16, u32),
    runtime_state: &mut WaveformRuntimeState,
) -> f64 {
    let config = normalize_waveform_simulation_config(config.clone());

    // 计算周期长度（tick 数）
    let cycle_ticks = resolve_cycle_ticks(&config);
    // 为每个点生成确定性的相位偏移（避免所有点同步变化）
    let phase_ticks = deterministic_point_unit(seed, key, 1) * cycle_ticks;
    // 应用相位偏移后的时钟
    let shifted_tick = tick as f64 + phase_ticks;
    // 当前周期内的相位位置（tick）
    let cycle_phase_ticks = if cycle_ticks <= 0.0 { 0.0 } else { shifted_tick.rem_euclid(cycle_ticks) };
    // 归一化相位 [0.0, 1.0]
    let t = normalize_waveform_cycle(shifted_tick, cycle_ticks);
    // 刷新周期（用于离散波形）
    let refresh_ticks = config.waveform_period.max(1.0);
    // 生成确定性随机噪声
    let signed_noise = deterministic_point_signed(seed, key, shifted_tick.floor() as u64 + 7);

    // 根据波形类型计算值
    let computed = match config.waveform_type {
        WaveformSimulationType::Fixed => config.fixed_value,
        WaveformSimulationType::Step => {
            // 方波：前半周期基准值，后半周期基准值+幅度
            if t < 0.5 { config.waveform_base } else { config.waveform_base + config.waveform_amplitude }
        }
        WaveformSimulationType::Pulse => {
            // 脉冲：脉宽内高电平，其余低电平
            if cycle_phase_ticks < config.waveform_pulse_width {
                config.waveform_base + config.waveform_amplitude
            } else {
                config.waveform_base
            }
        }
        WaveformSimulationType::Square => {
            // 占空比方波
            if t < config.waveform_duty_cycle / 100.0 {
                config.waveform_base + config.waveform_amplitude
            } else {
                config.waveform_base
            }
        }
        WaveformSimulationType::SawUp => config.waveform_base + config.waveform_amplitude * t,
        WaveformSimulationType::SawDown => config.waveform_base - config.waveform_amplitude * t,
        WaveformSimulationType::Triangle => {
            // 三角波：0-0.25 上升，0.25-0.75 下降，0.75-1.0 上升
            let triangle = if t < 0.25 {
                t * 4.0
            } else if t < 0.75 {
                2.0 - t * 4.0
            } else {
                t * 4.0 - 4.0
            };
            config.waveform_base + config.waveform_amplitude * triangle
        }
        WaveformSimulationType::Sine => {
            config.waveform_base + config.waveform_amplitude * (std::f64::consts::PI * 2.0 * t).sin()
        }
        WaveformSimulationType::RampHoldFall => {
            // 上升-保持-下降波形
            if cycle_phase_ticks <= config.waveform_rise_time {
                // 上升阶段
                config.waveform_base
                    + config.waveform_amplitude * (cycle_phase_ticks / config.waveform_rise_time.max(1.0))
            } else if cycle_phase_ticks <= config.waveform_rise_time + config.waveform_hold_time {
                // 保持阶段
                config.waveform_base + config.waveform_amplitude
            } else {
                // 下降阶段
                let fall_phase = cycle_phase_ticks - config.waveform_rise_time - config.waveform_hold_time;
                config.waveform_base
                    + config.waveform_amplitude * (1.0 - fall_phase / config.waveform_fall_time.max(1.0))
            }
        }
        WaveformSimulationType::Stair => {
            // 阶梯波：每个刷新周期递增一个台阶
            let bucket = (shifted_tick / refresh_ticks).floor() as i64 % config.waveform_step_count.max(1) as i64;
            config.waveform_base + config.waveform_step_size * bucket as f64
        }
        WaveformSimulationType::ExpApproach => {
            // 指数逼近目标值
            let factor = 1.0 - (-5.0 * t).exp();
            config.waveform_base + (config.waveform_target_value - config.waveform_base) * factor
        }
        WaveformSimulationType::DampedOscillation => {
            // 阻尼振荡
            let envelope = (-(config.waveform_damping * t * 4.0)).exp();
            config.waveform_base + config.waveform_amplitude * (std::f64::consts::PI * 2.0 * t).sin() * envelope
        }
        WaveformSimulationType::Random => config.waveform_base + config.waveform_amplitude * signed_noise,
        WaveformSimulationType::RandomWalk => {
            // 随机游走：每个刷新周期随机步进
            let bucket = (shifted_tick / refresh_ticks).floor() as i64;
            let mut current_value = runtime_state.current_value.unwrap_or(config.waveform_base);
            let mut last_bucket = runtime_state.last_bucket.unwrap_or(bucket - 1);

            // 补齐缺失的步进
            if bucket > last_bucket {
                for index in (last_bucket + 1)..=bucket {
                    let step_noise = deterministic_point_signed(seed, key, index as u64 + 17);
                    current_value = (current_value + config.waveform_step_size * step_noise)
                        .clamp(config.waveform_min_value, config.waveform_max_value);
                }
                last_bucket = bucket;
            }

            // 保存状态
            runtime_state.current_value = Some(current_value);
            runtime_state.last_bucket = Some(last_bucket);
            current_value
        }
        WaveformSimulationType::Counter => {
            // 计数器：每个刷新周期递增，超过最大值复位
            let bucket = (shifted_tick / refresh_ticks).floor() as i64;
            let mut current_value = runtime_state.current_value.unwrap_or(config.waveform_base);
            let mut last_bucket = runtime_state.last_bucket.unwrap_or(bucket - 1);

            // 补齐缺失的计数
            if bucket > last_bucket {
                for _index in (last_bucket + 1)..=bucket {
                    current_value += config.waveform_step_size;
                    if current_value > config.waveform_max_value {
                        current_value = config.waveform_reset_value;
                    }
                }
                last_bucket = bucket;
            }

            // 保存状态
            runtime_state.current_value = Some(current_value);
            runtime_state.last_bucket = Some(last_bucket);
            current_value
        }
    };

    // 根据数据点类型规范化输出值
    normalize_simulated_value_by_type(point.type_id, computed)
}

/// 为数据点生成默认波形配置
///
/// 根据数据点类型生成适合的默认波形配置，不同类型使用不同的波形和参数范围。
pub(crate) fn default_waveform_config_for_point(point: &DataPoint) -> WaveformSimulationConfig {
    // 规范化参考值
    let reference_value = normalize_simulated_value_by_type(point.type_id, point.value);

    let value_model = lookup_capability(point.type_id).map(|capability| capability.value_model);

    // 单点信息：使用方波在 0/1 之间切换
    if value_model == Some(ValueModel::Boolean) {
        return normalize_waveform_simulation_config(WaveformSimulationConfig {
            waveform_type: WaveformSimulationType::Step,
            fixed_value: if reference_value >= 0.5 { 1.0 } else { 0.0 },
            waveform_base: 0.0,
            waveform_amplitude: 1.0,
            waveform_period: 4.0,
            waveform_min_value: 0.0,
            waveform_max_value: 1.0,
            waveform_reset_value: if reference_value >= 0.5 { 1.0 } else { 0.0 },
            ..WaveformSimulationConfig::default()
        });
    }

    // 双点信息：保留完整的四态协议值域
    if value_model == Some(ValueModel::DoublePoint) {
        let normalized = reference_value.round().clamp(0.0, 3.0);
        return normalize_waveform_simulation_config(WaveformSimulationConfig {
            waveform_type: WaveformSimulationType::Step,
            fixed_value: normalized,
            waveform_base: 1.0,
            waveform_amplitude: 1.0,
            waveform_period: 4.0,
            waveform_min_value: 0.0,
            waveform_max_value: 3.0,
            waveform_reset_value: normalized,
            ..WaveformSimulationConfig::default()
        });
    }

    // 步位置信息：使用阶梯波在 -64 到 63 范围内变化
    if value_model == Some(ValueModel::StepPosition) {
        let normalized = reference_value.round().clamp(-64.0, 63.0);
        return normalize_waveform_simulation_config(WaveformSimulationConfig {
            waveform_type: WaveformSimulationType::Stair,
            fixed_value: normalized,
            waveform_base: normalized,
            waveform_period: 1.0,
            waveform_step_size: 1.0,
            waveform_step_count: 8,
            waveform_min_value: -64.0,
            waveform_max_value: 63.0,
            waveform_reset_value: normalized,
            ..WaveformSimulationConfig::default()
        });
    }

    if matches!(value_model, Some(ValueModel::Bitstring32)) {
        return normalize_waveform_simulation_config(WaveformSimulationConfig {
            waveform_type: WaveformSimulationType::Fixed,
            fixed_value: reference_value,
            waveform_base: reference_value,
            waveform_min_value: 0.0,
            waveform_max_value: u32::MAX as f64,
            waveform_reset_value: reference_value,
            ..WaveformSimulationConfig::default()
        });
    }

    // 累计量：使用计数器波形递增
    if value_model == Some(ValueModel::Counter) {
        let normalized = reference_value.round();
        return normalize_waveform_simulation_config(WaveformSimulationConfig {
            waveform_type: WaveformSimulationType::Counter,
            fixed_value: normalized,
            waveform_base: normalized,
            waveform_period: 1.0,
            waveform_step_size: 1.0,
            waveform_min_value: normalized.min(0.0),
            waveform_max_value: normalized + 100.0,
            waveform_reset_value: normalized,
            ..WaveformSimulationConfig::default()
        });
    }

    // 通用测量值：使用正弦波在参考值附近波动
    let amplitude = (reference_value.abs() * 0.2).max(5.0);
    let range = (amplitude * 3.0).max(5.0);
    normalize_waveform_simulation_config(WaveformSimulationConfig {
        waveform_type: WaveformSimulationType::Sine,
        fixed_value: reference_value,
        waveform_base: reference_value,
        waveform_amplitude: amplitude,
        waveform_period: 10.0,
        waveform_min_value: reference_value - range,
        waveform_max_value: reference_value + range,
        waveform_reset_value: reference_value,
        ..WaveformSimulationConfig::default()
    })
}

/// 根据数据点类型规范化模拟值
///
/// 不同类型的数据点有不同的值域限制，此函数确保模拟值符合类型规范。
pub(crate) fn normalize_simulated_value_by_type(type_id: u8, value: f64) -> f64 {
    let Some(capability) = lookup_capability(type_id) else {
        return value;
    };
    let Some(simulation) = capability.simulation else {
        return value;
    };
    let finite_value = if value.is_finite() { value } else { 0.0 };
    match capability.value_model {
        ValueModel::Boolean => {
            if finite_value >= 0.5 {
                1.0
            } else {
                0.0
            }
        }
        ValueModel::DoublePoint
        | ValueModel::StepPosition
        | ValueModel::Bitstring32
        | ValueModel::ScaledI16
        | ValueModel::Counter => finite_value.round().clamp(simulation.min, simulation.max),
        ValueModel::Normalized | ValueModel::Float32 => finite_value.clamp(simulation.min, simulation.max),
        ValueModel::None | ValueModel::Structured => finite_value,
    }
}

/// 生成确定性的单位随机数 [0.0, 1.0]
///
/// 使用数据点键和盐值生成确定性的随机数，确保相同输入产生相同输出。
#[inline]
fn deterministic_point_unit(seed: u64, key: (u16, u32), salt: u64) -> f64 {
    let hashed = ((seed as f64 + salt as f64) * 12.9898 + (key.0 as f64) * 78.233 + (key.1 as f64) * 37.719).sin()
        * 43_758.545_312_3;
    hashed.fract().abs()
}

/// 生成确定性的有符号随机数 [-1.0, 1.0]
///
/// 基于 `deterministic_point_unit` 转换为有符号范围。
#[inline]
fn deterministic_point_signed(seed: u64, key: (u16, u32), salt: u64) -> f64 {
    deterministic_point_unit(seed, key, salt) * 2.0 - 1.0
}

/// 规范化 tick 计数值
///
/// 确保 tick 计数值在有效范围内，无效值使用回退值。
#[inline]
fn normalize_tick_count(value: f64, fallback: f64, min: f64, max: f64) -> f64 {
    let candidate = if value.is_finite() { value } else { fallback };
    candidate.round().clamp(min, max)
}

/// 计算波形周期长度（tick 数）
///
/// 根据波形类型计算一个完整周期的 tick 数量。
#[inline]
fn resolve_cycle_ticks(config: &WaveformSimulationConfig) -> f64 {
    match config.waveform_type {
        WaveformSimulationType::RampHoldFall => {
            (config.waveform_rise_time + config.waveform_hold_time + config.waveform_fall_time).max(1.0)
        }
        WaveformSimulationType::Stair => (config.waveform_step_count as f64 * config.waveform_period).max(1.0),
        _ => config.waveform_period.max(1.0),
    }
}
