use std::sync::Arc;

use crate::core::{
    protocol_adapter::SlaveCommandEvent,
    slave::protocol::{asdu_dispatch::RequestContext, interrogation::spawn_general_interrogation_response_task},
};

pub(crate) async fn handle(
    ctx: &RequestContext,
    event: &SlaveCommandEvent,
    request_cause: u16,
    target_common_address: u16,
    enable_sq1_upload: bool,
    max_asdu_bytes: u16,
) -> bool {
    let SlaveCommandEvent::GeneralInterrogation { ioa, qualifier } = event else {
        return false;
    };

    spawn_general_interrogation_response_task(
        &ctx.station_id,
        &ctx.peer_addr,
        request_cause,
        target_common_address,
        *ioa,
        qualifier.as_u8(),
        enable_sq1_upload,
        max_asdu_bytes,
        *ctx.station_time_offset_ms.read().await,
        Arc::clone(&ctx.server),
        Arc::clone(&ctx.protocol),
        Arc::clone(&ctx.connections),
        Arc::clone(&ctx.peer_local_addrs),
        Arc::clone(&ctx.message_records),
        Arc::clone(&ctx.next_message_id),
        Arc::clone(&ctx.capture_packets),
        Arc::clone(&ctx.data_points),
    );
    true
}
