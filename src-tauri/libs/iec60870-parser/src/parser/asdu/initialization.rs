//! 初始化结束 ASDU（TI 70）解析。

use crate::{
    error::ParseError,
    parser::asdu::{AsduHeader, for_each_entry, types::lookup_type_descriptor},
};

/// 初始化结束原因（COI）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CauseOfInitialization {
    pub raw: u8,
    pub cause: u8,
    pub local_parameter_change: bool,
}

/// 初始化结束帧。
#[derive(Debug, Clone)]
pub struct InitializationFrame {
    pub header: AsduHeader,
    pub causes: Vec<CauseOfInitialization>,
}

pub fn parse_initialization(header: AsduHeader, payload: &[u8]) -> Result<InitializationFrame, ParseError> {
    if header.type_id.raw() != 70 {
        return Err(ParseError::unsupported_type_id(header.type_id, None));
    }

    let descriptor =
        lookup_type_descriptor(header.type_id).ok_or(ParseError::unsupported_type_id(header.type_id, None))?;
    let mut causes = Vec::new();
    for_each_entry(header.type_id, payload, descriptor, header.vsq, |_ioa, body| {
        let raw = body[0];
        causes.push(CauseOfInitialization { raw, cause: raw & 0x7F, local_parameter_change: raw & 0x80 != 0 });
        Ok(())
    })?;

    Ok(InitializationFrame { header, causes })
}
