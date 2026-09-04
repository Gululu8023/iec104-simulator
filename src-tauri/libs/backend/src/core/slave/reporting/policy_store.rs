//! 从站站点策略解析模块。
//!
//! 本模块负责从站站点策略的解析和合并,支持多层级配置覆盖,包括:
//!
//! - **全局默认策略**:应用于所有公共地址的默认配置
//! - **连接级覆盖**:基于连接参数的策略覆盖(可选启用)
//! - **站点级覆盖**:针对特定公共地址的策略覆盖
//! - **参数校验**:对超时时间等参数进行范围限制(1-3600秒)
//! - **策略合并**:按优先级合并多层配置,站点级 > 连接级 > 全局级
//!
//! # 策略优先级
//!
//! ```text
//! 站点级覆盖 (最高优先级)
//!   ↓ (如果未设置)
//! 连接级覆盖 (如果启用)
//!   ↓ (如果未设置或未启用)
//! 全局默认策略 (最低优先级)
//! ```

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::core::types::{LinkParams, SlaveStationPolicy, SlaveStationPolicyOverride};

pub(crate) fn sanitize_station_policy_defaults(policy: SlaveStationPolicy) -> SlaveStationPolicy {
    SlaveStationPolicy {
        select_timeout_seconds: policy.select_timeout_seconds.clamp(1, 3600),
        enable_sq1_upload: policy.enable_sq1_upload,
        enable_soe: policy.enable_soe,
        control_execution_mode: policy.control_execution_mode,
    }
}

pub(crate) fn sanitize_station_policy_override(policy: SlaveStationPolicyOverride) -> SlaveStationPolicyOverride {
    SlaveStationPolicyOverride {
        select_timeout_seconds: policy.select_timeout_seconds.map(|value| value.clamp(1, 3600)),
        enable_sq1_upload: policy.enable_sq1_upload,
        enable_soe: policy.enable_soe,
        control_execution_mode: policy.control_execution_mode,
    }
}

pub(crate) async fn resolve_effective_station_policy(
    link_params: &Arc<RwLock<LinkParams>>,
    station_policy_defaults: &Arc<RwLock<SlaveStationPolicy>>,
    station_policy_overrides: &Arc<RwLock<HashMap<u16, SlaveStationPolicyOverride>>>,
    station_policy_connection_override_enabled: &Arc<RwLock<bool>>,
    common_address: u16,
) -> SlaveStationPolicy {
    let global_defaults = sanitize_station_policy_defaults(*station_policy_defaults.read().await);
    let use_connection_override = *station_policy_connection_override_enabled.read().await;
    let connection_defaults = if use_connection_override {
        let params = *link_params.read().await;
        SlaveStationPolicy {
            select_timeout_seconds: params.select_timeout_seconds.clamp(1, 3600),
            enable_sq1_upload: params.enable_sq1_upload,
            enable_soe: params.enable_soe,
            control_execution_mode: global_defaults.control_execution_mode,
        }
    } else {
        global_defaults
    };
    let station_override = sanitize_station_policy_override(
        station_policy_overrides.read().await.get(&common_address).copied().unwrap_or_default(),
    );

    SlaveStationPolicy {
        select_timeout_seconds: station_override
            .select_timeout_seconds
            .unwrap_or(connection_defaults.select_timeout_seconds)
            .clamp(1, 3600),
        enable_sq1_upload: station_override.enable_sq1_upload.unwrap_or(connection_defaults.enable_sq1_upload),
        enable_soe: station_override.enable_soe.unwrap_or(connection_defaults.enable_soe),
        control_execution_mode: station_override
            .control_execution_mode
            .unwrap_or(connection_defaults.control_execution_mode),
    }
}
