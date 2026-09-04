use crate::{
    core::{
        protocol_adapter::{SlaveCommandEvent, build_clock_sync_command_payload, build_ioa_qualifier_payload},
        shared::runtime_hint::emit_slave_runtime_hint,
        slave::{
            connection::{connection_metrics::reset_connection_runtime_stats, connection_registry::PeerLinkState},
            protocol::asdu_dispatch::{RequestContext, ResponseSender},
        },
        types::AsduInfo,
    },
    errors::AppResult,
};

pub(crate) async fn handle(
    ctx: &RequestContext,
    response_sender: &ResponseSender<'_>,
    event: &SlaveCommandEvent,
    target_common_address: u16,
    peer_link_state: &mut PeerLinkState,
) -> AppResult<bool> {
    let info = match event {
        SlaveCommandEvent::ClockSynchronization { ioa, timestamp } => {
            *ctx.station_time_offset_ms.write().await =
                timestamp.signed_duration_since(chrono::Utc::now()).num_milliseconds();
            if let Some(connection) = ctx.connections.write().await.get_mut(&ctx.peer_addr) {
                connection.last_clock_sync_at = Some(*timestamp);
            }
            let payload = build_clock_sync_command_payload(*ioa, *timestamp);
            response_sender
                .provided_single(AsduInfo {
                    type_id: 103,
                    cause: 7,
                    common_address: target_common_address,
                    data: payload.clone(),
                    timestamp: chrono::Utc::now(),
                })
                .await?;
            emit_slave_runtime_hint(&ctx.runtime_event_handle, &ctx.station_id, Some(&ctx.peer_addr), "clock-synced")
                .await;
            AsduInfo {
                type_id: 103,
                cause: 10,
                common_address: target_common_address,
                data: payload,
                timestamp: chrono::Utc::now(),
            }
        }
        SlaveCommandEvent::TestCommand { type_id, ioa, pattern, timestamp } => {
            let payload = if *type_id == 107 {
                iec60870_parser::parser::asdu::encode::encode_timed_test_command(
                    *ioa,
                    *pattern,
                    *timestamp.as_ref().expect("validated Type 107 timestamp"),
                )
            } else {
                iec60870_parser::parser::asdu::encode::encode_test_command(*ioa, *pattern)
            };
            AsduInfo {
                type_id: *type_id,
                cause: 7,
                common_address: target_common_address,
                data: payload,
                timestamp: chrono::Utc::now(),
            }
        }
        SlaveCommandEvent::ResetProcess { ioa, qualifier } => {
            ctx.soe_events.write().await.retain(|event| event.common_address != target_common_address);
            ctx.soe_records.write().await.retain(|event| event.common_address != target_common_address);

            if qualifier.as_u8() == 1 {
                peer_link_state.clear_pending_selects();
                peer_link_state.file_transfer = None;
                peer_link_state.pending_ack_count = 0;
                peer_link_state.pending_ack_since = None;
                peer_link_state.pending_test = None;
                peer_link_state.unacked_since = None;
                peer_link_state.retransmit_attempts = 0;
                peer_link_state.frame_count_recovery_attempted = false;
                reset_connection_runtime_stats(&ctx.connections, &ctx.peer_addr).await;

                ctx.data_points.transaction().await.for_each_mut(|_, point| {
                    if point.connection_id != ctx.peer_addr || point.common_address != Some(target_common_address) {
                        return false;
                    }
                    if point.control_status_snapshot.is_none() {
                        return false;
                    }
                    point.control_status_snapshot = None;
                    true
                });
                emit_slave_runtime_hint(
                    &ctx.runtime_event_handle,
                    &ctx.station_id,
                    Some(&ctx.peer_addr),
                    "points-changed",
                )
                .await;
            }
            emit_slave_runtime_hint(&ctx.runtime_event_handle, &ctx.station_id, Some(&ctx.peer_addr), "soe-cleared")
                .await;
            AsduInfo {
                type_id: 105,
                cause: 7,
                common_address: target_common_address,
                data: build_ioa_qualifier_payload(*ioa, qualifier.as_u8()),
                timestamp: chrono::Utc::now(),
            }
        }
        _ => return Ok(false),
    };

    response_sender.provided_single(info).await?;
    Ok(true)
}
