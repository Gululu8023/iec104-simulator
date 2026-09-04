//! 从站出站帧编码模块。
//!
//! 本模块负责将 ASDU 信息编码为可发送的字节流,支持单个和批量编码,包括:
//!
//! - **单帧编码**:编码单个 ASDU 对象,自动提取 IOA 用于日志
//! - **批量编码**:编码多个 ASDU 对象到单帧,支持 SQ=0 和 SQ=1 模式
//! - **消息描述**:生成人类可读的 ASDU 描述用于日志和跟踪
//! - **元数据提取**:提取类型标识、传输原因、公共地址等关键信息
//!
//! # 编码模式
//!
//! - **SQ=0 (独立地址)**:每个对象包含完整的 IOA
//! - **SQ=1 (连续地址)**:仅首个对象包含 IOA,后续对象地址连续递增

use crate::core::{
    protocol_adapter::{encode_asdu, encode_asdu_with_vsq},
    slave::protocol::response_cause::ResponseEnvelope,
    types::AsduInfo,
};

#[derive(Debug)]
pub(crate) struct EncodedOutboundFrame {
    pub(crate) asdu_bytes: Vec<u8>,
    pub(crate) content: String,
    pub(crate) type_id: Option<u8>,
    pub(crate) cause: Option<u16>,
    pub(crate) common_address: Option<u16>,
}

pub(crate) fn encode_single_outbound_frame(asdu: &AsduInfo) -> EncodedOutboundFrame {
    let ioa = iec60870_parser::parser::asdu::encode::decode_ioa(&asdu.data);
    let content = match ioa {
        Some(value) => {
            format!("发送 ASDU type_id={} cot={} ca={} ioa={}", asdu.type_id, asdu.cause, asdu.common_address, value)
        }
        None => format!("发送 ASDU type_id={} cot={} ca={}", asdu.type_id, asdu.cause, asdu.common_address),
    };
    EncodedOutboundFrame {
        asdu_bytes: encode_asdu(asdu),
        content,
        type_id: Some(asdu.type_id),
        cause: Some(asdu.cause),
        common_address: Some(asdu.common_address),
    }
}

pub(crate) fn encode_batched_outbound_frame(
    asdu: &AsduInfo,
    object_count: u8,
    sq_sequence: bool,
) -> EncodedOutboundFrame {
    EncodedOutboundFrame {
        asdu_bytes: encode_asdu_with_vsq(asdu, object_count, sq_sequence),
        content: format!(
            "发送 ASDU type_id={} cot={} ca={} 对象数={} sq={}",
            asdu.type_id, asdu.cause, asdu.common_address, object_count, sq_sequence
        ),
        type_id: Some(asdu.type_id),
        cause: Some(asdu.cause),
        common_address: Some(asdu.common_address),
    }
}

pub(crate) fn encode_response_outbound_frame(response: &ResponseEnvelope) -> EncodedOutboundFrame {
    let asdu = &response.asdu;
    let payload_policy = match response.payload_policy {
        crate::core::slave::protocol::response_cause::ResponsePayloadPolicy::Provided => "provided",
        crate::core::slave::protocol::response_cause::ResponsePayloadPolicy::EchoRequest => "echo",
    };
    let timestamp_policy = match response.timestamp_policy {
        crate::core::slave::protocol::response_cause::ResponseTimestampPolicy::None => "none",
        crate::core::slave::protocol::response_cause::ResponseTimestampPolicy::Cp24 => "cp24",
        crate::core::slave::protocol::response_cause::ResponseTimestampPolicy::Cp56 => "cp56",
    };
    EncodedOutboundFrame {
        asdu_bytes: encode_asdu_with_vsq(asdu, response.object_count, response.sequence),
        content: format!(
            "发送响应 ASDU type_id={} cot={} ca={} 对象数={} sq={} payload={} timestamp={}",
            asdu.type_id,
            asdu.cause,
            asdu.common_address,
            response.object_count,
            response.sequence,
            payload_policy,
            timestamp_policy
        ),
        type_id: Some(asdu.type_id),
        cause: Some(asdu.cause),
        common_address: Some(asdu.common_address),
    }
}
