//! 数据点存储管理
//!
//! 本模块提供数据点缓存的辅助操作函数，包括数据点合并和 SOE 事件名称填充。
//!
//! # 核心功能
//!
//! - **数据点合并**：将接收到的数据点合并到内存缓存，保留用户配置字段
//! - **SOE 名称填充**：为 SOE 事件填充数据点名称，便于前端显示
//!
//! # 合并策略
//!
//! 数据点合并时采用以下策略：
//! - **保留字段**：name、description、control_ioa、point_source、control_status_snapshot
//!   （这些字段通常由用户配置或数据库预加载，不应被运行时数据覆盖）
//! - **更新字段**：value、quality、timestamp、latest_cause、report_count
//!   （这些字段反映运行时状态，应随接收到的数据更新）
//! - **合并字段**：link_profile_id、slave_id、common_address、latest_event_timestamp （使用 `or`
//!   逻辑，优先保留已有值）

use std::sync::Arc;

use log::warn;

use super::{
    point_scope::{MasterPointKey, MasterPointScope, data_point_key},
    soe_store::MasterSoeMeasurement,
};
use crate::core::{
    iec104_registry::same_type_family, shared::versioned_point_store::VersionedPointStore, types::DataPoint,
};

pub(crate) type MasterPointStore = VersionedPointStore<MasterPointKey, DataPoint>;

/// 为 SOE 事件填充数据点名称
///
/// 从数据点缓存中查找对应的数据点，将名称填充到 SOE 事件中。
/// 用于在返回 SOE 事件给前端时，附加数据点名称以便显示。
///
/// # 参数
///
/// * `data_points` - 数据点缓存（key: "{scope_key}:{ioa}", value: DataPoint）
/// * `scope_key` - 作用域键（用于生成数据点键）
/// * `events` - SOE 事件列表（可变引用，会被原地修改）
///
/// # 执行流程
///
/// 1. 如果事件列表为空，直接返回
/// 2. 获取数据点缓存的读锁
/// 3. 遍历每个 SOE 事件：
///    - 根据 scope_key 和 ioa 生成数据点键
///    - 从缓存中查找对应的数据点
///    - 如果找到，将数据点名称填充到事件的 point_name 字段
///
/// # 线程安全
///
/// 本函数通过 RwLock 读锁访问共享状态，支持并发调用。
pub(crate) async fn hydrate_soe_point_names(
    data_points: &Arc<MasterPointStore>,
    scope: &MasterPointScope,
    events: &mut [MasterSoeMeasurement],
) {
    // 空列表直接返回
    if events.is_empty() {
        return;
    }

    // 获取数据点缓存的读锁
    let guard = data_points.read().await;
    // 遍历每个 SOE 事件
    for item in events {
        // 生成数据点键
        let key = data_point_key(scope, item.event.common_address, item.event.ioa);
        // 查找数据点并填充名称
        if let Some(point) = guard.get(&key) {
            item.event.point_name = Some(point.name.clone());
        }
    }
}

/// 合并接收到的数据点到缓存
///
/// 将从 ASDU 中解析出的数据点合并到内存缓存中。
/// 采用智能合并策略，保留用户配置字段，更新运行时状态字段。
///
/// # 参数
///
/// * `data_points` - 数据点缓存（key: "{scope_key}:{ioa}", value: DataPoint）
/// * `scope_key` - 作用域键（用于生成数据点键）
/// * `cause` - 传输原因（Cause of Transmission）
/// * `points` - 接收到的数据点列表
///
/// # 合并策略
///
/// ## 保留字段（不覆盖已有值）
/// - `name`：数据点名称（用户配置）
/// - `description`：数据点描述（用户配置）
/// - `control_ioa`：控制点地址（用户配置）
/// - `point_source`：数据点来源（优先保留已有值）
/// - `control_status_snapshot`：控制状态快照（保留控制命令状态）
///
/// ## 更新字段（使用新值）
/// - `value`：数据点值（运行时数据）
/// - `quality`：质量描述符（运行时数据）
/// - `timestamp`：时标（运行时数据）
/// - `latest_cause`：最新传输原因（设置为参数 cause）
/// - `report_count`：上报次数（递增计数）
///
/// ## 合并字段（优先保留已有值）
/// - `link_profile_id`：配置文件 ID（使用 or 逻辑）
/// - `slave_id`：从站 ID（使用 or 逻辑）
/// - `common_address`：公共地址（使用 or 逻辑）
/// - `latest_event_timestamp`：最新事件时标（使用 or 逻辑）
///
/// # 执行流程
///
/// 1. 获取数据点缓存的写锁
/// 2. 遍历每个接收到的数据点：
///    - 生成数据点键
///    - 克隆接收到的数据点作为合并基础
///    - 如果缓存中已存在该数据点：
///      - 保留用户配置字段（name、description、control_ioa 等）
///      - 递增上报次数
///      - 合并可选字段（link_profile_id、slave_id 等）
///    - 如果是新数据点：
///      - 初始化上报次数为 1
///    - 设置最新传输原因
///    - 插入或更新缓存
///
/// # 线程安全
///
/// 本函数通过 RwLock 写锁访问共享状态，确保数据一致性。
pub(crate) async fn merge_received_points(
    data_points: &Arc<MasterPointStore>,
    scope: &MasterPointScope,
    cause: u16,
    points: &[DataPoint],
) {
    // 获取数据点缓存的写锁
    let mut guard = data_points.transaction().await;
    // 遍历每个接收到的数据点
    for point in points {
        // 生成数据点键
        let common_address = point.common_address.unwrap_or(0);
        let key = data_point_key(scope, common_address, point.address);
        let fallback_key = data_point_key(scope, 0, point.address);
        // 克隆接收到的数据点作为合并基础
        let mut merged = point.clone();
        // 如果缓存中已存在该数据点
        if let Some(existing) = guard.get(&key).or_else(|| guard.get(&fallback_key)) {
            if !same_type_family(existing.type_id, point.type_id) {
                warn!(
                    "[master] reject point type-family conflict scope={:?} ca={} ioa={} existing_type={} incoming_type={}",
                    scope, common_address, point.address, existing.type_id, point.type_id
                );
                continue;
            }
            // 保留用户配置字段
            merged.name = existing.name.clone();
            merged.description = existing.description.clone();
            merged.control_ioa = existing.control_ioa;
            // 保留数据点来源（优先已有值）
            merged.point_source = existing.point_source.or(merged.point_source);
            // 保留控制状态快照
            merged.control_status_snapshot = existing.control_status_snapshot.clone();
            // 递增上报次数
            merged.report_count = Some(existing.report_count.unwrap_or(0).saturating_add(1));
            // 合并可选字段（优先保留已有值）
            merged.link_profile_id = merged.link_profile_id.or(existing.link_profile_id);
            merged.slave_id = merged.slave_id.or(existing.slave_id);
            merged.common_address = merged.common_address.or(existing.common_address);
            merged.latest_event_timestamp = merged.latest_event_timestamp.or(existing.latest_event_timestamp);
        } else {
            // 新数据点，初始化上报次数
            merged.report_count = Some(1);
        }
        // 设置最新传输原因
        merged.latest_cause = Some(cause);
        // 插入或更新缓存
        guard.remove(&fallback_key);
        guard.insert(key, merged);
    }
}
