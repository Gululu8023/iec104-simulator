//! IEC104 主站与从站模拟器共享后端库。

pub mod api;
mod app;
pub mod core;
pub mod db;
pub mod errors;
pub mod network;
pub mod services;
pub mod utils;

pub use core::types::*;

pub use app::{AppRole, setup_app};
pub use db::{DatabaseManager, DatabaseService};
pub use errors::{
    AppError, AppResult, ConfigError, DatabaseError, Iec104ErrorCode, Iec104ExecutionError, Iec104ValidationError,
    LogError, NetworkError, ProtocolError,
};
pub use network::{NetworkConfig, NetworkEvent};
pub use services::{RuntimeEventSink, StationManager};
