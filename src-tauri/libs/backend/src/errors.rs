//! 应用级统一错误定义。

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("协议错误: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("网络错误: {0}")]
    Network(#[from] NetworkError),
    #[error("数据库错误: {0}")]
    Database(#[from] DatabaseError),
    #[error("配置错误: {0}")]
    Config(#[from] ConfigError),
    #[error("日志错误: {0}")]
    Log(#[from] LogError),
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error(transparent)]
    ParserParse(#[from] iec60870_parser::ParseError),
    #[error(transparent)]
    ParserValidation(#[from] iec60870_parser::ValidationError),
    #[error(transparent)]
    ParserStream(#[from] iec60870_parser::StreamError),
    #[error(transparent)]
    ParserConvert(#[from] iec60870_parser::ConvertError),
    #[error("无效数据: {0}")]
    InvalidData(String),
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("协议错误: {0}")]
    Protocol(String),
    #[error("连接超时")]
    Timeout,
    #[error("连接已关闭")]
    ConnectionClosed,
    #[error("未连接")]
    NotConnected,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLite错误: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("数据库结构不兼容: {0}")]
    IncompatibleSchema(String),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("站点未找到: {0}")]
    StationNotFound(String),
    #[error("配置错误: {0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum LogError {
    #[error("日志IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("日志系统错误: {0}")]
    Flexi(#[from] flexi_logger::FlexiLoggerError),
    #[error("日志初始化锁已损坏")]
    InitializationLockPoisoned,
    #[error("日志系统尚未初始化")]
    NotInitialized,
}

pub type AppResult<T> = Result<T, AppError>;
pub type ProtocolResult<T> = Result<T, ProtocolError>;
pub type NetworkResult<T> = Result<T, NetworkError>;
pub type DatabaseResult<T> = Result<T, DatabaseError>;
pub type ConfigResult<T> = Result<T, ConfigError>;
pub type LogResult<T> = Result<T, LogError>;

/// IEC104 命令执行阶段使用的稳定错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Iec104ErrorCode {
    InvalidCommonAddress,
    InvalidObjectAddress,
    InvalidQualifier,
    InvalidValue,
    UnsupportedCommand,
    NotConnected,
    ProtocolState,
    EncodeFailed,
    SendFailed,
    InternalError,
}

impl Iec104ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCommonAddress => "INVALID_COMMON_ADDRESS",
            Self::InvalidObjectAddress => "INVALID_OBJECT_ADDRESS",
            Self::InvalidQualifier => "INVALID_QUALIFIER",
            Self::InvalidValue => "INVALID_VALUE",
            Self::UnsupportedCommand => "UNSUPPORTED_COMMAND",
            Self::NotConnected => "NOT_CONNECTED",
            Self::ProtocolState => "PROTOCOL_STATE",
            Self::EncodeFailed => "ENCODE_FAILED",
            Self::SendFailed => "SEND_FAILED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iec104ValidationError {
    pub code: Iec104ErrorCode,
    pub message: String,
}

impl Iec104ValidationError {
    pub fn new(code: Iec104ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for Iec104ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for Iec104ValidationError {}

#[derive(Debug, Clone)]
pub struct Iec104ExecutionError {
    pub code: Iec104ErrorCode,
    pub message: String,
    pub trace_id: Option<String>,
}

impl Iec104ExecutionError {
    pub fn new(code: Iec104ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), trace_id: None }
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }
}

impl fmt::Display for Iec104ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(trace_id) = &self.trace_id {
            write!(f, "{}: {} (trace_id={})", self.code.as_str(), self.message, trace_id)
        } else {
            write!(f, "{}: {}", self.code.as_str(), self.message)
        }
    }
}

impl std::error::Error for Iec104ExecutionError {}

impl From<Iec104ValidationError> for Iec104ExecutionError {
    fn from(value: Iec104ValidationError) -> Self {
        Self::new(value.code, value.message)
    }
}
