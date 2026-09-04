//! 数据摄取运行时
//!
//! 本模块负责处理主站接收到的 ASDU 数据，是数据处理的核心入口。
//!
//! # 核心功能
//!
//! - **I 帧处理**：处理接收到的信息帧（I-frame），提取数据点和 SOE 事件
//! - **命令响应处理**：识别控制命令响应，更新命令状态和控制点快照
//! - **数据点提取**：从 ASDU 中解码数据点信息（地址、值、质量、时标等）
//! - **SOE 事件提取**：提取带时标的事件序列（Sequence of Events）
//! - **数据合并**：将接收到的数据点合并到内存缓存
//! - **消息记录**：记录接收到的报文和命令状态变化
//! - **文件传输处理**：处理文件传输相关的 ASDU
//! - **运行时提示**：向前端发送运行时事件通知
//!
//! # 数据流
//!
//! ```text
//! 接收 I 帧
//!   ↓
//! 记录消息日志
//!   ↓
//! 检查是否为控制命令响应
//!   ├─ 是 → 更新命令状态 → 更新控制点快照 → 发送运行时提示
//!   └─ 否 → 继续
//!   ↓
//! 解析从站 ID 和作用域键
//!   ↓
//! 提取 SOE 事件（如果是带时标的 ASDU）
//!   ↓
//! 解码数据点
//!   ↓
//! 应用事件时标到数据点
//!   ↓
//! 合并数据点到内存缓存
//!   ↓
//! 填充 SOE 事件名称并记录
//!   ↓
//! 处理文件传输（如果是文件传输 ASDU）
//!   ↓
//! 发送运行时提示
//! ```
//!
//! # 线程安全
//!
//! 所有共享状态通过 Arc<RwLock<T>> 或 Arc<Mutex<T>> 保护，支持多连接并发访问。

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, atomic::AtomicU64},
};

use chrono::Utc;
use log::{debug, info, warn};
use tokio::sync::RwLock;

use super::{
    control_store::apply_control_response_snapshot,
    point_scope::{MasterPointAdmission, resolve_point_scope},
    point_store::{MasterPointStore, hydrate_soe_point_names, merge_received_points},
    soe_store::{apply_latest_event_timestamp_to_points, extract_master_soe_measurements},
};
use crate::{
    core::{
        iec104_registry::lookup_capability,
        master::{
            command::store::{
                CommandResponseIdentity, PendingCommand, is_control_command_type_id,
                resolve_pending_command_state_with_identity,
            },
            file_transfer::store::{MasterFileTransferSessionRuntime, handle_file_transfer_incoming},
            message_store::{
                MAX_MASTER_SOE_RECORDS, append_master_message, append_master_message_meta, append_soe_records,
            },
        },
        protocol_adapter::{decode_data_points, first_ioa_from_decoded_asdu},
        shared::runtime_hint::{emit_master_runtime_hint, emit_master_runtime_hint_with_cursor},
        types::{BackendSoeEvent, MasterMessageRecord, MessageDirection},
    },
    db::DatabaseService,
    errors::Iec104ErrorCode,
    network::{DecodedAsdu, DecodedAsduBody, DecodedCommandDetail},
    utils::{bytes_to_hex, logger::iec104_log_kv},
};

/// 主站数据摄取运行时
///
/// 封装了数据摄取所需的所有共享状态和资源。
/// 该结构体在主站服务启动时创建，并在整个生命周期中共享。
///
/// # 字段说明
///
/// - `data_points`：数据点缓存（key: "{scope_key}:{ioa}", value: DataPoint）
/// - `message_records`：消息记录队列（用于前端显示报文日志）
/// - `soe_records`：SOE 事件记录队列（用于前端显示事件序列）
/// - `pending_commands`：待完成命令队列（用于跟踪控制命令状态）
/// - `next_message_id`：下一个消息 ID（原子计数器）
/// - `next_soe_id`：下一个 SOE 事件 ID（原子计数器）
/// - `runtime_event_handle`：Tauri 应用句柄（用于发送运行时事件到前端）
/// - `connection_profile_map`：连接到配置文件的映射（key: connection_id, value: profile_id）
/// - `file_transfer_sessions`：文件传输会话（key: connection_id, value: 会话运行时）
/// - `db_service`：数据库服务（可选，用于查询从站配置）
///
/// # 线程安全
///
/// 所有字段都通过 Arc 共享，可变状态通过 RwLock 或 AtomicU64 保护。
#[derive(Clone)]
pub(crate) struct MasterIngestRuntime {
    /// 数据点缓存
    pub(crate) data_points: Arc<MasterPointStore>,
    /// 持久化点表派生的禁用点准入索引
    pub(crate) point_admission: Arc<RwLock<MasterPointAdmission>>,
    /// 消息记录队列
    pub(crate) message_records: Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    /// SOE 事件记录队列
    pub(crate) soe_records: Arc<RwLock<VecDeque<BackendSoeEvent>>>,
    /// 待完成命令队列
    pub(crate) pending_commands: Arc<RwLock<VecDeque<PendingCommand>>>,
    /// 下一个消息 ID（原子计数器）
    pub(crate) next_message_id: Arc<AtomicU64>,
    /// 下一个 SOE 事件 ID（原子计数器）
    pub(crate) next_soe_id: Arc<AtomicU64>,
    /// Tauri 应用句柄（用于发送运行时事件）
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    /// 文件传输会话
    pub(crate) file_transfer_sessions: Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    /// 数据库服务（可选）
    pub(crate) db_service: Arc<DatabaseService>,
}

/// 处理接收到的 I 帧
///
/// 这是数据摄取的核心入口函数，负责处理从传输运行时接收到的 I 帧（信息帧）。
/// 该函数执行完整的数据处理流程，包括命令响应识别、数据点提取、SOE 事件处理等。
///
/// # 参数
///
/// * `runtime` - 数据摄取运行时（包含所有共享状态）
/// * `station_id` - 主站 ID
/// * `connection_id` - 连接 ID
/// * `profile_id` - 配置文件 ID（可选）
/// * `decoded_asdu` - 已解码的 ASDU
/// * `raw_frame` - 原始帧数据（用于日志记录和时标提取）
///
/// # 执行流程
///
/// 1. **记录消息日志**：
///    - 提取第一个 IOA（信息对象地址）
///    - 记录调试日志
///    - 将 I 帧添加到消息记录队列
///
/// 2. **检查控制命令响应**：
///    - 调用 resolve_pending_command_state 检查是否为待完成命令的响应
///    - 如果是命令响应：
///      - 记录命令状态变化日志
///      - 如果是控制命令，更新控制点快照
///      - 如果是激活终止或去激活确认，发送 "command-confirmed" 运行时提示
///
/// 3. **解析作用域信息**：
///    - 从数据库查询从站 ID（根据 profile_id 和 common_address）
///    - 解析数据点作用域键（slave > profile > connection）
///
/// 4. **提取和处理数据**：
///    - 提取 SOE 事件（如果是带时标的 ASDU）
///    - 解码数据点（从 ASDU 中提取地址、值、质量等）
///    - 如果有 SOE 事件，将事件时标应用到数据点
///    - 如果有数据点，合并到内存缓存
///    - 如果有 SOE 事件，填充数据点名称并记录，发送 "soe-updated" 运行时提示
///
/// 5. **处理文件传输**：
///    - 检查是否为文件传输相关的 ASDU
///    - 如果是，处理文件传输并发送 "file-transfer-updated" 运行时提示
///
/// # 线程安全
///
/// 本函数通过 RwLock 访问共享状态，支持多连接并发调用。
pub(crate) async fn handle_received_iframe(
    runtime: &MasterIngestRuntime,
    station_id: &str,
    connection_id: &str,
    profile_id: Option<i64>,
    decoded_asdu: &DecodedAsdu,
    raw_frame: &[u8],
) {
    // 提取第一个 IOA（用于日志记录）
    let ioa = first_ioa_from_decoded_asdu(decoded_asdu);
    // 记录调试日志
    debug!(
        "[master] iframe_received {} ca={}",
        iec104_log_kv(
            station_id,
            Some(connection_id),
            Some(decoded_asdu.type_id),
            Some(decoded_asdu.cause),
            ioa,
            None,
            None,
        ),
        decoded_asdu.common_address
    );
    // 将 I 帧添加到消息记录队列
    append_master_message(
        &runtime.message_records,
        &runtime.next_message_id,
        MessageDirection::Received,
        connection_id,
        format!(
            "收到 I 帧 type_id={} cot={} ca={}",
            decoded_asdu.type_id, decoded_asdu.cause, decoded_asdu.common_address
        ),
        Some(bytes_to_hex(raw_frame)),
        Some(decoded_asdu.type_id),
        Some(decoded_asdu.cause),
        Some(decoded_asdu.common_address),
    )
    .await;

    // 检查是否为待完成命令的响应
    if let Some((command, state)) = resolve_pending_command_state_with_identity(
        &runtime.pending_commands,
        connection_id,
        decoded_asdu.type_id,
        decoded_asdu.cause,
        decoded_asdu.common_address,
        ioa,
        command_response_identity(decoded_asdu),
    )
    .await
    {
        let trace_id = command.trace_id.clone();
        // 根据命令状态确定消息方向（激活否定为错误，其他为接收）
        let direction = if matches!(state, "activation-negative" | "unknown-information-object") {
            MessageDirection::Error
        } else {
            MessageDirection::Received
        };
        // 记录命令状态变化日志
        append_master_message_meta(
            &runtime.message_records,
            &runtime.next_message_id,
            direction,
            connection_id,
            format!(
                "命令状态 trace_id={} state={} type_id={} cot={}",
                trace_id,
                state,
                decoded_asdu.type_id,
                decoded_asdu.cause & 0x3F
            ),
            None,
            Some(decoded_asdu.type_id),
            Some(decoded_asdu.cause),
            Some(decoded_asdu.common_address),
            Some(trace_id.clone()),
            Some(state.to_string()),
        )
        .await;

        // 记录信息日志
        info!(
            "[master] command_state {} state={}",
            iec104_log_kv(
                station_id,
                Some(connection_id),
                Some(decoded_asdu.type_id),
                Some(decoded_asdu.cause),
                ioa,
                if state == "activation-negative" { Some(Iec104ErrorCode::ProtocolState.as_str()) } else { None },
                Some(&trace_id),
            ),
            state
        );

        // 如果是控制命令，更新控制点快照
        if is_control_command_type_id(command.type_id) {
            let scope = resolve_point_scope(connection_id, command.slave_id);
            let disabled = if let Some(address) = command.ioa {
                runtime.point_admission.read().await.is_disabled(&scope, command.common_address, address)
            } else {
                false
            };
            if !disabled {
                apply_control_response_snapshot(&runtime.data_points, &command, state, decoded_asdu.cause).await;
            }
        }

        // 只有收到协议定义的成功终态后才发送完成提示。
        if matches!(
            state,
            "activation-termination" | "deactivation-confirmation" | "completion-on-confirmation" | "request-response"
        ) {
            emit_master_runtime_hint(
                &runtime.runtime_event_handle,
                station_id,
                Some(connection_id),
                "command-confirmed",
            )
            .await;
        }
    }

    // 从数据库查询从站 ID（根据 profile_id 和 common_address）
    if lookup_capability(decoded_asdu.type_id).is_some_and(|capability| !capability.master_receive) {
        warn!(
            "[master] unsupported_receive_type station_id={} connection_id={} type_id={} classification=parser_only",
            station_id, connection_id, decoded_asdu.type_id
        );
    }

    let slave_id = resolve_slave_id(&runtime.db_service, profile_id, decoded_asdu.common_address);
    // 解析数据点作用域键（优先级：slave > profile > connection）
    let point_scope = resolve_point_scope(connection_id, slave_id);

    // 提取 SOE 事件（如果是带时标的 ASDU）
    let mut soe_measurements =
        extract_master_soe_measurements(station_id, connection_id, slave_id, decoded_asdu, raw_frame);
    // 解码数据点（从 ASDU 中提取地址、值、质量等）
    let mut points = decode_data_points(
        connection_id,
        profile_id,
        slave_id,
        Some(decoded_asdu.common_address),
        decoded_asdu,
        Utc::now(),
    );
    {
        let admission = runtime.point_admission.read().await;
        points.retain(|point| {
            !admission.is_disabled(
                &point_scope,
                point.common_address.unwrap_or(decoded_asdu.common_address),
                point.address,
            )
        });
        soe_measurements.retain(|measurement| {
            !admission.is_disabled(&point_scope, measurement.event.common_address, measurement.event.ioa)
        });
    }
    // 如果有 SOE 事件，将事件时标应用到数据点
    if !soe_measurements.is_empty() {
        apply_latest_event_timestamp_to_points(&mut points, &soe_measurements);
    }
    // 如果有数据点，合并到内存缓存
    if !points.is_empty() {
        merge_received_points(&runtime.data_points, &point_scope, decoded_asdu.cause, &points).await;
        let cursor = runtime.data_points.cursor().await;
        emit_master_runtime_hint_with_cursor(
            &runtime.runtime_event_handle,
            station_id,
            Some(connection_id),
            "points-changed",
            cursor,
        )
        .await;
    }
    // 如果有 SOE 事件，填充数据点名称并记录
    if !soe_measurements.is_empty() {
        hydrate_soe_point_names(&runtime.data_points, &point_scope, &mut soe_measurements).await;
        let soe_events = soe_measurements.iter().map(|item| item.event.clone()).collect::<Vec<_>>();
        append_soe_records(&runtime.soe_records, &runtime.next_soe_id, &soe_events, MAX_MASTER_SOE_RECORDS).await;
        // 发送 SOE 更新运行时提示
        emit_master_runtime_hint(&runtime.runtime_event_handle, station_id, Some(connection_id), "soe-updated").await;
    }

    // 处理文件传输（如果是文件传输相关的 ASDU）
    if handle_file_transfer_incoming(&runtime.file_transfer_sessions, connection_id, decoded_asdu).await {
        // 发送文件传输更新运行时提示
        emit_master_runtime_hint(
            &runtime.runtime_event_handle,
            station_id,
            Some(connection_id),
            "file-transfer-updated",
        )
        .await;
    }
}

fn command_response_identity(decoded_asdu: &DecodedAsdu) -> Option<CommandResponseIdentity> {
    if let DecodedAsduBody::Interrogations(items) = &decoded_asdu.body {
        return items.first().map(|item| CommandResponseIdentity {
            target_value: None,
            qualifier: Some(item.qualifier),
            cp56: None,
        });
    }
    let DecodedAsduBody::Commands(commands) = &decoded_asdu.body else {
        return None;
    };
    let command = commands.first()?;
    let (target_value, qualifier) = match command.detail {
        DecodedCommandDetail::Single { value, .. } => {
            (Some(if value { 1.0 } else { 0.0 }), Some((command.qualifier.as_u8() >> 2) & 0x1F))
        }
        DecodedCommandDetail::Double { value, .. } => {
            (Some(f64::from(value)), Some((command.qualifier.as_u8() >> 2) & 0x1F))
        }
        DecodedCommandDetail::Step { position, .. } => {
            (Some(f64::from(position)), Some((command.qualifier.as_u8() >> 2) & 0x1F))
        }
        DecodedCommandDetail::SetPointNormalized(value) => {
            (Some(f64::from(value) / 32768.0), Some(command.qualifier.as_u8() & 0x7F))
        }
        DecodedCommandDetail::SetPointScaled(value) => (Some(f64::from(value)), Some(command.qualifier.as_u8() & 0x7F)),
        DecodedCommandDetail::SetPointFloat(value) => (Some(f64::from(value)), Some(command.qualifier.as_u8() & 0x7F)),
        DecodedCommandDetail::BitString(value) => (Some(f64::from(value)), None),
        _ => return None,
    };
    let cp56 = command.timestamp.as_deref().and_then(|raw| raw.try_into().ok());
    Some(CommandResponseIdentity { target_value, qualifier, cp56 })
}

/// 解析从站 ID
///
/// 根据配置文件 ID 和公共地址从数据库查询对应的从站 ID。
/// 这是一个内部辅助函数，用于确定数据点的作用域。
///
/// # 参数
///
/// * `db_service` - 数据库服务（可选）
/// * `profile_id` - 配置文件 ID（可选）
/// * `common_address` - 公共地址（ASDU 中的 CA 字段）
///
/// # 返回
///
/// - `Some(slave_id)` - 找到匹配的从站 ID
/// - `None` - 未找到匹配的从站或参数不足
///
/// # 查询逻辑
///
/// 1. 检查 profile_id 是否存在（如果不存在则返回 None）
/// 2. 从数据库查询该配置文件下的所有从站
/// 3. 查找 common_address 匹配的从站
/// 4. 返回匹配从站的 ID
///
/// # 使用场景
///
/// 在数据摄取时，需要根据 ASDU 的公共地址确定数据点属于哪个从站。
/// 这样可以实现从站级别的数据点隔离，多个连接可以共享同一个从站的数据点。
fn resolve_slave_id(db_service: &Arc<DatabaseService>, profile_id: Option<i64>, common_address: u16) -> Option<i64> {
    // 检查 profile_id 是否存在
    let profile_id = profile_id?;
    // 从数据库查询该配置文件下的所有从站，并查找匹配的从站 ID
    db_service
        .master_slaves
        .get_by_connection(profile_id)
        .ok()
        .and_then(|rows| rows.into_iter().find(|row| row.common_address == common_address).and_then(|row| row.id))
}
