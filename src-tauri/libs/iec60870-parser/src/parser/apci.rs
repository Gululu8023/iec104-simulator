//! APCI（Application Protocol Control Information）解析器。
//!
//! 本模块实现 IEC 60870-5-104 协议的 APCI 层解析，负责识别和解析三种帧类型：
//!
//! - **I 帧（Information Frame）**：信息帧，携带 ASDU 数据和序号
//! - **S 帧（Supervisory Frame）**：监督帧，用于确认接收
//! - **U 帧（Unnumbered Control Frame）**：未编号控制帧，用于链路控制
//!
//! # APCI 帧格式
//!
//! ```text
//! +------+------+------+------+------+------+--------+
//! | 0x68 | LEN  |    CONTROL (4 bytes)    | ASDU   |
//! +------+------+------+------+------+------+--------+
//! ```
//!
//! - 起始字节：固定为 0x68
//! - 长度字节：APCI 控制域 + ASDU 的总长度
//! - 控制域：4 字节，根据帧类型有不同的编码方式

use crate::error::ParseError;

/// IEC104 帧类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// I 帧（信息帧）：携带 ASDU 数据，包含发送序号和接收序号
    I,
    /// S 帧（监督帧）：用于确认接收，仅包含接收序号
    S,
    /// U 帧（未编号控制帧）：用于链路控制（STARTDT/STOPDT/TESTFR）
    U,
}

/// U 帧控制类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum UControl {
    /// 启动数据传输激活（STARTDT ACT）：主站发起，请求启动数据传输
    STARTDT_ACT,
    /// 启动数据传输确认（STARTDT CON）：从站响应，确认启动数据传输
    STARTDT_CON,
    /// 停止数据传输激活（STOPDT ACT）：主站发起，请求停止数据传输
    STOPDT_ACT,
    /// 停止数据传输确认（STOPDT CON）：从站响应，确认停止数据传输
    STOPDT_CON,
    /// 测试帧激活（TESTFR ACT）：发起测试，检测链路是否正常
    TESTFR_ACT,
    /// 测试帧确认（TESTFR CON）：响应测试，确认链路正常
    TESTFR_CON,
}

impl UControl {
    /// STARTDT ACT 控制字节（0x07）
    pub const STARTDT_ACT_BYTE: u8 = 0x07;
    /// STARTDT CON 控制字节（0x0B）
    pub const STARTDT_CON_BYTE: u8 = 0x0B;
    /// STOPDT ACT 控制字节（0x13）
    pub const STOPDT_ACT_BYTE: u8 = 0x13;
    /// STOPDT CON 控制字节（0x23）
    pub const STOPDT_CON_BYTE: u8 = 0x23;
    /// TESTFR ACT 控制字节（0x43）
    pub const TESTFR_ACT_BYTE: u8 = 0x43;
    /// TESTFR CON 控制字节（0x83）
    pub const TESTFR_CON_BYTE: u8 = 0x83;

    /// 从控制字节解析 U 帧类型。
    ///
    /// # 参数
    ///
    /// * `byte` - U 帧控制字节
    ///
    /// # 返回
    ///
    /// - `Some(UControl)` - 识别的 U 帧类型
    /// - `None` - 未知的控制字节
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            Self::STARTDT_ACT_BYTE => Some(Self::STARTDT_ACT),
            Self::STARTDT_CON_BYTE => Some(Self::STARTDT_CON),
            Self::STOPDT_ACT_BYTE => Some(Self::STOPDT_ACT),
            Self::STOPDT_CON_BYTE => Some(Self::STOPDT_CON),
            Self::TESTFR_ACT_BYTE => Some(Self::TESTFR_ACT),
            Self::TESTFR_CON_BYTE => Some(Self::TESTFR_CON),
            _ => None,
        }
    }

    /// 将 U 帧类型转换为控制字节。
    ///
    /// # 返回
    ///
    /// 对应的控制字节（0x07/0x0B/0x13/0x23/0x43/0x83）
    pub fn to_byte(self) -> u8 {
        match self {
            Self::STARTDT_ACT => Self::STARTDT_ACT_BYTE,
            Self::STARTDT_CON => Self::STARTDT_CON_BYTE,
            Self::STOPDT_ACT => Self::STOPDT_ACT_BYTE,
            Self::STOPDT_CON => Self::STOPDT_CON_BYTE,
            Self::TESTFR_ACT => Self::TESTFR_ACT_BYTE,
            Self::TESTFR_CON => Self::TESTFR_CON_BYTE,
        }
    }
}

/// APCI 头部。
#[derive(Debug, Clone, Copy)]
pub struct ApciHeader {
    /// 帧类型（I/S/U）
    pub frame_type: FrameType,
    /// 发送序号 N(S)，仅 I 帧有效，范围 0-32767
    pub send_seq: u16,
    /// 接收序号 N(R)，I 帧和 S 帧有效，范围 0-32767
    pub recv_seq: u16,
    /// 原始控制域（4 字节，小端序）
    control: u32,
}

/// 解析 APCI 头部。
///
/// 从字节流中解析 IEC104 APCI 头部，识别帧类型并提取序号信息。
///
/// # 参数
///
/// * `control` - 包含 APCI 头部的字节切片，至少 6 字节：
///   - `[0]`: 起始字节（固定为 0x68）
///   - `[1]`: 长度字节
///   - `[2..6]`: 控制域（4 字节，小端序）
///
/// # 返回
///
/// - `Ok(ApciHeader)` - 解析成功，返回 APCI 头部信息
/// - `Err(ParseError::Incomplete)` - 字节流长度不足
/// - `Err(ParseError::InvalidStart)` - 起始字节不是 0x68
/// - `Err(ParseError::InvalidApci)` - 控制域格式错误或保留位非法
///
/// # 帧类型判定规则
///
/// - **I 帧**：控制域 bit0 == 0
///   - bit1-15: 发送序号 N(S)
///   - bit16: 保留位（必须为 0）
///   - bit17-31: 接收序号 N(R)
/// - **S 帧**：控制域 bit0 == 1 && bit1 == 0
///   - bit0-15: 固定为 0x0001
///   - bit16: 保留位（必须为 0）
///   - bit17-31: 接收序号 N(R)
/// - **U 帧**：控制域 bit0 == 1 && bit1 == 1
///   - bit0-7: U 控制字节（STARTDT/STOPDT/TESTFR）
///   - bit8-31: 保留位（必须为 0）
pub fn parse_apci(control: &[u8]) -> Result<ApciHeader, ParseError> {
    if control.len() < 6 {
        return Err(ParseError::Incomplete);
    }

    if control[0] != 0x68 {
        return Err(ParseError::invalid_start(control[0], None));
    }

    let ctrl = u32::from_le_bytes([control[2], control[3], control[4], control[5]]);

    // 判定帧类型。
    if ctrl & 0x01 == 0 {
        // I 帧
        // 第 3 字节（第二个 16-bit 字）最低位必须为 0（保留位）。
        if (ctrl & 0x0001_0000) != 0 {
            return Err(ParseError::invalid_apci("控制域保留位非法", None));
        }
        let send_seq = (ctrl >> 1) as u16 & 0x7FFF;
        let recv_seq = (ctrl >> 17) as u16 & 0x7FFF;
        Ok(ApciHeader { frame_type: FrameType::I, send_seq, recv_seq, control: ctrl })
    } else if ctrl & 0x02 == 0 {
        // S 帧
        // S 帧格式：低 16-bit 必须为 0x0001，且第 3 字节最低位必须为 0。
        if (ctrl & 0x0000_FFFF) != 0x0001 || (ctrl & 0x0001_0000) != 0 {
            return Err(ParseError::invalid_apci("S 帧控制域保留位非法", None));
        }
        let recv_seq = (ctrl >> 17) as u16 & 0x7FFF;
        Ok(ApciHeader { frame_type: FrameType::S, send_seq: 0, recv_seq, control: ctrl })
    } else {
        // U 帧：仅低 8 bit 有效，其余字节必须为 0。
        if (ctrl & 0xFFFF_FF00) != 0 {
            return Err(ParseError::invalid_apci("U 帧控制域保留位非法", None));
        }
        let bits = (ctrl & 0xFF) as u8;
        if UControl::from_byte(bits).is_none() {
            return Err(ParseError::invalid_apci("未知 U 控制位", None));
        }
        Ok(ApciHeader { frame_type: FrameType::U, send_seq: 0, recv_seq: 0, control: ctrl })
    }
}

impl ApciHeader {
    /// 若为 U 帧，解析其控制位。
    ///
    /// # 返回
    ///
    /// - `Some(UControl)` - U 帧类型（STARTDT/STOPDT/TESTFR）
    /// - `None` - 非 U 帧或控制字节无效
    pub fn u_control(&self) -> Option<UControl> {
        if self.frame_type != FrameType::U {
            return None;
        }
        UControl::from_byte((self.control & 0xFF) as u8)
    }

    /// 返回原始 U 控制字节。
    ///
    /// # 返回
    ///
    /// - `Some(u8)` - U 帧的控制字节（0x07/0x0B/0x13/0x23/0x43/0x83）
    /// - `None` - 非 U 帧
    pub fn u_control_byte(&self) -> Option<u8> {
        if self.frame_type != FrameType::U { None } else { Some((self.control & 0xFF) as u8) }
    }

    /// 构造测试/自定义使用的 APCI 头。
    ///
    /// # 参数
    ///
    /// * `frame_type` - 帧类型（I/S/U）
    /// * `send_seq` - 发送序号（仅 I 帧使用）
    /// * `recv_seq` - 接收序号（I 帧和 S 帧使用）
    ///
    /// # 注意
    ///
    /// 此方法主要用于测试场景，control 字段被设置为 0。
    /// 实际使用中应通过 `parse_apci` 解析真实帧数据。
    pub fn new(frame_type: FrameType, send_seq: u16, recv_seq: u16) -> Self {
        Self { frame_type, send_seq, recv_seq, control: 0 }
    }
}
