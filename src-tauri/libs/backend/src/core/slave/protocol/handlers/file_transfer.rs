use crate::{
    core::{
        shared::runtime_hint::emit_slave_runtime_hint,
        slave::{
            connection::connection_registry::PeerLinkState,
            protocol::{asdu_dispatch::RequestContext, file_transfer_runtime::handle_slave_file_transfer_records},
        },
    },
    errors::AppResult,
    network::{DecodedAsdu, DecodedAsduBody},
};

pub(crate) async fn handle(
    ctx: &RequestContext,
    asdu: &DecodedAsdu,
    target_common_address: u16,
    max_asdu_bytes: u16,
    peer_link_state: &mut PeerLinkState,
) -> AppResult<bool> {
    let DecodedAsduBody::FileTransfers(records) = &asdu.body else {
        return Ok(false);
    };

    handle_slave_file_transfer_records(
        &ctx.station_id,
        &ctx.peer_addr,
        asdu.cause,
        target_common_address,
        records,
        &ctx.server,
        &ctx.protocol,
        &ctx.connections,
        &ctx.peer_local_addrs,
        &ctx.message_records,
        &ctx.next_message_id,
        &ctx.capture_packets,
        &ctx.runtime_event_handle,
        peer_link_state,
        max_asdu_bytes,
    )
    .await?;
    emit_slave_runtime_hint(&ctx.runtime_event_handle, &ctx.station_id, Some(&ctx.peer_addr), "file-transfer-updated")
        .await;
    Ok(true)
}
