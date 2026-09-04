//! 协议配置（Profile）模块。
//!
//! 本模块定义了 [`ProtocolProfile`] trait，用于支持不同实施细则的扩展。
//!
//! # 架构设计
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    iec60870-parser (核心库)                  │
//! ├─────────────────────────────────────────────────────────────┤
//! │  TypeIdEnum: 只含国际标准 TI (1-127)                         │
//! │  ProtocolProfile trait: 扩展点                              │
//! │  DefaultProfile: 只支持国际标准                              │
//! └─────────────────────────────────────────────────────────────┘
//!                               ▲
//!                               │ impl ProtocolProfile
//!           ┌───────────────────┼───────────────────┐
//!           │                   │                   │
//! ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
//! │DistributionProfile│  │TransmissionProfile│  │ OtherProfile    │
//! │ (配电自动化)      │  │ (主网调度，待实现) │  │ (其他，待实现)  │
//! └─────────────────┘  └─────────────────┘  └─────────────────┘
//! ```
//!
//! # 使用示例
//!
//! ```ignore
//! use iec60870_parser::profile::{ProtocolProfile, DefaultProfile};
//! use iec60870_parser::profiles::distribution::DistributionProfile;
//!
//! // 使用默认 Profile（仅国际标准）
//! let default = DefaultProfile;
//! assert!(!default.is_private_type(TypeId::new(200)));
//!
//! // 使用配电 Profile（支持私有类型）
//! let distribution = DistributionProfile;
//! assert!(distribution.is_private_type(TypeId::new(200)));
//! ```

use std::{any::Any, fmt::Debug};

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, TypeDescriptor, TypeId, lookup_type_descriptor},
};

/// Profile 特定 ASDU 数据的 trait。
///
/// 不同实施细则的私有类型解析结果需要实现此 trait。
pub trait ProfileAsdu: Debug + Send + Sync {
    /// 返回此 ASDU 的 TypeId
    fn type_id(&self) -> TypeId;

    /// 返回类型名称（如 "C_SR_NA_1"）
    fn type_name(&self) -> &'static str;

    /// 转换为 Any，用于向下转型
    fn as_any(&self) -> &dyn Any;
}

/// 协议配置 trait，可注入自定义 TypeDescriptor 和私有类型解析。
///
/// 不同的实施细则（如配电自动化、主网调度）可以通过实现此 trait
/// 来支持其特有的 TypeID 定义。
pub trait ProtocolProfile: Send + Sync {
    /// Profile 名称（用于日志和调试）
    fn name(&self) -> &'static str;

    /// 查找 TypeDescriptor
    ///
    /// 对于私有类型，返回 Profile 定义的描述符；
    /// 对于标准类型，回退到核心库的查找表。
    fn lookup_descriptor(&self, ti: TypeId) -> Option<&'static TypeDescriptor>;

    /// 判断 TI 是否为该 Profile 支持的私有类型
    ///
    /// 私有类型是指不在 IEC 60870-5-104 标准中定义，
    /// 而是由特定实施细则扩展的类型。
    fn is_private_type(&self, ti: TypeId) -> bool;

    /// 获取私有类型的语义名称
    ///
    /// 对于标准类型，返回 None；
    /// 对于私有类型，返回其在实施细则中定义的名称。
    fn type_name(&self, ti: TypeId) -> Option<&'static str> {
        self.lookup_descriptor(ti).map(|d| d.name)
    }

    /// 解析私有类型 ASDU
    ///
    /// 当 `is_private_type(header.type_id)` 返回 true 时调用。
    /// 返回 Profile 特定的解析结果。
    ///
    /// # 参数
    ///
    /// - `header`: ASDU 头部信息
    /// - `payload`: ASDU 信息体数据（不含头部）
    ///
    /// # 返回
    ///
    /// - `Ok(Box<dyn ProfileAsdu>)`: 解析成功，返回私有类型数据
    /// - `Err(ParseError)`: 解析失败
    fn parse_private_asdu<'a>(
        &self,
        header: &AsduHeader,
        payload: &'a [u8],
    ) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError>;
}

/// 默认配置：使用内置的 `TypeDescriptor` 查找表，不支持私有类型。
///
/// 此 Profile 仅支持 IEC 60870-5-104 国际标准定义的类型。
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultProfile;

impl ProtocolProfile for DefaultProfile {
    fn name(&self) -> &'static str {
        "IEC 60870-5-104 (Default)"
    }

    fn lookup_descriptor(&self, ti: TypeId) -> Option<&'static TypeDescriptor> {
        lookup_type_descriptor(ti)
    }

    fn is_private_type(&self, _ti: TypeId) -> bool {
        false
    }

    fn type_name(&self, _ti: TypeId) -> Option<&'static str> {
        None
    }

    fn parse_private_asdu<'a>(
        &self,
        header: &AsduHeader,
        _payload: &'a [u8],
    ) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
        Err(ParseError::UnsupportedTypeId { ti: header.type_id, offset: None })
    }
}
