//! 控制/命令类 ASDU 解析（TI 45~64, 100~107 等）。
//!
//! 这些类型的 payload 包含 IOA + QOC/QOS/QOI/QCC，某些还会带 CP56Time2a。
//! 本模块利用 [`for_each_entry`] 遍历信息体，并将命令拆解为 [`CommandDetail`]，
//! 供语义层和 TypeConverter 直接复用。

use std::fmt;

use smallvec::SmallVec;

use crate::{
    error::ParseError,
    parser::asdu::{
        AsduHeader, TypeId, for_each_entry,
        qualifiers::{
            CounterInterrogationQualifier, InterrogationQualifier, ResetProcessQualifier, SetpointQualifier, parse_qcc,
            parse_qoi, parse_qos, parse_qrp,
        },
        split_timestamp,
        timestamp::Timestamp,
        types::{TypeDescriptor, lookup_type_descriptor},
    },
};

/// 单点状态（Single Point State）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinglePointState {
    /// 断开/关闭
    Off = 0,
    /// 接通/打开
    On = 1,
}

impl SinglePointState {
    /// 从bool值创建（true=On, false=Off）
    pub fn from_bool(value: bool) -> Self {
        if value { Self::On } else { Self::Off }
    }

    /// 转换为bool值（On=true, Off=false）
    pub fn as_bool(self) -> bool {
        matches!(self, Self::On)
    }

    /// 是否为On状态
    pub fn is_on(self) -> bool {
        matches!(self, Self::On)
    }

    /// 是否为Off状态
    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }

    /// 是否为有效状态（单点状态总是有效的）
    ///
    /// 注意：此方法仅检查枚举值本身是否有效。
    /// 实际应用中，还需要结合质量标志位（QDS）来判断数据的完整有效性。
    /// 例如，即使状态值有效，QDS可能标记数据无效（IV位）或不可信。
    pub fn is_valid(self) -> bool {
        true
    }
}

impl fmt::Display for SinglePointState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::On => write!(f, "On"),
        }
    }
}

impl From<bool> for SinglePointState {
    fn from(value: bool) -> Self {
        Self::from_bool(value)
    }
}

impl From<SinglePointState> for bool {
    fn from(state: SinglePointState) -> Self {
        state.as_bool()
    }
}

/// 双点状态（Double Point State）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoublePointState {
    /// 中间态/未确定（Indeterminate or intermediate state）
    Indeterminate = 0,
    /// 断开/关闭（Determined state OFF）
    Off = 1,
    /// 接通/打开（Determined state ON）
    On = 2,
    /// 未确定（Indeterminate state）
    Invalid = 3,
}

impl DoublePointState {
    /// 从u8值创建
    pub fn from_u8(value: u8) -> Self {
        match value & 0x03 {
            1 => Self::Off,
            2 => Self::On,
            3 => Self::Invalid,
            _ => Self::Indeterminate,
        }
    }

    /// 转换为u8值
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// 是否为On状态
    pub fn is_on(self) -> bool {
        matches!(self, Self::On)
    }

    /// 是否为Off状态
    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }

    /// 是否为中间态（Indeterminate，值为0）
    pub fn is_intermediate(self) -> bool {
        matches!(self, Self::Indeterminate)
    }

    /// 是否为无效状态（Invalid，值为3）
    pub fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    /// 是否为有效状态（不是Invalid）
    ///
    /// 注意：此方法判断状态值本身是否在"值域有效"范围内。
    /// - 测量数据中：Indeterminate（中间态）是有效的，表示状态转换过程
    /// - 命令数据中：命令层需要额外校验，禁止使用Indeterminate和Invalid
    ///
    /// 实际应用中，还需要结合：
    /// 1. 质量标志位（QDS）来判断数据的完整有效性
    /// 2. 命令层校验规则（见validator/rules/command）
    pub fn is_valid(self) -> bool {
        !matches!(self, Self::Invalid)
    }

    /// 是否为确定状态（On或Off）
    pub fn is_determinate(self) -> bool {
        matches!(self, Self::On | Self::Off)
    }

    /// 是否为未确定状态（Indeterminate或Invalid）
    pub fn is_indeterminate(self) -> bool {
        matches!(self, Self::Indeterminate | Self::Invalid)
    }
}

impl fmt::Display for DoublePointState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Indeterminate => write!(f, "Indeterminate"),
            Self::Off => write!(f, "Off"),
            Self::On => write!(f, "On"),
            Self::Invalid => write!(f, "Invalid"),
        }
    }
}

impl From<u8> for DoublePointState {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<DoublePointState> for u8 {
    fn from(state: DoublePointState) -> Self {
        state.as_u8()
    }
}

/// 命令集合。
#[derive(Debug, Clone)]
pub struct CommandSet<'a> {
    /// ASDU 头部信息。
    pub header: AsduHeader,
    /// 命令列表（SmallVec 以减少少量信息体分配）。
    pub items: SmallVec<[Command<'a>; 4]>,
}

/// 单条命令。
#[derive(Debug, Clone)]
pub struct Command<'a> {
    /// 信息体地址。
    pub ioa: u32,
    /// 解析后的命令类型。
    pub detail: CommandDetail<'a>,
    /// 强类型限定词（QOS/QOI/QCC/SCO/DCO/RCO 等）。
    pub qualifier: CommandQualifier,
    /// 可选时间戳（CP56 Time2a）。
    pub timestamp: Option<&'a [u8]>,
}

impl<'a> Command<'a> {
    /// 解析时间戳（如果存在）
    pub fn parsed_timestamp(&self) -> Result<Option<Timestamp>, ParseError> {
        self.timestamp.map(Timestamp::parse).transpose()
    }
}

/// 命令限定词（强类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandQualifier {
    Sco { raw: u8, select: bool },
    Dco { raw: u8, select: bool },
    Rco { raw: u8, select: bool },
    Qos(SetpointQualifier),
    Qoi(InterrogationQualifier),
    Qcc(CounterInterrogationQualifier),
    Qrp(ResetProcessQualifier),
    None,
    Raw(u8),
}

impl CommandQualifier {
    /// 导出线路字节表示（用于透传/日志/上层兼容字段）。
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Sco { raw, .. } | Self::Dco { raw, .. } | Self::Rco { raw, .. } | Self::Raw(raw) => raw,
            Self::Qos(q) => q.as_u8(),
            Self::Qoi(q) => q.as_u8(),
            Self::Qcc(q) => q.as_u8(),
            Self::Qrp(q) => q.as_u8(),
            Self::None => 0,
        }
    }

    /// Select/Execute 位（当限定词定义包含该语义时）。
    pub fn select(self) -> Option<bool> {
        match self {
            Self::Sco { select, .. } | Self::Dco { select, .. } | Self::Rco { select, .. } => Some(select),
            Self::Qos(q) => Some(q.select),
            _ => None,
        }
    }
}

/// 命令的特定类型。
#[derive(Debug, Clone)]
pub enum CommandDetail<'a> {
    Single { state: SinglePointState, select: bool },
    Double { state: DoublePointState, select: bool },
    Step { position: u8 },
    SetPointNormalized(i16),
    SetPointScaled(i16),
    SetPointFloat(f32),
    BitString(&'a [u8]),
    Read,
    ClockSync { raw: &'a [u8], timestamp: Timestamp },
    ResetProcess,
    Raw(&'a [u8]),
}

impl<'a> CommandDetail<'a> {
    /// 获取单点命令的状态（如果是Single命令）
    pub fn single_state(&self) -> Option<SinglePointState> {
        match self {
            Self::Single { state, .. } => Some(*state),
            _ => None,
        }
    }

    /// 获取双点命令的状态（如果是Double命令）
    pub fn double_state(&self) -> Option<DoublePointState> {
        match self {
            Self::Double { state, .. } => Some(*state),
            _ => None,
        }
    }

    /// 是否为选择命令（Select）
    pub fn is_select(&self) -> bool {
        match self {
            Self::Single { select, .. } | Self::Double { select, .. } => *select,
            _ => false,
        }
    }

    /// 是否为执行命令（Execute，即非Select）
    pub fn is_execute(&self) -> bool {
        !self.is_select()
    }

    /// 命令是否为On状态（适用于Single/Double命令）
    pub fn is_on(&self) -> Option<bool> {
        match self {
            Self::Single { state, .. } => Some(state.is_on()),
            Self::Double { state, .. } => Some(state.is_on()),
            _ => None,
        }
    }

    /// 命令是否为Off状态（适用于Single/Double命令）
    pub fn is_off(&self) -> Option<bool> {
        match self {
            Self::Single { state, .. } => Some(state.is_off()),
            Self::Double { state, .. } => Some(state.is_off()),
            _ => None,
        }
    }

    /// 获取设定值（如果是设定值命令）
    pub fn setpoint_value(&self) -> Option<f32> {
        match self {
            Self::SetPointNormalized(v) => Some(*v as f32 / 32768.0),
            Self::SetPointScaled(v) => Some(*v as f32),
            Self::SetPointFloat(v) => Some(*v),
            _ => None,
        }
    }
}

impl<'a> fmt::Display for CommandDetail<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single { state, select } => {
                let mode = if *select { "SELECT" } else { "EXECUTE" };
                write!(f, "SingleCommand({}, {})", state, mode)
            }
            Self::Double { state, select } => {
                let mode = if *select { "SELECT" } else { "EXECUTE" };
                write!(f, "DoubleCommand({}, {})", state, mode)
            }
            Self::Step { position } => {
                write!(f, "StepCommand(position={})", position)
            }
            Self::SetPointNormalized(v) => {
                write!(f, "SetPointNormalized({}, {:.3})", v, *v as f32 / 32768.0)
            }
            Self::SetPointScaled(v) => {
                write!(f, "SetPointScaled({})", v)
            }
            Self::SetPointFloat(v) => {
                write!(f, "SetPointFloat({:.6})", v)
            }
            Self::BitString(bits) => {
                write!(f, "BitString(len={})", bits.len())
            }
            Self::Read => {
                write!(f, "ReadCommand")
            }
            Self::ClockSync { .. } => {
                write!(f, "ClockSync")
            }
            Self::ResetProcess => {
                write!(f, "ResetProcess")
            }
            Self::Raw(data) => {
                write!(f, "RawCommand(len={})", data.len())
            }
        }
    }
}

/// 解析命令类 ASDU（45/46/47/58/59/100/101/102/103 等）。
pub fn parse_commands<'a>(header: AsduHeader, payload: &'a [u8]) -> Result<CommandSet<'a>, ParseError> {
    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut items = SmallVec::new();
    for_each_entry(header.type_id, payload, descriptor, header.vsq, |ioa, body| {
        let command = decode_command(header.type_id, descriptor, ioa, body)?;
        items.push(command);
        Ok(())
    })?;
    Ok(CommandSet { header, items })
}

fn decode_command<'a>(
    ti: TypeId,
    descriptor: &TypeDescriptor,
    ioa: u32,
    body: &'a [u8],
) -> Result<Command<'a>, ParseError> {
    let (core, timestamp) = split_timestamp(ti, body, descriptor.timestamp_len)?;

    let (detail, qualifier) = match ti.raw() {
        45 | 58 => decode_single_command(ti, core)?,
        46 | 59 => decode_double_command(ti, core)?,
        47 | 60 => decode_step_command(ti, core)?,
        48 | 61 => decode_setpoint_normalized(ti, core)?,
        49 | 62 => decode_setpoint_scaled(ti, core)?,
        50 | 63 => decode_setpoint_float(ti, core)?,
        51 | 64 => decode_bitstring_command(ti, core)?,
        100 | 101 | 105 => decode_simple_qualifier(ti, core)?,
        102 => decode_read_command(ti, core)?,
        103 => decode_clock_sync(ti, body)?,
        _ => (
            CommandDetail::Raw(core),
            CommandQualifier::Raw(core.get(core.len().saturating_sub(1)).copied().unwrap_or(0)),
        ),
    };

    Ok(Command { ioa, detail, qualifier, timestamp })
}

/// 解析单点命令（SCO）。
fn decode_single_command<'a>(ti: TypeId, input: &'a [u8]) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 1 {
        return Err(ParseError::insufficient_info_object("SCO 长度不足", ti, None, None, 1, input.len()));
    }
    let sco = input[0];
    let select = sco & 0x80 != 0;
    Ok((CommandDetail::Single { state: SinglePointState::from_bool(sco & 0x01 != 0), select }, CommandQualifier::Sco {
        raw: sco,
        select,
    }))
}

/// 解析双点命令（DCO）。
fn decode_double_command<'a>(ti: TypeId, input: &'a [u8]) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 1 {
        return Err(ParseError::insufficient_info_object("DCO 长度不足", ti, None, None, 1, input.len()));
    }
    let dco = input[0];
    let select = dco & 0x80 != 0;
    Ok((CommandDetail::Double { state: DoublePointState::from_u8(dco & 0x03), select }, CommandQualifier::Dco {
        raw: dco,
        select,
    }))
}

/// 解析步进命令（RCO）。
fn decode_step_command<'a>(ti: TypeId, input: &'a [u8]) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 1 {
        return Err(ParseError::insufficient_info_object("RCO 长度不足", ti, None, None, 1, input.len()));
    }
    let rco = input[0];
    let select = rco & 0x80 != 0;
    // IEC 60870-5-104: RCO 仅 bit0-1 表示命令状态（RCS），bit7 为 S/E。
    // bit2-6 为保留位，留给校验规则处理。
    Ok((CommandDetail::Step { position: rco & 0x03 }, CommandQualifier::Rco { raw: rco, select }))
}

/// 解析归一化设定值（C_SE_NA/T A）。
fn decode_setpoint_normalized<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 3 {
        return Err(ParseError::insufficient_info_object("设定值长度不足", ti, None, None, 3, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    let qualifier = input[2];
    Ok((CommandDetail::SetPointNormalized(value), CommandQualifier::Qos(parse_qos(qualifier))))
}

/// 解析标度化设定值（C_SE_NB/TB）。
fn decode_setpoint_scaled<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 3 {
        return Err(ParseError::insufficient_info_object("设定值长度不足", ti, None, None, 3, input.len()));
    }
    let value = i16::from_le_bytes([input[0], input[1]]);
    let qualifier = input[2];
    Ok((CommandDetail::SetPointScaled(value), CommandQualifier::Qos(parse_qos(qualifier))))
}

/// 解析短浮点设定值（C_SE_NC/TC）。
fn decode_setpoint_float<'a>(ti: TypeId, input: &'a [u8]) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 5 {
        return Err(ParseError::insufficient_info_object("设定值长度不足", ti, None, None, 5, input.len()));
    }
    let value = f32::from_bits(u32::from_le_bytes([input[0], input[1], input[2], input[3]]));
    let qualifier = input[4];
    Ok((CommandDetail::SetPointFloat(value), CommandQualifier::Qos(parse_qos(qualifier))))
}

/// 解析 32bit 位串命令（C_BO_*）。
fn decode_bitstring_command<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 4 {
        return Err(ParseError::insufficient_info_object("位串命令长度不足", ti, None, None, 4, input.len()));
    }
    Ok((CommandDetail::BitString(&input[..4]), CommandQualifier::None))
}

/// 解析简单限定词（QOI/QCC/RPQ...），保留原始数据。
fn decode_simple_qualifier<'a>(
    ti: TypeId,
    input: &'a [u8],
) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 1 {
        return Err(ParseError::insufficient_info_object("限定词长度不足", ti, None, None, 1, input.len()));
    }
    let qualifier = input[0];
    let typed = match ti.raw() {
        100 => {
            let qoi = parse_qoi(qualifier);
            if !qoi.is_valid() {
                return Err(ParseError::invalid_info_object("QOI 取值必须为 20~36", ti, None, None, None));
            }
            CommandQualifier::Qoi(qoi)
        }
        101 => {
            let qcc = parse_qcc(qualifier);
            if !qcc.is_valid_request() {
                return Err(ParseError::invalid_info_object("QCC 请求组号必须为 1~5", ti, None, None, None));
            }
            CommandQualifier::Qcc(qcc)
        }
        105 => {
            let qrp = parse_qrp(qualifier);
            if !qrp.is_valid() {
                return Err(ParseError::invalid_info_object("QRP 取值必须为 1 或 2", ti, None, None, None));
            }
            CommandQualifier::Qrp(qrp)
        }
        _ => CommandQualifier::Raw(qualifier),
    };
    let detail = if ti.raw() == 105 { CommandDetail::ResetProcess } else { CommandDetail::Raw(input) };
    Ok((detail, typed))
}

/// 解析读命令（C_RD_NA_1），不允许附带限定词。
fn decode_read_command<'a>(ti: TypeId, input: &'a [u8]) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if !input.is_empty() {
        return Err(ParseError::invalid_info_object("读命令不应包含限定词", ti, None, None, None));
    }
    Ok((CommandDetail::Read, CommandQualifier::None))
}

/// 解析时钟同步（C_CS_NA_1），仅校验长度。
fn decode_clock_sync<'a>(ti: TypeId, input: &'a [u8]) -> Result<(CommandDetail<'a>, CommandQualifier), ParseError> {
    if input.len() < 7 {
        return Err(ParseError::insufficient_info_object("时钟同步长度不足", ti, None, None, 7, input.len()));
    }
    // 严格模式：在解析阶段校验 CP56Time2a 编码合法性。
    let timestamp = Timestamp::parse(&input[..7])?;
    Ok((CommandDetail::ClockSync { raw: &input[..7], timestamp }, CommandQualifier::None))
}
