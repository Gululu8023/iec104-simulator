//! 待完成命令存储
//!
//! 本模块管理主站发送的待完成命令队列,负责命令的入队、出队、
//! 状态解析和超时检测。

use std::{collections::VecDeque, sync::Arc};

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use super::super::SelectExecute;
use crate::core::iec104_registry::lookup_capability;

/// 待完成命令
///
/// 记录已发送但尚未收到最终响应的命令信息。
/// 用于跟踪命令的执行状态和超时检测。
#[derive(Debug, Clone)]
pub(crate) struct PendingCommand {
    /// 命令追踪 ID（UUID）,用于关联请求和响应
    pub(crate) trace_id: String,
    /// 连接 ID
    pub(crate) connection_id: String,
    /// ASDU 类型标识（TypeID）
    pub(crate) type_id: u8,
    /// 链路配置文件 ID（用于数据库关联）
    pub(crate) link_profile_id: Option<i64>,
    /// 从站 ID（用于数据库关联）
    pub(crate) slave_id: Option<i64>,
    /// 公共地址
    pub(crate) common_address: u16,
    /// 信息对象地址（IOA）
    pub(crate) ioa: Option<u32>,
    /// 选择/执行标志（用于区分 Select 和 Execute）
    pub(crate) select_execute: Option<SelectExecute>,
    /// CP56 identity for timed controls.
    pub(crate) cp56: Option<[u8; 7]>,
    pub(crate) target_value: Option<f64>,
    pub(crate) qualifier: Option<u8>,
    /// 命令创建时间（用于超时检测）
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandResponseIdentity {
    pub(crate) target_value: Option<f64>,
    pub(crate) qualifier: Option<u8>,
    pub(crate) cp56: Option<[u8; 7]>,
}

/// 将命令加入待完成队列
///
/// # 参数
///
/// * `pending` - 待完成命令队列
/// * `command` - 待加入的命令
pub(crate) async fn enqueue_pending_command(pending: &Arc<RwLock<VecDeque<PendingCommand>>>, command: PendingCommand) {
    pending.write().await.push_back(command);
}

/// 根据追踪 ID 移除待完成命令
///
/// # 参数
///
/// * `pending` - 待完成命令队列
/// * `trace_id` - 命令追踪 ID
///
/// # 返回
///
/// - `Some(PendingCommand)` - 找到并移除的命令
/// - `None` - 未找到匹配的命令
pub(crate) async fn remove_pending_command_by_trace(
    pending: &Arc<RwLock<VecDeque<PendingCommand>>>,
    trace_id: &str,
) -> Option<PendingCommand> {
    let mut guard = pending.write().await;
    let idx = guard.iter().position(|cmd| cmd.trace_id == trace_id)?;
    guard.remove(idx)
}

/// 移除指定连接的所有待完成命令
///
/// 用于连接断开时清理该连接的所有待完成命令。
///
/// # 参数
///
/// * `pending` - 待完成命令队列
/// * `connection_id` - 连接 ID
pub(crate) async fn remove_pending_commands_for_connection(
    pending: &Arc<RwLock<VecDeque<PendingCommand>>>,
    connection_id: &str,
) {
    let mut guard = pending.write().await;
    guard.retain(|cmd| cmd.connection_id != connection_id);
}

/// 提取指定连接的超时命令
///
/// 遍历队列,找出指定连接中超时的命令并移除。
///
/// # 参数
///
/// * `pending` - 待完成命令队列
/// * `connection_id` - 连接 ID
/// * `now` - 当前时间
/// * `timeout_seconds` - 超时阈值（秒）
///
/// # 返回
///
/// 返回所有超时的命令向量
pub(crate) async fn take_expired_pending_commands(
    pending: &Arc<RwLock<VecDeque<PendingCommand>>>,
    connection_id: &str,
    now: DateTime<Utc>,
    timeout_seconds: i64,
) -> Vec<PendingCommand> {
    let mut expired = Vec::new();
    let mut guard = pending.write().await;
    let mut idx = 0usize;
    while idx < guard.len() {
        let stale =
            guard[idx].connection_id == connection_id && (now - guard[idx].created_at).num_seconds() >= timeout_seconds;
        if stale {
            if let Some(cmd) = guard.remove(idx) {
                expired.push(cmd);
                continue;
            }
        }
        idx += 1;
    }
    expired
}

/// 解析待完成命令的状态
///
/// 根据从站返回的 ASDU 响应,匹配待完成队列中的命令并解析其状态。
/// 支持多种响应类型:激活确认、激活否定、激活终止、选择确认等。
///
/// # 参数
///
/// * `pending` - 待完成命令队列
/// * `connection_id` - 连接 ID
/// * `type_id` - ASDU 类型标识
/// * `cause` - 传送原因（COT）,包含否定标志位
/// * `common_address` - 公共地址
/// * `ioa` - 信息对象地址
///
/// # 返回
///
/// - `Some((PendingCommand, &'static str))` - 匹配的命令和状态字符串
/// - `None` - 未找到匹配的命令或无法解析状态
///
/// # 状态说明
///
/// - `"activation-negative"` - 激活否定（COT 否定位为 1）
/// - `"activation-termination"` - 激活终止（COT=10）
/// - `"deactivation-confirmation"` - 停止激活确认（COT=9）
/// - `"selection-confirmation"` - 选择确认（COT=7 且为 Select 命令）
/// - `"activation-confirmation"` - 激活确认（COT=7）
/// - `"completion-on-confirmation"` - 确认时完成（其他情况）
/// - `"request-response"` - 读命令已收到被读点响应（COT=5）
/// - `"unknown-information-object"` - 读命令目标不存在或不可读（Type 102/COT=47）
pub(crate) async fn resolve_pending_command_state_with_identity(
    pending: &Arc<RwLock<VecDeque<PendingCommand>>>,
    connection_id: &str,
    type_id: u8,
    cause: u16,
    common_address: u16,
    ioa: Option<u32>,
    identity: Option<CommandResponseIdentity>,
) -> Option<(PendingCommand, &'static str)> {
    let cot = (cause & 0x3F) as u8;
    let negative = (cause & 0x40) != 0;

    let mut guard = pending.write().await;
    let idx = guard.iter().position(|cmd| {
        let type_matches = cmd.type_id == type_id
            || (cmd.type_id == 102
                && cot == 5
                && lookup_capability(type_id)
                    .is_some_and(|capability| capability.point_configurable && capability.master_receive));
        cmd.connection_id == connection_id
            && type_matches
            && cmd.common_address == common_address
            && pending_ioa_matches(cmd.ioa, ioa)
            && (cmd.type_id == 102
                || identity.is_none_or(|identity| {
                    cmd.target_value.map(f64::to_bits) == identity.target_value.map(f64::to_bits)
                        && cmd.qualifier == identity.qualifier
                        && cmd.cp56 == identity.cp56
                }))
    })?;

    if guard[idx].type_id == 102 {
        if cot == 5 {
            return guard.remove(idx).map(|cmd| (cmd, "request-response"));
        }
        if type_id == 102 && cot == 47 {
            return guard.remove(idx).map(|cmd| (cmd, "unknown-information-object"));
        }
    }

    if negative {
        if let Some(cmd) = guard.remove(idx) {
            return Some((cmd, "activation-negative"));
        }
        return None;
    }

    if cot == 10 {
        if let Some(cmd) = guard.remove(idx) {
            return Some((cmd, "activation-termination"));
        }
        return None;
    }

    if cot == 9 {
        if let Some(cmd) = guard.remove(idx) {
            return Some((cmd, "deactivation-confirmation"));
        }
        return None;
    }

    if cot == 7 {
        if let Some(cmd) = guard.get(idx) {
            if cmd.select_execute == Some(SelectExecute::Select) {
                if let Some(cmd) = guard.remove(idx) {
                    return Some((cmd, "selection-confirmation"));
                }
                return None;
            }
        }

        if matches!(type_id, 100 | 101 | 103) {
            if let Some(cmd) = guard.get(idx).cloned() {
                return Some((cmd, "activation-confirmation"));
            }
            return None;
        }

        if is_control_command_type_id(type_id) {
            if let Some(cmd) = guard.get_mut(idx) {
                return Some((cmd.clone(), "activation-confirmation"));
            }
            return None;
        }

        if let Some(cmd) = guard.remove(idx) {
            return Some((cmd, "completion-on-confirmation"));
        }
        return None;
    }

    None
}

/// 判断是否为控制命令类型
///
/// 控制命令包括单点控制、双点控制、设定值命令等。
///
/// # 参数
///
/// * `type_id` - ASDU 类型标识
///
/// # 返回
///
/// - `true` - 是控制命令（TypeID 45-51 或 58-64）
/// - `false` - 不是控制命令
///
/// # 控制命令类型
///
/// - 45-51: 单点/双点/步调节/设定值命令（不带时标）
/// - 58-64: 单点/双点/步调节/设定值命令（带时标 CP56Time2a）
pub(crate) fn is_control_command_type_id(type_id: u8) -> bool {
    matches!(type_id, 45..=51 | 58..=64)
}

/// 判断 IOA 是否匹配
///
/// 用于匹配待完成命令和响应的 IOA。
/// 支持特殊情况:0 和 None 视为匹配。
///
/// # 参数
///
/// * `expected` - 期望的 IOA（来自待完成命令）
/// * `actual` - 实际的 IOA（来自响应）
///
/// # 返回
///
/// - `true` - IOA 匹配
/// - `false` - IOA 不匹配
fn pending_ioa_matches(expected: Option<u32>, actual: Option<u32>) -> bool {
    match (expected, actual) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        (Some(0), None) | (None, Some(0)) => true,
        _ => false,
    }
}
