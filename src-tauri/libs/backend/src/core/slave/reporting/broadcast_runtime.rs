//! 从站广播运行时模块。
//!
//! 本模块负责从站向所有连接的主站广播数据点变化和 SOE 事件,包括:
//!
//! - **数据点广播**:向所有就绪的主站连接广播数据点变化
//! - **SOE 事件管理**:维护 SOE 事件队列(最多 10000 条),支持时间偏移
//! - **变化上报**:检测数据点变化并触发自发上报(COT=3)
//! - **合并优化**:使用 SpontaneousCoalescer 合并短时间内的多次变化
//! - **站点策略**:根据公共地址和连接配置解析有效的站点策略
//! - **优先级调度**:支持高优先级和普通优先级的 ASDU 发送
//! - **批量发送**:对多个数据点进行批量打包发送以提高效率

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use chrono::Datelike;
use log::warn;
use tokio::{
    sync::{Mutex, RwLock},
    time::{Duration, sleep},
};

use super::{
    policy_store::resolve_effective_station_policy,
    soe_support::{
        SoeEvent, apply_station_time_offset, build_point_measurement_payload_with_offset, build_soe_event_payload,
        map_point_type_to_soe_type,
    },
    spontaneous::{SpontaneousCoalescer, sanitize_spontaneous_flush_ms},
};
use crate::{
    core::{
        protocol_adapter::encode_cp56_time2a,
        shared::runtime_hint::emit_slave_runtime_hint,
        slave::{
            connection::connection_registry::{
                increment_coalesced_updates_for_all_peers, update_soe_backlog_for_all_peers,
                update_soe_queue_peak_for_peer,
            },
            protocol::{asdu_dispatch::RequestContext, response_cause::ResponseEnvelope},
            service::send_response_from_ctx,
            simulation::engine::{PointKey, point_key},
            transport::{
                outbound_dispatcher::{send_asdu_batched_to_peer_with_priority, send_asdu_to_peer_with_priority},
                outbound_queue::OutboundPriority,
                payload_batching,
            },
        },
        types::{
            AsduInfo, BackendSoeEvent, ConnectionInfo, DataPoint, DataPointTimestampDetail, LinkParams,
            SlaveMessageRecord, SlaveStationPolicy, SlaveStationPolicyOverride, is_quality_business_usable,
        },
    },
    errors::AppResult,
    network::{Iec104Protocol, ProtocolState, TcpServer},
    utils::{logger::iec104_log_kv, pcap::CapturePacket},
};

const MAX_SOE_EVENTS: usize = 10000;
const COT_SPONTANEOUS: u16 = 3;

#[derive(Clone)]
pub(crate) struct SlaveBroadcastRuntime {
    pub(crate) station_id: String,
    pub(crate) default_common_address: u16,
    pub(crate) server: Arc<RwLock<Option<Arc<TcpServer>>>>,
    pub(crate) protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    pub(crate) peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    pub(crate) message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    pub(crate) next_message_id: Arc<AtomicU64>,
    pub(crate) capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    pub(crate) link_params: Arc<RwLock<LinkParams>>,
    pub(crate) station_policy_defaults: Arc<RwLock<SlaveStationPolicy>>,
    pub(crate) station_policy_overrides: Arc<RwLock<HashMap<u16, SlaveStationPolicyOverride>>>,
    pub(crate) station_policy_connection_override_enabled: Arc<RwLock<bool>>,
    pub(crate) spontaneous_coalescer: Arc<Mutex<SpontaneousCoalescer>>,
    pub(crate) soe_events: Arc<RwLock<VecDeque<SoeEvent>>>,
    pub(crate) soe_records: Arc<RwLock<VecDeque<BackendSoeEvent>>>,
    pub(crate) station_time_offset_ms: Arc<RwLock<i64>>,
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
    pub(crate) next_soe_id: Arc<AtomicU64>,
}

pub(crate) async fn resolve_station_policy_for_peer(ctx: &RequestContext, common_address: u16) -> SlaveStationPolicy {
    resolve_station_policy(
        &ctx.link_params,
        &ctx.station_policy_defaults,
        &ctx.station_policy_overrides,
        &ctx.station_policy_connection_override_enabled,
        common_address,
    )
    .await
}

pub(crate) async fn broadcast_points(
    runtime: &SlaveBroadcastRuntime,
    points: &[DataPoint],
    cause: u16,
) -> AppResult<()> {
    let Some((server, ready_peer_protocols)) = collect_ready_peer_protocols(runtime).await else {
        return Ok(());
    };

    let mut station_policy_cache: HashMap<u16, SlaveStationPolicy> = HashMap::new();
    let mut normal_points: Vec<DataPoint> = Vec::with_capacity(points.len());
    let mut soe_points_by_key: HashMap<PointKey, DataPoint> = HashMap::new();

    for point in points {
        let common_address = point.common_address.unwrap_or(runtime.default_common_address);
        let policy = if let Some(value) = station_policy_cache.get(&common_address).copied() {
            value
        } else {
            let resolved = resolve_station_policy(
                &runtime.link_params,
                &runtime.station_policy_defaults,
                &runtime.station_policy_overrides,
                &runtime.station_policy_connection_override_enabled,
                common_address,
            )
            .await;
            station_policy_cache.insert(common_address, resolved);
            resolved
        };

        if cause == COT_SPONTANEOUS && policy.enable_soe {
            soe_points_by_key.insert(point_key(common_address, point.address), point.clone());
        } else {
            normal_points.push(point.clone());
        }
    }

    let soe_points: Vec<DataPoint> = soe_points_by_key.into_values().collect();

    if cause == COT_SPONTANEOUS && !soe_points.is_empty() {
        let station_time_offset_ms = *runtime.station_time_offset_ms.read().await;
        let (accepted, dropped, queue_peak) = enqueue_soe_events(
            &runtime.soe_events,
            &runtime.soe_records,
            &runtime.next_soe_id,
            &soe_points,
            runtime.default_common_address,
            station_time_offset_ms,
        )
        .await;
        let soe_backlog = runtime.soe_events.read().await.len() as u64;
        {
            let mut guard = runtime.connections.write().await;
            for conn in guard.values_mut() {
                conn.soe_queue_peak = conn.soe_queue_peak.max(queue_peak as u64);
                conn.soe_backlog = soe_backlog;
            }
        }
        if dropped > 0 {
            warn!("[slave] soe_queue_overflow_drop station_id={} dropped={}", runtime.station_id, dropped);
            emit_slave_runtime_hint(&runtime.runtime_event_handle, &runtime.station_id, None, "soe-overflow").await;
        }
        if accepted == 0 && normal_points.is_empty() {
            return Ok(());
        }
        if accepted > 0 {
            emit_slave_runtime_hint(&runtime.runtime_event_handle, &runtime.station_id, None, "soe-updated").await;
        }

        let events = drain_soe_events(&runtime.soe_events).await;
        let soe_backlog = runtime.soe_events.read().await.len() as u64;
        update_soe_backlog_for_all_peers(&runtime.connections, soe_backlog).await;
        for event in events {
            let Some(payload) = build_soe_event_payload(&event) else {
                continue;
            };
            let asdu = AsduInfo {
                type_id: event.type_id,
                cause,
                common_address: event.common_address,
                data: payload,
                timestamp: event.timestamp,
            };
            for (peer_addr, protocol) in &ready_peer_protocols {
                if let Err(err) = send_asdu_to_peer_with_priority(
                    peer_addr,
                    &server,
                    protocol,
                    &asdu,
                    &runtime.connections,
                    &runtime.peer_local_addrs,
                    &runtime.message_records,
                    &runtime.next_message_id,
                    &runtime.capture_packets,
                    OutboundPriority::Soe,
                )
                .await
                {
                    warn!(
                        "[slave] broadcast_soe_failed station_id={} peer={} type_id={} ca={} ioa={} error={}",
                        runtime.station_id, peer_addr, event.type_id, event.common_address, event.address, err
                    );
                }
            }
        }
    }

    if normal_points.is_empty() {
        return Ok(());
    }

    let link_params_snapshot = *runtime.link_params.read().await;
    if cause == COT_SPONTANEOUS && link_params_snapshot.simulation_latest_only {
        return enqueue_spontaneous_points_with_flush(runtime, normal_points).await;
    }

    broadcast_normal_points_immediate(runtime, &normal_points, cause).await
}

pub(crate) async fn emit_command_spontaneous_updates(
    ctx: &RequestContext,
    request_cause: u16,
    target_common_address: u16,
    spontaneous_updates: Vec<DataPoint>,
    enable_soe: bool,
    points_changed: bool,
) -> AppResult<()> {
    let mut dedup_spontaneous_updates: HashMap<PointKey, DataPoint> = HashMap::new();
    for point in spontaneous_updates {
        let common_address = point.common_address.unwrap_or(target_common_address);
        dedup_spontaneous_updates.insert(point_key(common_address, point.address), point);
    }
    let spontaneous_updates: Vec<DataPoint> = dedup_spontaneous_updates.into_values().collect();

    if enable_soe {
        let offset_ms = *ctx.station_time_offset_ms.read().await;
        let (accepted, dropped, queue_peak) = enqueue_soe_events(
            &ctx.soe_events,
            &ctx.soe_records,
            &ctx.next_soe_id,
            &spontaneous_updates,
            target_common_address,
            offset_ms,
        )
        .await;
        update_soe_queue_peak_for_peer(&ctx.connections, &ctx.peer_addr, queue_peak).await;
        if dropped > 0 {
            warn!(
                "[slave] command_soe_queue_overflow_drop {} dropped={}",
                iec104_log_kv(
                    &ctx.station_id,
                    Some(&ctx.peer_addr),
                    None,
                    Some(COT_SPONTANEOUS),
                    None,
                    Some("SOE_QUEUE_OVERFLOW"),
                    None,
                ),
                dropped
            );
            emit_slave_runtime_hint(&ctx.runtime_event_handle, &ctx.station_id, Some(&ctx.peer_addr), "soe-overflow")
                .await;
        }
        if accepted > 0 {
            emit_slave_runtime_hint(&ctx.runtime_event_handle, &ctx.station_id, Some(&ctx.peer_addr), "soe-updated")
                .await;
            for event in drain_soe_events(&ctx.soe_events).await {
                let Some(payload) = build_soe_event_payload(&event) else {
                    continue;
                };
                send_response_from_ctx(
                    ctx,
                    ResponseEnvelope::provided_single(request_cause, AsduInfo {
                        type_id: event.type_id,
                        cause: COT_SPONTANEOUS,
                        common_address: event.common_address,
                        data: payload,
                        timestamp: event.timestamp,
                    }),
                )
                .await?;
            }
            if let Some(conn) = ctx.connections.write().await.get_mut(&ctx.peer_addr) {
                conn.soe_backlog = 0;
            }
        }
    } else {
        let offset_ms = *ctx.station_time_offset_ms.read().await;
        for point in spontaneous_updates {
            let Some(payload) = build_point_measurement_payload_with_offset(point.type_id, &point, offset_ms) else {
                warn!(
                    "[slave] command_spontaneous_skip_unsupported {} target_type_id={} ioa={} ca={}",
                    iec104_log_kv(
                        &ctx.station_id,
                        Some(&ctx.peer_addr),
                        Some(point.type_id),
                        Some(COT_SPONTANEOUS),
                        Some(point.address),
                        Some("UNSUPPORTED_SPONTANEOUS_TYPE"),
                        None,
                    ),
                    point.type_id,
                    point.address,
                    point.common_address.unwrap_or(target_common_address)
                );
                continue;
            };
            send_response_from_ctx(
                ctx,
                ResponseEnvelope::provided_single(request_cause, AsduInfo {
                    type_id: point.type_id,
                    cause: COT_SPONTANEOUS,
                    common_address: point.common_address.unwrap_or(target_common_address),
                    data: payload,
                    timestamp: chrono::Utc::now(),
                }),
            )
            .await?;
        }
    }

    if points_changed {
        emit_slave_runtime_hint(&ctx.runtime_event_handle, &ctx.station_id, Some(&ctx.peer_addr), "points-changed")
            .await;
    }
    Ok(())
}

async fn collect_ready_peer_protocols(
    runtime: &SlaveBroadcastRuntime,
) -> Option<(Arc<TcpServer>, Vec<(String, Arc<Mutex<Iec104Protocol>>)>)> {
    let server = runtime.server.read().await.clone()?;
    let peer_protocols: Vec<(String, Arc<Mutex<Iec104Protocol>>)> =
        runtime.protocols.read().await.iter().map(|(peer, protocol)| (peer.clone(), Arc::clone(protocol))).collect();
    if peer_protocols.is_empty() {
        return None;
    }

    let mut ready_peer_protocols = Vec::new();
    for (peer_addr, protocol) in peer_protocols {
        let state = {
            let guard = protocol.lock().await;
            guard.state().clone()
        };
        if state == ProtocolState::DataTransfer {
            ready_peer_protocols.push((peer_addr, protocol));
        }
    }
    if ready_peer_protocols.is_empty() {
        return None;
    }
    Some((server, ready_peer_protocols))
}

async fn enqueue_spontaneous_points_with_flush(
    runtime: &SlaveBroadcastRuntime,
    points: Vec<DataPoint>,
) -> AppResult<()> {
    let flush_ms = sanitize_spontaneous_flush_ms(runtime.link_params.read().await.spontaneous_flush_ms);
    let (should_spawn, replaced_count) = {
        let mut coalescer = runtime.spontaneous_coalescer.lock().await;
        let mut replaced_count = 0u64;
        for point in points {
            let common_address = point.common_address.unwrap_or(runtime.default_common_address);
            if coalescer.pending_points.insert(point_key(common_address, point.address), point).is_some() {
                replaced_count = replaced_count.saturating_add(1);
            }
        }
        let should_spawn = !coalescer.flush_active;
        if should_spawn {
            coalescer.flush_active = true;
        }
        (should_spawn, replaced_count)
    };
    if replaced_count > 0 {
        increment_coalesced_updates_for_all_peers(&runtime.connections, replaced_count).await;
    }
    if should_spawn {
        let runtime = runtime.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(flush_ms)).await;
            let pending_points = {
                let mut coalescer = runtime.spontaneous_coalescer.lock().await;
                let points = coalescer.pending_points.drain().map(|(_, point)| point).collect::<Vec<_>>();
                coalescer.flush_active = false;
                points
            };
            if !pending_points.is_empty() {
                let _ = broadcast_normal_points_immediate(&runtime, &pending_points, COT_SPONTANEOUS).await;
            }
        });
    }
    Ok(())
}

async fn broadcast_normal_points_immediate(
    runtime: &SlaveBroadcastRuntime,
    points: &[DataPoint],
    cause: u16,
) -> AppResult<()> {
    let Some((server, ready_peer_protocols)) = collect_ready_peer_protocols(runtime).await else {
        return Ok(());
    };
    let max_asdu_bytes = runtime.link_params.read().await.max_asdu_bytes;
    let station_time_offset_ms = *runtime.station_time_offset_ms.read().await;
    let mut station_policy_cache: HashMap<u16, SlaveStationPolicy> = HashMap::new();
    let mut grouped_payloads: BTreeMap<(u16, u8), Vec<(u32, Vec<u8>)>> = BTreeMap::new();

    for point in points {
        let type_id = point.type_id;
        let Some(payload) = build_point_measurement_payload_with_offset(type_id, point, station_time_offset_ms) else {
            warn!(
                "[slave] skip unsupported point type station_id={} ioa={} type_id={}",
                runtime.station_id, point.address, type_id
            );
            continue;
        };

        let common_address = point.common_address.unwrap_or(runtime.default_common_address);
        grouped_payloads.entry((common_address, type_id)).or_default().push((point.address, payload));
    }

    for ((common_address, type_id), mut entries) in grouped_payloads {
        entries.sort_by_key(|(ioa, _)| *ioa);
        let payloads: Vec<Vec<u8>> = entries.into_iter().map(|(_, payload)| payload).collect();
        let policy = if let Some(value) = station_policy_cache.get(&common_address).copied() {
            value
        } else {
            let resolved = resolve_station_policy(
                &runtime.link_params,
                &runtime.station_policy_defaults,
                &runtime.station_policy_overrides,
                &runtime.station_policy_connection_override_enabled,
                common_address,
            )
            .await;
            station_policy_cache.insert(common_address, resolved);
            resolved
        };
        let max_batch_count = payloads
            .first()
            .map(|payload| payload_batching::max_objects_per_asdu(payload.len(), max_asdu_bytes))
            .unwrap_or(1);

        for chunk in payloads.chunks(max_batch_count) {
            let object_count = chunk.len() as u8;
            let (combined_data, sq_sequence) = if policy.enable_sq1_upload {
                if let Some(sq1_payload) = payload_batching::try_compose_sq1_payload(chunk) {
                    (sq1_payload, true)
                } else {
                    (payload_batching::combine_payloads(chunk), false)
                }
            } else {
                (payload_batching::combine_payloads(chunk), false)
            };

            let asdu = AsduInfo { type_id, cause, common_address, data: combined_data, timestamp: chrono::Utc::now() };

            for (peer_addr, protocol) in &ready_peer_protocols {
                if let Err(err) = send_asdu_batched_to_peer_with_priority(
                    peer_addr,
                    &server,
                    protocol,
                    &asdu,
                    object_count,
                    sq_sequence,
                    &runtime.connections,
                    &runtime.peer_local_addrs,
                    &runtime.message_records,
                    &runtime.next_message_id,
                    &runtime.capture_packets,
                    OutboundPriority::Spontaneous,
                )
                .await
                {
                    warn!(
                        "[slave] broadcast_point_failed station_id={} peer={} type_id={} ca={} error={}",
                        runtime.station_id, peer_addr, type_id, common_address, err
                    );
                }
            }
        }
    }

    Ok(())
}

async fn enqueue_soe_events(
    queue: &Arc<RwLock<VecDeque<SoeEvent>>>,
    records: &Arc<RwLock<VecDeque<BackendSoeEvent>>>,
    next_id: &Arc<AtomicU64>,
    points: &[DataPoint],
    default_common_address: u16,
    station_time_offset_ms: i64,
) -> (usize, usize, usize) {
    let mut accepted = 0usize;
    let mut dropped = 0usize;
    let mut queue_peak = 0usize;
    let mut queue_guard = queue.write().await;
    let mut record_guard = records.write().await;

    for point in points {
        let Some(type_id) = map_point_type_to_soe_type(point.type_id) else {
            continue;
        };
        let common_address = point.common_address.unwrap_or(default_common_address);
        let event_timestamp = apply_station_time_offset(point.timestamp, station_time_offset_ms);
        queue_guard.push_back(SoeEvent {
            common_address,
            type_id,
            address: point.address,
            value: point.value,
            quality: point.quality,
            timestamp: event_timestamp,
        });
        while record_guard.len() >= MAX_SOE_EVENTS {
            record_guard.pop_front();
        }
        record_guard.push_back(BackendSoeEvent {
            id: next_id.fetch_add(1, Ordering::Relaxed).saturating_add(1),
            logged_at: chrono::Utc::now(),
            event_timestamp,
            connection_id: point.connection_id.clone(),
            slave_id: point.slave_id,
            point_name: Some(point.name.clone()),
            common_address,
            ioa: point.address,
            type_id,
            cause: COT_SPONTANEOUS,
            value: point.value,
            quality: point.quality,
            quality_common: point.quality_common,
            quality_detail: point.quality_detail.clone(),
            timestamp_detail: Some(DataPointTimestampDetail::Cp56 {
                raw: encode_cp56_time2a(event_timestamp).to_vec(),
                invalid: false,
                summer_time: false,
                weekday: event_timestamp.weekday().number_from_monday() as u8,
                resolved: Some(event_timestamp),
            }),
            business_usable: is_quality_business_usable(type_id, point.quality),
        });
        accepted = accepted.saturating_add(1);
        queue_peak = queue_peak.max(queue_guard.len());
    }

    while queue_guard.len() > MAX_SOE_EVENTS {
        queue_guard.pop_front();
        dropped = dropped.saturating_add(1);
    }

    (accepted, dropped, queue_peak)
}

async fn drain_soe_events(queue: &Arc<RwLock<VecDeque<SoeEvent>>>) -> Vec<SoeEvent> {
    let mut guard = queue.write().await;
    guard.drain(..).collect()
}

async fn resolve_station_policy(
    link_params: &Arc<RwLock<LinkParams>>,
    station_policy_defaults: &Arc<RwLock<SlaveStationPolicy>>,
    station_policy_overrides: &Arc<RwLock<HashMap<u16, SlaveStationPolicyOverride>>>,
    station_policy_connection_override_enabled: &Arc<RwLock<bool>>,
    common_address: u16,
) -> SlaveStationPolicy {
    resolve_effective_station_policy(
        link_params,
        station_policy_defaults,
        station_policy_overrides,
        station_policy_connection_override_enabled,
        common_address,
    )
    .await
}
