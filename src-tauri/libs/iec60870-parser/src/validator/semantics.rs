//! 语义辅助函数：提供命令/参数 bitflag 视图以及 COT 分类。
//!
//! 这些 helper 被 `validator` 中的规则复用，外部调用方也可以直接使用，
//! 以避免手动位运算或重复实现 enum。

use thiserror::Error;

use crate::parser::asdu::{
    AsduHeader, TypeId, TypeIdEnum,
    commands::{Command, CommandDetail, DoublePointState},
    spec,
};

/// 单点命令视图，暴露 select/execute 位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleCommandView {
    pub select: bool,
    pub on: bool,
}

/// 双点命令状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleCommandState {
    Off,
    On,
    Intermediate,
    Invalid,
}

/// 设定值限定词视图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QosView {
    /// QL (bits 0..6)
    pub qualifier: u8,
    /// S/E (bit7): true=Select, false=Execute
    pub select: bool,
}

/// 参数限定词视图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QpmView {
    pub kind: u8,
    pub change: bool,
    pub in_operation: bool,
}

/// 命令 COT 分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCotClass {
    Activation,
    ActivationConfirmation,
    Deactivation,
    DeactivationConfirmation,
    Termination,
    Other,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticError {
    #[error("COT 不适用于命令: {0}")]
    InvalidCot(&'static str),
}

pub fn single_command_view(cmd: &Command<'_>) -> Option<SingleCommandView> {
    match cmd.detail {
        CommandDetail::Single { state, select } => Some(SingleCommandView { select, on: state.as_bool() }),
        _ => None,
    }
}

pub fn double_command_state(cmd: &Command<'_>) -> Option<DoubleCommandState> {
    match cmd.detail {
        CommandDetail::Double { state, .. } => Some(match state {
            DoublePointState::Off => DoubleCommandState::Off,
            DoublePointState::On => DoubleCommandState::On,
            DoublePointState::Indeterminate => DoubleCommandState::Intermediate,
            DoublePointState::Invalid => DoubleCommandState::Invalid,
        }),
        _ => None,
    }
}

pub fn qos_view(qos: u8) -> QosView {
    QosView { qualifier: qos & 0x7F, select: qos & 0x80 != 0 }
}

pub fn qpm_view(qpm: u8) -> QpmView {
    QpmView { kind: qpm & 0x3F, change: qpm & 0x40 != 0, in_operation: qpm & 0x80 == 0 }
}

pub fn is_valid_qpm_kind(kind: u8) -> bool {
    spec::is_valid_qpm_kind(kind)
}

pub fn classify_command_cot(cot: u16) -> CommandCotClass {
    match cot & 0x3F {
        6 => CommandCotClass::Activation,
        7 => CommandCotClass::ActivationConfirmation,
        8 => CommandCotClass::Deactivation,
        9 => CommandCotClass::DeactivationConfirmation,
        10 => CommandCotClass::Termination,
        _ => CommandCotClass::Other,
    }
}

/// 判断是否为命令类 TypeId
pub fn is_command_type(ti: TypeId) -> bool {
    ti.kind().is_command()
}

/// 判断枚举类型是否为命令类（类型安全版本）。
///
/// 推荐使用此函数避免魔数。
///
/// # 示例
///
/// ```
/// # use iec60870_parser::parser::asdu::TypeIdEnum;
/// # use iec60870_parser::validator::semantics::is_command_type_by_kind;
/// assert!(is_command_type_by_kind(TypeIdEnum::CScNa1));
/// assert!(!is_command_type_by_kind(TypeIdEnum::MSpNa1));
/// ```
pub fn is_command_type_by_kind(kind: TypeIdEnum) -> bool {
    kind.is_command()
}

/// 检查命令类 TypeId 的 COT 是否合法
///
/// C_RD_NA_1 (读命令) 特殊处理：允许 Activation 系列 + COT=5（请求）
pub fn is_command_cot_allowed(ti: TypeId, cot: u16) -> bool {
    let cot_class = classify_command_cot(cot);
    let ti_kind = ti.kind();

    // C_RD_NA_1 (读命令) 特殊处理
    if ti_kind == TypeIdEnum::CRdNa1 {
        return matches!(
            cot_class,
            CommandCotClass::Activation
                | CommandCotClass::ActivationConfirmation
                | CommandCotClass::Deactivation
                | CommandCotClass::DeactivationConfirmation
                | CommandCotClass::Termination
        ) || (cot & 0x3F) == 5; // COT=5 (请求)
    }

    // 其他命令类型的标准 COT 检查
    matches!(
        cot_class,
        CommandCotClass::Activation
            | CommandCotClass::ActivationConfirmation
            | CommandCotClass::Deactivation
            | CommandCotClass::DeactivationConfirmation
            | CommandCotClass::Termination
    )
}

/// 检查命令类枚举的 COT 是否合法（类型安全版本）。
///
/// 推荐使用此函数避免魔数。
///
/// # 参数
///
/// - `kind`: 命令类型的枚举值
/// - `cot`: 传送原因（COT）原始值
///
/// # 示例
///
/// ```
/// # use iec60870_parser::parser::asdu::TypeIdEnum;
/// # use iec60870_parser::validator::semantics::is_command_cot_allowed_by_kind;
/// // 单点命令允许 activation (COT=6)
/// assert!(is_command_cot_allowed_by_kind(TypeIdEnum::CScNa1, 6));
/// // 单点命令不允许 spontaneous (COT=3)
/// assert!(!is_command_cot_allowed_by_kind(TypeIdEnum::CScNa1, 3));
/// ```
pub fn is_command_cot_allowed_by_kind(kind: TypeIdEnum, cot: u16) -> bool {
    let cot_class = classify_command_cot(cot);

    // C_RD_NA_1 (读命令) 特殊处理
    if kind == TypeIdEnum::CRdNa1 {
        return matches!(
            cot_class,
            CommandCotClass::Activation
                | CommandCotClass::ActivationConfirmation
                | CommandCotClass::Deactivation
                | CommandCotClass::DeactivationConfirmation
                | CommandCotClass::Termination
        ) || (cot & 0x3F) == 5; // COT=5 (请求)
    }

    // 其他命令类型的标准 COT 检查
    matches!(
        cot_class,
        CommandCotClass::Activation
            | CommandCotClass::ActivationConfirmation
            | CommandCotClass::Deactivation
            | CommandCotClass::DeactivationConfirmation
            | CommandCotClass::Termination
    )
}

pub fn validate_command_cot(header: &AsduHeader) -> Result<(), SemanticError> {
    if !is_command_type(header.type_id) {
        return Ok(());
    }
    if is_command_cot_allowed(header.type_id, header.cot.as_u16()) {
        Ok(())
    } else {
        Err(SemanticError::InvalidCot("COT 与命令类型不匹配"))
    }
}
