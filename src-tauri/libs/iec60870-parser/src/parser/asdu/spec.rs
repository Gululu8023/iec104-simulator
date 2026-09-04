//! IEC 60870-5-104 规范常量（单一真源）。
//!
//! 本模块集中定义 ASDU 解析/校验中会复用的位掩码与取值范围，
//! 避免 parser/validator/docs 各自维护“近似一致”的魔数。

// ============================================================================
// 品质字节（SIQ/DIQ/QDS）线路位布局
// ============================================================================

pub const QUALITY_WIRE_BL_MASK: u8 = 0x10;
pub const QUALITY_WIRE_SB_MASK: u8 = 0x20;
pub const QUALITY_WIRE_NT_MASK: u8 = 0x40;
pub const QUALITY_WIRE_IV_MASK: u8 = 0x80;

// ============================================================================
// QCC / QPM / QPA
// ============================================================================

/// QCC 请求组号上限（RQT）。
///
/// 约束：1..=5（1=总召，2..5=组1..4）
pub const QCC_REQUEST_MIN: u8 = 1;
pub const QCC_REQUEST_MAX: u8 = 5;

#[inline]
pub fn is_valid_qcc_request(request: u8) -> bool {
    (QCC_REQUEST_MIN..=QCC_REQUEST_MAX).contains(&request)
}

/// QPM(KPA) 合法取值范围。
///
/// 约束：1..=63（0 为无效）
pub const QPM_KIND_MIN: u8 = 1;
pub const QPM_KIND_MAX: u8 = 63;

#[inline]
pub fn is_valid_qpm_kind(kind: u8) -> bool {
    (QPM_KIND_MIN..=QPM_KIND_MAX).contains(&kind)
}

/// QPA 合法取值范围。
///
/// 约束：1..=4
pub const QPA_MIN: u8 = 1;
pub const QPA_MAX: u8 = 4;

#[inline]
pub fn is_valid_qpa(value: u8) -> bool {
    (QPA_MIN..=QPA_MAX).contains(&value)
}

// ============================================================================
// COT 位定义
// ============================================================================

pub const COT_REASON_MASK: u8 = 0x3F;
pub const COT_NEGATIVE_MASK: u8 = 0x40;
pub const COT_TEST_MASK: u8 = 0x80;

// ============================================================================
// CP24Time2a 位定义
// ============================================================================

pub const CP24_MINUTE_VALUE_MASK: u8 = 0x3F;
pub const CP24_MINUTE_RESERVED_MASK: u8 = 0x40;
pub const CP24_INVALID_MASK: u8 = 0x80;

// ============================================================================
// CP56Time2a 位定义
// ============================================================================

pub const CP56_MINUTE_VALUE_MASK: u8 = 0x3F;
pub const CP56_MINUTE_RESERVED_MASK: u8 = 0x40;
pub const CP56_INVALID_MASK: u8 = 0x80;

pub const CP56_HOUR_VALUE_MASK: u8 = 0x1F;
pub const CP56_HOUR_RESERVED_MASK: u8 = 0x60;
pub const CP56_SUMMER_TIME_MASK: u8 = 0x80;

pub const CP56_DAY_VALUE_MASK: u8 = 0x1F;
pub const CP56_WEEKDAY_MASK: u8 = 0xE0;

pub const CP56_MONTH_VALUE_MASK: u8 = 0x0F;
pub const CP56_MONTH_RESERVED_MASK: u8 = 0xF0;

pub const CP56_YEAR_VALUE_MASK: u8 = 0x7F;
pub const CP56_YEAR_RESERVED_MASK: u8 = 0x80;
pub const CP56_YEAR_MAX: u8 = 99;

#[inline]
pub fn is_valid_cp56_weekday(weekday: u8) -> bool {
    weekday <= 7
}

// ============================================================================
// DL/T 634.5104 国内标准扩展常量
// ============================================================================

/// C_TS_NA_1 (TI 104) 测试命令固定测试码 (FBP)
///
/// DL/T 634.5104-2009 国内标准定义的测试命令固定测试码
pub const DLT_TEST_FBP: u16 = 0x55AA;
