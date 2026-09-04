//! 从站数据上报子模块。
//!
//! 本模块负责从站的数据上报功能，包括：
//!
//! - **广播运行时**：管理数据点变化的广播上报
//! - **策略存储**：管理上报策略配置
//! - **SOE 支持**：事件序列（Sequence of Events）记录和上报
//! - **突发上报**：数据点变化的突发上报和合并优化

pub(crate) mod broadcast_runtime;
pub(crate) mod policy_store;
pub(crate) mod soe_support;
pub(crate) mod spontaneous;
