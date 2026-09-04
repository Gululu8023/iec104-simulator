//! IEC 60870-5-104 限定词解析
//!
//! 本模块提供协议中各种限定词（Qualifier）的解析和类型定义，包括：
//! - QOI (Qualifier of Interrogation) - 召唤限定词
//! - QCC (Qualifier of Counter Interrogation) - 计数器召唤限定词
//! - QOS (Qualifier of Set-point command) - 设点命令限定词
//! - QPM (Qualifier of Parameter) - 参数限定词
//! - QRP (Qualifier of Reset Process) - 进程复位限定词
//! - QDP (Quality Descriptor for Protection) - 保护设备事件质量
//! - QPA (Qualifier of Parameter Activation) - 参数激活限定词
//! - QDS (Quality Descriptor) - 品质描述符

use std::fmt;

use crate::{
    error::ParseError,
    parser::asdu::{spec, types::QualityKind},
};

// ============================================================================
// QOI - 召唤限定词 (Qualifier of Interrogation)
// ============================================================================

/// 召唤限定词，用于总召唤命令（C_IC_NA_1, TI=100）
///
/// 根据 IEC 60870-5-104 标准：
/// - 20: 总召唤（全站）
/// - 21-36: 组召唤（第1-16组）
/// - 37-255: 预留/厂商自定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterrogationQualifier {
    /// 总召唤（全站）- 值20
    Station,
    /// 组召唤 - 第1组 - 值21
    Group1,
    /// 组召唤 - 第2组 - 值22
    Group2,
    /// 组召唤 - 第3组 - 值23
    Group3,
    /// 组召唤 - 第4组 - 值24
    Group4,
    /// 组召唤 - 第5组 - 值25
    Group5,
    /// 组召唤 - 第6组 - 值26
    Group6,
    /// 组召唤 - 第7组 - 值27
    Group7,
    /// 组召唤 - 第8组 - 值28
    Group8,
    /// 组召唤 - 第9组 - 值29
    Group9,
    /// 组召唤 - 第10组 - 值30
    Group10,
    /// 组召唤 - 第11组 - 值31
    Group11,
    /// 组召唤 - 第12组 - 值32
    Group12,
    /// 组召唤 - 第13组 - 值33
    Group13,
    /// 组召唤 - 第14组 - 值34
    Group14,
    /// 组召唤 - 第15组 - 值35
    Group15,
    /// 组召唤 - 第16组 - 值36
    Group16,
    /// 预留或厂商自定义值
    Reserved(u8),
}

impl InterrogationQualifier {
    /// 从u8值解析召唤限定词
    pub fn from_u8(value: u8) -> Self {
        match value {
            20 => Self::Station,
            21 => Self::Group1,
            22 => Self::Group2,
            23 => Self::Group3,
            24 => Self::Group4,
            25 => Self::Group5,
            26 => Self::Group6,
            27 => Self::Group7,
            28 => Self::Group8,
            29 => Self::Group9,
            30 => Self::Group10,
            31 => Self::Group11,
            32 => Self::Group12,
            33 => Self::Group13,
            34 => Self::Group14,
            35 => Self::Group15,
            36 => Self::Group16,
            v => Self::Reserved(v),
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Station => 20,
            Self::Group1 => 21,
            Self::Group2 => 22,
            Self::Group3 => 23,
            Self::Group4 => 24,
            Self::Group5 => 25,
            Self::Group6 => 26,
            Self::Group7 => 27,
            Self::Group8 => 28,
            Self::Group9 => 29,
            Self::Group10 => 30,
            Self::Group11 => 31,
            Self::Group12 => 32,
            Self::Group13 => 33,
            Self::Group14 => 34,
            Self::Group15 => 35,
            Self::Group16 => 36,
            Self::Reserved(v) => v,
        }
    }

    /// 是否为总召唤
    pub fn is_station(self) -> bool {
        matches!(self, Self::Station)
    }

    /// 是否为组召唤
    pub fn is_group(self) -> bool {
        matches!(
            self,
            Self::Group1
                | Self::Group2
                | Self::Group3
                | Self::Group4
                | Self::Group5
                | Self::Group6
                | Self::Group7
                | Self::Group8
                | Self::Group9
                | Self::Group10
                | Self::Group11
                | Self::Group12
                | Self::Group13
                | Self::Group14
                | Self::Group15
                | Self::Group16
        )
    }

    /// 是否为有效的召唤值（20-36）
    pub fn is_valid(self) -> bool {
        !matches!(self, Self::Reserved(_))
    }

    /// 获取组号（如果是组召唤）
    pub fn group_number(self) -> Option<u8> {
        if self.is_group() { Some(self.as_u8() - 20) } else { None }
    }
}

impl fmt::Display for InterrogationQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Station => write!(f, "Station(20)"),
            Self::Reserved(v) => write!(f, "Reserved({v})"),
            _ => {
                // 所有组召唤
                write!(f, "Group{}({})", self.group_number().unwrap(), self.as_u8())
            }
        }
    }
}

// ============================================================================
// QCC - 计数器召唤限定词 (Qualifier of Counter Interrogation)
// ============================================================================

/// 冻结限定词，用于计数器召唤
///
/// bits 6-7:
/// - 0: 读取（不冻结不复位）
/// - 1: 冻结（但不复位）
/// - 2: 冻结并复位
/// - 3: 复位（不冻结）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeQualifier {
    /// 读取（不冻结不复位）
    Read = 0,
    /// 冻结（但不复位）
    Freeze = 1,
    /// 冻结并复位
    FreezeAndReset = 2,
    /// 复位（不冻结）
    Reset = 3,
}

impl FreezeQualifier {
    /// 从u8值解析冻结限定词
    pub fn from_u8(value: u8) -> Self {
        match value & 0x03 {
            0 => Self::Read,
            1 => Self::Freeze,
            2 => Self::FreezeAndReset,
            3 => Self::Reset,
            _ => unreachable!(),
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for FreezeQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Freeze => write!(f, "Freeze"),
            Self::FreezeAndReset => write!(f, "FreezeAndReset"),
            Self::Reset => write!(f, "Reset"),
        }
    }
}

/// 计数器召唤限定词，用于计数器召唤命令（C_CI_NA_1, TI=101）
///
/// 格式：
/// - bits 0-5: RQT (Request) - 请求类型（1=总召，2-5=组1-4）
/// - bits 6-7: FRZ (Freeze) - 冻结类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterInterrogationQualifier {
    /// 请求类型（1=总召，2-5=组1-4，其他值无效）
    pub request: u8,
    /// 冻结类型
    pub freeze: FreezeQualifier,
}

impl CounterInterrogationQualifier {
    /// 从u8值解析计数器召唤限定词
    pub fn from_u8(value: u8) -> Self {
        let request = value & 0x3F; // bits 0-5
        let freeze = FreezeQualifier::from_u8(value >> 6); // bits 6-7
        Self { request, freeze }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        (self.freeze.as_u8() << 6) | (self.request & 0x3F)
    }

    /// 是否为总召唤
    pub fn is_general(self) -> bool {
        self.request == spec::QCC_REQUEST_MIN
    }

    /// 是否为组召唤
    pub fn is_group(self) -> bool {
        (spec::QCC_REQUEST_MIN + 1..=spec::QCC_REQUEST_MAX).contains(&self.request)
    }

    /// 获取计数器组号（如果是组召唤）
    pub fn group_number(self) -> Option<u8> {
        if self.is_group() { self.request.checked_sub(1) } else { None }
    }

    /// 是否为有效的请求值（1-5）
    pub fn is_valid_request(self) -> bool {
        spec::is_valid_qcc_request(self.request)
    }
}

impl fmt::Display for CounterInterrogationQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QCC(request={}, freeze={})", self.request, self.freeze)
    }
}

// ============================================================================
// QOS - 设点命令限定词 (Qualifier of Set-point command)
// ============================================================================

/// 设点命令限定词，用于设定值命令（C_SE_NA/NB/NC, TI=48/49/50）
///
/// 格式：
/// - bits 0-6: QL (Qualifier) - 限定等级（0=无/缺省，1-127=厂商定义）
/// - bit 7: S/E (Select/Execute) - 选择(1)/执行(0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetpointQualifier {
    /// 限定等级（0-127）
    pub qualifier: u8,
    /// 选择/执行标志（true=选择，false=执行）
    pub select: bool,
}

impl SetpointQualifier {
    /// 从u8值解析设点命令限定词
    pub fn from_u8(value: u8) -> Self {
        let qualifier = value & 0x7F; // bits 0-6
        let select = (value & 0x80) != 0; // bit 7
        Self { qualifier, select }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        let select_bit = if self.select { 0x80 } else { 0x00 };
        select_bit | (self.qualifier & 0x7F)
    }

    /// 是否为选择命令
    pub fn is_select(self) -> bool {
        self.select
    }

    /// 是否为执行命令
    pub fn is_execute(self) -> bool {
        !self.select
    }

    /// IEC 60870-5-104 中 QOS 的 bit0..6 均属于 QL（限定等级），无固定“保留位必须为0”约束。
    pub fn has_valid_reserved_bits(self, _raw_value: u8) -> bool {
        true
    }
}

impl fmt::Display for SetpointQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = if self.select { "SELECT" } else { "EXECUTE" };
        write!(f, "QOS(QL={}, mode={})", self.qualifier, mode)
    }
}

// ============================================================================
// QPM - 参数限定词 (Qualifier of Parameter)
// ============================================================================

/// 参数限定词，用于参数传输（P_ME_NA/NB/NC, TI=110/111/112）
///
/// 格式：
/// - bits 0-5: KPA (Kind of Parameter) - 参数类型（1-63 有效，0 无效）
/// - bit 6: LPC (Local Parameter Change) - 本地参数修改标志
/// - bit 7: POP (Parameter in Operation) - 参数运行中标志（0=运行，1=未运行）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterQualifier {
    /// 参数类型（0-63）
    pub kind: u8,
    /// 本地参数修改标志
    pub local_change: bool,
    /// 参数运行中标志（false=运行中，true=未运行）
    pub not_in_operation: bool,
}

impl ParameterQualifier {
    /// 从u8值解析参数限定词
    pub fn from_u8(value: u8) -> Self {
        let kind = value & 0x3F; // bits 0-5
        let local_change = (value & 0x40) != 0; // bit 6
        let not_in_operation = (value & 0x80) != 0; // bit 7
        Self { kind, local_change, not_in_operation }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        let lpc_bit = if self.local_change { 0x40 } else { 0x00 };
        let pop_bit = if self.not_in_operation { 0x80 } else { 0x00 };
        pop_bit | lpc_bit | (self.kind & 0x3F)
    }

    /// 参数类型是否有效（1-63）
    pub fn is_valid_kind(self) -> bool {
        spec::is_valid_qpm_kind(self.kind)
    }

    /// 参数是否在运行中
    pub fn is_in_operation(self) -> bool {
        !self.not_in_operation
    }
}

impl fmt::Display for ParameterQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_in_operation() { "InOperation" } else { "NotInOperation" };
        let change = if self.local_change { ", LocalChange" } else { "" };
        write!(f, "QPM(kind={}, {}{})", self.kind, status, change)
    }
}

// ============================================================================
// QRP - 进程复位限定词 (Qualifier of Reset Process)
// ============================================================================

/// 进程复位限定词，用于复位进程命令（C_RP_NA_1, TI=105）
///
/// 取值：
/// - 0: 未使用/保留
/// - 1: 一般复位进程
/// - 2: 清除时间标记的事件缓冲区等待传输
/// - 3-255: 预留
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetProcessQualifier {
    /// 未使用/保留 - 值0
    NotUsed,
    /// 一般复位进程 - 值1
    GeneralReset,
    /// 清除时间标记的事件缓冲区 - 值2
    ClearEventBuffer,
    /// 预留值
    Reserved(u8),
}

impl ResetProcessQualifier {
    /// 从u8值解析进程复位限定词
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::NotUsed,
            1 => Self::GeneralReset,
            2 => Self::ClearEventBuffer,
            v => Self::Reserved(v),
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        match self {
            Self::NotUsed => 0,
            Self::GeneralReset => 1,
            Self::ClearEventBuffer => 2,
            Self::Reserved(v) => v,
        }
    }

    /// 是否为有效值（1-2）
    pub fn is_valid(self) -> bool {
        matches!(self, Self::GeneralReset | Self::ClearEventBuffer)
    }
}

impl fmt::Display for ResetProcessQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUsed => write!(f, "NotUsed(0)"),
            Self::GeneralReset => write!(f, "GeneralReset(1)"),
            Self::ClearEventBuffer => write!(f, "ClearEventBuffer(2)"),
            Self::Reserved(v) => write!(f, "Reserved({v})"),
        }
    }
}

// ============================================================================
// QDP - 保护设备事件质量 (Quality Descriptor for Protection)
// ============================================================================

/// 保护设备事件质量描述符，用于保护事件类型（如M_EP_*）
///
/// 格式（8位）：
/// - bit 0: IV (Invalid) - 无效
/// - bit 1: NT (Not Topical) - 非当前值
/// - bit 2: SB (Substituted) - 被替代
/// - bit 3: BL (Blocked) - 被闭锁
/// - bit 4: EI (Elapsed Time Invalid) - 时间标签无效
/// - bits 5-7: 保留（应为0）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProtectionQuality {
    /// 无效标志
    pub iv: bool,
    /// 非当前值标志
    pub nt: bool,
    /// 被替代标志
    pub sb: bool,
    /// 被闭锁标志
    pub bl: bool,
    /// 时间标签无效标志
    pub ei: bool,
}

impl ProtectionQuality {
    /// 从u8值解析保护设备事件质量
    pub fn from_u8(value: u8) -> Self {
        Self {
            iv: (value & 0b0000_0001) != 0,
            nt: (value & 0b0000_0010) != 0,
            sb: (value & 0b0000_0100) != 0,
            bl: (value & 0b0000_1000) != 0,
            ei: (value & 0b0001_0000) != 0,
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        let mut value = 0u8;
        if self.iv {
            value |= 0b0000_0001;
        }
        if self.nt {
            value |= 0b0000_0010;
        }
        if self.sb {
            value |= 0b0000_0100;
        }
        if self.bl {
            value |= 0b0000_1000;
        }
        if self.ei {
            value |= 0b0001_0000;
        }
        value
    }

    /// 事件是否无效
    pub fn is_invalid(&self) -> bool {
        self.iv
    }

    /// 事件是否非当前
    pub fn is_not_topical(&self) -> bool {
        self.nt
    }

    /// 事件是否被替代
    pub fn is_substituted(&self) -> bool {
        self.sb
    }

    /// 事件是否被闭锁
    pub fn is_blocked(&self) -> bool {
        self.bl
    }

    /// 时间标签是否无效
    pub fn has_elapsed_time_invalid(&self) -> bool {
        self.ei
    }

    /// 质量是否良好（所有标志位均为false）
    pub fn is_good(&self) -> bool {
        !self.iv && !self.nt && !self.sb && !self.bl && !self.ei
    }

    /// 是否存在任何质量问题
    pub fn has_any_issue(&self) -> bool {
        self.iv || self.nt || self.sb || self.bl || self.ei
    }

    /// 保留位是否为0（bits 5-7）
    pub fn has_valid_reserved_bits(raw_value: u8) -> bool {
        (raw_value & 0b1110_0000) == 0
    }
}

impl fmt::Display for ProtectionQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_good() {
            return write!(f, "GOOD");
        }

        let mut flags = Vec::new();
        if self.iv {
            flags.push("IV");
        }
        if self.nt {
            flags.push("NT");
        }
        if self.sb {
            flags.push("SB");
        }
        if self.bl {
            flags.push("BL");
        }
        if self.ei {
            flags.push("EI");
        }

        write!(f, "{}", flags.join("|"))
    }
}

// ============================================================================
// QPA - 参数激活限定词 (Qualifier of Parameter Activation)
// ============================================================================

/// 参数激活限定词，用于参数激活命令（P_AC_NA_1, TI=113）
///
/// 取值：
/// - 1: 激活当前参数集
/// - 2: 停用当前参数集
/// - 3: 激活对象参数
/// - 4: 停用对象参数
/// - 其他: 非标准/预留
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterActivationQualifier {
    /// 激活当前参数集 - 值1
    Activate,
    /// 停用当前参数集 - 值2
    Deactivate,
    /// 激活对象参数 - 值3
    ActivateObject,
    /// 停用对象参数 - 值4
    DeactivateObject,
    /// 预留或厂商自定义
    Reserved(u8),
}

impl ParameterActivationQualifier {
    /// 从u8值解析参数激活限定词
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Activate,
            2 => Self::Deactivate,
            3 => Self::ActivateObject,
            4 => Self::DeactivateObject,
            v => Self::Reserved(v),
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Activate => 1,
            Self::Deactivate => 2,
            Self::ActivateObject => 3,
            Self::DeactivateObject => 4,
            Self::Reserved(v) => v,
        }
    }

    /// 是否为有效值（1-4）
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Activate | Self::Deactivate | Self::ActivateObject | Self::DeactivateObject)
    }
}

impl fmt::Display for ParameterActivationQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Activate => write!(f, "Activate(1)"),
            Self::Deactivate => write!(f, "Deactivate(2)"),
            Self::ActivateObject => write!(f, "ActivateObject(3)"),
            Self::DeactivateObject => write!(f, "DeactivateObject(4)"),
            Self::Reserved(v) => write!(f, "Reserved({v})"),
        }
    }
}

// ============================================================================
// QDS - 品质描述符 (Quality Descriptor)
// ============================================================================

/// 品质标志统一结构，用于测量值（Measurement）和事件（Event）。
///
/// 对外 API 按 IEC 线路语义定义位布局（SIQ/DIQ/QDS）：
/// - bit7 = IV
/// - bit6 = NT
/// - bit5 = SB
/// - bit4 = BL
///
/// `ei` 不属于 SIQ/DIQ/QDS 线路字节，仅用于保护事件质量等扩展语义。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QualityFlags {
    /// 无效标志
    pub iv: bool,
    /// 非当前值标志
    pub nt: bool,
    /// 被替代标志
    pub sb: bool,
    /// 被闭锁标志
    pub bl: bool,
    /// 时间标签无效标志
    pub ei: bool,
}

impl QualityFlags {
    /// 从 IEC 线路质量字节（SIQ/DIQ/QDS）解析品质标志。
    ///
    /// 线路格式布局：
    /// - bit4 = BL
    /// - bit5 = SB
    /// - bit6 = NT
    /// - bit7 = IV
    ///
    /// 线路质量字节不携带 EI，`ei` 固定为 `false`。
    pub fn from_u8(byte: u8) -> Self {
        Self {
            iv: byte & spec::QUALITY_WIRE_IV_MASK != 0,
            nt: byte & spec::QUALITY_WIRE_NT_MASK != 0,
            sb: byte & spec::QUALITY_WIRE_SB_MASK != 0,
            bl: byte & spec::QUALITY_WIRE_BL_MASK != 0,
            ei: false,
        }
    }

    /// 转换为 IEC 线路质量字节（SIQ/DIQ/QDS）布局。
    ///
    /// - `iv -> bit7`
    /// - `nt -> bit6`
    /// - `sb -> bit5`
    /// - `bl -> bit4`
    ///
    /// `ei` 不属于 SIQ/DIQ/QDS 线路布局，因此不会写入输出字节。
    pub fn as_u8(self) -> u8 {
        (if self.iv { spec::QUALITY_WIRE_IV_MASK } else { 0 })
            | (if self.nt { spec::QUALITY_WIRE_NT_MASK } else { 0 })
            | (if self.sb { spec::QUALITY_WIRE_SB_MASK } else { 0 })
            | (if self.bl { spec::QUALITY_WIRE_BL_MASK } else { 0 })
    }

    /// 信息是否无效（Invalid）
    pub fn is_invalid(&self) -> bool {
        self.iv
    }

    /// 信息是否非当前值（Not Topical）
    pub fn is_not_topical(&self) -> bool {
        self.nt
    }

    /// 信息是否被替换（Substituted）
    pub fn is_substituted(&self) -> bool {
        self.sb
    }

    /// 信息是否被闭塞（Blocked）
    pub fn is_blocked(&self) -> bool {
        self.bl
    }

    /// 时间标签是否无效（Elapsed Time Invalid）
    pub fn has_elapsed_time_invalid(&self) -> bool {
        self.ei
    }

    /// 信息质量是否良好（所有标志位均为false）
    pub fn is_good(&self) -> bool {
        !self.iv && !self.nt && !self.sb && !self.bl && !self.ei
    }

    /// 是否存在任何质量问题
    pub fn has_any_issue(&self) -> bool {
        self.iv || self.nt || self.sb || self.bl || self.ei
    }

    /// 信息是否可用于控制决策（不是invalid、blocked或substituted）
    pub fn is_usable_for_control(&self) -> bool {
        !self.iv && !self.bl && !self.sb
    }
}

/// Normalize a quality byte to the wire layout required by the descriptor.
///
/// The low-nibble input form is retained at this boundary because simulator
/// configuration may express the four common flags as IV/NT/SB/BL bits 0..3.
pub fn normalize_quality_byte(kind: QualityKind, byte: u8) -> u8 {
    let common_wire = || {
        let wire = byte & 0xF0;
        if wire != 0 || (byte & 0x0F) == 0 {
            return wire;
        }
        (if byte & 0x01 != 0 { spec::QUALITY_WIRE_IV_MASK } else { 0 })
            | (if byte & 0x02 != 0 { spec::QUALITY_WIRE_NT_MASK } else { 0 })
            | (if byte & 0x04 != 0 { spec::QUALITY_WIRE_SB_MASK } else { 0 })
            | (if byte & 0x08 != 0 { spec::QUALITY_WIRE_BL_MASK } else { 0 })
    };

    match kind {
        QualityKind::None => 0,
        QualityKind::Siq | QualityKind::Diq => common_wire(),
        QualityKind::Qds => {
            let raw = byte & 0xF1;
            if byte & 0xF0 != 0 || byte == 0 {
                raw
            } else {
                (byte & 0x01)
                    | (if byte & 0x02 != 0 { spec::QUALITY_WIRE_NT_MASK } else { 0 })
                    | (if byte & 0x04 != 0 { spec::QUALITY_WIRE_SB_MASK } else { 0 })
                    | (if byte & 0x08 != 0 { spec::QUALITY_WIRE_BL_MASK } else { 0 })
            }
        }
        QualityKind::Bcr => byte,
        QualityKind::Qdp => byte & 0xF8,
    }
}

impl fmt::Display for QualityFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_good() {
            return write!(f, "GOOD");
        }

        let mut flags = Vec::new();
        if self.iv {
            flags.push("IV");
        }
        if self.nt {
            flags.push("NT");
        }
        if self.sb {
            flags.push("SB");
        }
        if self.bl {
            flags.push("BL");
        }
        if self.ei {
            flags.push("EI");
        }

        write!(f, "{}", flags.join("|"))
    }
}

// ============================================================================
// 解析函数
// ============================================================================

/// 解析召唤限定词（QOI）
pub fn parse_qoi(value: u8) -> InterrogationQualifier {
    InterrogationQualifier::from_u8(value)
}

/// 解析计数器召唤限定词（QCC）
pub fn parse_qcc(value: u8) -> CounterInterrogationQualifier {
    CounterInterrogationQualifier::from_u8(value)
}

/// 解析设点命令限定词（QOS）
pub fn parse_qos(value: u8) -> SetpointQualifier {
    SetpointQualifier::from_u8(value)
}

/// 解析参数限定词（QPM）
pub fn parse_qpm(value: u8) -> ParameterQualifier {
    ParameterQualifier::from_u8(value)
}

/// 解析进程复位限定词（QRP）
pub fn parse_qrp(value: u8) -> ResetProcessQualifier {
    ResetProcessQualifier::from_u8(value)
}

/// 解析保护设备事件质量（QDP）
pub fn parse_qdp(value: u8) -> ProtectionQuality {
    ProtectionQuality::from_u8(value)
}

/// 解析参数激活限定词（QPA）
pub fn parse_qpa(value: u8) -> ParameterActivationQualifier {
    ParameterActivationQualifier::from_u8(value)
}

/// 解析品质描述符（QDS）
pub fn parse_qds(byte: u8) -> QualityFlags {
    // IEC 线路 QDS/SIQ/DIQ 的质量语义位于高半字节（IV 在 bit7）。
    QualityFlags::from_u8(byte)
}

/// 从 IEC 线路格式品质字节 (SIQ/DIQ/QDS) 提取品质标志。
///
/// IEC 线路格式品质位布局：
///   bit4 = BL (Blocked)
///   bit5 = SB (Substituted)
///   bit6 = NT (Not Topical)
///   bit7 = IV (Invalid)
///
/// 该函数与 `parse_qds` 等价，仅保留为兼容命名。
pub fn parse_wire_quality(byte: u8) -> QualityFlags {
    QualityFlags::from_u8(byte)
}

/// 确保品质标志有效（IV位为0）
pub fn ensure_valid(flags: QualityFlags) -> Result<(), ParseError> {
    if flags.iv {
        return Err(ParseError::invalid_asdu_header("信息无效 (IV=1)", None, None));
    }
    Ok(())
}
