//! 主站和从站共享的运行时提示发送逻辑。

use serde::Serialize;

use crate::{
    core::types::{MasterRuntimeHintEvent, PointSyncCursor, SlaveRuntimeHintEvent},
    services::RuntimeEventSink,
};

async fn emit<T: Serialize + Clone>(event_sink: &RuntimeEventSink, event_name: &str, event: T) {
    if let Err(error) = event_sink.emit(event_name, event) {
        log::warn!("发送运行时提示失败 event={} error={}", event_name, error);
    }
}

pub(crate) async fn emit_master_runtime_hint(
    event_sink: &RuntimeEventSink,
    station_id: &str,
    connection_id: Option<&str>,
    reason: &str,
) {
    emit_master_runtime_hint_with_details(event_sink, station_id, connection_id, reason, None, None, None).await;
}

pub(crate) async fn emit_master_runtime_hint_with_attempts(
    event_sink: &RuntimeEventSink,
    station_id: &str,
    connection_id: Option<&str>,
    reason: &str,
    attempt: Option<u32>,
    max_attempts: Option<u32>,
) {
    emit_master_runtime_hint_with_details(event_sink, station_id, connection_id, reason, attempt, max_attempts, None)
        .await;
}

pub(crate) async fn emit_master_runtime_hint_with_cursor(
    event_sink: &RuntimeEventSink,
    station_id: &str,
    connection_id: Option<&str>,
    reason: &str,
    point_cursor: PointSyncCursor,
) {
    emit_master_runtime_hint_with_details(
        event_sink,
        station_id,
        connection_id,
        reason,
        None,
        None,
        Some(point_cursor),
    )
    .await;
}

async fn emit_master_runtime_hint_with_details(
    event_sink: &RuntimeEventSink,
    station_id: &str,
    connection_id: Option<&str>,
    reason: &str,
    attempt: Option<u32>,
    max_attempts: Option<u32>,
    point_cursor: Option<PointSyncCursor>,
) {
    emit(event_sink, "master-runtime-hint", MasterRuntimeHintEvent {
        station_id: station_id.to_string(),
        connection_id: connection_id.map(str::to_string),
        reason: reason.to_string(),
        attempt,
        max_attempts,
        point_cursor,
    })
    .await;
}

pub(crate) async fn emit_slave_runtime_hint(
    event_sink: &RuntimeEventSink,
    station_id: &str,
    connection_id: Option<&str>,
    reason: &str,
) {
    emit_slave_runtime_hint_with_cursor(event_sink, station_id, connection_id, reason, None).await;
}

pub(crate) async fn emit_slave_runtime_hint_with_cursor(
    event_sink: &RuntimeEventSink,
    station_id: &str,
    connection_id: Option<&str>,
    reason: &str,
    point_cursor: Option<PointSyncCursor>,
) {
    emit(event_sink, "slave-runtime-hint", SlaveRuntimeHintEvent {
        station_id: station_id.to_string(),
        connection_id: connection_id.map(str::to_string),
        reason: reason.to_string(),
        point_cursor,
    })
    .await;
}
