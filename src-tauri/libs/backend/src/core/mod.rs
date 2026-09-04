//! 核心业务逻辑模块
//!
//! 这个模块包含了IEC104协议模拟器的核心业务逻辑，
//! 分为主站和从站两个独立的子模块。

pub mod iec104_registry;
pub mod master;
pub mod protocol_adapter;
pub(crate) mod shared;
pub mod slave;
pub mod types;

// 重新导出核心类型
pub use types::*;
