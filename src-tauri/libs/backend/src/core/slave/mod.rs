//! 从站核心逻辑模块。
//!
//! 本模块实现 IEC104 从站（服务器端）的完整业务逻辑，包括：
//!
//! - **连接管理**：TCP 监听、多客户端连接管理、连接状态跟踪
//! - **协议处理**：ASDU 解析、命令响应、总召唤/电能召唤处理
//! - **数据管理**：数据点缓存、文件传输、消息存储
//! - **上报机制**：周期上报、变化上报、SOE（事件顺序记录）
//! - **仿真引擎**：波形生成、数据点模拟、场景驱动
//! - **传输层**：帧队列、批量发送、流量控制

/// 连接管理子模块（TCP 监听、连接注册、生命周期管理）
pub(crate) mod connection;
/// 数据管理子模块（数据点运行时、消息存储、文件仓库）
pub(crate) mod data;
/// 协议处理子模块（ASDU 分发、命令响应、召唤处理）
pub(crate) mod protocol;
/// 上报机制子模块（周期上报、变化上报、SOE 支持）
pub(crate) mod reporting;
/// 从站服务主模块（对外 API 入口）
pub mod service;
/// 仿真引擎子模块（波形生成、场景驱动、数据模拟）
pub mod simulation;
/// 传输层子模块（帧队列、批量发送、出站调度）
pub(crate) mod transport;

/// 导出从站服务核心类型
pub use service::{SlaveCommandMismatchPolicy, SlaveService};
/// 导出仿真引擎相关类型
pub use simulation::{
    InitializationProfile, SelectedSimulationPoint, SimulationProfile, SimulationScenario,
    StartSelectedPointSimulationRequest, WaveformSimulationConfig, WaveformSimulationType, WaveformType,
};
