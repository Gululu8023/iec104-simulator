//! IEC 60870-5-104 时间戳解析
//!
//! 本模块提供两种时间戳格式的解析：
//! - CP24Time2a: 3字节时间戳（毫秒 + 分钟）
//! - CP56Time2a: 7字节时间戳（完整日期时间）

use std::fmt;

use crate::{error::ParseError, parser::asdu::spec};

/// CP24Time2a - 3字节相对时间戳
///
/// 格式：
/// - Byte 0-1: 毫秒 (0-59999)
/// - Byte 2: 分钟 (0-59, bits 0-5) + IV位 (bit 7)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CP24Time2a {
    /// 毫秒 (0-59999)
    pub milliseconds: u16,
    /// 分钟 (0-59)
    pub minutes: u8,
    /// 时间无效标志
    pub invalid: bool,
}

/// CP56Time2a - 7字节完整时间戳
///
/// 格式：
/// - Byte 0-1: 毫秒 (0-59999)
/// - Byte 2: 分钟 (0-59) + IV位
/// - Byte 3: 小时 (0-23) + SU位（夏令时）
/// - Byte 4: 日 (1-31) + 星期几 (0/1-7；0 表示未提供)
/// - Byte 5: 月 (1-12)
/// - Byte 6: 年 (0-99, 相对2000年)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CP56Time2a {
    /// 毫秒 (0-59999)
    pub milliseconds: u16,
    /// 分钟 (0-59)
    pub minutes: u8,
    /// 小时 (0-23)
    pub hours: u8,
    /// 日 (1-31)
    pub day: u8,
    /// 月 (1-12)
    pub month: u8,
    /// 年 (2000-2099)
    pub year: u16,
    /// 星期几 (0=未提供, 1=周一, ..., 7=周日)
    pub weekday: u8,
    /// 夏令时标志
    pub summer_time: bool,
    /// 时间无效标志
    pub invalid: bool,
}

/// 时间戳类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timestamp {
    /// 3字节相对时间戳
    CP24(CP24Time2a),
    /// 7字节完整时间戳
    CP56(CP56Time2a),
}

impl CP24Time2a {
    pub const fn to_bytes(self) -> [u8; 3] {
        let minute =
            (self.minutes & spec::CP24_MINUTE_VALUE_MASK) | if self.invalid { spec::CP24_INVALID_MASK } else { 0 };
        [self.milliseconds as u8, (self.milliseconds >> 8) as u8, minute]
    }

    /// 解析3字节CP24Time2a时间戳
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() < 3 {
            return Err(ParseError::invalid_asdu_header("CP24Time2a长度不足", None, None));
        }

        let milliseconds = u16::from_le_bytes([data[0], data[1]]);
        let minutes_byte = data[2];
        let minutes = minutes_byte & spec::CP24_MINUTE_VALUE_MASK; // bits 0-5
        let invalid = (minutes_byte & spec::CP24_INVALID_MASK) != 0; // bit 7

        if minutes_byte & spec::CP24_MINUTE_RESERVED_MASK != 0 {
            return Err(ParseError::invalid_asdu_header("CP24Time2a分钟保留位非法", None, None));
        }

        // 验证范围
        if milliseconds > 59999 {
            return Err(ParseError::invalid_asdu_header("CP24Time2a毫秒值超出范围", None, None));
        }
        if minutes > 59 {
            return Err(ParseError::invalid_asdu_header("CP24Time2a分钟值超出范围", None, None));
        }

        Ok(Self { milliseconds, minutes, invalid })
    }

    /// 获取总秒数
    pub fn total_seconds(&self) -> u32 {
        self.minutes as u32 * 60 + self.milliseconds as u32 / 1000
    }

    /// 获取秒（0-59）
    pub fn seconds(&self) -> u8 {
        (self.milliseconds / 1000) as u8
    }

    /// 获取毫秒部分（0-999）
    pub fn millis(&self) -> u16 {
        self.milliseconds % 1000
    }
}

impl CP56Time2a {
    pub const fn to_bytes(self) -> [u8; 7] {
        let minute =
            (self.minutes & spec::CP56_MINUTE_VALUE_MASK) | if self.invalid { spec::CP56_INVALID_MASK } else { 0 };
        let hour =
            (self.hours & spec::CP56_HOUR_VALUE_MASK) | if self.summer_time { spec::CP56_SUMMER_TIME_MASK } else { 0 };
        let day = (self.day & spec::CP56_DAY_VALUE_MASK) | ((self.weekday & 0x07) << 5);
        [
            self.milliseconds as u8,
            (self.milliseconds >> 8) as u8,
            minute,
            hour,
            day,
            self.month & spec::CP56_MONTH_VALUE_MASK,
            (self.year % 100) as u8,
        ]
    }

    /// 解析7字节CP56Time2a时间戳
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() < 7 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a长度不足", None, None));
        }

        let milliseconds = u16::from_le_bytes([data[0], data[1]]);

        let minutes_byte = data[2];
        let minutes = minutes_byte & spec::CP56_MINUTE_VALUE_MASK; // bits 0-5
        let invalid = (minutes_byte & spec::CP56_INVALID_MASK) != 0; // bit 7
        if minutes_byte & spec::CP56_MINUTE_RESERVED_MASK != 0 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a分钟保留位非法", None, None));
        }

        let hours_byte = data[3];
        let hours = hours_byte & spec::CP56_HOUR_VALUE_MASK; // bits 0-4
        let summer_time = (hours_byte & spec::CP56_SUMMER_TIME_MASK) != 0; // bit 7
        if hours_byte & spec::CP56_HOUR_RESERVED_MASK != 0 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a小时保留位非法", None, None));
        }

        let day_byte = data[4];
        let day = day_byte & spec::CP56_DAY_VALUE_MASK; // bits 0-4
        let weekday = (day_byte & spec::CP56_WEEKDAY_MASK) >> 5; // bits 5-7

        let month_byte = data[5];
        let month = month_byte & spec::CP56_MONTH_VALUE_MASK; // bits 0-3
        if month_byte & spec::CP56_MONTH_RESERVED_MASK != 0 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a月份保留位非法", None, None));
        }

        let year_byte = data[6];
        let year_offset = year_byte & spec::CP56_YEAR_VALUE_MASK; // bits 0-6
        if year_byte & spec::CP56_YEAR_RESERVED_MASK != 0 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a年份保留位非法", None, None));
        }
        // IEC 60870-5-104 标准：年份字段仅支持 0-99 (对应2000-2099)
        if year_offset > spec::CP56_YEAR_MAX {
            return Err(ParseError::invalid_asdu_header("CP56Time2a年份超出范围", None, None));
        }
        let year = 2000 + year_offset as u16;

        // 验证范围
        if milliseconds > 59999 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a毫秒值超出范围", None, None));
        }
        if minutes > 59 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a分钟值超出范围", None, None));
        }
        if hours > 23 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a小时值超出范围", None, None));
        }
        if day < 1 || day > 31 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a日值超出范围", None, None));
        }
        if month < 1 || month > 12 {
            return Err(ParseError::invalid_asdu_header("CP56Time2a月值超出范围", None, None));
        }
        // IEC 60870-5-104 标准：weekday 允许 0（未提供）或 1..7。
        if !spec::is_valid_cp56_weekday(weekday) {
            return Err(ParseError::invalid_asdu_header("CP56Time2a星期值超出范围", None, None));
        }
        // 检查日期合法性（考虑大小月和闰年）
        if day > days_in_month(year, month) {
            return Err(ParseError::invalid_asdu_header("CP56Time2a日期无效", None, None));
        }

        Ok(Self { milliseconds, minutes, hours, day, month, year, weekday, summer_time, invalid })
    }

    /// 获取秒（0-59）
    pub fn seconds(&self) -> u8 {
        (self.milliseconds / 1000) as u8
    }

    /// 获取毫秒部分（0-999）
    pub fn millis(&self) -> u16 {
        self.milliseconds % 1000
    }

    /// 是否为有效的日期时间
    pub fn is_valid_datetime(&self) -> bool {
        !self.invalid
            && self.year >= 2000
            && self.year <= 2099
            && self.month >= 1
            && self.month <= 12
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
            && self.hours <= 23
            && self.minutes <= 59
            && self.milliseconds <= 59999
            && self.weekday <= 7
    }
}

impl Timestamp {
    /// 从字节切片解析时间戳
    ///
    /// 根据长度自动判断时间戳类型：
    /// - 3字节 -> CP24Time2a
    /// - 7字节 -> CP56Time2a
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        match data.len() {
            3 => Ok(Timestamp::CP24(CP24Time2a::parse(data)?)),
            7 => Ok(Timestamp::CP56(CP56Time2a::parse(data)?)),
            len => Err(ParseError::invalid_asdu_header(
                if len < 3 { "时间戳长度不足" } else { "时间戳长度无效" },
                None,
                None,
            )),
        }
    }

    /// 时间戳是否标记为无效
    pub fn is_invalid(&self) -> bool {
        match self {
            Timestamp::CP24(t) => t.invalid,
            Timestamp::CP56(t) => t.invalid,
        }
    }
}

impl fmt::Display for CP24Time2a {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.seconds();
        let millis = self.millis();
        let invalid_str = if self.invalid { " [INVALID]" } else { "" };
        write!(f, "{:02}:{:02}.{:03}{}", self.minutes, secs, millis, invalid_str)
    }
}

impl fmt::Display for CP56Time2a {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.seconds();
        let millis = self.millis();
        let invalid_str = if self.invalid { " [INVALID]" } else { "" };
        let summer_str = if self.summer_time { " DST" } else { "" };

        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}{}{}",
            self.year, self.month, self.day, self.hours, self.minutes, secs, millis, summer_str, invalid_str
        )
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Timestamp::CP24(t) => write!(f, "{}", t),
            Timestamp::CP56(t) => write!(f, "{}", t),
        }
    }
}

/// 获取指定年月的天数
fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// 判断是否为闰年
fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
