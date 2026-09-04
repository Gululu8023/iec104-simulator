//! 主站命令分发实现
//!
//! 本模块实现了 IEC104 主站命令的分发逻辑,负责命令的编码、发送、
//! 响应跟踪和状态更新。
//!
//! # 核心功能
//!
//! - **命令验证**：验证命令参数的合法性（公共地址、IOA、限定词等）
//! - **ASDU 构建**：将高层命令转换为 ASDU（应用服务数据单元）
//! - **命令追踪**：为每个命令生成唯一的 trace_id,用于关联请求和响应
//! - **待完成队列**：将命令加入待完成队列,等待从站响应
//! - **网络发送**：通过 TCP 连接发送 I-frame（信息帧）
//! - **状态检查**：确保连接已建立且数据传输已启动
//! - **控制快照**：对于控制命令,记录控制操作的快照
//! - **报文记录**：记录所有发送的命令报文,用于追踪和调试
//!
//! # 命令分发流程
//!
//! 1. 提取命令元数据（IOA、选择/执行标志、目标值等）
//! 2. 构建 ASDU（验证参数、编码命令）
//! 3. 注册待完成命令（加入队列,等待响应）
//! 4. 发送 ASDU 到连接（检查状态、编码帧、发送）
//! 5. 更新统计信息（发送计数、字节数）
//! 6. 记录报文和快照（用于追踪和展示）
//! 7. 返回分发回执（包含 trace_id）
//!
//! # 错误处理
//!
//! - 命令验证失败：返回 InvalidCommonAddress/InvalidObjectAddress 等错误
//! - 连接不存在：返回 NotConnected 错误
//! - 数据传输未启动：返回 ProtocolState 错误
//! - 发送失败：返回 SendFailed 错误
//! - 所有错误都会记录日志并清理待完成队列

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, atomic::AtomicU64},
};

use chrono::Utc;
use log::{info, warn};
use tokio::sync::{Mutex, RwLock};

use super::{
    super::{
        CommandDispatchReceipt, Iec104Command, MasterCommands, QualifierMode,
        connection::registry::try_get_connection_profile_id, data::control_store::upsert_control_dispatch_snapshot,
        message_store::append_master_message_meta,
    },
    store::{PendingCommand, enqueue_pending_command, is_control_command_type_id, remove_pending_command_by_trace},
    support::{
        command_cp56_identity, command_cp56_text, command_primary_ioa, command_qualifier_identity,
        command_select_execute, command_target_value,
    },
};
use crate::{
    core::{
        master::data::{
            point_scope::{MasterPointAdmission, resolve_point_scope},
            point_store::MasterPointStore,
        },
        protocol_adapter::encode_asdu,
        shared::runtime_hint::emit_master_runtime_hint,
        types::{AsduInfo, ConnectionInfo, DataTransferState, MasterMessageRecord, MessageDirection},
    },
    errors::{Iec104ErrorCode, Iec104ExecutionError, NetworkError},
    network::{Iec104Protocol, TcpClient},
    utils::{bytes_to_hex, logger::iec104_log_kv},
};

/// 主站命令分发运行时
///
/// 封装了命令分发所需的所有共享状态和资源引用。
/// 通过 Arc 引用计数实现多线程共享,通过 RwLock/Mutex 实现并发安全。
#[derive(Clone)]
pub(crate) struct MasterCommandDispatchRuntime {
    /// 主站 ID（用于日志记录和事件标识）
    pub(crate) station_id: String,
    /// 连接列表（key: connection_id, value: 连接信息）
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    /// 数据点缓存（用于更新控制命令的快照状态）
    pub(crate) data_points: Arc<MasterPointStore>,
    /// 持久化点表派生的禁用点准入索引
    pub(crate) point_admission: Arc<RwLock<MasterPointAdmission>>,
    /// 通信报文缓存（用于记录命令发送报文）
    pub(crate) message_records: Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    /// 待完成命令队列（用于跟踪命令的激活确认/否定响应）
    pub(crate) pending_commands: Arc<RwLock<VecDeque<PendingCommand>>>,
    /// 报文 ID 生成器（原子递增,用于唯一标识每条报文）
    pub(crate) next_message_id: Arc<AtomicU64>,
    /// TCP 客户端映射（key: connection_id, value: TCP 客户端实例）
    pub(crate) clients: Arc<RwLock<HashMap<String, Arc<TcpClient>>>>,
    /// IEC104 协议处理器映射（key: connection_id, value: 协议编解码器）
    pub(crate) protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    /// 运行时事件发布器
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    /// 运行时连接 UUID → profile_id 映射（用于关联配置文件）
    pub(crate) connection_profile_map: Arc<RwLock<HashMap<String, i64>>>,
}

/// 通过运行时分发命令
///
/// 这是命令分发的核心入口函数,负责完整的命令分发流程：
/// 从命令验证、ASDU 构建、待完成队列注册、网络发送,到报文记录和快照更新。
///
/// # 参数
///
/// * `runtime` - 命令分发运行时,包含所有必需的共享状态
/// * `connection_id` - 目标连接 ID（运行时 UUID）
/// * `slave_id` - 从站 ID（用于数据库关联,通常为 0）
/// * `common_address` - ASDU 公共地址
/// * `command` - 要分发的 IEC104 命令
///
/// # 返回
///
/// - `Ok(CommandDispatchReceipt)` - 命令分发回执,包含 trace_id、type_id、common_address
/// - `Err(Iec104ExecutionError)` - 命令执行失败,包含错误码和详细信息
///
/// # 执行流程
///
/// 1. **生成追踪 ID**：为命令生成唯一的 UUID trace_id
/// 2. **提取元数据**：提取 IOA、选择/执行标志、目标值、时间戳等
/// 3. **构建 ASDU**：调用 MasterCommands::build_asdu() 验证并构建 ASDU
/// 4. **注册待完成命令**：将命令加入待完成队列,等待从站响应
///    - 注意：必须在发送前注册,防止快速响应导致的竞态条件
/// 5. **发送 ASDU**：调用 send_asdu_to_connection() 发送到网络
///    - 如果发送失败,会从待完成队列中移除该命令
/// 6. **更新控制快照**：对于控制命令（TypeID 45-51, 58-64）,更新数据点的控制状态快照
/// 7. **推送运行时提示**：向前端推送 "command-dispatched" 事件
/// 8. **返回回执**：返回包含 trace_id 的分发回执
///
/// # 错误处理
///
/// - 命令验证失败时,记录警告日志并返回错误（不会加入待完成队列）
/// - 发送失败时,从待完成队列中移除命令,记录警告日志并返回错误
/// - 所有错误都会附带 trace_id,便于追踪和调试
///
/// # 线程安全
///
/// 本函数是异步的,所有共享状态访问都通过 RwLock/Mutex 保护,支持并发调用。
pub(crate) async fn dispatch_command_via_runtime(
    runtime: &MasterCommandDispatchRuntime,
    connection_id: &str,
    slave_id: i64,
    common_address: u16,
    command: Iec104Command,
    qualifier_mode: QualifierMode,
) -> Result<CommandDispatchReceipt, Iec104ExecutionError> {
    let trace_id = uuid::Uuid::new_v4().to_string();
    let ioa = command_primary_ioa(&command);
    let select_execute = command_select_execute(&command);
    let target_value = command_target_value(&command);
    let command_cp56 = command_cp56_text(&command);
    let cp56_identity = command_cp56_identity(&command);
    let qualifier_identity = command_qualifier_identity(&command);
    let normalized_slave_id = (slave_id > 0).then_some(slave_id);
    let link_profile_id = runtime
        .connections
        .read()
        .await
        .get(connection_id)
        .and_then(|conn| conn.profile_id)
        .or_else(|| try_get_connection_profile_id(&runtime.connection_profile_map, connection_id));
    let asdu = match MasterCommands::build_asdu_with_qualifier_mode(common_address, &command, qualifier_mode) {
        Ok(asdu) => asdu,
        Err(err) => {
            warn!(
                "[master] command_validation_failed {} message={}",
                iec104_log_kv(
                    &runtime.station_id,
                    Some(connection_id),
                    None,
                    None,
                    ioa,
                    Some(err.code.as_str()),
                    Some(&trace_id),
                ),
                err.message
            );
            return Err(Iec104ExecutionError::from(err).with_trace_id(&trace_id));
        }
    };

    if is_control_command_type_id(asdu.type_id) {
        if let Some(address) = ioa {
            let scope = resolve_point_scope(connection_id, normalized_slave_id);
            if runtime.point_admission.read().await.is_disabled(&scope, common_address, address) {
                return Err(Iec104ExecutionError::new(
                    Iec104ErrorCode::ProtocolState,
                    format!("configured point is disabled for CA={common_address} IOA={address}"),
                )
                .with_trace_id(&trace_id));
            }
        }
    }

    if is_control_command_type_id(asdu.type_id)
        && runtime.pending_commands.read().await.iter().any(|pending| {
            pending.connection_id == connection_id && pending.common_address == common_address && pending.ioa == ioa
        })
    {
        return Err(Iec104ExecutionError::new(
            Iec104ErrorCode::ProtocolState,
            format!("a command is already pending for CA={common_address} IOA={}", ioa.unwrap_or(0)),
        )
        .with_trace_id(&trace_id));
    }

    info!(
        "[master] dispatch_command {} ca={}",
        iec104_log_kv(
            &runtime.station_id,
            Some(connection_id),
            Some(asdu.type_id),
            Some(asdu.cause as u16),
            ioa,
            None,
            Some(&trace_id),
        ),
        asdu.common_address
    );

    // Register pending state before sending so a fast local peer cannot race the ACTCON ahead of trace
    // registration.
    enqueue_pending_command(&runtime.pending_commands, PendingCommand {
        trace_id: trace_id.clone(),
        connection_id: connection_id.to_string(),
        type_id: asdu.type_id,
        link_profile_id,
        slave_id: normalized_slave_id,
        common_address: asdu.common_address,
        ioa,
        select_execute,
        cp56: cp56_identity,
        target_value,
        qualifier: qualifier_identity,
        created_at: Utc::now(),
    })
    .await;

    if let Err(err) = send_asdu_to_connection(runtime, connection_id, asdu.clone(), &trace_id).await {
        remove_pending_command_by_trace(&runtime.pending_commands, &trace_id).await;
        warn!(
            "[master] dispatch_failed {} message={}",
            iec104_log_kv(
                &runtime.station_id,
                Some(connection_id),
                Some(asdu.type_id),
                Some(asdu.cause as u16),
                ioa,
                Some(err.code.as_str()),
                Some(&trace_id),
            ),
            err.message
        );
        return Err(err);
    }

    if is_control_command_type_id(asdu.type_id) {
        if let Some(address) = ioa {
            upsert_control_dispatch_snapshot(
                &runtime.data_points,
                connection_id,
                link_profile_id,
                normalized_slave_id,
                common_address,
                address,
                asdu.type_id,
                target_value,
                select_execute,
                command_cp56,
            )
            .await;
        }
    }
    emit_master_runtime_hint(
        &runtime.runtime_event_handle,
        &runtime.station_id,
        Some(connection_id),
        "command-dispatched",
    )
    .await;

    Ok(CommandDispatchReceipt { trace_id, type_id: asdu.type_id, common_address: asdu.common_address })
}

/// 发送 ASDU 到指定连接
///
/// 这是命令发送的底层实现函数,负责状态检查、ASDU 编码、帧封装和网络发送。
///
/// # 参数
///
/// * `runtime` - 命令分发运行时
/// * `connection_id` - 目标连接 ID
/// * `asdu` - 要发送的 ASDU 信息
/// * `trace_id` - 命令追踪 ID（用于错误关联）
///
/// # 返回
///
/// - `Ok(())` - 发送成功
/// - `Err(Iec104ExecutionError)` - 发送失败,包含错误码和详细信息
///
/// # 执行流程
///
/// 1. **获取资源**：从运行时获取 TCP 客户端和协议处理器
///    - 如果连接不存在,返回 NotConnected 错误
/// 2. **检查连接状态**：验证传输状态和数据传输状态
///    - 必须处于 Connected 和 Started 状态才能发送 I-frame
/// 3. **编码 ASDU**：将 ASDU 信息编码为字节流
/// 4. **封装帧**：调用协议处理器的 send_asdu() 封装为 I-frame
///    - 自动处理序号管理和流量控制
/// 5. **网络发送**：通过 TCP 客户端发送帧数据
/// 6. **更新统计**：更新连接的发送计数和字节数
/// 7. **记录报文**：将发送的报文记录到报文缓存
///
/// # 错误码映射
///
/// - `NotConnected` - 连接不存在或 TCP 客户端/协议处理器未找到
/// - `ProtocolState` - 数据传输未启动或协议状态错误
/// - `SendFailed` - TCP 发送失败
///
/// # 线程安全
///
/// 本函数通过 RwLock 和 Mutex 保护共享状态,支持并发调用。
/// 协议处理器的 send_asdu() 调用会获取独占锁,确保序号的原子性。
async fn send_asdu_to_connection(
    runtime: &MasterCommandDispatchRuntime,
    connection_id: &str,
    asdu: AsduInfo,
    trace_id: &str,
) -> Result<(), Iec104ExecutionError> {
    let client = runtime.clients.read().await.get(connection_id).cloned().ok_or_else(|| {
        Iec104ExecutionError::new(Iec104ErrorCode::NotConnected, format!("connection {connection_id} not found"))
            .with_trace_id(trace_id)
    })?;
    let protocol = runtime.protocols.read().await.get(connection_id).cloned().ok_or_else(|| {
        Iec104ExecutionError::new(
            Iec104ErrorCode::NotConnected,
            format!("protocol context for {connection_id} not found"),
        )
        .with_trace_id(trace_id)
    })?;

    let (transport_state, data_transfer_state) = {
        let guard = runtime.connections.read().await;
        let conn = guard.get(connection_id).ok_or_else(|| {
            Iec104ExecutionError::new(Iec104ErrorCode::NotConnected, format!("connection {connection_id} not found"))
                .with_trace_id(trace_id)
        })?;
        (conn.transport_state, conn.data_transfer_state)
    };

    if data_transfer_state != DataTransferState::Started {
        return Err(Iec104ExecutionError::new(
            Iec104ErrorCode::ProtocolState,
            format!("data transfer not started: transport={transport_state:?} data_transfer={data_transfer_state:?}"),
        )
        .with_trace_id(trace_id));
    }

    let asdu_bytes = encode_asdu(&asdu);
    let frame = {
        let mut protocol = protocol.lock().await;
        protocol.send_asdu(&asdu_bytes).map_err(|err| {
            let code = if matches!(err, NetworkError::NotConnected) {
                Iec104ErrorCode::NotConnected
            } else {
                Iec104ErrorCode::ProtocolState
            };
            Iec104ExecutionError::new(code, err.to_string()).with_trace_id(trace_id)
        })?
    };

    client.send(&frame).await.map_err(|err| {
        Iec104ExecutionError::new(Iec104ErrorCode::SendFailed, err.to_string()).with_trace_id(trace_id)
    })?;

    if let Some(conn) = runtime.connections.write().await.get_mut(connection_id) {
        conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
        conn.tx_bytes = conn.tx_bytes.saturating_add(frame.len() as u64);
    }

    append_master_message_meta(
        &runtime.message_records,
        &runtime.next_message_id,
        MessageDirection::Sent,
        connection_id,
        format!("发送 I 帧 type_id={} cot={} ca={}", asdu.type_id, asdu.cause, asdu.common_address),
        Some(bytes_to_hex(&frame)),
        Some(asdu.type_id),
        Some(asdu.cause as u16),
        Some(asdu.common_address),
        Some(trace_id.to_string()),
        Some("dispatch".to_string()),
    )
    .await;

    Ok(())
}
