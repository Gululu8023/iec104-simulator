//! 服务层模块。
//!
//! 提供高级的业务服务，协调核心模块和 API 层之间的交互。
//!
//! # 核心组件
//!
//! - **StationManager**:站点管理器，统一管理主站和从站的生命周期

pub mod runtime_event_sink;
pub mod station_manager;

pub use runtime_event_sink::RuntimeEventSink;
pub use station_manager::StationManager;
