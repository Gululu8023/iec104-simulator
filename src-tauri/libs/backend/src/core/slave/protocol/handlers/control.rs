use iec60870_parser::parser::asdu::CP56Time2a;
use log::warn;

use crate::{
    core::{
        protocol_adapter::{CommandCotAction, SetPointCommandValue, SlaveCommandEvent, build_ioa_qualifier_payload},
        shared::versioned_point_store::PointStoreTransaction,
        slave::{
            connection::connection_registry::{PeerLinkState, SelectCommandKind},
            data::point_runtime::{
                log_allowed_type_mismatch, update_control_target_point,
                upsert_slave_control_point_snapshot as persist_slave_control_point_snapshot,
            },
            protocol::{
                asdu_dispatch::{RequestContext, ResponseSender},
                control_target::{ControlTargetValidationError, validate_control_target},
                handlers::PointMutationResult,
                quality::is_blocked_quality,
            },
            service::{SlaveCommandMismatchPolicy, build_setpoint_command_payload},
            simulation::engine::{PointKey, point_key},
        },
        types::{AsduInfo, ControlStatusKind},
    },
    errors::AppResult,
    utils::logger::iec104_log_kv,
};

pub(crate) async fn handle(
    ctx: &RequestContext,
    response_sender: &ResponseSender<'_>,
    event: SlaveCommandEvent,
    target_common_address: u16,
    requires_select: bool,
    select_timeout_seconds: u64,
    peer_link_state: &mut PeerLinkState,
    mutation: &mut PointMutationResult,
) -> AppResult<bool> {
    let station_id = ctx.station_id.as_str();
    let peer_addr = ctx.peer_addr.as_str();
    let data_points = &ctx.data_points;
    let command_mismatch_policy = &ctx.command_mismatch_policy;
    let send_asdu = |info: AsduInfo| response_sender.provided_single(info);
    let (command_select_execute, command_cp56) = match &event {
        SlaveCommandEvent::SingleControl { select, timestamp, .. }
        | SlaveCommandEvent::DoubleControl { select, timestamp, .. }
        | SlaveCommandEvent::RegulatingStep { select, timestamp, .. } => {
            (Some(*select), timestamp.as_ref().map(format_control_cp56))
        }
        SlaveCommandEvent::SetPoint { qualifier, timestamp, .. } => {
            (Some(qualifier.select), timestamp.as_ref().map(format_control_cp56))
        }
        SlaveCommandEvent::BitStringCommand { timestamp, .. } => (None, timestamp.as_ref().map(format_control_cp56)),
        _ => (None, None),
    };
    if matches!(
        &event,
        SlaveCommandEvent::SingleControl { .. }
            | SlaveCommandEvent::DoubleControl { .. }
            | SlaveCommandEvent::RegulatingStep { .. }
            | SlaveCommandEvent::SetPoint { .. }
            | SlaveCommandEvent::BitStringCommand { .. }
    ) {
        mutation.points_changed = true;
    }
    let upsert_slave_control_point_snapshot =
        |map: &mut PointStoreTransaction<'_, PointKey, crate::core::types::DataPoint>,
         connection_id: &str,
         common_address: u16,
         address: u32,
         type_id: u8,
         state: ControlStatusKind,
         cause: u16,
         command_value: Option<f64>| {
            persist_slave_control_point_snapshot(
                map,
                connection_id,
                common_address,
                address,
                type_id,
                state,
                cause,
                command_value,
                command_select_execute,
                command_cp56.as_deref(),
            );
        };

    match event {
        SlaveCommandEvent::SingleControl { type_id, ioa, value, select, action, qualifier, timestamp } => {
            let qualifier_raw = qualifier.as_u8();
            let command_kind = SelectCommandKind::Single { value, qu: (qualifier_raw >> 2) & 0x1F, timestamp };
            if select && !requires_select {
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            let policy = *command_mismatch_policy.read().await;
            let validation = {
                let points = data_points.read().await;
                validate_control_target(&points, target_common_address, ioa, type_id, policy)
            };
            let (target, mismatch_allowed) = match validation {
                Ok(result) => result,
                Err(err) => {
                    match &err {
                        ControlTargetValidationError::NoMapping => {
                            warn!(
                                "[slave] command_rejected_no_mapping {} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("NO_CONTROL_MAPPING"),
                                    None,
                                ),
                                target_common_address
                            );
                        }
                        ControlTargetValidationError::TypeMismatch { target_type_id } => {
                            warn!(
                                "[slave] command_rejected_type_mismatch {} target_type_id={} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("COMMAND_TYPE_MISMATCH"),
                                    None,
                                ),
                                target_type_id,
                                target_common_address
                            );
                        }
                    }
                    let reject_cause = if matches!(err, ControlTargetValidationError::NoMapping) {
                        47
                    } else if matches!(action, CommandCotAction::Deactivation)
                        && peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind)
                    {
                        9
                    } else if matches!(action, CommandCotAction::Deactivation) {
                        9 | 0x40
                    } else {
                        7 | 0x40
                    };
                    upsert_slave_control_point_snapshot(
                        &mut data_points.transaction().await,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        reject_cause,
                        None,
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: reject_cause,
                        common_address: target_common_address,
                        data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
            };

            if mismatch_allowed && matches!(policy, SlaveCommandMismatchPolicy::Debug) {
                log_allowed_type_mismatch(station_id, peer_addr, target_common_address, type_id, target.type_id, ioa);
            }
            if !matches!(action, CommandCotAction::Deactivation)
                && target.mapped_monitor_target.is_some_and(|monitor| is_blocked_quality(monitor.quality))
            {
                warn!(
                    "[slave] command_rejected_blocked {} ca={}",
                    iec104_log_kv(
                        station_id,
                        Some(peer_addr),
                        Some(type_id),
                        Some(7),
                        Some(ioa),
                        Some("POINT_BLOCKED"),
                        None,
                    ),
                    target_common_address
                );
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if matches!(action, CommandCotAction::Deactivation) {
                let deact_ok = peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind);
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    if deact_ok { ControlStatusKind::Idle } else { ControlStatusKind::Ng },
                    if deact_ok { 9 } else { 9 | 0x40 },
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: if deact_ok { 9 } else { 9 | 0x40 },
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if select && requires_select {
                peer_link_state.register_select_with_timeout(
                    target_common_address,
                    type_id,
                    ioa,
                    command_kind,
                    select_timeout_seconds,
                );
            } else if !select
                && requires_select
                && !peer_link_state.consume_select(target_common_address, type_id, ioa, command_kind)
            {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            if !select {
                let mut points = data_points.transaction().await;
                let cmd_value = if value { 1.0 } else { 0.0 };
                if !update_control_target_point(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    target,
                    cmd_value,
                    0,
                    chrono::Utc::now(),
                ) {
                    warn!(
                        "[slave] command_rejected_target_missing {} target_ioa={} ca={}",
                        iec104_log_kv(
                            station_id,
                            Some(peer_addr),
                            Some(type_id),
                            Some(7),
                            Some(ioa),
                            Some("MAPPED_TARGET_MISSING"),
                            None,
                        ),
                        target.address,
                        target_common_address
                    );
                    upsert_slave_control_point_snapshot(
                        &mut points,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        7 | 0x40,
                        Some(cmd_value),
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: 7 | 0x40,
                        common_address: target_common_address,
                        data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
                if let Some(mapped_target) = target.mapped_monitor_target {
                    mutation.points_changed = true;
                    if let Some(snapshot) =
                        points.get(&point_key(target_common_address, mapped_target.address)).cloned()
                    {
                        mutation.persisted_points.push(snapshot.clone());
                        mutation.spontaneous_updates.push(snapshot);
                    }
                }
                upsert_slave_control_point_snapshot(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    7,
                    Some(cmd_value),
                );
            } else {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Selected,
                    7,
                    Some(if value { 1.0 } else { 0.0 }),
                );
            }
            send_asdu(AsduInfo {
                type_id,
                cause: 7,
                common_address: target_common_address,
                data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                timestamp: chrono::Utc::now(),
            })
            .await?;
            if !select {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    10,
                    Some(if value { 1.0 } else { 0.0 }),
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 10,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
            }
        }
        SlaveCommandEvent::DoubleControl { type_id, ioa, value, select, action, qualifier, timestamp } => {
            let qualifier_raw = qualifier.as_u8();
            let command_kind = SelectCommandKind::Double { value, qu: (qualifier_raw >> 2) & 0x1F, timestamp };
            if select && !requires_select {
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            let policy = *command_mismatch_policy.read().await;
            let validation = {
                let points = data_points.read().await;
                validate_control_target(&points, target_common_address, ioa, type_id, policy)
            };
            let (target, mismatch_allowed) = match validation {
                Ok(result) => result,
                Err(err) => {
                    match &err {
                        ControlTargetValidationError::NoMapping => {
                            warn!(
                                "[slave] command_rejected_no_mapping {} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("NO_CONTROL_MAPPING"),
                                    None,
                                ),
                                target_common_address
                            );
                        }
                        ControlTargetValidationError::TypeMismatch { target_type_id } => {
                            warn!(
                                "[slave] command_rejected_type_mismatch {} target_type_id={} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("COMMAND_TYPE_MISMATCH"),
                                    None,
                                ),
                                target_type_id,
                                target_common_address
                            );
                        }
                    }
                    let reject_cause = if matches!(err, ControlTargetValidationError::NoMapping) {
                        47
                    } else if matches!(action, CommandCotAction::Deactivation)
                        && peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind)
                    {
                        9
                    } else if matches!(action, CommandCotAction::Deactivation) {
                        9 | 0x40
                    } else {
                        7 | 0x40
                    };
                    upsert_slave_control_point_snapshot(
                        &mut data_points.transaction().await,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        reject_cause,
                        None,
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: reject_cause,
                        common_address: target_common_address,
                        data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
            };

            if mismatch_allowed && matches!(policy, SlaveCommandMismatchPolicy::Debug) {
                log_allowed_type_mismatch(station_id, peer_addr, target_common_address, type_id, target.type_id, ioa);
            }
            if !matches!(action, CommandCotAction::Deactivation)
                && target.mapped_monitor_target.is_some_and(|monitor| is_blocked_quality(monitor.quality))
            {
                warn!(
                    "[slave] command_rejected_blocked {} ca={}",
                    iec104_log_kv(
                        station_id,
                        Some(peer_addr),
                        Some(type_id),
                        Some(7),
                        Some(ioa),
                        Some("POINT_BLOCKED"),
                        None,
                    ),
                    target_common_address
                );
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if matches!(action, CommandCotAction::Deactivation) {
                let deact_ok = peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind);
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    if deact_ok { ControlStatusKind::Idle } else { ControlStatusKind::Ng },
                    if deact_ok { 9 } else { 9 | 0x40 },
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: if deact_ok { 9 } else { 9 | 0x40 },
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if select && requires_select {
                peer_link_state.register_select_with_timeout(
                    target_common_address,
                    type_id,
                    ioa,
                    command_kind,
                    select_timeout_seconds,
                );
            } else if !select
                && requires_select
                && !peer_link_state.consume_select(target_common_address, type_id, ioa, command_kind)
            {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            if !select {
                let mut points = data_points.transaction().await;
                let cmd_value = value as f64;
                if !update_control_target_point(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    target,
                    cmd_value,
                    0,
                    chrono::Utc::now(),
                ) {
                    warn!(
                        "[slave] command_rejected_target_missing {} target_ioa={} ca={}",
                        iec104_log_kv(
                            station_id,
                            Some(peer_addr),
                            Some(type_id),
                            Some(7),
                            Some(ioa),
                            Some("MAPPED_TARGET_MISSING"),
                            None,
                        ),
                        target.address,
                        target_common_address
                    );
                    upsert_slave_control_point_snapshot(
                        &mut points,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        7 | 0x40,
                        Some(cmd_value),
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: 7 | 0x40,
                        common_address: target_common_address,
                        data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
                if let Some(mapped_target) = target.mapped_monitor_target {
                    mutation.points_changed = true;
                    if let Some(snapshot) =
                        points.get(&point_key(target_common_address, mapped_target.address)).cloned()
                    {
                        mutation.persisted_points.push(snapshot.clone());
                        mutation.spontaneous_updates.push(snapshot);
                    }
                }
                upsert_slave_control_point_snapshot(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    7,
                    Some(cmd_value),
                );
            } else {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Selected,
                    7,
                    Some(value as f64),
                );
            }
            send_asdu(AsduInfo {
                type_id,
                cause: 7,
                common_address: target_common_address,
                data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                timestamp: chrono::Utc::now(),
            })
            .await?;
            if !select {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    10,
                    Some(value as f64),
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 10,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
            }
        }
        SlaveCommandEvent::RegulatingStep { type_id, ioa, step, select, action, qualifier, timestamp } => {
            let qualifier_raw = qualifier.as_u8();
            let command_kind = SelectCommandKind::RegulatingStep { step, qu: (qualifier_raw >> 2) & 0x1F, timestamp };
            if select && !requires_select {
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            let policy = *command_mismatch_policy.read().await;
            let validation = {
                let points = data_points.read().await;
                validate_control_target(&points, target_common_address, ioa, type_id, policy)
            };
            let (target, mismatch_allowed) = match validation {
                Ok(result) => result,
                Err(err) => {
                    match &err {
                        ControlTargetValidationError::NoMapping => {
                            warn!(
                                "[slave] command_rejected_no_mapping {} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("NO_CONTROL_MAPPING"),
                                    None,
                                ),
                                target_common_address
                            );
                        }
                        ControlTargetValidationError::TypeMismatch { target_type_id } => {
                            warn!(
                                "[slave] command_rejected_type_mismatch {} target_type_id={} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("COMMAND_TYPE_MISMATCH"),
                                    None,
                                ),
                                target_type_id,
                                target_common_address
                            );
                        }
                    }
                    let reject_cause = if matches!(err, ControlTargetValidationError::NoMapping) {
                        47
                    } else if matches!(action, CommandCotAction::Deactivation)
                        && peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind)
                    {
                        9
                    } else if matches!(action, CommandCotAction::Deactivation) {
                        9 | 0x40
                    } else {
                        7 | 0x40
                    };
                    upsert_slave_control_point_snapshot(
                        &mut data_points.transaction().await,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        reject_cause,
                        None,
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: reject_cause,
                        common_address: target_common_address,
                        data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
            };

            if mismatch_allowed && matches!(policy, SlaveCommandMismatchPolicy::Debug) {
                log_allowed_type_mismatch(station_id, peer_addr, target_common_address, type_id, target.type_id, ioa);
            }
            if !matches!(action, CommandCotAction::Deactivation)
                && target.mapped_monitor_target.is_some_and(|monitor| is_blocked_quality(monitor.quality))
            {
                warn!(
                    "[slave] command_rejected_blocked {} ca={}",
                    iec104_log_kv(
                        station_id,
                        Some(peer_addr),
                        Some(type_id),
                        Some(7),
                        Some(ioa),
                        Some("POINT_BLOCKED"),
                        None,
                    ),
                    target_common_address
                );
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if matches!(action, CommandCotAction::Deactivation) {
                let deact_ok = peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind);
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    if deact_ok { ControlStatusKind::Idle } else { ControlStatusKind::Ng },
                    if deact_ok { 9 } else { 9 | 0x40 },
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: if deact_ok { 9 } else { 9 | 0x40 },
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if select && requires_select {
                peer_link_state.register_select_with_timeout(
                    target_common_address,
                    type_id,
                    ioa,
                    command_kind,
                    select_timeout_seconds,
                );
            } else if !select
                && requires_select
                && !peer_link_state.consume_select(target_common_address, type_id, ioa, command_kind)
            {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            if !select {
                let mut points = data_points.transaction().await;
                // RCS: 1=lower(-1), 2=higher(+1)
                let step_delta: f64 = match step & 0x03 {
                    1 => -1.0,
                    2 => 1.0,
                    _ => 0.0,
                };
                let target_key = point_key(target_common_address, target.address);
                let current_value = points.get(&target_key).map(|point| point.value).unwrap_or(target.current_value);
                let new_value = (current_value + step_delta).clamp(-64.0, 63.0);
                if !update_control_target_point(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    target,
                    new_value,
                    0,
                    chrono::Utc::now(),
                ) {
                    warn!(
                        "[slave] command_rejected_target_missing {} target_ioa={} ca={}",
                        iec104_log_kv(
                            station_id,
                            Some(peer_addr),
                            Some(type_id),
                            Some(7),
                            Some(ioa),
                            Some("MAPPED_TARGET_MISSING"),
                            None,
                        ),
                        target.address,
                        target_common_address
                    );
                    upsert_slave_control_point_snapshot(
                        &mut points,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        7 | 0x40,
                        Some(new_value),
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: 7 | 0x40,
                        common_address: target_common_address,
                        data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
                if let Some(mapped_target) = target.mapped_monitor_target {
                    mutation.points_changed = true;
                    if let Some(snapshot) =
                        points.get(&point_key(target_common_address, mapped_target.address)).cloned()
                    {
                        mutation.persisted_points.push(snapshot.clone());
                        mutation.spontaneous_updates.push(snapshot);
                    }
                }
                upsert_slave_control_point_snapshot(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    7,
                    Some(new_value),
                );
            } else {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Selected,
                    7,
                    Some(step as f64),
                );
            }
            send_asdu(AsduInfo {
                type_id,
                cause: 7,
                common_address: target_common_address,
                data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                timestamp: chrono::Utc::now(),
            })
            .await?;
            if !select {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    10,
                    Some(step as f64),
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 10,
                    common_address: target_common_address,
                    data: build_control_echo_payload(type_id, ioa, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
            }
        }
        SlaveCommandEvent::SetPoint { type_id, ioa, value, action, qualifier, timestamp } => {
            let qualifier_raw = qualifier.as_u8();
            let response_payload =
                build_setpoint_control_echo_payload(type_id, ioa, value, qualifier_raw, timestamp.as_ref());
            let command_kind = SelectCommandKind::SetPoint { type_id, value, ql: qualifier.qualifier, timestamp };
            if qualifier.select && !requires_select {
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: response_payload.clone(),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            let policy = *command_mismatch_policy.read().await;
            let validation = {
                let points = data_points.read().await;
                validate_control_target(&points, target_common_address, ioa, type_id, policy)
            };
            let (target, mismatch_allowed) = match validation {
                Ok(result) => result,
                Err(err) => {
                    match &err {
                        ControlTargetValidationError::NoMapping => {
                            warn!(
                                "[slave] command_rejected_no_mapping {} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("NO_CONTROL_MAPPING"),
                                    None,
                                ),
                                target_common_address
                            );
                        }
                        ControlTargetValidationError::TypeMismatch { target_type_id } => {
                            warn!(
                                "[slave] command_rejected_type_mismatch {} target_type_id={} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("COMMAND_TYPE_MISMATCH"),
                                    None,
                                ),
                                target_type_id,
                                target_common_address
                            );
                        }
                    }
                    let reject_cause = if matches!(err, ControlTargetValidationError::NoMapping) {
                        47
                    } else if matches!(action, CommandCotAction::Deactivation)
                        && peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind)
                    {
                        9
                    } else if matches!(action, CommandCotAction::Deactivation) {
                        9 | 0x40
                    } else {
                        7 | 0x40
                    };
                    upsert_slave_control_point_snapshot(
                        &mut data_points.transaction().await,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        reject_cause,
                        None,
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: reject_cause,
                        common_address: target_common_address,
                        data: response_payload.clone(),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
            };

            if mismatch_allowed && matches!(policy, SlaveCommandMismatchPolicy::Debug) {
                log_allowed_type_mismatch(station_id, peer_addr, target_common_address, type_id, target.type_id, ioa);
            }
            if !matches!(action, CommandCotAction::Deactivation)
                && target.mapped_monitor_target.is_some_and(|monitor| is_blocked_quality(monitor.quality))
            {
                warn!(
                    "[slave] command_rejected_blocked {} ca={}",
                    iec104_log_kv(
                        station_id,
                        Some(peer_addr),
                        Some(type_id),
                        Some(7),
                        Some(ioa),
                        Some("POINT_BLOCKED"),
                        None,
                    ),
                    target_common_address
                );
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: response_payload.clone(),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if matches!(action, CommandCotAction::Deactivation) {
                let deact_ok = peer_link_state.cancel_select(target_common_address, type_id, ioa, command_kind);
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    if deact_ok { ControlStatusKind::Idle } else { ControlStatusKind::Ng },
                    if deact_ok { 9 } else { 9 | 0x40 },
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: if deact_ok { 9 } else { 9 | 0x40 },
                    common_address: target_common_address,
                    data: response_payload,
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            if qualifier.select && requires_select {
                peer_link_state.register_select_with_timeout(
                    target_common_address,
                    type_id,
                    ioa,
                    command_kind,
                    select_timeout_seconds,
                );
            } else if !qualifier.select
                && requires_select
                && !peer_link_state.consume_select(target_common_address, type_id, ioa, command_kind)
            {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: response_payload,
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            if !qualifier.select {
                let mut points = data_points.transaction().await;
                let cmd_value = value.as_f64();
                if !update_control_target_point(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    target,
                    cmd_value,
                    0,
                    chrono::Utc::now(),
                ) {
                    warn!(
                        "[slave] command_rejected_target_missing {} target_ioa={} ca={}",
                        iec104_log_kv(
                            station_id,
                            Some(peer_addr),
                            Some(type_id),
                            Some(7),
                            Some(ioa),
                            Some("MAPPED_TARGET_MISSING"),
                            None,
                        ),
                        target.address,
                        target_common_address
                    );
                    upsert_slave_control_point_snapshot(
                        &mut points,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        7 | 0x40,
                        Some(cmd_value),
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: 7 | 0x40,
                        common_address: target_common_address,
                        data: response_payload.clone(),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
                if let Some(mapped_target) = target.mapped_monitor_target {
                    mutation.points_changed = true;
                    if let Some(snapshot) =
                        points.get(&point_key(target_common_address, mapped_target.address)).cloned()
                    {
                        mutation.persisted_points.push(snapshot.clone());
                        mutation.spontaneous_updates.push(snapshot);
                    }
                }
                upsert_slave_control_point_snapshot(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    7,
                    Some(cmd_value),
                );
            } else {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Selected,
                    7,
                    Some(value.as_f64()),
                );
            }
            send_asdu(AsduInfo {
                type_id,
                cause: 7,
                common_address: target_common_address,
                data: response_payload,
                timestamp: chrono::Utc::now(),
            })
            .await?;
            if !qualifier.select {
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ok,
                    10,
                    Some(value.as_f64()),
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 10,
                    common_address: target_common_address,
                    data: build_setpoint_control_echo_payload(type_id, ioa, value, qualifier_raw, timestamp.as_ref()),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
            }
        }
        SlaveCommandEvent::BitStringCommand { type_id, ioa, value, timestamp } => {
            let payload = append_control_timestamp(
                type_id,
                iec60870_parser::parser::asdu::encode::encode_bitstring_command(ioa, value),
                timestamp.as_ref(),
            );
            let policy = *command_mismatch_policy.read().await;
            let validation = {
                let points = data_points.read().await;
                validate_control_target(&points, target_common_address, ioa, type_id, policy)
            };
            let (target, mismatch_allowed) = match validation {
                Ok(result) => result,
                Err(err) => {
                    let reject_cause = match &err {
                        ControlTargetValidationError::NoMapping => {
                            warn!(
                                "[slave] command_rejected_no_mapping {} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("NO_CONTROL_MAPPING"),
                                    None,
                                ),
                                target_common_address
                            );
                            47
                        }
                        ControlTargetValidationError::TypeMismatch { target_type_id } => {
                            warn!(
                                "[slave] command_rejected_type_mismatch {} target_type_id={} ca={}",
                                iec104_log_kv(
                                    station_id,
                                    Some(peer_addr),
                                    Some(type_id),
                                    Some(7),
                                    Some(ioa),
                                    Some("COMMAND_TYPE_MISMATCH"),
                                    None,
                                ),
                                target_type_id,
                                target_common_address
                            );
                            7 | 0x40
                        }
                    };
                    upsert_slave_control_point_snapshot(
                        &mut data_points.transaction().await,
                        peer_addr,
                        target_common_address,
                        ioa,
                        type_id,
                        ControlStatusKind::Ng,
                        reject_cause,
                        None,
                    );
                    send_asdu(AsduInfo {
                        type_id,
                        cause: reject_cause,
                        common_address: target_common_address,
                        data: payload.clone(),
                        timestamp: chrono::Utc::now(),
                    })
                    .await?;
                    return Ok(true);
                }
            };

            if mismatch_allowed && matches!(policy, SlaveCommandMismatchPolicy::Debug) {
                log_allowed_type_mismatch(station_id, peer_addr, target_common_address, type_id, target.type_id, ioa);
            }
            if target.mapped_monitor_target.is_some_and(|monitor| is_blocked_quality(monitor.quality)) {
                warn!(
                    "[slave] command_rejected_blocked {} ca={}",
                    iec104_log_kv(
                        station_id,
                        Some(peer_addr),
                        Some(type_id),
                        Some(7),
                        Some(ioa),
                        Some("POINT_BLOCKED"),
                        None,
                    ),
                    target_common_address
                );
                upsert_slave_control_point_snapshot(
                    &mut data_points.transaction().await,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: payload.clone(),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }

            let mut points = data_points.transaction().await;
            let cmd_value = value as f64;
            if !update_control_target_point(
                &mut points,
                peer_addr,
                target_common_address,
                target,
                cmd_value,
                0,
                chrono::Utc::now(),
            ) {
                warn!(
                    "[slave] command_rejected_target_missing {} target_ioa={} ca={}",
                    iec104_log_kv(
                        station_id,
                        Some(peer_addr),
                        Some(type_id),
                        Some(7),
                        Some(ioa),
                        Some("MAPPED_TARGET_MISSING"),
                        None,
                    ),
                    target.address,
                    target_common_address
                );
                upsert_slave_control_point_snapshot(
                    &mut points,
                    peer_addr,
                    target_common_address,
                    ioa,
                    type_id,
                    ControlStatusKind::Ng,
                    7 | 0x40,
                    None,
                );
                send_asdu(AsduInfo {
                    type_id,
                    cause: 7 | 0x40,
                    common_address: target_common_address,
                    data: payload.clone(),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
                return Ok(true);
            }
            if let Some(mapped_target) = target.mapped_monitor_target {
                mutation.points_changed = true;
                if let Some(snapshot) = points.get(&point_key(target_common_address, mapped_target.address)).cloned() {
                    mutation.persisted_points.push(snapshot.clone());
                    mutation.spontaneous_updates.push(snapshot);
                }
            }
            // Send activation confirmation
            send_asdu(AsduInfo {
                type_id,
                cause: 7, // activation confirmation
                common_address: target_common_address,
                data: payload.clone(),
                timestamp: chrono::Utc::now(),
            })
            .await?;
            upsert_slave_control_point_snapshot(
                &mut points,
                peer_addr,
                target_common_address,
                ioa,
                type_id,
                ControlStatusKind::Executing,
                7,
                None,
            );
            send_asdu(AsduInfo {
                type_id,
                cause: 10,
                common_address: target_common_address,
                data: payload,
                timestamp: chrono::Utc::now(),
            })
            .await?;
            upsert_slave_control_point_snapshot(
                &mut points,
                peer_addr,
                target_common_address,
                ioa,
                type_id,
                ControlStatusKind::Ok,
                10,
                None,
            );
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn append_control_timestamp(type_id: u8, payload: Vec<u8>, timestamp: Option<&[u8; 7]>) -> Vec<u8> {
    if matches!(type_id, 58..=64) {
        iec60870_parser::parser::asdu::encode::append_cp56_time2a(
            payload,
            *timestamp.expect("timed control event must retain its validated CP56Time2a"),
        )
    } else {
        assert!(timestamp.is_none(), "untimed control event must not carry CP56Time2a");
        payload
    }
}

fn format_control_cp56(raw: &[u8; 7]) -> String {
    let timestamp = CP56Time2a::parse(raw).expect("control event must retain its validated CP56Time2a");
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        timestamp.year,
        timestamp.month,
        timestamp.day,
        timestamp.hours,
        timestamp.minutes,
        timestamp.seconds(),
        timestamp.millis(),
    )
}

fn build_control_echo_payload(type_id: u8, ioa: u32, qualifier: u8, timestamp: Option<&[u8; 7]>) -> Vec<u8> {
    append_control_timestamp(type_id, build_ioa_qualifier_payload(ioa, qualifier), timestamp)
}

fn build_setpoint_control_echo_payload(
    type_id: u8,
    ioa: u32,
    value: SetPointCommandValue,
    qualifier: u8,
    timestamp: Option<&[u8; 7]>,
) -> Vec<u8> {
    append_control_timestamp(type_id, build_setpoint_command_payload(type_id, ioa, value, qualifier), timestamp)
}
