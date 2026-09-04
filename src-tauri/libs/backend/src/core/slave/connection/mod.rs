//! 从站连接管理子模块。
//!
//! 本模块负责从站的连接管理功能，包括：
//!
//! - **连接指标**：跟踪连接的统计信息（收发字节数、APDU 计数、丢帧计数等）
//! - **连接注册表**：管理活动连接的协议实例和链路状态
//! - **监听器生命周期**：TCP 监听器的启动、停止和事件处理

/// 连接指标模块（统计信息跟踪）
pub(crate) mod connection_metrics;
/// 连接注册表模块（协议实例管理）
pub(crate) mod connection_registry;
/// 监听器生命周期模块（TCP 监听器管理）
pub(crate) mod listener_lifecycle;
