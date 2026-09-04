//! 基于 flexi_logger 的应用日志封装。

use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use flexi_logger::{
    Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, LoggerHandle, Naming, TS_DASHES_BLANK_COLONS_DOT_BLANK,
    WriteMode, style,
};
use serde::{Deserialize, Serialize};

use crate::errors::{LogError, LogResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rotation {
    Daily,
    Hourly,
    Never,
}

impl Default for Rotation {
    fn default() -> Self {
        Self::Daily
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoggerConfig {
    pub level: LogLevel,
    pub log_dir: PathBuf,
    pub rotation: Rotation,
    pub retention_days: u32,
    pub async_write: bool,
    pub duplicate_to_stdout: bool,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            log_dir: PathBuf::from("./logs"),
            rotation: Rotation::Daily,
            retention_days: 30,
            async_write: true,
            duplicate_to_stdout: true,
        }
    }
}

impl LoggerConfig {
    pub fn with_file(level: LogLevel, log_dir: impl Into<PathBuf>) -> Self {
        Self { level, log_dir: log_dir.into(), ..Default::default() }
    }
}

static LOGGER_HANDLE: OnceLock<LoggerHandle> = OnceLock::new();
static LOGGER_INIT_LOCK: Mutex<()> = Mutex::new(());

pub struct LoggerGuard;

impl Drop for LoggerGuard {
    fn drop(&mut self) {
        if let Some(handle) = LOGGER_HANDLE.get() {
            handle.flush();
        }
    }
}

pub fn init_logger(config: &LoggerConfig) -> LogResult<LoggerGuard> {
    if LOGGER_HANDLE.get().is_some() {
        return Ok(LoggerGuard);
    }

    let _lock = LOGGER_INIT_LOCK.lock().map_err(|_| LogError::InitializationLockPoisoned)?;
    if LOGGER_HANDLE.get().is_some() {
        return Ok(LoggerGuard);
    }

    let handle = init_logger_internal(config)?;
    let _ = LOGGER_HANDLE.set(handle);
    Ok(LoggerGuard)
}

pub fn is_initialized() -> bool {
    LOGGER_HANDLE.get().is_some()
}

pub fn get_handle() -> Option<&'static LoggerHandle> {
    LOGGER_HANDLE.get()
}

pub fn set_log_level(level: LogLevel) -> LogResult<()> {
    let effective_level = if debug_mode() { LogLevel::Debug } else { level };
    let handle = LOGGER_HANDLE.get().ok_or(LogError::NotInitialized)?;
    handle.parse_new_spec(effective_level.as_str())?;
    Ok(())
}

pub fn iec104_log_kv(
    station_id: &str,
    connection_id: Option<&str>,
    type_id: Option<u8>,
    cot: Option<u16>,
    ioa: Option<u32>,
    error_code: Option<&str>,
    trace_id: Option<&str>,
) -> String {
    format!(
        "station_id={} connection_id={} type_id={} cot={} ioa={} error_code={} trace_id={}",
        station_id,
        connection_id.unwrap_or("-"),
        type_id.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
        cot.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
        ioa.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
        error_code.unwrap_or("-"),
        trace_id.unwrap_or("-"),
    )
}

fn debug_mode() -> bool {
    std::env::args().any(|arg| arg == "-d")
}

fn file_format(
    writer: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> std::io::Result<()> {
    write!(
        writer,
        "[{}] {} [{}:{}] {}",
        now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK),
        record.level(),
        record.file().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        record.args()
    )
}

fn stdout_format(
    writer: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> std::io::Result<()> {
    let level = record.level();
    write!(
        writer,
        "[{}] {} [{}:{}] {}",
        style(level).paint(now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK).to_string()),
        style(level).paint(level.to_string()),
        record.file().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        style(level).paint(record.args().to_string())
    )
}

fn init_logger_internal(config: &LoggerConfig) -> LogResult<LoggerHandle> {
    std::fs::create_dir_all(&config.log_dir)?;

    let level = if debug_mode() { LogLevel::Debug } else { config.level };
    let criterion = match config.rotation {
        Rotation::Daily => Criterion::Age(Age::Day),
        Rotation::Hourly => Criterion::Age(Age::Hour),
        Rotation::Never => Criterion::Size(u64::MAX),
    };
    let cleanup =
        if config.retention_days == 0 { Cleanup::Never } else { Cleanup::KeepForDays(config.retention_days as usize) };

    let mut logger = Logger::try_with_str(level.as_str())?
        .log_to_file(FileSpec::default().directory(&config.log_dir))
        .append()
        .rotate(criterion, Naming::Timestamps, cleanup)
        .format_for_files(file_format)
        .print_message();

    if config.async_write {
        logger = logger.write_mode(WriteMode::Async);
    }

    if config.duplicate_to_stdout || debug_mode() {
        let duplicate_level = if debug_mode() {
            Duplicate::All
        } else {
            match config.level {
                LogLevel::Trace | LogLevel::Debug => Duplicate::All,
                LogLevel::Info => Duplicate::Info,
                LogLevel::Warn => Duplicate::Warn,
                LogLevel::Error => Duplicate::Error,
            }
        };
        logger = logger.duplicate_to_stdout(duplicate_level).format_for_stdout(stdout_format);
    }

    Ok(logger.start()?)
}
