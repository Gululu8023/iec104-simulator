//! 主站命令超时检测和处理
//!
//! 本模块负责检测和处理超时的待完成命令,确保命令不会无限期等待响应。
//!
//! # 核心功能
//!
//! - **超时检测**：定期扫描待完成命令队列,识别超时的命令
//! - **超时处理**：对超时命令执行清理操作,更新控制快照状态
//! - **报文记录**：记录超时事件到报文缓存,便于追踪和调试
//!
//! # 超时机制
//!
//! 命令超时检测通常由连接的后台监控任务定期触发（如每秒一次）。
//! 超时阈值可配置,默认通常为 10-30 秒。
//!
//! # 超时后果
//!
//! - 命令从待完成队列中移除
//! - 如果是控制命令,数据点的控制状态快照会被标记为超时
//! - 记录超时报文到报文缓存,方向为 Error
//! - 前端可以通过报文列表看到超时事件

use std::{
    collections::VecDeque,
    sync::{Arc, atomic::AtomicU64},
};

use chrono::Utc;
use tokio::sync::RwLock;

use crate::core::{
    master::{
        command::store::{PendingCommand, take_expired_pending_commands},
        data::{
            control_store::apply_control_timeout_snapshot,
            point_scope::{MasterPointAdmission, resolve_point_scope},
            point_store::MasterPointStore,
        },
        message_store::append_master_message_meta,
    },
    types::{MasterMessageRecord, MessageDirection},
};

/// 使待完成命令过期（超时处理）
///
/// 扫描指定连接的待完成命令队列,找出超时的命令并执行清理操作。
/// 通常由连接的后台监控任务定期调用（如每秒一次）。
///
/// # 参数
///
/// * `pending` - 待完成命令队列
/// * `data_points` - 数据点缓存（用于更新控制快照状态）
/// * `records` - 通信报文缓存（用于记录超时事件）
/// * `next_id` - 报文 ID 生成器
/// * `connection_id` - 连接 ID（只处理该连接的命令）
/// * `timeout_seconds` - 超时阈值（秒）
///
/// # 执行流程
///
/// 1. **获取当前时间**：使用 UTC 时间作为基准
/// 2. **提取超时命令**：调用 take_expired_pending_commands() 从队列中移除超时命令
/// 3. **遍历超时命令**：对每个超时命令执行以下操作：
///    - 应用控制超时快照：如果是控制命令,更新数据点的控制状态为超时
///    - 记录超时报文：将超时事件记录到报文缓存,方向为 Error
///
/// # 超时判定
///
/// 命令的超时判定基于创建时间（created_at）：
/// - 如果 `(当前时间 - 创建时间) >= timeout_seconds`,则视为超时
///
/// # 报文格式
///
/// 超时报文的格式为：
/// - 方向：Error
/// - 描述：`"命令超时 trace_id={} type_id={} ioa={}"`
/// - 命令状态：`"timeout"`
///
/// # 线程安全
///
/// 本函数是异步的,所有共享状态访问都通过 RwLock 保护,支持并发调用。
pub(crate) async fn expire_pending_commands(
    pending: &Arc<RwLock<VecDeque<PendingCommand>>>,
    data_points: &Arc<MasterPointStore>,
    point_admission: &Arc<RwLock<MasterPointAdmission>>,
    records: &Arc<RwLock<VecDeque<MasterMessageRecord>>>,
    next_id: &Arc<AtomicU64>,
    connection_id: &str,
    timeout_seconds: i64,
) {
    let now = Utc::now();
    let expired = take_expired_pending_commands(pending, connection_id, now, timeout_seconds).await;

    for cmd in expired {
        let scope = resolve_point_scope(&cmd.connection_id, cmd.slave_id);
        let disabled = if let Some(address) = cmd.ioa {
            point_admission.read().await.is_disabled(&scope, cmd.common_address, address)
        } else {
            false
        };
        if !disabled {
            apply_control_timeout_snapshot(data_points, &cmd).await;
        }
        append_master_message_meta(
            records,
            next_id,
            MessageDirection::Error,
            connection_id,
            format!("命令超时 trace_id={} type_id={} ioa={}", cmd.trace_id, cmd.type_id, cmd.ioa.unwrap_or(0)),
            None,
            Some(cmd.type_id),
            None,
            Some(cmd.common_address),
            Some(cmd.trace_id),
            Some("timeout".to_string()),
        )
        .await;
    }
}
