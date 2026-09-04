//! 十六进制转换工具模块。
//!
//! 提供字节数组到十六进制字符串的转换功能，用于日志输出和调试。

use std::fmt::Write;

/// 大型原始负载（如 TCP 读缓冲区）格式化为十六进制的字节数上限。
///
/// 此值有意设置得较为保守，以避免流量突发时过度消耗 CPU/内存。
pub const HEX_PREVIEW_MAX_BYTES: usize = 512;

/// 将字节数组格式化为大写十六进制字符串，如 `68 04 07 00 00 00`。
///
/// # 示例
///
/// ```
/// let bytes = vec![0x68, 0x04, 0x07, 0x00];
/// let hex = bytes_to_hex(&bytes);
/// assert_eq!(hex, "68 04 07 00");
/// ```
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    // 每个字节占 2 个字符，字节之间有一个空格
    let mut out = String::with_capacity(bytes.len().saturating_mul(3).saturating_sub(1));
    for (idx, b) in bytes.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        // write! 到 String 是不会失败的
        let _ = write!(&mut out, "{:02X}", b);
    }
    out
}

/// 将字节数组格式化为十六进制，但最多只输出 `max_bytes` 个字节。
///
/// 用于限制大型数据包的输出长度。
pub fn bytes_to_hex_limit(bytes: &[u8], max_bytes: usize) -> String {
    let slice = if bytes.len() > max_bytes { &bytes[..max_bytes] } else { bytes };
    bytes_to_hex(slice)
}

/// 大型原始缓冲区（TCP 数据块）的默认预览格式化。
///
/// 使用 [`HEX_PREVIEW_MAX_BYTES`] 作为最大字节数限制。
pub fn bytes_to_hex_preview(bytes: &[u8]) -> String {
    bytes_to_hex_limit(bytes, HEX_PREVIEW_MAX_BYTES)
}
