//! 协议实施细则 Profile 模块。
//!
//! 本模块包含不同行业实施细则的 Profile 实现。
//!
//! # 可用 Profile
//!
//! - [`distribution::DistributionProfile`]: 配电自动化实施细则 (DL/T 634.5104-2009)
//!
//! # 使用示例
//!
//! ```ignore
//! use iec60870_parser::profiles::distribution::DistributionProfile;
//! use iec60870_parser::profile::ProtocolProfile;
//!
//! let profile = DistributionProfile;
//! if profile.is_private_type(TypeId::new(200)) {
//!     // 使用配电 Profile 解析 TI 200
//! }
//! ```

pub mod distribution;

pub use distribution::DistributionProfile;
