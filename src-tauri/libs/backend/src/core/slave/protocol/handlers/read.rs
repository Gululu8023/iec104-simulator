use crate::{
    core::{
        iec104_registry::{PointRole, lookup_capability},
        protocol_adapter::SlaveCommandEvent,
        slave::{
            protocol::asdu_dispatch::{RequestContext, ResponseSender},
            reporting::soe_support::build_point_measurement_payload_with_offset,
            simulation::engine::point_key,
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
) -> AppResult<bool> {
    let SlaveCommandEvent::ReadCommand { ioa } = event else {
        return Ok(false);
    };

    let time_offset_ms = *ctx.station_time_offset_ms.read().await;
    let points = ctx.data_points.read().await;
    let response = points.get(&point_key(target_common_address, *ioa)).and_then(|point| {
        lookup_capability(point.type_id)
            .is_some_and(|capability| capability.point_role == PointRole::Monitor)
            .then(|| build_point_measurement_payload_with_offset(point.type_id, point, time_offset_ms))
            .flatten()
            .map(|payload| (point.type_id, payload))
    });
    drop(points);

    let info = if let Some((type_id, payload)) = response {
        AsduInfo {
            type_id,
            cause: 5,
            common_address: target_common_address,
            data: payload,
            timestamp: chrono::Utc::now(),
        }
    } else {
        AsduInfo {
            type_id: 102,
            cause: 47,
            common_address: target_common_address,
            data: iec60870_parser::parser::asdu::encode::encode_read_command(*ioa),
            timestamp: chrono::Utc::now(),
        }
    };
    response_sender.provided_single(info).await?;
    Ok(true)
}
