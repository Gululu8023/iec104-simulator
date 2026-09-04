//! 工具库模块。
//!
//! 提供日志、十六进制转换、PCAP 导出等工具函数。
//!
//! # 模块
//!
//! - [`logger`] - 日志模块，支持控制台和文件输出，支持日志轮转
//! - [`hex`] - 十六进制转换模块，支持字节数组转十六进制字符串
//! - [`pcap`] - PCAP 文件导出模块，支持 PCAP/PCAPNG 格式

pub mod hex;
pub mod logger;
pub mod pcap;

/// 生成当前代码位置的字符串（文件名、行号、列号）。
#[macro_export]
macro_rules! here {
    () => {
        concat!("at ", file!(), " line ", line!(), " column ", column!())
    };
}

// Re-exports for convenience
pub use hex::{HEX_PREVIEW_MAX_BYTES, bytes_to_hex, bytes_to_hex_limit, bytes_to_hex_preview};
pub use logger::{LoggerGuard, get_handle, init_logger, is_initialized};
