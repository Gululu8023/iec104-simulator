//! 从站仿真引擎模块。
//!
//! 本模块提供从站数据点的动态模拟和场景注入功能,包括:
//!
//! - **初始化模板**:批量生成数据点的初始值配置
//! - **持续模拟**:周期性随机变化模拟,支持可复现的随机种子
//! - **选点波形**:对指定点应用 15 种波形类型(正弦、方波、锯齿波、阻尼振荡等)
//! - **场景注入**:雪崩场景、波形场景、通信故障场景
//! - **数学引擎**:波形生成算法和数值计算
//! - **运行时管理**:模拟任务的启动、停止和状态跟踪
//!
//! # 子模块
//!
//! - `engine` - 波形生成引擎,实现 15 种波形算法
//! - `math` - 数学工具函数,提供数值计算支持
//! - `runtime` - 模拟运行时,管理模拟任务的生命周期

pub(crate) mod engine;
pub(crate) mod math;
pub(crate) mod runtime;

use serde::{Deserialize, Serialize};

/// 数据点初始化模板。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializationProfile {
    /// 初始化点数量。
    #[serde(default = "default_point_count")]
    pub point_count: u32,
    /// 起始 IOA。
    #[serde(default = "default_start_address")]
    pub start_address: u32,
    /// 初始值基线。
    #[serde(default = "default_base_value")]
    pub base_value: f64,
    /// 相邻点增量。
    #[serde(default = "default_step")]
    pub step: f64,
    /// 初始品质位。
    #[serde(default)]
    pub quality: u8,
}

impl Default for InitializationProfile {
    fn default() -> Self {
        Self {
            point_count: default_point_count(),
            start_address: default_start_address(),
            base_value: default_base_value(),
            step: default_step(),
            quality: 0,
        }
    }
}

/// 从站动态模拟配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationProfile {
    /// 是否启用持续模拟任务。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 模拟步进周期（毫秒）。
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    /// 变化比例（以当前值为基准）。
    #[serde(default = "default_delta_ratio")]
    pub delta_ratio: f64,
    /// 值下限。
    #[serde(default = "default_min_value")]
    pub min_value: f64,
    /// 值上限。
    #[serde(default = "default_max_value")]
    pub max_value: f64,
    /// 随机种子（用于可复现模拟）。
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// 模拟后是否立即主动上送。
    #[serde(default = "default_auto_broadcast")]
    pub auto_broadcast: bool,
}

impl Default for SimulationProfile {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            interval_ms: default_interval_ms(),
            delta_ratio: default_delta_ratio(),
            min_value: default_min_value(),
            max_value: default_max_value(),
            seed: default_seed(),
            auto_broadcast: default_auto_broadcast(),
        }
    }
}

/// 选点波形模拟类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WaveformSimulationType {
    Fixed,
    Step,
    Pulse,
    Square,
    SawUp,
    SawDown,
    Triangle,
    Sine,
    RampHoldFall,
    Stair,
    ExpApproach,
    DampedOscillation,
    Random,
    RandomWalk,
    Counter,
}

impl Default for WaveformSimulationType {
    fn default() -> Self {
        Self::Sine
    }
}

impl WaveformSimulationType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Step => "step",
            Self::Pulse => "pulse",
            Self::Square => "square",
            Self::SawUp => "saw-up",
            Self::SawDown => "saw-down",
            Self::Triangle => "triangle",
            Self::Sine => "sine",
            Self::RampHoldFall => "ramp-hold-fall",
            Self::Stair => "stair",
            Self::ExpApproach => "exp-approach",
            Self::DampedOscillation => "damped-oscillation",
            Self::Random => "random",
            Self::RandomWalk => "random-walk",
            Self::Counter => "counter",
        }
    }
}

/// 选点波形模拟配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformSimulationConfig {
    /// 固定值（Fixed 波形使用）
    #[serde(default)]
    pub fixed_value: f64,
    /// 波形类型
    #[serde(default)]
    pub waveform_type: WaveformSimulationType,
    /// 波形基准值
    #[serde(default)]
    pub waveform_base: f64,
    /// 波形幅度
    #[serde(default)]
    pub waveform_amplitude: f64,
    /// 波形周期（tick 数）
    #[serde(default = "default_waveform_tick_period")]
    pub waveform_period: f64,
    /// 占空比（百分比，用于 Square 波形）
    #[serde(default = "default_waveform_duty_cycle")]
    pub waveform_duty_cycle: f64,
    /// 脉冲宽度（tick 数，用于 Pulse 波形）
    #[serde(default = "default_waveform_pulse_width")]
    pub waveform_pulse_width: f64,
    /// 上升时间（tick 数，用于 RampHoldFall 波形）
    #[serde(default = "default_waveform_rise_ticks")]
    pub waveform_rise_time: f64,
    /// 保持时间（tick 数，用于 RampHoldFall 波形）
    #[serde(default = "default_waveform_hold_ticks")]
    pub waveform_hold_time: f64,
    /// 下降时间（tick 数，用于 RampHoldFall 波形）
    #[serde(default = "default_waveform_fall_ticks")]
    pub waveform_fall_time: f64,
    /// 步进大小（用于 Stair、RandomWalk、Counter 波形）
    #[serde(default = "default_waveform_step_size")]
    pub waveform_step_size: f64,
    /// 步进数量（用于 Stair 波形）
    #[serde(default = "default_waveform_step_count")]
    pub waveform_step_count: i32,
    /// 目标值（用于 ExpApproach 波形）
    #[serde(default)]
    pub waveform_target_value: f64,
    /// 阻尼系数（用于 DampedOscillation 波形）
    #[serde(default = "default_waveform_damping")]
    pub waveform_damping: f64,
    /// 值下限
    #[serde(default = "default_waveform_min_value")]
    pub waveform_min_value: f64,
    /// 值上限
    #[serde(default = "default_waveform_max_value")]
    pub waveform_max_value: f64,
    /// 复位值（用于 Counter 波形）
    #[serde(default)]
    pub waveform_reset_value: f64,
}

impl Default for WaveformSimulationConfig {
    fn default() -> Self {
        Self {
            fixed_value: 0.0,
            waveform_type: WaveformSimulationType::Sine,
            waveform_base: 0.0,
            waveform_amplitude: 1.0,
            waveform_period: default_waveform_tick_period(),
            waveform_duty_cycle: default_waveform_duty_cycle(),
            waveform_pulse_width: default_waveform_pulse_width(),
            waveform_rise_time: default_waveform_rise_ticks(),
            waveform_hold_time: default_waveform_hold_ticks(),
            waveform_fall_time: default_waveform_fall_ticks(),
            waveform_step_size: default_waveform_step_size(),
            waveform_step_count: default_waveform_step_count(),
            waveform_target_value: 1.0,
            waveform_damping: default_waveform_damping(),
            waveform_min_value: default_waveform_min_value(),
            waveform_max_value: default_waveform_max_value(),
            waveform_reset_value: 0.0,
        }
    }
}

/// 选点模拟目标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedSimulationPoint {
    /// 信息对象地址
    pub address: u32,
    /// 公共地址（可选，未指定时使用默认公共地址）
    #[serde(default)]
    pub common_address: Option<u16>,
}

/// 选点模拟请求。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StartSelectedPointSimulationRequest {
    /// 要模拟的数据点列表
    #[serde(default)]
    pub points: Vec<SelectedSimulationPoint>,
    /// 波形配置
    #[serde(default)]
    pub config: WaveformSimulationConfig,
}

/// 场景波形类型（用于场景注入）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WaveformType {
    /// 固定值
    Fixed,
    /// 线性
    Linear,
    /// 正弦波
    Sine,
    /// 三角波
    Triangle,
    /// 方波
    Step,
    /// 随机
    Random,
}

impl Default for WaveformType {
    fn default() -> Self {
        Self::Sine
    }
}

fn default_waveform_period_seconds() -> f64 {
    10.0
}

fn default_waveform_tick_period() -> f64 {
    4.0
}

fn default_waveform_duty_cycle() -> f64 {
    50.0
}

fn default_waveform_pulse_width() -> f64 {
    1.0
}

fn default_waveform_rise_ticks() -> f64 {
    2.0
}

fn default_waveform_hold_ticks() -> f64 {
    2.0
}

fn default_waveform_fall_ticks() -> f64 {
    2.0
}

fn default_waveform_step_size() -> f64 {
    1.0
}

fn default_waveform_step_count() -> i32 {
    8
}

fn default_waveform_damping() -> f64 {
    0.8
}

fn default_waveform_min_value() -> f64 {
    -1000.0
}

fn default_waveform_max_value() -> f64 {
    1000.0
}

/// 从站场景注入请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SimulationScenario {
    /// 雪崩场景：批量翻转点值。
    Avalanche {
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        common_address: Option<u16>,
    },
    /// 波形场景：按指定波形更新指定点。
    Waveform {
        address: u32,
        #[serde(default)]
        common_address: Option<u16>,
        #[serde(default)]
        waveform_type: WaveformType,
        #[serde(default)]
        amplitude: f64,
        #[serde(default)]
        base: f64,
        #[serde(default = "default_waveform_period_seconds")]
        period_seconds: f64,
        #[serde(default)]
        phase: f64,
        #[serde(default)]
        fixed_value: f64,
    },
    /// 通信故障场景：仅注入故障日志（不主动断链）。
    CommunicationFault {
        #[serde(default = "default_fault_message")]
        reason: String,
    },
}

impl Default for SimulationScenario {
    fn default() -> Self {
        Self::Avalanche { count: None, common_address: None }
    }
}

fn default_point_count() -> u32 {
    64
}

fn default_start_address() -> u32 {
    1
}

fn default_base_value() -> f64 {
    10.0
}

fn default_step() -> f64 {
    1.0
}

fn default_enabled() -> bool {
    false
}

fn default_interval_ms() -> u64 {
    1000
}

fn default_delta_ratio() -> f64 {
    0.05
}

fn default_min_value() -> f64 {
    -1000.0
}

fn default_max_value() -> f64 {
    1000.0
}

fn default_seed() -> u64 {
    1
}

fn default_auto_broadcast() -> bool {
    true
}

fn default_fault_message() -> String {
    "模拟通信故障".to_string()
}
