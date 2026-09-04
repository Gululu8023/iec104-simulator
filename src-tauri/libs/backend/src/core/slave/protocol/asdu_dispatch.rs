//! 从站 ASDU 路由与命令结果提交。

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, atomic::AtomicU64},
    time::Instant,
};

use log::{debug, warn};
use tokio::sync::{Mutex, RwLock};

use crate::{
    core::{
        slave::{
            connection::connection_registry::PeerLinkState,
            data::{point_runtime::persist_runtime_point_values, point_store::SlavePointStore},
            protocol::response_cause::ResponseEnvelope,
            reporting::{
                broadcast_runtime::{
                    emit_command_spontaneous_updates as run_emit_command_spontaneous_updates,
                    resolve_station_policy_for_peer as run_resolve_station_policy_for_peer,
                },
                soe_support::SoeEvent,
            },
            service::{
                SlaveCommandMismatchPolicy, decode_slave_command_events, enqueue_response_from_ctx,
                first_ioa_from_decoded_asdu,
            },
        },
        types::{
            AsduInfo, BackendSoeEvent, ConnectionInfo, ControlExecutionMode, LinkParams, SlaveMessageRecord,
            SlaveStationPolicy, SlaveStationPolicyOverride,
        },
    },
    db::DatabaseService,
    errors::AppResult,
    network::{DecodedAsdu, Iec104Protocol, TcpServer},
    utils::{logger::iec104_log_kv, pcap::CapturePacket},
};

/// Per-connection resources required while dispatching one decoded ASDU.
#[derive(Clone)]
pub(crate) struct RequestContext {
    pub(crate) station_id: String,
    pub(crate) peer_addr: String,
    pub(crate) slave_common_address: u16,
    pub(crate) server: Arc<TcpServer>,
    pub(crate) protocol: Arc<Mutex<Iec104Protocol>>,
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    pub(crate) peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    pub(crate) message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    pub(crate) next_message_id: Arc<AtomicU64>,
    pub(crate) capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    pub(crate) data_points: Arc<SlavePointStore>,
    pub(crate) link_params: Arc<RwLock<LinkParams>>,
    pub(crate) station_policy_defaults: Arc<RwLock<SlaveStationPolicy>>,
    pub(crate) station_policy_overrides: Arc<RwLock<HashMap<u16, SlaveStationPolicyOverride>>>,
    pub(crate) station_policy_connection_override_enabled: Arc<RwLock<bool>>,
    pub(crate) command_mismatch_policy: Arc<RwLock<SlaveCommandMismatchPolicy>>,
    pub(crate) db_service: Arc<DatabaseService>,
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    pub(crate) soe_events: Arc<RwLock<VecDeque<SoeEvent>>>,
    pub(crate) soe_records: Arc<RwLock<VecDeque<BackendSoeEvent>>>,
    pub(crate) next_soe_id: Arc<AtomicU64>,
    pub(crate) station_time_offset_ms: Arc<RwLock<i64>>,
}

/// Response construction is bound to the originating request so handlers cannot
/// accidentally lose the request COT flags or VSQ shape.
pub(crate) struct ResponseSender<'a> {
    ctx: &'a RequestContext,
    request_cause: u16,
    request_vsq: u8,
}

impl<'a> ResponseSender<'a> {
    fn new(ctx: &'a RequestContext, request: &DecodedAsdu) -> Self {
        Self { ctx, request_cause: request.cause, request_vsq: request.vsq }
    }

    pub(crate) async fn provided_single(&self, info: AsduInfo) -> AppResult<()> {
        enqueue_response_from_ctx(self.ctx, ResponseEnvelope::provided_single(self.request_cause, info)).await
    }

    async fn echoed_request(&self, info: AsduInfo) -> AppResult<()> {
        enqueue_response_from_ctx(
            self.ctx,
            ResponseEnvelope::echoed_request(self.request_cause, info, self.request_vsq),
        )
        .await
    }
}

/// 将一个已解码 ASDU 路由到对应命令族，并统一提交点值变更。
pub(crate) async fn handle_master_asdu(
    ctx: &RequestContext,
    asdu: &DecodedAsdu,
    peer_link_state: &mut PeerLinkState,
) -> AppResult<()> {
    let station_id = ctx.station_id.as_str();
    let peer_addr = ctx.peer_addr.as_str();
    let slave_common_address = ctx.slave_common_address;
    let data_points = &ctx.data_points;
    let link_params = &ctx.link_params;
    let ioa = first_ioa_from_decoded_asdu(asdu);
    let target_common_address = asdu.common_address;
    let oa = (asdu.cause >> 8) as u8;
    let pn = ((asdu.cause as u8) & 0x40) != 0;
    debug!(
        "[slave] iframe_received {} ca={} vsq={} sq={} oa={} pn={}",
        iec104_log_kv(station_id, Some(peer_addr), Some(asdu.type_id), Some(asdu.cause), ioa, None, None,),
        target_common_address,
        asdu.vsq,
        (asdu.vsq & 0x80) != 0,
        oa,
        pn
    );

    peer_link_state.cleanup_expired_selects(Instant::now());
    let link_params_snapshot = *link_params.read().await;
    let station_policy = run_resolve_station_policy_for_peer(ctx, target_common_address).await;
    let select_timeout_seconds = station_policy.select_timeout_seconds.max(1);
    let requires_select = matches!(station_policy.control_execution_mode, ControlExecutionMode::Sbo);
    let enable_sq1_upload = station_policy.enable_sq1_upload;
    let max_asdu_bytes = link_params_snapshot.max_asdu_bytes;
    let unknown_typeid_negative_ack = link_params_snapshot.unknown_typeid_negative_ack;
    let enable_soe = station_policy.enable_soe;
    let response_sender = ResponseSender::new(ctx, asdu);

    let has_target_common_address = if target_common_address == slave_common_address {
        true
    } else {
        let points = data_points.read().await;
        points.keys().any(|(common_address, _)| *common_address == target_common_address)
    };

    if !has_target_common_address {
        warn!(
            "[slave] unknown_common_address {} request_ca={} local_ca={}",
            iec104_log_kv(station_id, Some(peer_addr), Some(asdu.type_id), Some(asdu.cause), ioa, None, None,),
            target_common_address,
            slave_common_address
        );
        response_sender
            .echoed_request(AsduInfo {
                type_id: asdu.type_id,
                cause: 46, // Unknown common address
                common_address: target_common_address,
                data: asdu.raw_payload.clone(),
                timestamp: chrono::Utc::now(),
            })
            .await?;
        return Ok(());
    }

    if super::handlers::file_transfer::handle(ctx, asdu, target_common_address, max_asdu_bytes, peer_link_state).await?
    {
        return Ok(());
    }

    let Some(events) =
        decode_slave_command_events(ctx, asdu, target_common_address, ioa, oa, unknown_typeid_negative_ack).await?
    else {
        return Ok(());
    };

    let mut mutation = super::handlers::PointMutationResult::default();
    for event in events {
        if super::handlers::interrogation::handle(
            ctx,
            &event,
            asdu.cause,
            target_common_address,
            enable_sq1_upload,
            max_asdu_bytes,
        )
        .await
        {
            continue;
        }
        if let Some(counter_mutation) = super::handlers::counter::handle(
            ctx,
            &event,
            asdu.cause,
            target_common_address,
            enable_sq1_upload,
            max_asdu_bytes,
        )
        .await
        {
            mutation.points_changed |= counter_mutation.points_changed;
            mutation.persisted_points.extend(counter_mutation.persisted_points);
            continue;
        }
        if super::handlers::system::handle(ctx, &response_sender, &event, target_common_address, peer_link_state)
            .await?
            || super::handlers::read::handle(ctx, &response_sender, &event, target_common_address).await?
        {
            continue;
        }
        if super::handlers::control::handle(
            ctx,
            &response_sender,
            event,
            target_common_address,
            requires_select,
            select_timeout_seconds,
            peer_link_state,
            &mut mutation,
        )
        .await?
        {
            continue;
        }
        unreachable!("decoded command event was not routed");
    }

    if !mutation.persisted_points.is_empty() {
        persist_runtime_point_values(station_id, target_common_address, &ctx.db_service, &mutation.persisted_points)
            .await;
    }
    run_emit_command_spontaneous_updates(
        ctx,
        asdu.cause,
        target_common_address,
        mutation.spontaneous_updates,
        enable_soe,
        mutation.points_changed,
    )
    .await?;
    Ok(())
}
