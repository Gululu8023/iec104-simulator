//! 控制命令快照存储
//!
//! 本模块负责管理控制命令的状态快照，记录控制命令的执行过程和结果。
//!
//! # 核心功能
//!
//! - **分发快照**：记录控制命令发送时的状态（Select/Execute/Cancel）
//! - **响应快照**：记录从站响应时的状态（ACT_CON/ACT_TERM 等）
//! - **超时快照**：记录控制命令超时时的状态
//! - **控制点管理**：自动创建和维护控制点的数据结构
//!
//! # 控制命令状态机
//!
//! IEC104 控制命令遵循以下状态转换：
//!
//! ```text
//! Idle（空闲）
//!   ↓ Select 命令
//! Selected（已选择）
//!   ↓ Execute 命令
//! Executing（执行中）
//!   ↓ ACT_CON（激活确认）
//! Executing（执行中）
//!   ↓ ACT_TERM（激活终止）
//! Ok（成功）
//!
//! 异常路径：
//! - ACT_CON negative → Ng（失败）
//! - 超时 → Timeout（超时）
//! - Cancel → Idle（取消）
//! ```
//!
//! # 快照字段
//!
//! 控制状态快照（ControlStatusSnapshot）包含以下字段：
//! - `state`：当前状态（Idle/Selected/Executing/Ok/Ng/Timeout）
//! - `updated_at`：更新时间
//! - `latest_cause`：最新传输原因
//! - `actcon_negative`：激活确认是否为否定（true=失败，false=成功）
//! - `actcon_at`：激活确认时间
//! - `actterm_at`：激活终止时间
//! - `timeout_type`：超时类型（Select/Execute）
//! - `command_cp56`：CP56Time2a 时标（用于带时标的控制命令）

use std::sync::Arc;

use chrono::Utc;

use crate::core::{
    iec104_registry::{iec104_default_point_name, iec104_type_name},
    master::{
        SelectExecute,
        command::store::{PendingCommand, is_control_command_type_id},
        data::{
            point_scope::{data_point_key, resolve_point_scope},
            point_store::MasterPointStore,
        },
    },
    shared::versioned_point_store::PointStoreTransaction,
    types::{ControlStatusKind, ControlStatusSnapshot, ControlTimeoutType, DataPoint, PointSource},
};

/// 更新控制分发快照
///
/// 在控制命令发送时调用，记录命令的初始状态。
/// 根据 select_execute 参数确定状态：Select → Selected，Cancel → Idle，Execute → Executing。
///
/// # 参数
///
/// * `data_points` - 数据点缓存
/// * `connection_id` - 连接 ID
/// * `link_profile_id` - 配置文件 ID（可选）
/// * `slave_id` - 从站 ID（可选）
/// * `common_address` - 公共地址
/// * `address` - 信息对象地址（IOA）
/// * `type_id` - 类型标识（ASDU Type ID）
/// * `target_value` - 目标值（可选，如果提供则更新数据点值）
/// * `select_execute` - 选择/执行/取消标志
/// * `command_cp56` - CP56Time2a 时标（可选，用于带时标的控制命令）
///
/// # 状态映射
///
/// - `SelectExecute::Select` → `ControlStatusKind::Selected`（已选择）
/// - `SelectExecute::Cancel` → `ControlStatusKind::Idle`（空闲）
/// - `None` 或其他 → `ControlStatusKind::Executing`（执行中）
///
/// # 执行流程
///
/// 1. 获取当前时间
/// 2. 根据 select_execute 确定状态
/// 3. 获取数据点缓存的写锁
/// 4. 确保控制点存在（如果不存在则创建）
/// 5. 更新数据点值（如果提供了 target_value）
/// 6. 更新数据点时标
/// 7. 创建控制状态快照并保存
///
/// # 线程安全
///
/// 本函数通过 RwLock 写锁访问共享状态，确保数据一致性。
pub(crate) async fn upsert_control_dispatch_snapshot(
    data_points: &Arc<MasterPointStore>,
    connection_id: &str,
    link_profile_id: Option<i64>,
    slave_id: Option<i64>,
    common_address: u16,
    address: u32,
    type_id: u8,
    target_value: Option<f64>,
    select_execute: Option<SelectExecute>,
    command_cp56: Option<String>,
) {
    // 获取当前时间
    let updated_at = Utc::now();
    // 根据 select_execute 确定状态
    let state = match select_execute {
        Some(SelectExecute::Select) => ControlStatusKind::Selected,
        Some(SelectExecute::Cancel) => ControlStatusKind::Idle,
        _ => ControlStatusKind::Executing,
    };

    // 获取数据点缓存的写锁
    let mut guard = data_points.transaction().await;
    // 确保控制点存在
    let point = ensure_master_control_point(
        &mut guard,
        connection_id,
        link_profile_id,
        slave_id,
        common_address,
        address,
        type_id,
    );
    // 更新数据点值（如果提供）
    if let Some(value) = target_value {
        point.value = value;
    }
    // 更新时标
    point.timestamp = updated_at;
    // 创建控制状态快照
    point.control_status_snapshot = Some(ControlStatusSnapshot {
        state,
        updated_at,
        latest_cause: Some(select_execute.map_or(6, SelectExecute::cause).into()),
        actcon_negative: None,
        actcon_at: None,
        actterm_at: None,
        timeout_type: None,
        select_execute: select_execute.map(SelectExecute::is_select),
        // 保留已有的 CP56 时标或使用新提供的
        command_cp56: command_cp56
            .or_else(|| point.control_status_snapshot.as_ref().and_then(|snapshot| snapshot.command_cp56.clone())),
    });
}

/// 应用控制响应快照
///
/// 在收到从站的控制响应时调用，更新控制命令的状态快照。
/// 根据响应类型（ACT_CON/ACT_TERM 等）更新状态和时间戳。
///
/// # 参数
///
/// * `data_points` - 数据点缓存
/// * `command` - 待完成命令（包含命令元信息）
/// * `state` - 响应状态字符串（如 "activation-confirmation"）
/// * `cause` - 传输原因（Cause of Transmission）
///
/// # 响应状态映射
///
/// - `"activation-negative"` → `ControlStatusKind::Ng`（失败）
///   - 设置 actcon_negative = true
///   - 记录 actcon_at 时间
/// - `"selection-confirmation"` → `ControlStatusKind::Selected`（选择确认）
///   - 设置 actcon_negative = false
///   - 记录 actcon_at 时间
/// - `"activation-confirmation"` → `ControlStatusKind::Executing`（激活确认）
///   - 设置 actcon_negative = false
///   - 记录 actcon_at 时间
/// - `"completion-on-confirmation"` → `ControlStatusKind::Ok`（完成确认）
///   - 设置 actcon_negative = false
///   - 记录 actcon_at 时间
/// - `"activation-termination"` → `ControlStatusKind::Ok`（激活终止）
///   - 记录 actterm_at 时间
/// - `"deactivation-confirmation"` → `ControlStatusKind::Idle`（去激活确认）
///
/// # 执行流程
///
/// 1. 检查命令是否包含 IOA（如果没有则直接返回）
/// 2. 检查是否为控制命令类型（如果不是则直接返回）
/// 3. 获取当前时间
/// 4. 获取数据点缓存的写锁
/// 5. 确保控制点存在
/// 6. 克隆或创建控制状态快照
/// 7. 更新快照的通用字段（updated_at、latest_cause、timeout_type）
/// 8. 根据响应状态更新特定字段
/// 9. 更新数据点时标和快照
///
/// # 线程安全
///
/// 本函数通过 RwLock 写锁访问共享状态，确保数据一致性。
pub(crate) async fn apply_control_response_snapshot(
    data_points: &Arc<MasterPointStore>,
    command: &PendingCommand,
    state: &str,
    cause: u16,
) {
    // 检查命令是否包含 IOA
    let Some(address) = command.ioa else {
        return;
    };
    // 检查是否为控制命令类型
    if !is_control_command_type_id(command.type_id) {
        return;
    }

    // 获取当前时间
    let updated_at = Utc::now();
    // 获取数据点缓存的写锁
    let mut guard = data_points.transaction().await;
    // 确保控制点存在
    let point = ensure_master_control_point(
        &mut guard,
        &command.connection_id,
        command.link_profile_id,
        command.slave_id,
        command.common_address,
        address,
        command.type_id,
    );

    // 克隆或创建控制状态快照
    let mut snapshot = point.control_status_snapshot.clone().unwrap_or(ControlStatusSnapshot {
        state: ControlStatusKind::Idle,
        updated_at,
        latest_cause: None,
        actcon_negative: None,
        actcon_at: None,
        actterm_at: None,
        timeout_type: None,
        select_execute: command.select_execute.map(SelectExecute::is_select),
        command_cp56: None,
    });

    // 更新快照的通用字段
    snapshot.updated_at = updated_at;
    snapshot.latest_cause = Some(cause);
    snapshot.timeout_type = None; // 清除超时标记

    // 根据响应状态更新特定字段
    match state {
        "activation-negative" => {
            // 激活否定（失败）
            snapshot.state = ControlStatusKind::Ng;
            snapshot.actcon_negative = Some(true);
            snapshot.actcon_at = Some(updated_at);
        }
        "selection-confirmation" => {
            // 选择确认
            snapshot.state = ControlStatusKind::Selected;
            snapshot.actcon_negative = Some(false);
            snapshot.actcon_at = Some(updated_at);
        }
        "activation-confirmation" => {
            // 激活确认
            snapshot.state = ControlStatusKind::Executing;
            snapshot.actcon_negative = Some(false);
            snapshot.actcon_at = Some(updated_at);
        }
        "completion-on-confirmation" => {
            // 完成确认
            snapshot.state = ControlStatusKind::Ok;
            snapshot.actcon_negative = Some(false);
            snapshot.actcon_at = Some(updated_at);
        }
        "activation-termination" => {
            // 激活终止
            snapshot.state = ControlStatusKind::Ok;
            snapshot.actterm_at = Some(updated_at);
        }
        "deactivation-confirmation" => {
            // 去激活确认
            snapshot.state = ControlStatusKind::Idle;
        }
        _ => {}
    }

    // 更新数据点时标和快照
    point.timestamp = updated_at;
    point.control_status_snapshot = Some(snapshot);
}

/// 应用控制超时快照
///
/// 在控制命令超时时调用，将控制点状态标记为超时。
/// 区分 Select 超时和 Execute 超时两种类型。
///
/// # 参数
///
/// * `data_points` - 数据点缓存
/// * `command` - 待完成命令（包含命令元信息）
///
/// # 超时类型
///
/// - **Select 超时**：Select 或 Cancel 命令超时
/// - **Execute 超时**：Execute 命令超时
///
/// # 执行流程
///
/// 1. 检查命令是否包含 IOA（如果没有则直接返回）
/// 2. 检查是否为控制命令类型（如果不是则直接返回）
/// 3. 获取当前时间
/// 4. 获取数据点缓存的写锁
/// 5. 确保控制点存在
/// 6. 克隆或创建控制状态快照
/// 7. 设置状态为 Timeout
/// 8. 根据 select_execute 确定超时类型
/// 9. 更新数据点时标和快照
///
/// # 线程安全
///
/// 本函数通过 RwLock 写锁访问共享状态，确保数据一致性。
pub(crate) async fn apply_control_timeout_snapshot(data_points: &Arc<MasterPointStore>, command: &PendingCommand) {
    // 检查命令是否包含 IOA
    let Some(address) = command.ioa else {
        return;
    };
    // 检查是否为控制命令类型
    if !is_control_command_type_id(command.type_id) {
        return;
    }

    // 获取当前时间
    let updated_at = Utc::now();
    // 获取数据点缓存的写锁
    let mut guard = data_points.transaction().await;
    // 确保控制点存在
    let point = ensure_master_control_point(
        &mut guard,
        &command.connection_id,
        command.link_profile_id,
        command.slave_id,
        command.common_address,
        address,
        command.type_id,
    );

    // 克隆或创建控制状态快照
    let mut snapshot = point.control_status_snapshot.clone().unwrap_or(ControlStatusSnapshot {
        state: ControlStatusKind::Idle,
        updated_at,
        latest_cause: None,
        actcon_negative: None,
        actcon_at: None,
        actterm_at: None,
        timeout_type: None,
        select_execute: command.select_execute.map(SelectExecute::is_select),
        command_cp56: None,
    });

    // 设置状态为超时
    snapshot.state = ControlStatusKind::Timeout;
    snapshot.updated_at = updated_at;
    // 根据 select_execute 确定超时类型
    snapshot.timeout_type = Some(match command.select_execute {
        Some(SelectExecute::Select) | Some(SelectExecute::Cancel) => ControlTimeoutType::Select,
        _ => ControlTimeoutType::Execute,
    });

    // 更新数据点时标和快照
    point.timestamp = updated_at;
    point.control_status_snapshot = Some(snapshot);
}

/// 确保主站控制点存在
///
/// 内部辅助函数，用于确保指定的控制点在数据点缓存中存在。
/// 如果控制点不存在，则创建一个默认的控制点结构。
///
/// # 参数
///
/// * `map` - 数据点缓存的可变引用
/// * `connection_id` - 连接 ID
/// * `link_profile_id` - 配置文件 ID（可选）
/// * `slave_id` - 从站 ID（可选）
/// * `common_address` - 公共地址
/// * `address` - 信息对象地址（IOA）
/// * `type_id` - 类型标识（ASDU Type ID）
///
/// # 返回
///
/// 返回数据点的可变引用，保证该数据点存在于缓存中。
///
/// # 默认值
///
/// 如果控制点不存在，创建时使用以下默认值：
/// - `name`：根据 type_id 和 address 生成默认名称（如 "M_SP_NA_1_100"）
/// - `data_type`：根据 type_id 获取类型名称
/// - `value`：0.0
/// - `quality`：0
/// - `quality_common`：默认质量描述符
/// - `quality_detail`：None
/// - `business_usable`：true
/// - `timestamp`：当前时间
/// - `point_source`：Manual（手动创建）
/// - `control_status_snapshot`：None（初始无快照）
///
/// # 更新逻辑
///
/// 如果控制点已存在，会更新以下字段：
/// - `connection_id`：更新为当前连接 ID
/// - `link_profile_id`：使用 or 逻辑合并（优先保留已有值）
/// - `slave_id`：使用 or 逻辑合并（优先保留已有值）
/// - `common_address`：更新为当前公共地址
///
/// # 生命周期
///
/// 返回的引用生命周期 `'a` 与输入的 map 引用生命周期相同。
fn ensure_master_control_point<'a>(
    map: &'a mut PointStoreTransaction<'_, crate::core::master::data::point_scope::MasterPointKey, DataPoint>,
    connection_id: &str,
    link_profile_id: Option<i64>,
    slave_id: Option<i64>,
    common_address: u16,
    address: u32,
    type_id: u8,
) -> &'a mut DataPoint {
    // 解析作用域键
    let scope = resolve_point_scope(connection_id, slave_id);
    // 生成数据点键
    let key = data_point_key(&scope, common_address, address);
    // 获取或创建数据点
    let point = map.entry(key).or_insert_with(|| DataPoint {
        connection_id: connection_id.to_string(),
        link_profile_id,
        slave_id,
        common_address: Some(common_address),
        address,
        name: iec104_default_point_name(type_id, address), // 生成默认名称
        description: None,
        control_ioa: None,
        gi_group: None,
        counter_group: None,
        type_id,
        data_type: iec104_type_name(type_id).to_string(), // 获取类型名称
        value: 0.0,
        quality: 0,
        quality_common: Some(crate::core::types::DataPointQualityCommon::default()),
        quality_detail: Some(crate::core::types::DataPointQualityDetail::None),
        business_usable: true,
        timestamp: Utc::now(),
        latest_event_timestamp: None,
        timestamp_detail: None,
        report_count: None,
        latest_cause: None,
        point_source: Some(PointSource::Manual), // 标记为手动创建
        control_status_snapshot: None,
    });
    // 更新连接信息（确保与当前连接一致）
    point.connection_id = connection_id.to_string();
    point.link_profile_id = point.link_profile_id.or(link_profile_id);
    point.slave_id = point.slave_id.or(slave_id);
    point.common_address = Some(common_address);
    point
}
