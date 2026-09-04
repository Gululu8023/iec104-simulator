//! IEC104 文件传输会话编排。
//!
//! 文件仓库存取位于 data::file_repo，连接级会话由 PeerLinkState 持有；
//! 本模块只负责目录响应、上传/下载协议流程及传输响应发送。

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, atomic::AtomicU64},
    time::{Duration, Instant},
};

use log::warn;
use tokio::sync::{Mutex, RwLock};

use super::response_cause::merge_response_cause_with_request_origin;
use crate::{
    core::{
        protocol_adapter::{
            build_ack_section_payload, build_directory_payload, build_file_ready_payload, build_last_section_payload,
            build_section_ready_payload, build_segment_payload, file_transfer_segment_payload_max,
            file_transfer_single_section_max_file_size,
        },
        slave::{
            connection::connection_registry::{PeerLinkState, SlaveFileTransferMode, SlaveFileTransferSession},
            data::file_repo::{
                SlaveFileRepoEntry, filter_slave_repo_entries_by_query_log, load_slave_repo_index,
                resolve_slave_file_repo_dir, save_slave_repo_index, slave_repo_file_path, upsert_slave_repo_entry,
            },
            transport::{
                outbound_dispatcher::{send_asdu_to_peer_wait_window, send_asdu_to_peer_with_priority},
                outbound_queue::OutboundPriority,
            },
        },
        types::{AsduInfo, ConnectionInfo, SlaveMessageRecord},
    },
    errors::AppResult,
    network::{DecodedFileTransferRecord, Iec104Protocol, TcpServer},
    utils::pcap::CapturePacket,
};

const FILE_TRANSFER_SINGLE_SECTION: u8 = 1;
const FILE_TRANSFER_SESSION_TTL: Duration = Duration::from_secs(30);

async fn send_file_transfer_response_asdu(
    peer_addr: &str,
    request_cause: u16,
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    asdu: AsduInfo,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
) -> AppResult<()> {
    let mut merged = asdu;
    merged.cause = merge_response_cause_with_request_origin(request_cause, merged.cause);
    send_asdu_to_peer_with_priority(
        peer_addr,
        server,
        protocol,
        &merged,
        connections,
        peer_local_addrs,
        message_records,
        next_message_id,
        capture_packets,
        OutboundPriority::Control,
    )
    .await
}

async fn send_file_transfer_response_asdu_wait_window(
    peer_addr: &str,
    request_cause: u16,
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    asdu: AsduInfo,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
) -> AppResult<()> {
    let mut merged = asdu;
    merged.cause = merge_response_cause_with_request_origin(request_cause, merged.cause);
    send_asdu_to_peer_wait_window(
        peer_addr,
        server,
        protocol,
        &merged,
        connections,
        peer_local_addrs,
        message_records,
        next_message_id,
        capture_packets,
    )
    .await
}

fn spawn_slave_download_transfer_task(
    station_id: String,
    peer_addr: String,
    request_cause: u16,
    common_address: u16,
    ioa: u32,
    file_name: u16,
    section: u8,
    file_bytes: Vec<u8>,
    server: Arc<TcpServer>,
    protocol: Arc<Mutex<Iec104Protocol>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: Arc<AtomicU64>,
    capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    segment_payload_max: usize,
) {
    tokio::spawn(async move {
        let section_ready = AsduInfo {
            type_id: 121,
            cause: 13,
            common_address,
            data: build_section_ready_payload(ioa, file_name, 0x00, section, file_bytes.len() as u32),
            timestamp: chrono::Utc::now(),
        };
        if let Err(err) = send_file_transfer_response_asdu_wait_window(
            &peer_addr,
            request_cause,
            &server,
            &protocol,
            section_ready,
            &connections,
            &peer_local_addrs,
            &message_records,
            &next_message_id,
            &capture_packets,
        )
        .await
        {
            warn!(
                "[slave] file-transfer-send-failed station_id={} peer_addr={} nof={} section={} stage=section-ready error={}",
                station_id, peer_addr, file_name, section, err
            );
            return;
        }

        let mut segment_no: u8 = 0;
        for chunk in file_bytes.chunks(segment_payload_max) {
            segment_no = segment_no.checked_add(1).expect("single-section capacity limits segment count to 255");
            let segment = AsduInfo {
                type_id: 125,
                cause: 13,
                common_address,
                data: build_segment_payload(ioa, file_name, section, chunk),
                timestamp: chrono::Utc::now(),
            };
            if let Err(err) = send_file_transfer_response_asdu_wait_window(
                &peer_addr,
                request_cause,
                &server,
                &protocol,
                segment,
                &connections,
                &peer_local_addrs,
                &message_records,
                &next_message_id,
                &capture_packets,
            )
            .await
            {
                warn!(
                    "[slave] file-transfer-send-failed station_id={} peer_addr={} nof={} section={} stage=segment-{} error={}",
                    station_id, peer_addr, file_name, section, segment_no, err
                );
                return;
            }
        }

        let last_section = AsduInfo {
            type_id: 123,
            cause: 13,
            common_address,
            data: build_last_section_payload(ioa, file_name, 0x03, section, segment_no),
            timestamp: chrono::Utc::now(),
        };
        if let Err(err) = send_file_transfer_response_asdu_wait_window(
            &peer_addr,
            request_cause,
            &server,
            &protocol,
            last_section,
            &connections,
            &peer_local_addrs,
            &message_records,
            &next_message_id,
            &capture_packets,
        )
        .await
        {
            warn!(
                "[slave] file-transfer-send-failed station_id={} peer_addr={} nof={} section={} stage=last-section error={}",
                station_id, peer_addr, file_name, section, err
            );
        }
    });
}

async fn send_slave_directory_listing(
    peer_addr: &str,
    request_cause: u16,
    common_address: u16,
    ioa: u32,
    repo_entries: &[SlaveFileRepoEntry],
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
) -> AppResult<()> {
    let mut entries = repo_entries.to_vec();
    if entries.is_empty() {
        send_file_transfer_response_asdu(
            peer_addr,
            request_cause,
            server,
            protocol,
            AsduInfo {
                type_id: 126,
                cause: 13,
                common_address,
                data: build_directory_payload(ioa, 0, 0, 0x02, chrono::Utc::now()),
                timestamp: chrono::Utc::now(),
            },
            connections,
            peer_local_addrs,
            message_records,
            next_message_id,
            capture_packets,
        )
        .await?;
        return Ok(());
    }

    entries.sort_by_key(|item| item.nof);
    let last_index = entries.len().saturating_sub(1);
    for (idx, item) in entries.iter().enumerate() {
        let mut status = 0x01u8;
        if idx == last_index {
            status |= 0x02;
        }
        send_file_transfer_response_asdu(
            peer_addr,
            request_cause,
            server,
            protocol,
            AsduInfo {
                type_id: 126,
                cause: 13,
                common_address,
                data: build_directory_payload(ioa, item.nof, item.length, status, item.updated_at),
                timestamp: chrono::Utc::now(),
            },
            connections,
            peer_local_addrs,
            message_records,
            next_message_id,
            capture_packets,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn handle_slave_file_transfer_records(
    station_id: &str,
    peer_addr: &str,
    request_cause: u16,
    common_address: u16,
    records: &[DecodedFileTransferRecord],
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    runtime_event_handle: &Arc<crate::services::RuntimeEventSink>,
    peer_link_state: &mut PeerLinkState,
    max_asdu_bytes: u16,
) -> AppResult<()> {
    if peer_link_state.file_transfer.as_ref().is_some_and(|session| Instant::now() >= session.expires_at) {
        peer_link_state.file_transfer = None;
    }
    let repo_dir = resolve_slave_file_repo_dir(runtime_event_handle, station_id, common_address).await;
    if let Err(err) = std::fs::create_dir_all(&repo_dir) {
        warn!("[slave] create file repo failed path={} error={}", repo_dir.display(), err);
    }
    let mut index = load_slave_repo_index(&repo_dir);

    for record in records {
        match record {
            DecodedFileTransferRecord::SelectCall { ioa, file_name, qualifier, section } => {
                let scq = *qualifier;
                if scq == 0x02 {
                    send_slave_directory_listing(
                        peer_addr,
                        request_cause,
                        common_address,
                        *ioa,
                        &index.entries,
                        server,
                        protocol,
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    continue;
                }

                if (scq & 0x03) == 0x03 {
                    let nof = *file_name;
                    let file_path = slave_repo_file_path(&repo_dir, nof);
                    let length = std::fs::metadata(&file_path)
                        .ok()
                        .filter(|metadata| metadata.is_file())
                        .and_then(|metadata| u32::try_from(metadata.len()).ok());
                    let max_file_size = file_transfer_single_section_max_file_size(max_asdu_bytes).unwrap_or(0);
                    let Some(length) = length.filter(|length| *length <= max_file_size) else {
                        send_file_transfer_response_asdu(
                            peer_addr,
                            request_cause,
                            server,
                            protocol,
                            AsduInfo {
                                type_id: 124,
                                cause: 13,
                                common_address,
                                data: build_ack_section_payload(*ioa, nof, 0x02, *section),
                                timestamp: chrono::Utc::now(),
                            },
                            connections,
                            peer_local_addrs,
                            message_records,
                            next_message_id,
                            capture_packets,
                        )
                        .await?;
                        continue;
                    };
                    peer_link_state.file_transfer = Some(SlaveFileTransferSession {
                        mode: SlaveFileTransferMode::DownloadToMaster,
                        common_address,
                        ioa: *ioa,
                        nof,
                        expected_length: length,
                        section: FILE_TRANSFER_SINGLE_SECTION,
                        local_path: file_path,
                        buffer: Vec::new(),
                        expires_at: Instant::now() + FILE_TRANSFER_SESSION_TTL,
                    });
                    send_file_transfer_response_asdu(
                        peer_addr,
                        request_cause,
                        server,
                        protocol,
                        AsduInfo {
                            type_id: 120,
                            cause: 13,
                            common_address,
                            data: build_file_ready_payload(*ioa, nof, 0x00, length),
                            timestamp: chrono::Utc::now(),
                        },
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    continue;
                }

                if (scq & 0x20) != 0 {
                    let Some(session) = peer_link_state.file_transfer.clone() else {
                        send_file_transfer_response_asdu(
                            peer_addr,
                            request_cause,
                            server,
                            protocol,
                            AsduInfo {
                                type_id: 124,
                                cause: 13,
                                common_address,
                                data: build_ack_section_payload(*ioa, *file_name, 0x08, *section),
                                timestamp: chrono::Utc::now(),
                            },
                            connections,
                            peer_local_addrs,
                            message_records,
                            next_message_id,
                            capture_packets,
                        )
                        .await?;
                        continue;
                    };
                    if session.mode != SlaveFileTransferMode::DownloadToMaster
                        || session.common_address != common_address
                        || session.ioa != *ioa
                        || session.nof != *file_name
                        || session.section != *section
                    {
                        send_file_transfer_response_asdu(
                            peer_addr,
                            request_cause,
                            server,
                            protocol,
                            AsduInfo {
                                type_id: 124,
                                cause: 13,
                                common_address,
                                data: build_ack_section_payload(*ioa, *file_name, 0x08, *section),
                                timestamp: chrono::Utc::now(),
                            },
                            connections,
                            peer_local_addrs,
                            message_records,
                            next_message_id,
                            capture_packets,
                        )
                        .await?;
                        continue;
                    }

                    let file_bytes = match std::fs::read(&session.local_path) {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            warn!(
                                "[slave] read download file failed path={} error={}",
                                session.local_path.display(),
                                err
                            );
                            send_file_transfer_response_asdu(
                                peer_addr,
                                request_cause,
                                server,
                                protocol,
                                AsduInfo {
                                    type_id: 124,
                                    cause: 13,
                                    common_address,
                                    data: build_ack_section_payload(*ioa, *file_name, 0x08, *section),
                                    timestamp: chrono::Utc::now(),
                                },
                                connections,
                                peer_local_addrs,
                                message_records,
                                next_message_id,
                                capture_packets,
                            )
                            .await?;
                            peer_link_state.file_transfer = None;
                            continue;
                        }
                    };
                    if file_bytes.len() != session.expected_length as usize {
                        warn!(
                            "[slave] download file length changed path={} expected={} actual={}",
                            session.local_path.display(),
                            session.expected_length,
                            file_bytes.len()
                        );
                        send_file_transfer_response_asdu(
                            peer_addr,
                            request_cause,
                            server,
                            protocol,
                            AsduInfo {
                                type_id: 124,
                                cause: 13,
                                common_address,
                                data: build_ack_section_payload(*ioa, *file_name, 0x08, *section),
                                timestamp: chrono::Utc::now(),
                            },
                            connections,
                            peer_local_addrs,
                            message_records,
                            next_message_id,
                            capture_packets,
                        )
                        .await?;
                        peer_link_state.file_transfer = None;
                        continue;
                    }
                    let Some(segment_payload_max) = file_transfer_segment_payload_max(max_asdu_bytes) else {
                        send_file_transfer_response_asdu(
                            peer_addr,
                            request_cause,
                            server,
                            protocol,
                            AsduInfo {
                                type_id: 124,
                                cause: 13,
                                common_address,
                                data: build_ack_section_payload(*ioa, *file_name, 0x08, *section),
                                timestamp: chrono::Utc::now(),
                            },
                            connections,
                            peer_local_addrs,
                            message_records,
                            next_message_id,
                            capture_packets,
                        )
                        .await?;
                        continue;
                    };

                    // 下载段发送放到后台任务里，避免阻塞接收循环而吃不到对端的 S 帧确认。
                    spawn_slave_download_transfer_task(
                        station_id.to_string(),
                        peer_addr.to_string(),
                        request_cause,
                        common_address,
                        *ioa,
                        *file_name,
                        *section,
                        file_bytes,
                        Arc::clone(server),
                        Arc::clone(protocol),
                        Arc::clone(connections),
                        Arc::clone(peer_local_addrs),
                        Arc::clone(message_records),
                        Arc::clone(next_message_id),
                        Arc::clone(capture_packets),
                        segment_payload_max,
                    );
                    continue;
                }
            }
            DecodedFileTransferRecord::FileReady { ioa, file_name, length, .. } => {
                let max_file_size = file_transfer_single_section_max_file_size(max_asdu_bytes).unwrap_or(0);
                if *length > max_file_size {
                    send_file_transfer_response_asdu(
                        peer_addr,
                        request_cause,
                        server,
                        protocol,
                        AsduInfo {
                            type_id: 124,
                            cause: 13,
                            common_address,
                            data: build_ack_section_payload(*ioa, *file_name, 0x02, FILE_TRANSFER_SINGLE_SECTION),
                            timestamp: chrono::Utc::now(),
                        },
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    continue;
                }
                let file_path = slave_repo_file_path(&repo_dir, *file_name);
                peer_link_state.file_transfer = Some(SlaveFileTransferSession {
                    mode: SlaveFileTransferMode::UploadFromMaster,
                    common_address,
                    ioa: *ioa,
                    nof: *file_name,
                    expected_length: *length,
                    section: FILE_TRANSFER_SINGLE_SECTION,
                    local_path: file_path,
                    buffer: Vec::new(),
                    expires_at: Instant::now() + FILE_TRANSFER_SESSION_TTL,
                });
                send_file_transfer_response_asdu(
                    peer_addr,
                    request_cause,
                    server,
                    protocol,
                    AsduInfo {
                        type_id: 124,
                        cause: 13,
                        common_address,
                        data: build_ack_section_payload(*ioa, *file_name, 0x01, FILE_TRANSFER_SINGLE_SECTION),
                        timestamp: chrono::Utc::now(),
                    },
                    connections,
                    peer_local_addrs,
                    message_records,
                    next_message_id,
                    capture_packets,
                )
                .await?;
            }
            DecodedFileTransferRecord::SectionReady { ioa, file_name, section, length, .. } => {
                let mut accepted = false;
                if let Some(session) = peer_link_state.file_transfer.as_mut() {
                    if session.mode == SlaveFileTransferMode::UploadFromMaster
                        && session.common_address == common_address
                        && session.ioa == *ioa
                        && session.nof == *file_name
                    {
                        if *section == FILE_TRANSFER_SINGLE_SECTION
                            && *length <= file_transfer_single_section_max_file_size(max_asdu_bytes).unwrap_or(0)
                            && *length == session.expected_length
                        {
                            accepted = true;
                            session.section = *section;
                            session.expires_at = Instant::now() + FILE_TRANSFER_SESSION_TTL;
                        }
                    }
                }
                send_file_transfer_response_asdu(
                    peer_addr,
                    request_cause,
                    server,
                    protocol,
                    AsduInfo {
                        type_id: 124,
                        cause: 13,
                        common_address,
                        data: build_ack_section_payload(*ioa, *file_name, if accepted { 0x04 } else { 0x08 }, *section),
                        timestamp: chrono::Utc::now(),
                    },
                    connections,
                    peer_local_addrs,
                    message_records,
                    next_message_id,
                    capture_packets,
                )
                .await?;
                if !accepted {
                    peer_link_state.file_transfer = None;
                }
            }
            DecodedFileTransferRecord::Segment { ioa, file_name, section, data, .. } => {
                let mut quota_exceeded = false;
                if let Some(session) = peer_link_state.file_transfer.as_mut() {
                    if session.mode == SlaveFileTransferMode::UploadFromMaster
                        && session.common_address == common_address
                        && session.ioa == *ioa
                        && session.nof == *file_name
                        && session.section == *section
                    {
                        let next_len = session.buffer.len().checked_add(data.len());
                        if next_len.is_none_or(|length| {
                            length > file_transfer_single_section_max_file_size(max_asdu_bytes).unwrap_or(0) as usize
                                || length > session.expected_length as usize
                        }) {
                            quota_exceeded = true;
                        } else {
                            session.buffer.extend_from_slice(data);
                            session.expires_at = Instant::now() + FILE_TRANSFER_SESSION_TTL;
                        }
                    }
                }
                if quota_exceeded {
                    send_file_transfer_response_asdu(
                        peer_addr,
                        request_cause,
                        server,
                        protocol,
                        AsduInfo {
                            type_id: 124,
                            cause: 13,
                            common_address,
                            data: build_ack_section_payload(*ioa, *file_name, 0x08, *section),
                            timestamp: chrono::Utc::now(),
                        },
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    peer_link_state.file_transfer = None;
                }
            }
            DecodedFileTransferRecord::LastSection { ioa, file_name, last_section_number, qualifier: _, .. } => {
                let Some(session) = peer_link_state.file_transfer.clone() else {
                    continue;
                };
                if session.mode != SlaveFileTransferMode::UploadFromMaster
                    || session.common_address != common_address
                    || session.ioa != *ioa
                    || session.nof != *file_name
                    || session.section != *last_section_number
                {
                    continue;
                }
                if let Err(err) = std::fs::create_dir_all(&repo_dir) {
                    warn!("[slave] create repo dir failed path={} error={}", repo_dir.display(), err);
                }
                let data = session.buffer;
                if data.len() > file_transfer_single_section_max_file_size(max_asdu_bytes).unwrap_or(0) as usize {
                    send_file_transfer_response_asdu(
                        peer_addr,
                        request_cause,
                        server,
                        protocol,
                        AsduInfo {
                            type_id: 124,
                            cause: 13,
                            common_address,
                            data: build_ack_section_payload(*ioa, *file_name, 0x02, session.section),
                            timestamp: chrono::Utc::now(),
                        },
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    peer_link_state.file_transfer = None;
                    continue;
                }
                if session.expected_length > 0 && data.len() != session.expected_length as usize {
                    send_file_transfer_response_asdu(
                        peer_addr,
                        request_cause,
                        server,
                        protocol,
                        AsduInfo {
                            type_id: 124,
                            cause: 13,
                            common_address,
                            data: build_ack_section_payload(*ioa, *file_name, 0x02, session.section),
                            timestamp: chrono::Utc::now(),
                        },
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    peer_link_state.file_transfer = None;
                    continue;
                }
                if let Err(err) = std::fs::write(&session.local_path, &data) {
                    warn!("[slave] write upload file failed path={} error={}", session.local_path.display(), err);
                    send_file_transfer_response_asdu(
                        peer_addr,
                        request_cause,
                        server,
                        protocol,
                        AsduInfo {
                            type_id: 124,
                            cause: 13,
                            common_address,
                            data: build_ack_section_payload(*ioa, *file_name, 0x02, session.section),
                            timestamp: chrono::Utc::now(),
                        },
                        connections,
                        peer_local_addrs,
                        message_records,
                        next_message_id,
                        capture_packets,
                    )
                    .await?;
                    peer_link_state.file_transfer = None;
                    continue;
                }
                upsert_slave_repo_entry(&mut index, *file_name, data.len() as u32);
                save_slave_repo_index(&repo_dir, &index);
                send_file_transfer_response_asdu(
                    peer_addr,
                    request_cause,
                    server,
                    protocol,
                    AsduInfo {
                        type_id: 124,
                        cause: 13,
                        common_address,
                        data: build_ack_section_payload(*ioa, *file_name, 0x01, session.section),
                        timestamp: chrono::Utc::now(),
                    },
                    connections,
                    peer_local_addrs,
                    message_records,
                    next_message_id,
                    capture_packets,
                )
                .await?;
                peer_link_state.file_transfer = None;
            }
            DecodedFileTransferRecord::AckSection { ioa, file_name, qualifier, section } => {
                if (*qualifier & 0x01) != 0 {
                    if let Some(session) = peer_link_state.file_transfer.as_ref() {
                        if session.mode == SlaveFileTransferMode::DownloadToMaster
                            && session.common_address == common_address
                            && session.ioa == *ioa
                            && session.nof == *file_name
                            && session.section == *section
                        {
                            peer_link_state.file_transfer = None;
                        }
                    }
                }
            }
            DecodedFileTransferRecord::QueryLog { ioa, file_name, range_start_time, range_end_time } => {
                let filtered_entries = filter_slave_repo_entries_by_query_log(
                    &index.entries,
                    *file_name,
                    range_start_time,
                    range_end_time,
                );
                send_slave_directory_listing(
                    peer_addr,
                    request_cause,
                    common_address,
                    *ioa,
                    &filtered_entries,
                    server,
                    protocol,
                    connections,
                    peer_local_addrs,
                    message_records,
                    next_message_id,
                    capture_packets,
                )
                .await?;
            }
            DecodedFileTransferRecord::Directory { .. } | DecodedFileTransferRecord::Raw(_) => {}
        }
    }

    Ok(())
}
