//! 配电自动化实施细则 Profile。
//!
//! 本模块实现《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义的私有类型。
//!
//! # 支持的私有 TypeID
//!
//! | TI | 名称 | 标识 | 说明 |
//! |----|------|------|------|
//! | 42 | 故障事件信息 | M_FT_NA_1 | 故障时刻遥测数据上送 |
//! | 200 | 切换定值区 | C_SR_NA_1 | 定值区切换命令 |
//! | 201 | 读定值区号 | C_RR_NA_1 | 读取当前定值区号 |
//! | 202 | 读参数和定值 | C_RS_NA_1 | 读取定值区参数 |
//! | 203 | 写参数和定值 | C_WS_NA_1 | 写入定值区参数 |
//! | 206 | 累计量(短浮点) | M_IT_NB_1 | 短浮点累计量 |
//! | 207 | 带时标累计量 | M_IT_TC_1 | 带时标短浮点累计量 |
//! | 210 | 文件传输 | F_FR_NA_1 | 配电特有文件传输 |
//! | 211 | 软件升级 | F_SR_NA_1 | 软件升级命令 |
//!
//! # 使用示例
//!
//! ```ignore
//! use iec60870_parser::profiles::distribution::DistributionProfile;
//! use iec60870_parser::profile::ProtocolProfile;
//! use iec60870_parser::parser::asdu::TypeId;
//!
//! let profile = DistributionProfile;
//!
//! // 检查是否为配电私有类型
//! assert!(profile.is_private_type(TypeId::new(200)));
//! assert!(!profile.is_private_type(TypeId::new(45)));  // 标准类型
//!
//! // 获取类型名称
//! assert_eq!(profile.type_name(TypeId::new(200)), Some("C_SR_NA_1"));
//! ```

mod descriptors;
mod fault_event;
mod file_ops;
mod integrated;
mod setting_zone;

use descriptors::DISTRIBUTION_DESCRIPTORS;
pub use fault_event::*;
pub use file_ops::*;
pub use integrated::*;
pub use setting_zone::*;

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, TypeDescriptor, TypeId, lookup_type_descriptor},
    profile::{ProfileAsdu, ProtocolProfile},
};

/// 配电自动化实施细则 Profile。
///
/// 实现《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义的私有类型。
#[derive(Debug, Clone, Copy, Default)]
pub struct DistributionProfile;

impl DistributionProfile {
    /// 配电私有类型 TypeID 范围
    const PRIVATE_TYPES: &'static [u8] = &[42, 200, 201, 202, 203, 206, 207, 210, 211];

    /// 查找配电私有类型描述符
    fn lookup_private_descriptor(ti: TypeId) -> Option<&'static TypeDescriptor> {
        let raw = ti.raw();
        DISTRIBUTION_DESCRIPTORS.iter().find(|d| d.type_id.raw() == raw)
    }
}

impl ProtocolProfile for DistributionProfile {
    fn name(&self) -> &'static str {
        "DL/T 634.5104 配电自动化实施细则"
    }

    fn lookup_descriptor(&self, ti: TypeId) -> Option<&'static TypeDescriptor> {
        // 先查私有类型
        if let Some(desc) = Self::lookup_private_descriptor(ti) {
            return Some(desc);
        }
        // 回退到国际标准
        lookup_type_descriptor(ti)
    }

    fn is_private_type(&self, ti: TypeId) -> bool {
        Self::PRIVATE_TYPES.contains(&ti.raw())
    }

    fn type_name(&self, ti: TypeId) -> Option<&'static str> {
        Self::lookup_private_descriptor(ti).map(|d| d.name)
    }

    fn parse_private_asdu<'a>(
        &self,
        header: &AsduHeader,
        payload: &'a [u8],
    ) -> Result<Box<dyn ProfileAsdu + 'a>, ParseError> {
        match header.type_id.raw() {
            42 => fault_event::parse_fault_event(header, payload),
            200 => setting_zone::parse_switch_setting_zone(header, payload),
            201 => setting_zone::parse_read_setting_zone_number(header, payload),
            202 => setting_zone::parse_read_setting_param(header, payload),
            203 => setting_zone::parse_write_setting_param(header, payload),
            206 => integrated::parse_integrated_total_float(header, payload),
            207 => integrated::parse_integrated_total_float_time(header, payload),
            210 => file_ops::parse_file_transfer(header, payload),
            211 => file_ops::parse_software_upgrade(header, payload),
            _ => Err(ParseError::UnsupportedTypeId { ti: header.type_id, offset: None }),
        }
    }
}
