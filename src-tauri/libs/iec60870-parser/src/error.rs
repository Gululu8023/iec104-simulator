use thiserror::Error;

/// 解析阶段错误：长度不足、格式非法等。
#[derive(Debug, Error)]
pub enum ParseError {
    /// 流式读取场景：缓冲区不足，但当前无法确定缺口大小（需等待更多数据）
    #[error("缓冲区数据不足以组成完整帧")]
    Incomplete,
    #[error("无效的起始字节，期望0x68，实际0x{found:02X}{}", fmt_offset(.offset))]
    InvalidStart { found: u8, offset: Option<usize> },
    #[error("APCI 控制域无效: {msg}{}", fmt_offset(.offset))]
    InvalidApci { msg: &'static str, offset: Option<usize> },
    #[error("ASDU 头部非法: {reason}{}", fmt_ctx(.ti, .offset))]
    InvalidAsduHeader { reason: &'static str, ti: Option<crate::parser::asdu::TypeId>, offset: Option<usize> },
    #[error("信息体解析失败: {reason}{}", fmt_info_obj_ctx(.ti, .ioa, .entry_index, .offset))]
    InvalidInfoObject {
        reason: &'static str,
        ti: crate::parser::asdu::TypeId,
        ioa: Option<u32>,
        entry_index: Option<usize>,
        offset: Option<usize>,
    },
    #[error("不支持的 TypeId: {ti:?}{}", fmt_offset(.offset))]
    UnsupportedTypeId { ti: crate::parser::asdu::TypeId, offset: Option<usize> },
    #[error("ASDU 头部长度不足：需要 {needed} 字节，实际 {available} 字节（{reason}）")]
    InsufficientAsduHeader { needed: usize, available: usize, reason: &'static str },
    #[error(
        "信息体长度不足：需要 {needed} 字节，实际 {available} 字节，ti={ti:?}, ioa={ioa:?}, entry={entry_index:?}（{reason}）"
    )]
    InsufficientInfoObject {
        needed: usize,
        available: usize,
        ti: crate::parser::asdu::TypeId,
        ioa: Option<u32>,
        entry_index: Option<usize>,
        reason: &'static str,
    },
}

impl ParseError {
    /// 便捷构造函数：创建InvalidStart错误
    pub fn invalid_start(found: u8, offset: Option<usize>) -> Self {
        Self::InvalidStart { found, offset }
    }

    /// 便捷构造函数：创建InvalidApci错误
    pub fn invalid_apci(msg: &'static str, offset: Option<usize>) -> Self {
        Self::InvalidApci { msg, offset }
    }

    /// 便捷构造函数：创建InvalidAsduHeader错误
    pub fn invalid_asdu_header(
        reason: &'static str,
        ti: Option<crate::parser::asdu::TypeId>,
        offset: Option<usize>,
    ) -> Self {
        Self::InvalidAsduHeader { reason, ti, offset }
    }

    /// 便捷构造函数：创建InvalidInfoObject错误
    pub fn invalid_info_object(
        reason: &'static str,
        ti: crate::parser::asdu::TypeId,
        ioa: Option<u32>,
        entry_index: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        Self::InvalidInfoObject { reason, ti, ioa, entry_index, offset }
    }

    /// 便捷构造函数：创建UnsupportedTypeId错误
    pub fn unsupported_type_id(ti: crate::parser::asdu::TypeId, offset: Option<usize>) -> Self {
        Self::UnsupportedTypeId { ti, offset }
    }

    /// 便捷构造函数：创建InsufficientAsduHeader错误
    pub fn insufficient_asdu_header(reason: &'static str, needed: usize, available: usize) -> Self {
        Self::InsufficientAsduHeader { needed, available, reason }
    }

    /// 便捷构造函数：创建InsufficientInfoObject错误
    pub fn insufficient_info_object(
        reason: &'static str,
        ti: crate::parser::asdu::TypeId,
        ioa: Option<u32>,
        entry_index: Option<usize>,
        needed: usize,
        available: usize,
    ) -> Self {
        Self::InsufficientInfoObject { needed, available, ti, ioa, entry_index, reason }
    }
}

/// 格式化offset信息
fn fmt_offset(offset: &Option<usize>) -> String {
    match offset {
        Some(o) => format!(" (offset={})", o),
        None => String::new(),
    }
}

/// 格式化完整上下文信息（ti + offset）
fn fmt_ctx(ti: &Option<crate::parser::asdu::TypeId>, offset: &Option<usize>) -> String {
    match (ti, offset) {
        (None, None) => String::new(),
        (Some(t), None) => format!(" (ti={})", t.raw()),
        (None, Some(o)) => format!(" (offset={})", o),
        (Some(t), Some(o)) => format!(" (ti={}, offset={})", t.raw(), o),
    }
}

/// 格式化信息体上下文信息（ti + ioa + entry_index + offset）
fn fmt_info_obj_ctx(
    ti: &crate::parser::asdu::TypeId,
    ioa: &Option<u32>,
    entry_index: &Option<usize>,
    offset: &Option<usize>,
) -> String {
    let mut parts = vec![format!("ti={}", ti.raw())];
    if let Some(ioa_val) = ioa {
        parts.push(format!("ioa={}", ioa_val));
    }
    if let Some(idx) = entry_index {
        parts.push(format!("entry={}", idx));
    }
    if let Some(off) = offset {
        parts.push(format!("offset={}", off));
    }
    format!(" ({})", parts.join(", "))
}

/// 错误严重性等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// 警告：可忽略或降级处理
    Warning,
    /// 错误：当前操作失败，但可继续
    Error,
    /// 致命：必须停止解析
    Fatal,
}

impl ParseError {
    /// 获取错误的严重性
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // 致命错误
            Self::Incomplete => ErrorSeverity::Fatal,
            Self::InvalidStart { .. } => ErrorSeverity::Fatal,
            Self::InvalidApci { .. } => ErrorSeverity::Fatal,
            Self::InvalidAsduHeader { .. } => ErrorSeverity::Fatal,
            Self::InsufficientAsduHeader { .. } => ErrorSeverity::Fatal,

            // 可恢复错误
            Self::InvalidInfoObject { .. } => ErrorSeverity::Error,
            Self::UnsupportedTypeId { .. } => ErrorSeverity::Error,
            Self::InsufficientInfoObject { .. } => ErrorSeverity::Error,
        }
    }

    /// 判断错误是否致命
    pub fn is_fatal(&self) -> bool {
        self.severity() == ErrorSeverity::Fatal
    }

    /// 判断错误是否可恢复
    pub fn is_recoverable(&self) -> bool {
        self.severity() <= ErrorSeverity::Error
    }

    /// 额外需要的字节数（缺口 = needed - available）
    pub fn needed_bytes(&self) -> Option<usize> {
        match self {
            Self::InsufficientAsduHeader { needed, available, .. }
            | Self::InsufficientInfoObject { needed, available, .. } => Some(needed.saturating_sub(*available)),
            _ => None,
        }
    }

    /// 总共需要的字节数
    pub fn total_needed(&self) -> Option<usize> {
        match self {
            Self::InsufficientAsduHeader { needed, .. } | Self::InsufficientInfoObject { needed, .. } => Some(*needed),
            _ => None,
        }
    }

    /// 当前可用的字节数
    pub fn available_bytes(&self) -> Option<usize> {
        match self {
            Self::InsufficientAsduHeader { available, .. } | Self::InsufficientInfoObject { available, .. } => {
                Some(*available)
            }
            _ => None,
        }
    }

    /// 获取错误位置
    pub fn offset(&self) -> Option<usize> {
        match self {
            Self::InvalidStart { offset, .. } => *offset,
            Self::InvalidApci { offset, .. } => *offset,
            Self::InvalidAsduHeader { offset, .. } => *offset,
            Self::InvalidInfoObject { offset, .. } => *offset,
            Self::UnsupportedTypeId { offset, .. } => *offset,
            _ => None,
        }
    }

    /// 获取错误上下文
    pub fn context(&self) -> Option<&'static str> {
        match self {
            Self::InsufficientAsduHeader { reason, .. } => Some(reason),
            Self::InsufficientInfoObject { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

/// 流式处理错误：缓冲区溢出、IO 异常等。
#[derive(Debug, Error)]
pub enum StreamError {
    #[error("帧长度超出上限：total={expected} 字节，limit={available} 字节")]
    BufferOverflow { expected: usize, available: usize },
    #[error("解析错误: {0}")]
    Parse(#[from] ParseError),
}

/// 语义校验错误。
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("VSQ/信息体数量非法: {reason}, count={count}, sq={sq}")]
    InvalidVsq { reason: &'static str, count: u8, sq: bool },
    #[error("质量字段非法: {reason}, flags=0b{flags:08b}")]
    InvalidQuality { reason: &'static str, flags: u8 },
    #[error("命令限定词非法: {reason}, value=0x{value:02X}")]
    InvalidQualifier { reason: &'static str, value: u8 },
    #[error("文件传输字段非法: {reason}, field={field}, value=0x{value:X}")]
    InvalidFileTransfer { reason: &'static str, field: &'static str, value: u32 },
    #[error("安全认证 ASDU 非法: {reason}")]
    InvalidSecurity { reason: &'static str },
    #[error("COT/命令语义非法: {reason}, cot=0x{cot:04X}")]
    InvalidCause { reason: &'static str, cot: u16 },
}

impl ValidationError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::InvalidSecurity { .. } => ErrorSeverity::Fatal,
            Self::InvalidVsq { count, .. } if *count == 0 => ErrorSeverity::Warning,
            _ => ErrorSeverity::Error,
        }
    }
}

/// 类型转换错误。
#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("不支持的 ASDU 类型: {0:?}")]
    Unsupported(super::parser::asdu::TypeId),
    #[error("帧类型不支持 ASDU 解析: {0:?}")]
    UnsupportedFrame(crate::parser::apci::FrameType),
    #[error("字段缺失或格式非法: {ctx}")]
    InvalidField {
        ctx: &'static str,
        #[source]
        source: ParseError,
    },
}

/// 链路管理错误（仅在启用 feature 时有效）。
#[cfg(feature = "link-manager")]
#[derive(Debug, Error)]
pub enum LinkError {
    #[error("链路状态无效: {0}")]
    InvalidState(&'static str),
}
