use std::sync::Arc;

use iec60870_parser::parser::asdu::qualifiers::FreezeQualifier;

use crate::core::{
    protocol_adapter::SlaveCommandEvent,
    slave::protocol::{
        asdu_dispatch::RequestContext, handlers::PointMutationResult,
        interrogation::spawn_counter_interrogation_response_task, point_type::is_integrated_total_type,
        quality::apply_quality_metadata,
    },
    types::DataPoint,
};

pub(crate) async fn handle(
    ctx: &RequestContext,
    event: &SlaveCommandEvent,
    request_cause: u16,
    target_common_address: u16,
    enable_sq1_upload: bool,
    max_asdu_bytes: u16,
) -> Option<PointMutationResult> {
    let SlaveCommandEvent::CounterInterrogation { ioa, qualifier } = event else {
        return None;
    };

    let counter_group = qualifier.group_number();
    let group_matches = |point: &DataPoint| qualifier.is_general() || point.counter_group == counter_group;
    let mut changed_points = Vec::new();
    let mut snapshot = match qualifier.freeze {
        FreezeQualifier::Read => ctx
            .data_points
            .read()
            .await
            .iter()
            .filter_map(|((common_address, _), point)| {
                (*common_address == target_common_address
                    && is_integrated_total_type(point.type_id)
                    && group_matches(point))
                .then(|| point.clone())
            })
            .collect::<Vec<_>>(),
        FreezeQualifier::Freeze => {
            let mut points = ctx.data_points.transaction().await;
            let now = chrono::Utc::now();
            let mut frozen = Vec::new();
            points.for_each_mut(|(common_address, _), point| {
                if *common_address != target_common_address
                    || !is_integrated_total_type(point.type_id)
                    || !group_matches(point)
                {
                    return false;
                }
                let next_sq = point.quality.wrapping_add(1) & 0x1F;
                apply_quality_metadata(point, (point.quality & 0xE0) | next_sq);
                point.timestamp = now;
                frozen.push(point.clone());
                changed_points.push(point.clone());
                true
            });
            frozen
        }
        FreezeQualifier::FreezeAndReset => {
            let mut points = ctx.data_points.transaction().await;
            let now = chrono::Utc::now();
            let mut frozen = Vec::new();
            points.for_each_mut(|(common_address, _), point| {
                if *common_address != target_common_address
                    || !is_integrated_total_type(point.type_id)
                    || !group_matches(point)
                {
                    return false;
                }
                let next_sq = point.quality.wrapping_add(1) & 0x1F;
                let mut frozen_point = point.clone();
                frozen_point.timestamp = now;
                apply_quality_metadata(&mut frozen_point, (point.quality & 0xE0) | next_sq);
                frozen.push(frozen_point);

                point.value = 0.0;
                point.timestamp = now;
                apply_quality_metadata(point, (point.quality & 0x80) | 0x40 | next_sq);
                changed_points.push(point.clone());
                true
            });
            frozen
        }
        FreezeQualifier::Reset => {
            let mut points = ctx.data_points.transaction().await;
            let now = chrono::Utc::now();
            let mut reset = Vec::new();
            points.for_each_mut(|(common_address, _), point| {
                if *common_address != target_common_address
                    || !is_integrated_total_type(point.type_id)
                    || !group_matches(point)
                {
                    return false;
                }
                point.value = 0.0;
                point.timestamp = now;
                let next_sq = point.quality.wrapping_add(1) & 0x1F;
                apply_quality_metadata(point, (point.quality & 0x80) | 0x40 | next_sq);
                reset.push(point.clone());
                changed_points.push(point.clone());
                true
            });
            reset
        }
    };

    snapshot.sort_by_key(|point| point.address);
    spawn_counter_interrogation_response_task(
        &ctx.station_id,
        &ctx.peer_addr,
        request_cause,
        target_common_address,
        *ioa,
        qualifier.as_u8(),
        enable_sq1_upload,
        max_asdu_bytes,
        snapshot,
        *ctx.station_time_offset_ms.read().await,
        Arc::clone(&ctx.server),
        Arc::clone(&ctx.protocol),
        Arc::clone(&ctx.connections),
        Arc::clone(&ctx.peer_local_addrs),
        Arc::clone(&ctx.message_records),
        Arc::clone(&ctx.next_message_id),
        Arc::clone(&ctx.capture_packets),
    );
    Some(PointMutationResult {
        points_changed: !changed_points.is_empty(),
        spontaneous_updates: Vec::new(),
        persisted_points: changed_points,
    })
}
