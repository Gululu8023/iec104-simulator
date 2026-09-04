use log::warn;

use super::RequestContext;
use crate::{
    core::{
        protocol_adapter::{
            SetPointCommandValue, SlaveCommandEvent, build_setpoint_float_payload, build_setpoint_normalized_payload,
            build_setpoint_scaled_payload, decode_slave_command,
        },
        slave::{
            connection::{
                connection_metrics::increment_dropped_frame_count, connection_registry::increment_unknown_typeid_count,
            },
            protocol::response_cause::{
                ResponseEnvelope, decode_error_response_cause, should_negative_ack_decode_error,
            },
            transport::outbound_dispatcher::{
                enqueue_response_to_peer_with_priority, send_response_to_peer_wait_window,
            },
        },
        types::AsduInfo,
    },
    errors::{AppResult, Iec104ErrorCode},
    network::{DecodedAsdu, DecodedAsduBody, DecodedFileTransferRecord},
    utils::logger::iec104_log_kv,
};

/// 从请求上下文发送 ASDU（等待发送窗口）
///
/// 将 ASDU 发送给对端主站，合并请求原因和响应原因，并等待发送窗口可用。
///
/// # 参数
///
/// * `ctx` - 对端请求上下文（包含连接信息和共享状态）
/// * `response` - 已应用字段合同的响应信封
///
/// # 返回
///
/// - `Ok(())` - 发送成功
/// - `Err(AppError)` - 发送失败（如发送窗口已满、网络错误等）
pub(crate) async fn send_response_from_ctx(ctx: &RequestContext, response: ResponseEnvelope) -> AppResult<()> {
    send_response_to_peer_wait_window(&ctx.peer_addr, &ctx.protocol, &response).await
}

/// 从请求上下文入队 ASDU（优先级队列）
///
/// 将 ASDU 加入对端主站的发送队列，使用控制优先级，不等待发送窗口。
/// 与 `send_response_from_ctx` 不同，本方法不会阻塞等待发送窗口可用，而是将响应加入优先级队列。
///
/// # 参数
///
/// * `ctx` - 对端请求上下文（包含连接信息和共享状态）
/// * `response` - 已应用字段合同的响应信封
///
/// # 返回
///
/// - `Ok(())` - 入队成功
/// - `Err(AppError)` - 入队失败
///
/// # 使用场景
///
/// 用于需要高优先级发送但不希望阻塞的场景，如控制命令响应。
pub(crate) async fn enqueue_response_from_ctx(ctx: &RequestContext, response: ResponseEnvelope) -> AppResult<()> {
    enqueue_response_to_peer_with_priority(&ctx.peer_addr, &ctx.protocol, &response).await
}

/// 解码从站命令事件
///
/// 从 ASDU 中解码主站发送的命令事件，并在解码失败时发送否定确认。
///
/// # 参数
///
/// * `ctx` - 对端请求上下文（包含连接信息和共享状态）
/// * `asdu` - 解码后的 ASDU（包含命令数据）
/// * `target_common_address` - 目标公共地址
/// * `ioa` - 信息对象地址（可选，用于日志记录）
/// * `oa` - 源地址（用于日志记录）
/// * `unknown_typeid_negative_ack` - 是否对未知类型 ID 发送否定确认
///
/// # 返回
///
/// - `Ok(Some(Vec<SlaveCommandEvent>))` - 解码成功，返回命令事件列表
/// - `Ok(None)` - 解码失败，已发送否定确认（如果策略允许）
/// - `Err(AppError)` - 发送否定确认失败
///
/// # 执行流程
///
/// 1. 调用 `decode_slave_command` 解码 ASDU
/// 2. 如果解码成功，返回命令事件列表
/// 3. 如果解码失败：
///    - 记录警告日志
///    - 增加丢帧计数
///    - 如果是未知类型 ID，增加未知类型计数
///    - 根据策略决定是否发送否定确认
///    - 如果需要发送否定确认，构建并发送否定 ASDU
///    - 返回 None
///
/// # 否定确认策略
///
/// - 未知类型 ID：使用 `unknown_type_negative_ack_cause`
/// - 其他解码错误：使用 `command_negative_ack_cause`
/// - 策略由 `should_negative_ack_decode_error` 决定
pub(crate) async fn decode_slave_command_events(
    ctx: &RequestContext,
    asdu: &DecodedAsdu,
    target_common_address: u16,
    ioa: Option<u32>,
    oa: u8,
    unknown_typeid_negative_ack: bool,
) -> AppResult<Option<Vec<SlaveCommandEvent>>> {
    match decode_slave_command(asdu) {
        Ok(events) => Ok(Some(events)),
        Err(err) => {
            let is_unknown_type = err.code == Iec104ErrorCode::UnsupportedCommand;
            warn!(
                "[slave] decode_slave_command_failed {} vsq={} sq={} oa={} message={}",
                iec104_log_kv(
                    &ctx.station_id,
                    Some(&ctx.peer_addr),
                    Some(asdu.type_id),
                    Some(asdu.cause),
                    ioa,
                    Some(err.code.as_str()),
                    None,
                ),
                asdu.vsq,
                (asdu.vsq & 0x80) != 0,
                oa,
                err.message
            );
            increment_dropped_frame_count(&ctx.connections, &ctx.peer_addr, 1).await;
            if is_unknown_type {
                increment_unknown_typeid_count(&ctx.connections, &ctx.peer_addr, 1).await;
            }

            if should_negative_ack_decode_error(asdu, &err, unknown_typeid_negative_ack) {
                let reject_cause = decode_error_response_cause(asdu.cause, &err);
                warn!(
                    "[slave] decode_command_rejected {} policy=negative_ack",
                    iec104_log_kv(
                        &ctx.station_id,
                        Some(&ctx.peer_addr),
                        Some(asdu.type_id),
                        Some(asdu.cause),
                        ioa,
                        Some(err.code.as_str()),
                        None,
                    ),
                );
                send_response_from_ctx(
                    ctx,
                    ResponseEnvelope::echoed_request(
                        asdu.cause,
                        AsduInfo {
                            type_id: asdu.type_id,
                            cause: reject_cause,
                            common_address: target_common_address,
                            data: asdu.raw_payload.clone(),
                            timestamp: chrono::Utc::now(),
                        },
                        asdu.vsq,
                    ),
                )
                .await?;
            }
            Ok(None)
        }
    }
}

/// 构建设点命令载荷
///
/// 根据类型 ID 和设点值构建设点命令的载荷字节流。
/// 支持归一化、标度化和短浮点三种设点格式，并处理类型不匹配的防御性转换。
///
/// # 参数
///
/// * `type_id` - 类型标识（48-50 或 61-63）
///   - 48/61：归一化设点（C_SE_NA_1/C_SE_TA_1）
///   - 49/62：标度化设点（C_SE_NB_1/C_SE_TB_1）
///   - 50/63：短浮点设点（C_SE_NC_1/C_SE_TC_1）
/// * `ioa` - 信息对象地址
/// * `value` - 设点值（归一化/标度化/短浮点）
/// * `qualifier_raw` - 限定词原始字节（QOS）
///
/// # 返回
///
/// 返回编码后的载荷字节流，格式：
/// - 归一化：IOA(3B) + i16(2B) + QOS(1B)
/// - 标度化：IOA(3B) + i16(2B) + QOS(1B)
/// - 短浮点：IOA(3B) + f32(4B) + QOS(1B)
///
/// # 防御性转换
///
/// 当类型 ID 与设点值类型不匹配时，执行防御性转换以保持线路格式有效：
/// - 归一化 ↔ 标度化：直接转换（i16 → i16）
/// - 归一化/标度化 → 短浮点：转换为 f32
/// - 短浮点 → 归一化/标度化：截断为 i16
///
/// # 使用场景
///
/// 用于构建设点命令的响应载荷，确保即使类型不匹配也能生成有效的协议帧。
pub(crate) fn build_setpoint_command_payload(
    type_id: u8,
    ioa: u32,
    value: SetPointCommandValue,
    qualifier_raw: u8,
) -> Vec<u8> {
    match (type_id, value) {
        (48 | 61, SetPointCommandValue::Normalized(v)) => build_setpoint_normalized_payload(ioa, v, qualifier_raw),
        (49 | 62, SetPointCommandValue::Scaled(v)) => build_setpoint_scaled_payload(ioa, v, qualifier_raw),
        (50 | 63, SetPointCommandValue::Float(v)) => build_setpoint_float_payload(ioa, v, qualifier_raw),
        // Defensive fallback: keep wire payload shape valid even if type/value mismatch leaks in.
        (48 | 61, SetPointCommandValue::Scaled(v)) => build_setpoint_normalized_payload(ioa, v, qualifier_raw),
        (48 | 61, SetPointCommandValue::Float(v)) => build_setpoint_normalized_payload(ioa, v as i16, qualifier_raw),
        (49 | 62, SetPointCommandValue::Normalized(v)) => build_setpoint_scaled_payload(ioa, v, qualifier_raw),
        (49 | 62, SetPointCommandValue::Float(v)) => build_setpoint_scaled_payload(ioa, v as i16, qualifier_raw),
        (50 | 63, SetPointCommandValue::Normalized(v)) => build_setpoint_float_payload(ioa, v as f32, qualifier_raw),
        (50 | 63, SetPointCommandValue::Scaled(v)) => build_setpoint_float_payload(ioa, v as f32, qualifier_raw),
        (_, SetPointCommandValue::Normalized(v)) => build_setpoint_normalized_payload(ioa, v, qualifier_raw),
        (_, SetPointCommandValue::Scaled(v)) => build_setpoint_scaled_payload(ioa, v, qualifier_raw),
        (_, SetPointCommandValue::Float(v)) => build_setpoint_float_payload(ioa, v, qualifier_raw),
    }
}

/// 从解码后的 ASDU 中提取第一个信息对象地址
///
/// 从 ASDU 的不同类型的消息体中提取第一个信息对象地址（IOA）。
/// 支持测量值、命令、召唤和文件传输等所有 ASDU 类型。
///
/// # 参数
///
/// * `asdu` - 解码后的 ASDU
///
/// # 返回
///
/// - `Some(u32)` - 第一个信息对象地址
/// - `None` - ASDU 为空或为原始类型（未解析）
///
/// # 支持的 ASDU 类型
///
/// - **Measurements**：测量值 ASDU（M_SP_NA_1、M_ME_NC_1 等）
/// - **Commands**：命令 ASDU（C_SC_NA_1、C_SE_NC_1 等）
/// - **Interrogations**：召唤 ASDU（C_IC_NA_1、C_CI_NA_1 等）
/// - **FileTransfers**：文件传输 ASDU（F_FR_NA_1、F_SG_NA_1 等）
/// - **Raw**：原始未解析 ASDU（返回 None）
///
/// # 使用场景
///
/// 用于日志记录、错误报告和调试，快速获取 ASDU 中的第一个 IOA。
#[inline]
pub(crate) fn first_ioa_from_decoded_asdu(asdu: &DecodedAsdu) -> Option<u32> {
    match &asdu.body {
        DecodedAsduBody::Measurements(items) => items.first().map(|x| x.ioa),
        DecodedAsduBody::Commands(items) => items.first().map(|x| x.ioa),
        DecodedAsduBody::Interrogations(items) => items.first().map(|x| x.ioa),
        DecodedAsduBody::FileTransfers(items) => items.first().map(|item| match item {
            DecodedFileTransferRecord::FileReady { ioa, .. }
            | DecodedFileTransferRecord::SectionReady { ioa, .. }
            | DecodedFileTransferRecord::SelectCall { ioa, .. }
            | DecodedFileTransferRecord::LastSection { ioa, .. }
            | DecodedFileTransferRecord::AckSection { ioa, .. }
            | DecodedFileTransferRecord::Segment { ioa, .. }
            | DecodedFileTransferRecord::Directory { ioa, .. }
            | DecodedFileTransferRecord::QueryLog { ioa, .. } => *ioa,
            DecodedFileTransferRecord::Raw(_) => 0,
        }),
        DecodedAsduBody::Raw => None,
    }
}
