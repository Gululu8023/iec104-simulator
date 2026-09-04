//! 主站文件传输运行时模块。
//!
//! 本模块负责主站文件传输会话的执行和管理,支持文件下载、目录查询等功能。

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use chrono::Utc;
use tokio::sync::{Mutex, RwLock};

use super::{
    super::service::MasterFileTransferRequest,
    store::{
        MasterFileTransferSessionRuntime, append_download_chunk, cancel_file_transfer_session,
        get_file_transfer_session, set_file_transfer_failed, set_file_transfer_stage, set_file_transfer_success,
        snapshot_download_result, start_file_transfer_session, update_file_transfer_progress,
        validate_downloaded_file_size, wait_for_ack_or_error, wait_for_directory_listing_complete,
        wait_for_file_ready_or_ack, wait_for_file_transfer_record, wait_for_section_ready_or_ack,
    },
};
use crate::{
    core::{
        protocol_adapter::{
            build_ack_section_payload, build_file_ready_payload, build_last_section_payload, build_query_log_payload,
            build_section_ready_payload, build_segment_payload, build_select_call_payload, encode_asdu,
            file_transfer_segment_payload_max, file_transfer_single_section_max_file_size,
        },
        shared::runtime_hint::emit_master_runtime_hint,
        types::{
            AsduInfo, ConnectionInfo, DataTransferState, MasterFileTransferMode, MasterFileTransferSession,
            MasterFileTransferStage, MessageDirection,
        },
    },
    errors::{AppError, AppResult, NetworkError, ProtocolError},
    network::{DecodedFileTransferRecord, Iec104Protocol, TcpClient},
    utils::bytes_to_hex,
};

const FILE_TRANSFER_TIMEOUT_SECONDS: u64 = 10;
const FILE_TRANSFER_SINGLE_SECTION: u8 = 1;

/// 主站文件传输运行时
#[derive(Clone)]
pub(crate) struct MasterFileTransferRuntime {
    pub(crate) station_id: String,
    pub(crate) sessions: Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    pub(crate) clients: Arc<RwLock<HashMap<String, Arc<TcpClient>>>>,
    pub(crate) protocols: Arc<RwLock<HashMap<String, Arc<Mutex<Iec104Protocol>>>>>,
    pub(crate) connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    pub(crate) message_records: Arc<RwLock<VecDeque<crate::core::types::MasterMessageRecord>>>,
    pub(crate) next_message_id: Arc<AtomicU64>,
    pub(crate) runtime_event_handle: Arc<crate::services::RuntimeEventSink>,
}

/// 启动主站文件传输
///
/// 创建文件传输会话并在后台执行传输任务。
pub(crate) async fn start_master_file_transfer(
    runtime: &MasterFileTransferRuntime,
    request: MasterFileTransferRequest,
) -> AppResult<MasterFileTransferSession> {
    let mut request = request;
    let connection_id = request.connection_id.trim().to_string();
    if connection_id.is_empty() {
        return Err(AppError::Protocol(ProtocolError::InvalidData("缺少 connection_id".to_string())));
    }
    request.connection_id = connection_id.clone();

    validate_connection_state(runtime, &connection_id).await?;
    validate_file_transfer_request(&request)?;

    let snapshot = start_file_transfer_session(
        &runtime.sessions,
        &runtime.station_id,
        &connection_id,
        request.slave_id,
        request.common_address,
        request.mode,
        request.ioa,
        request.nof,
        request.local_path.clone(),
    )
    .await
    .map_err(|err| AppError::Network(NetworkError::Protocol(err)))?;

    let task_runtime = runtime.clone();
    let request_for_task = request.clone();
    let connection_id_for_task = connection_id.clone();
    tokio::spawn(async move {
        let run_result = run_master_file_transfer_task(&task_runtime, &request_for_task).await;
        if let Err(err) = run_result {
            set_file_transfer_failed(&task_runtime.sessions, &connection_id_for_task, err.clone()).await;
        }
        emit_master_runtime_hint(
            &task_runtime.runtime_event_handle,
            &task_runtime.station_id,
            Some(&connection_id_for_task),
            "file-transfer-updated",
        )
        .await;
    });

    emit_master_runtime_hint(
        &runtime.runtime_event_handle,
        &runtime.station_id,
        Some(&connection_id),
        "file-transfer-updated",
    )
    .await;
    Ok(snapshot)
}

pub(crate) async fn get_master_file_transfer_session(
    runtime: &MasterFileTransferRuntime,
    connection_id: &str,
) -> Option<MasterFileTransferSession> {
    get_file_transfer_session(&runtime.sessions, connection_id).await
}

pub(crate) async fn cancel_master_file_transfer(
    runtime: &MasterFileTransferRuntime,
    connection_id: &str,
) -> AppResult<MasterFileTransferSession> {
    let Some(snapshot) = cancel_file_transfer_session(&runtime.sessions, connection_id).await else {
        return Err(AppError::Protocol(ProtocolError::InvalidData("当前连接不存在文件传输会话".to_string())));
    };
    emit_master_runtime_hint(
        &runtime.runtime_event_handle,
        &runtime.station_id,
        Some(connection_id),
        "file-transfer-updated",
    )
    .await;
    Ok(snapshot)
}

fn validate_file_transfer_request(request: &MasterFileTransferRequest) -> AppResult<()> {
    if request.common_address == 0 {
        return Err(AppError::Protocol(ProtocolError::InvalidData("文件传输 CA 必须在 1..=65535".to_string())));
    }
    if request.ioa > 0x00FF_FFFF {
        return Err(AppError::Protocol(ProtocolError::InvalidData("文件传输 IOA 超过 24 位".to_string())));
    }
    let max_file_size = file_transfer_single_section_max_file_size(request.max_asdu_bytes).ok_or_else(|| {
        AppError::Protocol(ProtocolError::InvalidData("max_asdu_bytes 过小，无法承载 TI125".to_string()))
    })?;
    if request.max_asdu_bytes <= 13 {
        return Err(AppError::Protocol(ProtocolError::InvalidData("max_asdu_bytes 过小，无法承载 TI125".to_string())));
    }
    if matches!(
        request.mode,
        MasterFileTransferMode::Download | MasterFileTransferMode::Upload | MasterFileTransferMode::QueryLog
    ) && request.nof.is_none_or(|nof| nof == 0)
    {
        return Err(AppError::Protocol(ProtocolError::InvalidData("下载/上传/日志查询必须提供非零 NOF".to_string())));
    }
    if matches!(request.mode, MasterFileTransferMode::Download | MasterFileTransferMode::Upload)
        && request.local_path.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(AppError::Protocol(ProtocolError::InvalidData("下载/上传必须提供 local_path".to_string())));
    }

    if request.mode == MasterFileTransferMode::QueryLog {
        let start = crate::core::master::data::soe_store::decode_cp56_time2a(&request.query_range_start_time)
            .ok_or_else(|| {
                AppError::Protocol(ProtocolError::InvalidData("query_range_start_time 不是合法 CP56".to_string()))
            })?;
        let end = crate::core::master::data::soe_store::decode_cp56_time2a(&request.query_range_end_time).ok_or_else(
            || AppError::Protocol(ProtocolError::InvalidData("query_range_end_time 不是合法 CP56".to_string())),
        )?;
        if start > end {
            return Err(AppError::Protocol(ProtocolError::InvalidData("查询起始时间不能晚于结束时间".to_string())));
        }
    }

    if request.mode == MasterFileTransferMode::Upload {
        let source_path = PathBuf::from(request.local_path.as_deref().unwrap_or_default());
        let metadata = std::fs::metadata(&source_path)
            .map_err(|err| AppError::Protocol(ProtocolError::InvalidData(format!("读取上传文件失败: {}", err))))?;
        if metadata.len() > u64::from(max_file_size) {
            return Err(AppError::Protocol(ProtocolError::InvalidData(format!(
                "上传文件超过上限 {} bytes",
                max_file_size
            ))));
        }
    }
    Ok(())
}

async fn validate_connection_state(runtime: &MasterFileTransferRuntime, connection_id: &str) -> AppResult<()> {
    let guard = runtime.connections.read().await;
    let Some(conn) = guard.get(connection_id) else {
        return Err(AppError::Network(NetworkError::NotConnected));
    };
    if conn.transport_state != crate::core::types::TransportState::Connected {
        return Err(AppError::Network(NetworkError::NotConnected));
    }
    if conn.data_transfer_state != DataTransferState::Started {
        return Err(AppError::Network(NetworkError::Protocol("业务链路未启动(STARTDT)".to_string())));
    }
    Ok(())
}

async fn run_master_file_transfer_task(
    runtime: &MasterFileTransferRuntime,
    request: &MasterFileTransferRequest,
) -> Result<(), String> {
    match request.mode {
        MasterFileTransferMode::Directory => run_master_directory_flow(runtime, request).await?,
        MasterFileTransferMode::Download => run_master_download_flow(runtime, request).await?,
        MasterFileTransferMode::Upload => run_master_upload_flow(runtime, request).await?,
        MasterFileTransferMode::QueryLog => run_master_query_log_flow(runtime, request).await?,
    }

    set_file_transfer_success(&runtime.sessions, &request.connection_id).await;
    Ok(())
}

async fn run_master_directory_flow(
    runtime: &MasterFileTransferRuntime,
    request: &MasterFileTransferRequest,
) -> Result<(), String> {
    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::DirectoryListing)
        .await?;
    let asdu = AsduInfo {
        type_id: 122,
        cause: 13,
        common_address: request.common_address,
        data: build_select_call_payload(request.ioa, 0, 0x02, 0),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, asdu).await?;
    wait_for_directory_listing_complete(
        &runtime.sessions,
        &request.connection_id,
        MasterFileTransferStage::DirectoryListing,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        "目录查询超时",
    )
    .await?;
    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::Finalizing).await?;
    Ok(())
}

async fn run_master_query_log_flow(
    runtime: &MasterFileTransferRuntime,
    request: &MasterFileTransferRequest,
) -> Result<(), String> {
    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::QueryLogListing)
        .await?;
    let asdu = AsduInfo {
        type_id: 127,
        cause: 13,
        common_address: request.common_address,
        data: build_query_log_payload(
            request.ioa,
            request.nof.unwrap_or_default(),
            &request.query_range_start_time,
            &request.query_range_end_time,
        ),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, asdu).await?;
    wait_for_directory_listing_complete(
        &runtime.sessions,
        &request.connection_id,
        MasterFileTransferStage::QueryLogListing,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        "日志查询超时",
    )
    .await?;
    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::Finalizing).await?;
    Ok(())
}

async fn run_master_download_flow(
    runtime: &MasterFileTransferRuntime,
    request: &MasterFileTransferRequest,
) -> Result<(), String> {
    let nof = request.nof.ok_or_else(|| "下载缺少 NOF".to_string())?;
    let local_path = request.local_path.as_ref().ok_or_else(|| "下载缺少目标路径".to_string())?;

    set_file_transfer_stage(
        &runtime.sessions,
        &request.connection_id,
        MasterFileTransferStage::AwaitingDownloadFileReady,
    )
    .await?;
    let select_and_call = AsduInfo {
        type_id: 122,
        cause: 13,
        common_address: request.common_address,
        data: build_select_call_payload(request.ioa, nof, 0x03, 0),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, select_and_call).await?;

    let expected_length = wait_for_file_ready_or_ack(
        &runtime.sessions,
        &request.connection_id,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        nof,
    )
    .await?;
    update_file_transfer_progress(
        &runtime.sessions,
        &request.connection_id,
        Some(0),
        Some(expected_length as u64),
        Some(FILE_TRANSFER_SINGLE_SECTION),
        Some(0),
    )
    .await;

    set_file_transfer_stage(
        &runtime.sessions,
        &request.connection_id,
        MasterFileTransferStage::ReceivingDownloadSection,
    )
    .await?;
    let request_section = AsduInfo {
        type_id: 122,
        cause: 13,
        common_address: request.common_address,
        data: build_select_call_payload(request.ioa, nof, 0x20, FILE_TRANSFER_SINGLE_SECTION),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, request_section).await?;

    let section_length = wait_for_section_ready_or_ack(
        &runtime.sessions,
        &request.connection_id,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        nof,
        FILE_TRANSFER_SINGLE_SECTION,
    )
    .await?;
    if expected_length > 0 && section_length != expected_length {
        return Err(format!(
            "下载节长度与文件长度不一致: file_ready={} bytes, section_ready={} bytes",
            expected_length, section_length
        ));
    }

    let mut done = false;
    while !done {
        let record = wait_for_file_transfer_record(
            &runtime.sessions,
            &request.connection_id,
            MasterFileTransferStage::ReceivingDownloadSection,
            Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
            |record| {
                matches!(
                    record,
                    DecodedFileTransferRecord::Segment { file_name, section, .. }
                        | DecodedFileTransferRecord::LastSection {
                            file_name,
                            last_section_number: section,
                            ..
                        } if *file_name == nof && *section == FILE_TRANSFER_SINGLE_SECTION
                )
            },
        )
        .await?;

        match record {
            DecodedFileTransferRecord::Segment { data, .. } => {
                let (done_bytes, seg) = append_download_chunk(&runtime.sessions, &request.connection_id, &data).await?;
                update_file_transfer_progress(
                    &runtime.sessions,
                    &request.connection_id,
                    Some(done_bytes),
                    None,
                    Some(FILE_TRANSFER_SINGLE_SECTION),
                    Some(seg),
                )
                .await;
            }
            DecodedFileTransferRecord::LastSection { .. } => {
                done = true;
            }
            _ => {}
        }
    }

    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::Finalizing).await?;
    let (final_bytes, session_id, bytes_total) =
        snapshot_download_result(&runtime.sessions, &request.connection_id).await?;
    validate_downloaded_file_size(bytes_total, final_bytes.len())?;
    let target_path = PathBuf::from(local_path);
    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|err| format!("创建下载目录失败: {err}"))?;
    }
    let tmp_path = target_path.with_extension(format!("{session_id}.part"));
    tokio::fs::write(&tmp_path, &final_bytes).await.map_err(|err| format!("写入临时文件失败: {err}"))?;
    if tokio::fs::try_exists(&target_path).await.unwrap_or(false) {
        let _ = tokio::fs::remove_file(&target_path).await;
    }
    tokio::fs::rename(&tmp_path, &target_path).await.map_err(|err| format!("原子替换目标文件失败: {err}"))?;

    let ack = AsduInfo {
        type_id: 124,
        cause: 13,
        common_address: request.common_address,
        data: build_ack_section_payload(request.ioa, nof, 0x01, FILE_TRANSFER_SINGLE_SECTION),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, ack).await?;
    Ok(())
}

async fn run_master_upload_flow(
    runtime: &MasterFileTransferRuntime,
    request: &MasterFileTransferRequest,
) -> Result<(), String> {
    let nof = request.nof.ok_or_else(|| "上传缺少 NOF".to_string())?;
    let source_path = request.local_path.as_ref().ok_or_else(|| "上传缺少源路径".to_string())?;
    let file_bytes = tokio::fs::read(source_path).await.map_err(|err| format!("读取上传文件失败: {err}"))?;
    let max_file_size = file_transfer_single_section_max_file_size(request.max_asdu_bytes)
        .ok_or_else(|| "max_asdu_bytes 过小，无法发送 TI125 段".to_string())?;
    if file_bytes.len() > max_file_size as usize {
        return Err(format!("上传文件超过单节上限 {} bytes", max_file_size));
    }

    update_file_transfer_progress(
        &runtime.sessions,
        &request.connection_id,
        Some(0),
        Some(file_bytes.len() as u64),
        Some(FILE_TRANSFER_SINGLE_SECTION),
        Some(0),
    )
    .await;

    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::AwaitingUploadFileAck)
        .await?;
    let file_ready = AsduInfo {
        type_id: 120,
        cause: 13,
        common_address: request.common_address,
        data: build_file_ready_payload(request.ioa, nof, 0x00, file_bytes.len() as u32),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, file_ready).await?;
    wait_for_ack_or_error(
        &runtime.sessions,
        &request.connection_id,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        nof,
        FILE_TRANSFER_SINGLE_SECTION,
        0x01,
        "上传文件握手",
        MasterFileTransferStage::AwaitingUploadFileAck,
    )
    .await?;

    set_file_transfer_stage(
        &runtime.sessions,
        &request.connection_id,
        MasterFileTransferStage::AwaitingUploadSectionAck,
    )
    .await?;
    let section_ready = AsduInfo {
        type_id: 121,
        cause: 13,
        common_address: request.common_address,
        data: build_section_ready_payload(
            request.ioa,
            nof,
            0x00,
            FILE_TRANSFER_SINGLE_SECTION,
            file_bytes.len() as u32,
        ),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, section_ready).await?;
    wait_for_ack_or_error(
        &runtime.sessions,
        &request.connection_id,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        nof,
        FILE_TRANSFER_SINGLE_SECTION,
        0x07,
        "上传节准备确认",
        MasterFileTransferStage::AwaitingUploadSectionAck,
    )
    .await?;

    let segment_payload_max = file_transfer_segment_payload_max(request.max_asdu_bytes)
        .ok_or_else(|| "max_asdu_bytes 过小，无法发送 TI125 段".to_string())?;

    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::SendingUploadData)
        .await?;
    let mut segment_no = 0u32;
    for chunk in file_bytes.chunks(segment_payload_max) {
        segment_no += 1;
        let segment = AsduInfo {
            type_id: 125,
            cause: 13,
            common_address: request.common_address,
            data: build_segment_payload(request.ioa, nof, FILE_TRANSFER_SINGLE_SECTION, chunk),
            timestamp: Utc::now(),
        };
        send_file_transfer_asdu(runtime, &request.connection_id, segment).await?;
        update_file_transfer_progress(
            &runtime.sessions,
            &request.connection_id,
            Some((segment_no as usize * segment_payload_max).min(file_bytes.len()) as u64),
            None,
            Some(FILE_TRANSFER_SINGLE_SECTION),
            Some(segment_no),
        )
        .await;
    }

    set_file_transfer_stage(
        &runtime.sessions,
        &request.connection_id,
        MasterFileTransferStage::AwaitingUploadCompletionAck,
    )
    .await?;
    let last_section = AsduInfo {
        type_id: 123,
        cause: 13,
        common_address: request.common_address,
        data: build_last_section_payload(
            request.ioa,
            nof,
            0x03,
            FILE_TRANSFER_SINGLE_SECTION,
            u8::try_from(segment_no).map_err(|_| "单节段号超过 255".to_string())?,
        ),
        timestamp: Utc::now(),
    };
    send_file_transfer_asdu(runtime, &request.connection_id, last_section).await?;

    wait_for_ack_or_error(
        &runtime.sessions,
        &request.connection_id,
        Duration::from_secs(FILE_TRANSFER_TIMEOUT_SECONDS),
        nof,
        FILE_TRANSFER_SINGLE_SECTION,
        0x01,
        "上传完成确认",
        MasterFileTransferStage::AwaitingUploadCompletionAck,
    )
    .await?;
    set_file_transfer_stage(&runtime.sessions, &request.connection_id, MasterFileTransferStage::Finalizing).await?;
    update_file_transfer_progress(
        &runtime.sessions,
        &request.connection_id,
        Some(file_bytes.len() as u64),
        Some(file_bytes.len() as u64),
        Some(FILE_TRANSFER_SINGLE_SECTION),
        Some(segment_no),
    )
    .await;
    Ok(())
}

async fn send_file_transfer_asdu(
    runtime: &MasterFileTransferRuntime,
    connection_id: &str,
    asdu: AsduInfo,
) -> Result<(), String> {
    let client = runtime.clients.read().await.get(connection_id).cloned().ok_or_else(|| "连接不存在".to_string())?;
    let protocol =
        runtime.protocols.read().await.get(connection_id).cloned().ok_or_else(|| "协议上下文不存在".to_string())?;

    {
        let guard = runtime.connections.read().await;
        let Some(conn) = guard.get(connection_id) else {
            return Err("连接不存在".to_string());
        };
        if conn.data_transfer_state != DataTransferState::Started {
            return Err("业务链路未启动(STARTDT)".to_string());
        }
    }

    let asdu_bytes = encode_asdu(&asdu);
    let frame = {
        let mut protocol = protocol.lock().await;
        protocol.send_asdu(&asdu_bytes).map_err(|err| format!("构造 I 帧失败: {err}"))?
    };
    client.send(&frame).await.map_err(|err| format!("发送 I 帧失败: {err}"))?;

    if let Some(conn) = runtime.connections.write().await.get_mut(connection_id) {
        conn.tx_apdu_count = conn.tx_apdu_count.saturating_add(1);
        conn.tx_bytes = conn.tx_bytes.saturating_add(frame.len() as u64);
    }
    super::super::message_store::append_master_message(
        &runtime.message_records,
        &runtime.next_message_id,
        MessageDirection::Sent,
        connection_id,
        format!("文件传输发送 I 帧 type_id={} cot={} ca={}", asdu.type_id, asdu.cause, asdu.common_address),
        Some(bytes_to_hex(&frame)),
        Some(asdu.type_id),
        Some(asdu.cause),
        Some(asdu.common_address),
    )
    .await;
    Ok(())
}
