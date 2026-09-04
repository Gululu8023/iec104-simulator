//! IEC 60870-5-104 传送原因 (Cause of Transmission, COT) 类型定义
//!
//! COT字段包含：
//! - 传送原因代码（6位）：表示ASDU的传送原因
//! - P/N位（bit 6）：肯定/否定确认标志
//! - T位（bit 7）：测试标志
//! - 原发地址（第2字节）：发起者地址（IEC 60870-5-104总是使用2字节COT）

use std::fmt;

use crate::parser::asdu::spec;

/// 传送原因（Cause of Transmission）
///
/// 根据IEC 60870-5-104标准定义的传送原因代码（6位）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CotReason {
    /// 未使用 - 值0
    NotUsed = 0,
    /// 周期/循环 - 值1
    Periodic = 1,
    /// 背景扫描 - 值2
    Background = 2,
    /// 突发/自发 - 值3
    Spontaneous = 3,
    /// 初始化 - 值4
    Initialized = 4,
    /// 请求或被请求 - 值5
    Request = 5,
    /// 激活 - 值6
    Activation = 6,
    /// 激活确认 - 值7
    ActivationConfirmation = 7,
    /// 停止激活 - 值8
    Deactivation = 8,
    /// 停止激活确认 - 值9
    DeactivationConfirmation = 9,
    /// 激活终止 - 值10
    ActivationTermination = 10,
    /// 远方命令引起的返送信息 - 值11
    ReturnInfoRemote = 11,
    /// 本地命令引起的返送信息 - 值12
    ReturnInfoLocal = 12,
    /// 文件传输 - 值13
    FileTransfer = 13,
    /// 响应总召唤 - 值20
    InterrogatedByStation = 20,
    /// 响应第1组召唤 - 值21
    InterrogatedByGroup1 = 21,
    /// 响应第2组召唤 - 值22
    InterrogatedByGroup2 = 22,
    /// 响应第3组召唤 - 值23
    InterrogatedByGroup3 = 23,
    /// 响应第4组召唤 - 值24
    InterrogatedByGroup4 = 24,
    /// 响应第5组召唤 - 值25
    InterrogatedByGroup5 = 25,
    /// 响应第6组召唤 - 值26
    InterrogatedByGroup6 = 26,
    /// 响应第7组召唤 - 值27
    InterrogatedByGroup7 = 27,
    /// 响应第8组召唤 - 值28
    InterrogatedByGroup8 = 28,
    /// 响应第9组召唤 - 值29
    InterrogatedByGroup9 = 29,
    /// 响应第10组召唤 - 值30
    InterrogatedByGroup10 = 30,
    /// 响应第11组召唤 - 值31
    InterrogatedByGroup11 = 31,
    /// 响应第12组召唤 - 值32
    InterrogatedByGroup12 = 32,
    /// 响应第13组召唤 - 值33
    InterrogatedByGroup13 = 33,
    /// 响应第14组召唤 - 值34
    InterrogatedByGroup14 = 34,
    /// 响应第15组召唤 - 值35
    InterrogatedByGroup15 = 35,
    /// 响应第16组召唤 - 值36
    InterrogatedByGroup16 = 36,
    /// 响应计数器总召唤 - 值37
    RequestedByGeneralCounter = 37,
    /// 响应计数器第1组召唤 - 值38
    RequestedByCounterGroup1 = 38,
    /// 响应计数器第2组召唤 - 值39
    RequestedByCounterGroup2 = 39,
    /// 响应计数器第3组召唤 - 值40
    RequestedByCounterGroup3 = 40,
    /// 响应计数器第4组召唤 - 值41
    RequestedByCounterGroup4 = 41,
    /// 未知原因类型或非标准值
    Unknown(u8),
}

impl CotReason {
    /// 从u8值创建传送原因
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::NotUsed,
            1 => Self::Periodic,
            2 => Self::Background,
            3 => Self::Spontaneous,
            4 => Self::Initialized,
            5 => Self::Request,
            6 => Self::Activation,
            7 => Self::ActivationConfirmation,
            8 => Self::Deactivation,
            9 => Self::DeactivationConfirmation,
            10 => Self::ActivationTermination,
            11 => Self::ReturnInfoRemote,
            12 => Self::ReturnInfoLocal,
            13 => Self::FileTransfer,
            20 => Self::InterrogatedByStation,
            21 => Self::InterrogatedByGroup1,
            22 => Self::InterrogatedByGroup2,
            23 => Self::InterrogatedByGroup3,
            24 => Self::InterrogatedByGroup4,
            25 => Self::InterrogatedByGroup5,
            26 => Self::InterrogatedByGroup6,
            27 => Self::InterrogatedByGroup7,
            28 => Self::InterrogatedByGroup8,
            29 => Self::InterrogatedByGroup9,
            30 => Self::InterrogatedByGroup10,
            31 => Self::InterrogatedByGroup11,
            32 => Self::InterrogatedByGroup12,
            33 => Self::InterrogatedByGroup13,
            34 => Self::InterrogatedByGroup14,
            35 => Self::InterrogatedByGroup15,
            36 => Self::InterrogatedByGroup16,
            37 => Self::RequestedByGeneralCounter,
            38 => Self::RequestedByCounterGroup1,
            39 => Self::RequestedByCounterGroup2,
            40 => Self::RequestedByCounterGroup3,
            41 => Self::RequestedByCounterGroup4,
            v => Self::Unknown(v),
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        match self {
            Self::NotUsed => 0,
            Self::Periodic => 1,
            Self::Background => 2,
            Self::Spontaneous => 3,
            Self::Initialized => 4,
            Self::Request => 5,
            Self::Activation => 6,
            Self::ActivationConfirmation => 7,
            Self::Deactivation => 8,
            Self::DeactivationConfirmation => 9,
            Self::ActivationTermination => 10,
            Self::ReturnInfoRemote => 11,
            Self::ReturnInfoLocal => 12,
            Self::FileTransfer => 13,
            Self::InterrogatedByStation => 20,
            Self::InterrogatedByGroup1 => 21,
            Self::InterrogatedByGroup2 => 22,
            Self::InterrogatedByGroup3 => 23,
            Self::InterrogatedByGroup4 => 24,
            Self::InterrogatedByGroup5 => 25,
            Self::InterrogatedByGroup6 => 26,
            Self::InterrogatedByGroup7 => 27,
            Self::InterrogatedByGroup8 => 28,
            Self::InterrogatedByGroup9 => 29,
            Self::InterrogatedByGroup10 => 30,
            Self::InterrogatedByGroup11 => 31,
            Self::InterrogatedByGroup12 => 32,
            Self::InterrogatedByGroup13 => 33,
            Self::InterrogatedByGroup14 => 34,
            Self::InterrogatedByGroup15 => 35,
            Self::InterrogatedByGroup16 => 36,
            Self::RequestedByGeneralCounter => 37,
            Self::RequestedByCounterGroup1 => 38,
            Self::RequestedByCounterGroup2 => 39,
            Self::RequestedByCounterGroup3 => 40,
            Self::RequestedByCounterGroup4 => 41,
            Self::Unknown(v) => v,
        }
    }

    /// 是否为激活类原因（激活、激活确认、停止激活等）
    pub fn is_activation(&self) -> bool {
        matches!(
            self,
            Self::Activation
                | Self::ActivationConfirmation
                | Self::Deactivation
                | Self::DeactivationConfirmation
                | Self::ActivationTermination
        )
    }

    /// 是否为自发传输
    pub fn is_spontaneous(&self) -> bool {
        matches!(self, Self::Spontaneous)
    }

    /// 是否为周期传输
    pub fn is_periodic(&self) -> bool {
        matches!(self, Self::Periodic)
    }

    /// 是否为请求类原因
    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request)
    }

    /// 是否为总召唤响应
    pub fn is_interrogation_response(&self) -> bool {
        matches!(
            self,
            Self::InterrogatedByStation
                | Self::InterrogatedByGroup1
                | Self::InterrogatedByGroup2
                | Self::InterrogatedByGroup3
                | Self::InterrogatedByGroup4
                | Self::InterrogatedByGroup5
                | Self::InterrogatedByGroup6
                | Self::InterrogatedByGroup7
                | Self::InterrogatedByGroup8
                | Self::InterrogatedByGroup9
                | Self::InterrogatedByGroup10
                | Self::InterrogatedByGroup11
                | Self::InterrogatedByGroup12
                | Self::InterrogatedByGroup13
                | Self::InterrogatedByGroup14
                | Self::InterrogatedByGroup15
                | Self::InterrogatedByGroup16
        )
    }

    /// 是否为计数器召唤响应
    pub fn is_counter_request_response(&self) -> bool {
        matches!(
            self,
            Self::RequestedByGeneralCounter
                | Self::RequestedByCounterGroup1
                | Self::RequestedByCounterGroup2
                | Self::RequestedByCounterGroup3
                | Self::RequestedByCounterGroup4
        )
    }

    /// 是否为远方命令引起的返送
    pub fn is_return_info_remote(&self) -> bool {
        matches!(self, Self::ReturnInfoRemote)
    }

    /// 是否为本地命令引起的返送
    pub fn is_return_info_local(&self) -> bool {
        matches!(self, Self::ReturnInfoLocal)
    }

    /// 是否为文件传输相关
    pub fn is_file_transfer(&self) -> bool {
        matches!(self, Self::FileTransfer)
    }

    /// 是否为未知/非标准值
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }

    /// 获取友好的描述文本
    pub fn description(&self) -> &'static str {
        match self {
            Self::NotUsed => "未使用",
            Self::Periodic => "周期/循环",
            Self::Background => "背景扫描",
            Self::Spontaneous => "突发/自发",
            Self::Initialized => "初始化",
            Self::Request => "请求",
            Self::Activation => "激活",
            Self::ActivationConfirmation => "激活确认",
            Self::Deactivation => "停止激活",
            Self::DeactivationConfirmation => "停止激活确认",
            Self::ActivationTermination => "激活终止",
            Self::ReturnInfoRemote => "远方命令引起的返送信息",
            Self::ReturnInfoLocal => "本地命令引起的返送信息",
            Self::FileTransfer => "文件传输",
            Self::InterrogatedByStation => "响应总召唤",
            Self::InterrogatedByGroup1 => "响应第1组召唤",
            Self::InterrogatedByGroup2 => "响应第2组召唤",
            Self::InterrogatedByGroup3 => "响应第3组召唤",
            Self::InterrogatedByGroup4 => "响应第4组召唤",
            Self::InterrogatedByGroup5 => "响应第5组召唤",
            Self::InterrogatedByGroup6 => "响应第6组召唤",
            Self::InterrogatedByGroup7 => "响应第7组召唤",
            Self::InterrogatedByGroup8 => "响应第8组召唤",
            Self::InterrogatedByGroup9 => "响应第9组召唤",
            Self::InterrogatedByGroup10 => "响应第10组召唤",
            Self::InterrogatedByGroup11 => "响应第11组召唤",
            Self::InterrogatedByGroup12 => "响应第12组召唤",
            Self::InterrogatedByGroup13 => "响应第13组召唤",
            Self::InterrogatedByGroup14 => "响应第14组召唤",
            Self::InterrogatedByGroup15 => "响应第15组召唤",
            Self::InterrogatedByGroup16 => "响应第16组召唤",
            Self::RequestedByGeneralCounter => "响应计数器总召唤",
            Self::RequestedByCounterGroup1 => "响应计数器第1组召唤",
            Self::RequestedByCounterGroup2 => "响应计数器第2组召唤",
            Self::RequestedByCounterGroup3 => "响应计数器第3组召唤",
            Self::RequestedByCounterGroup4 => "响应计数器第4组召唤",
            Self::Unknown(_) => "未知原因",
        }
    }
}

impl fmt::Display for CotReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(v) => write!(f, "Unknown({})", v),
            _ => write!(f, "{}({})", self.description(), self.as_u8()),
        }
    }
}

/// 传送原因完整结构（COT）
///
/// IEC 60870-5-104总是使用2字节COT格式：
/// - 第1字节：传送原因代码（低6位）+ P/N位（bit 6）+ T位（bit 7）
/// - 第2字节：原发地址（OA, 8位）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cot {
    /// 传送原因代码
    pub reason: CotReason,
    /// 测试标志（T位）
    pub test: bool,
    /// 否定确认标志（P/N位，true表示否定）
    pub negative_confirm: bool,
    /// 原发地址（OA，0-255）
    pub origin: u8,
}

impl Cot {
    /// 创建新的COT
    pub fn new(reason: CotReason) -> Self {
        Self { reason, test: false, negative_confirm: false, origin: 0 }
    }

    /// 从原始字节解析COT（2字节模式）
    ///
    /// # 参数
    /// - `byte1`: 第1字节（传送原因 + P/N + T）
    /// - `byte2`: 第2字节（原发地址）
    pub fn from_bytes(byte1: u8, byte2: u8) -> Self {
        let reason = CotReason::from_u8(byte1 & spec::COT_REASON_MASK); // 低6位
        let negative_confirm = (byte1 & spec::COT_NEGATIVE_MASK) != 0; // bit 6
        let test = (byte1 & spec::COT_TEST_MASK) != 0; // bit 7
        let origin = byte2;

        Self { reason, test, negative_confirm, origin }
    }

    /// 转换为2字节格式
    pub fn to_bytes(self) -> [u8; 2] {
        let mut byte1 = self.reason.as_u8() & spec::COT_REASON_MASK;
        if self.negative_confirm {
            byte1 |= spec::COT_NEGATIVE_MASK;
        }
        if self.test {
            byte1 |= spec::COT_TEST_MASK;
        }

        let byte2 = self.origin;

        [byte1, byte2]
    }

    /// 转换为小端 u16 值（低字节=原因/P/N/T，高字节=OA）
    pub fn as_u16(self) -> u16 {
        let bytes = self.to_bytes();
        u16::from_le_bytes(bytes)
    }

    /// 获取传送原因代码（低6位）
    pub fn cause_of_transmission(&self) -> u8 {
        self.reason.as_u8()
    }

    /// 是否为测试模式
    pub fn is_test(&self) -> bool {
        self.test
    }

    /// 是否为否定确认
    pub fn is_negative_confirm(&self) -> bool {
        self.negative_confirm
    }

    /// 是否为肯定确认
    pub fn is_positive_confirm(&self) -> bool {
        !self.negative_confirm
    }

    /// 设置测试标志
    pub fn with_test(mut self, test: bool) -> Self {
        self.test = test;
        self
    }

    /// 设置否定确认标志
    pub fn with_negative_confirm(mut self, negative: bool) -> Self {
        self.negative_confirm = negative;
        self
    }

    /// 设置原发地址
    pub fn with_origin(mut self, origin: u8) -> Self {
        self.origin = origin;
        self
    }
}

impl fmt::Display for Cot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "COT(reason={}, ", self.reason)?;
        if self.test {
            write!(f, "TEST, ")?;
        }
        if self.negative_confirm {
            write!(f, "NEGATIVE, ")?;
        }
        write!(f, "origin={})", self.origin)
    }
}
