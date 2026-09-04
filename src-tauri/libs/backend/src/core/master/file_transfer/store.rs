//! 主站文件传输存储模块。
//!
//! 本模块负责文件传输会话的状态存储和管理,包括会话创建、状态更新、数据缓存等。

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::Utc;
use iec60870_parser::parser::asdu::CotReason;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    core::{
        master::data::soe_store::decode_cp56_time2a,
        types::{
            MasterFileTransferMode, MasterFileTransferSession, MasterFileTransferStage, MasterFileTransferState,
            MasterRemoteDirectoryEntry,
        },
    },
    network::{DecodedAsdu, DecodedAsduBody, DecodedFileTransferRecord},
};

const FILE_TRANSFER_MAX_INBOX_RECORDS: usize = 1024;
const FILE_TRANSFER_MAX_DIRECTORY_ENTRIES: usize = 4096;

/// 主站文件传输会话运行时
#[derive(Debug, Clone)]
pub(crate) struct MasterFileTransferSessionRuntime {
    pub(crate) snapshot: MasterFileTransferSession,
    pub(crate) stage: MasterFileTransferStage,
    pub(crate) stage_epoch: u64,
    inbox: VecDeque<MasterFileTransferInboxRecord>,
    pub(crate) cancel_requested: bool,
    pub(crate) download_buffer: Vec<u8>,
}

#[derive(Debug, Clone)]
struct MasterFileTransferInboxRecord {
    stage: MasterFileTransferStage,
    stage_epoch: u64,
    record: DecodedFileTransferRecord,
}

/// 启动文件传输会话
///
/// 创建新的文件传输会话并初始化运行时状态。
pub(crate) async fn start_file_transfer_session(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    station_id: &str,
    connection_id: &str,
    slave_id: i64,
    common_address: u16,
    mode: MasterFileTransferMode,
    ioa: u32,
    nof: Option<u16>,
    local_path: Option<String>,
) -> Result<MasterFileTransferSession, String> {
    let mut guard = sessions.write().await;
    if let Some(existing) = guard.get(connection_id) {
        if existing.snapshot.state == MasterFileTransferState::Running {
            return Err("当前连接已有运行中的文件传输会话".to_string());
        }
    }

    let now = Utc::now();
    let runtime = MasterFileTransferSessionRuntime {
        snapshot: MasterFileTransferSession {
            session_id: Uuid::new_v4().to_string(),
            station_id: station_id.to_string(),
            connection_id: connection_id.to_string(),
            slave_id,
            common_address,
            mode,
            state: MasterFileTransferState::Running,
            ioa,
            nof,
            local_path,
            bytes_total: 0,
            bytes_done: 0,
            current_section: 0,
            current_segment: 0,
            failure_reason: None,
            directory_entries: Vec::new(),
            created_at: now,
            updated_at: now,
        },
        stage: MasterFileTransferStage::Preparing,
        stage_epoch: 0,
        inbox: VecDeque::new(),
        cancel_requested: false,
        download_buffer: Vec::new(),
    };
    let snapshot = runtime.snapshot.clone();
    guard.insert(connection_id.to_string(), runtime);
    Ok(snapshot)
}

pub(crate) async fn get_file_transfer_session(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
) -> Option<MasterFileTransferSession> {
    sessions.read().await.get(connection_id).map(|session| session.snapshot.clone())
}

pub(crate) async fn set_file_transfer_stage(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    stage: MasterFileTransferStage,
) -> Result<(), String> {
    let mut guard = sessions.write().await;
    let session = guard.get_mut(connection_id).ok_or_else(|| "文件传输会话不存在".to_string())?;
    if session.snapshot.state != MasterFileTransferState::Running {
        return Err("文件传输会话已结束".to_string());
    }
    session.stage = stage;
    session.stage_epoch = session.stage_epoch.wrapping_add(1);
    session.inbox.clear();
    Ok(())
}

pub(crate) async fn cancel_file_transfer_session(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
) -> Option<MasterFileTransferSession> {
    let mut guard = sessions.write().await;
    let session = guard.get_mut(connection_id)?;
    if session.snapshot.state != MasterFileTransferState::Running {
        return Some(session.snapshot.clone());
    }
    session.cancel_requested = true;
    session.snapshot.state = MasterFileTransferState::Cancelled;
    session.snapshot.failure_reason = Some("用户取消".to_string());
    session.snapshot.updated_at = Utc::now();
    finish_file_transfer_runtime(session);
    Some(session.snapshot.clone())
}

pub(crate) async fn handle_file_transfer_incoming(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    decoded_asdu: &DecodedAsdu,
) -> bool {
    let DecodedAsduBody::FileTransfers(records) = &decoded_asdu.body else {
        return false;
    };

    let mut guard = sessions.write().await;
    let Some(session) = guard.get_mut(connection_id) else {
        return false;
    };
    if session.snapshot.state != MasterFileTransferState::Running
        || decoded_asdu.cause as u8 != CotReason::FileTransfer.as_u8()
    {
        return false;
    }

    let now = Utc::now();
    let mut updated = false;
    let mut matched = false;
    for record in records {
        if !file_transfer_record_matches_session(&session.snapshot, session.stage, decoded_asdu.common_address, record)
        {
            continue;
        }
        matched = true;
        if session.inbox.len() >= FILE_TRANSFER_MAX_INBOX_RECORDS {
            session.snapshot.state = MasterFileTransferState::Failed;
            session.snapshot.failure_reason = Some("文件传输 inbox 超过记录上限".to_string());
            finish_file_transfer_runtime(session);
            break;
        }
        session.inbox.push_back(MasterFileTransferInboxRecord {
            stage: session.stage,
            stage_epoch: session.stage_epoch,
            record: record.clone(),
        });
        if let DecodedFileTransferRecord::Directory {
            ioa,
            file_name,
            length,
            status_raw,
            is_file,
            is_last_file_of_directory,
            timestamp,
            ..
        } = record
        {
            if !should_store_directory_entry(*file_name, *length, *is_file) {
                continue;
            }
            if session.snapshot.directory_entries.len() >= FILE_TRANSFER_MAX_DIRECTORY_ENTRIES {
                session.snapshot.state = MasterFileTransferState::Failed;
                session.snapshot.failure_reason = Some("远端目录条目超过上限".to_string());
                finish_file_transfer_runtime(session);
                break;
            }
            let ts = timestamp.as_ref().and_then(|raw| decode_cp56_time2a(raw.as_slice()));
            session.snapshot.directory_entries.push(MasterRemoteDirectoryEntry {
                ioa: *ioa,
                nof: *file_name,
                length: *length,
                status_raw: *status_raw,
                is_file: *is_file,
                is_last: *is_last_file_of_directory,
                timestamp: ts,
            });
            updated = true;
        }
    }
    if matched {
        session.snapshot.updated_at = now;
    }
    updated || matched
}

fn file_transfer_record_matches_session(
    session: &MasterFileTransferSession,
    stage: MasterFileTransferStage,
    common_address: u16,
    record: &DecodedFileTransferRecord,
) -> bool {
    if session.common_address != common_address || !file_transfer_stage_accepts_record(session.mode, stage, record) {
        return false;
    }
    let (ioa, nof, section) = match record {
        DecodedFileTransferRecord::FileReady { ioa, file_name, .. }
        | DecodedFileTransferRecord::SectionReady { ioa, file_name, .. }
        | DecodedFileTransferRecord::SelectCall { ioa, file_name, .. }
        | DecodedFileTransferRecord::LastSection { ioa, file_name, .. }
        | DecodedFileTransferRecord::AckSection { ioa, file_name, .. }
        | DecodedFileTransferRecord::Segment { ioa, file_name, .. }
        | DecodedFileTransferRecord::Directory { ioa, file_name, .. }
        | DecodedFileTransferRecord::QueryLog { ioa, file_name, .. } => (*ioa, *file_name, match record {
            DecodedFileTransferRecord::SectionReady { section, .. }
            | DecodedFileTransferRecord::SelectCall { section, .. }
            | DecodedFileTransferRecord::AckSection { section, .. }
            | DecodedFileTransferRecord::Segment { section, .. } => Some(*section),
            DecodedFileTransferRecord::LastSection { last_section_number, .. } => Some(*last_section_number),
            _ => None,
        }),
        DecodedFileTransferRecord::Raw(_) => return false,
    };
    if ioa != session.ioa {
        return false;
    }
    if !matches!(session.mode, MasterFileTransferMode::Directory | MasterFileTransferMode::QueryLog)
        && session.nof != Some(nof)
    {
        return false;
    }
    section.is_none_or(|section| session.current_section == section)
}

fn file_transfer_stage_accepts_record(
    mode: MasterFileTransferMode,
    stage: MasterFileTransferStage,
    record: &DecodedFileTransferRecord,
) -> bool {
    match (mode, stage) {
        (MasterFileTransferMode::Directory, MasterFileTransferStage::DirectoryListing)
        | (MasterFileTransferMode::QueryLog, MasterFileTransferStage::QueryLogListing) => {
            matches!(record, DecodedFileTransferRecord::Directory { .. })
        }
        (MasterFileTransferMode::Download, MasterFileTransferStage::AwaitingDownloadFileReady) => {
            matches!(record, DecodedFileTransferRecord::FileReady { .. } | DecodedFileTransferRecord::AckSection { .. })
        }
        (MasterFileTransferMode::Download, MasterFileTransferStage::ReceivingDownloadSection) => matches!(
            record,
            DecodedFileTransferRecord::SectionReady { .. }
                | DecodedFileTransferRecord::AckSection { .. }
                | DecodedFileTransferRecord::Segment { .. }
                | DecodedFileTransferRecord::LastSection { .. }
        ),
        (MasterFileTransferMode::Upload, MasterFileTransferStage::AwaitingUploadFileAck)
        | (MasterFileTransferMode::Upload, MasterFileTransferStage::AwaitingUploadSectionAck)
        | (MasterFileTransferMode::Upload, MasterFileTransferStage::AwaitingUploadCompletionAck) => {
            matches!(record, DecodedFileTransferRecord::AckSection { .. })
        }
        _ => false,
    }
}

fn finish_file_transfer_runtime(session: &mut MasterFileTransferSessionRuntime) {
    session.stage = MasterFileTransferStage::Finished;
    session.stage_epoch = session.stage_epoch.wrapping_add(1);
    session.inbox.clear();
}

pub(crate) fn validate_downloaded_file_size(expected_length: u64, actual_length: usize) -> Result<(), String> {
    if expected_length == actual_length as u64 {
        return Ok(());
    }
    Err(format!("下载文件长度不匹配: expected={} bytes, got={} bytes", expected_length, actual_length))
}

pub(crate) async fn set_file_transfer_failed(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    reason: String,
) {
    let mut guard = sessions.write().await;
    let Some(session) = guard.get_mut(connection_id) else {
        return;
    };
    if session.snapshot.state != MasterFileTransferState::Running {
        return;
    }
    session.snapshot.state = MasterFileTransferState::Failed;
    session.snapshot.failure_reason = Some(reason);
    session.snapshot.updated_at = Utc::now();
    finish_file_transfer_runtime(session);
}

pub(crate) async fn set_file_transfer_success(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
) {
    let mut guard = sessions.write().await;
    let Some(session) = guard.get_mut(connection_id) else {
        return;
    };
    if session.snapshot.state != MasterFileTransferState::Running {
        return;
    }
    session.snapshot.state = MasterFileTransferState::Success;
    session.snapshot.failure_reason = None;
    session.snapshot.updated_at = Utc::now();
    finish_file_transfer_runtime(session);
}

pub(crate) async fn update_file_transfer_progress(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    bytes_done: Option<u64>,
    bytes_total: Option<u64>,
    section: Option<u8>,
    segment: Option<u32>,
) {
    let mut guard = sessions.write().await;
    let Some(session) = guard.get_mut(connection_id) else {
        return;
    };
    if session.snapshot.state != MasterFileTransferState::Running {
        return;
    }
    if let Some(v) = bytes_done {
        session.snapshot.bytes_done = v;
    }
    if let Some(v) = bytes_total {
        session.snapshot.bytes_total = v;
    }
    if let Some(v) = section {
        session.snapshot.current_section = v;
    }
    if let Some(v) = segment {
        session.snapshot.current_segment = v;
    }
    session.snapshot.updated_at = Utc::now();
}

pub(crate) async fn wait_for_file_transfer_record<F>(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    expected_stage: MasterFileTransferStage,
    timeout: Duration,
    mut matcher: F,
) -> Result<DecodedFileTransferRecord, String>
where
    F: FnMut(&DecodedFileTransferRecord) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        {
            let mut guard = sessions.write().await;
            let Some(session) = guard.get_mut(connection_id) else {
                return Err("文件传输会话不存在".to_string());
            };
            if session.cancel_requested || session.snapshot.state == MasterFileTransferState::Cancelled {
                return Err("用户取消".to_string());
            }
            if session.snapshot.state == MasterFileTransferState::Failed {
                return Err(session.snapshot.failure_reason.clone().unwrap_or_else(|| "文件传输失败".to_string()));
            }
            if session.snapshot.state != MasterFileTransferState::Running {
                return Err("文件传输会话已结束".to_string());
            }
            if session.stage != expected_stage {
                return Err("文件传输会话阶段已变化".to_string());
            }

            let stage_epoch = session.stage_epoch;
            if let Some(index) = session.inbox.iter().position(|item| {
                item.stage == expected_stage && item.stage_epoch == stage_epoch && matcher(&item.record)
            }) {
                if let Some(item) = session.inbox.remove(index) {
                    session.snapshot.updated_at = Utc::now();
                    return Ok(item.record);
                }
            }
        }

        if Instant::now() >= deadline {
            return Err("等待文件传输响应超时".to_string());
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}

pub(crate) async fn wait_for_file_ready_or_ack(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    timeout: Duration,
    file_name: u16,
) -> Result<u32, String> {
    let record = wait_for_file_transfer_record(
        sessions,
        connection_id,
        MasterFileTransferStage::AwaitingDownloadFileReady,
        timeout,
        |record| {
            matches!(
                record,
                DecodedFileTransferRecord::FileReady {
                    file_name: current_file_name,
                    ..
                } if *current_file_name == file_name
            ) || matches!(
                record,
                DecodedFileTransferRecord::AckSection {
                    file_name: current_file_name,
                    ..
                } if *current_file_name == file_name
            )
        },
    )
    .await?;

    match record {
        DecodedFileTransferRecord::FileReady { length, .. } => Ok(length),
        DecodedFileTransferRecord::AckSection { qualifier, .. } => {
            Err(format_file_transfer_ack_error("下载文件准备", qualifier))
        }
        _ => unreachable!("matcher only returns file-ready or ack-section"),
    }
}

pub(crate) async fn wait_for_section_ready_or_ack(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    timeout: Duration,
    file_name: u16,
    section: u8,
) -> Result<u32, String> {
    let record = wait_for_file_transfer_record(
        sessions,
        connection_id,
        MasterFileTransferStage::ReceivingDownloadSection,
        timeout,
        |record| {
            matches!(
                record,
                DecodedFileTransferRecord::SectionReady {
                    file_name: current_file_name,
                    section: current_section,
                    ..
                } if *current_file_name == file_name && *current_section == section
            ) || matches!(
                record,
                DecodedFileTransferRecord::AckSection {
                    file_name: current_file_name,
                    section: current_section,
                    ..
                } if *current_file_name == file_name && *current_section == section
            )
        },
    )
    .await?;

    match record {
        DecodedFileTransferRecord::SectionReady { length, .. } => Ok(length),
        DecodedFileTransferRecord::AckSection { qualifier, .. } => {
            Err(format_file_transfer_ack_error("请求文件节", qualifier))
        }
        _ => unreachable!("matcher only returns section-ready or ack-section"),
    }
}

pub(crate) async fn wait_for_ack_or_error(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    timeout: Duration,
    file_name: u16,
    section: u8,
    success_mask: u8,
    context: &str,
    expected_stage: MasterFileTransferStage,
) -> Result<(), String> {
    let record = wait_for_file_transfer_record(sessions, connection_id, expected_stage, timeout, |record| {
        matches!(
            record,
            DecodedFileTransferRecord::AckSection {
                file_name: current_file_name,
                section: current_section,
                ..
            } if *current_file_name == file_name && *current_section == section
        )
    })
    .await?;

    match record {
        DecodedFileTransferRecord::AckSection { qualifier, .. } if qualifier & success_mask != 0 => Ok(()),
        DecodedFileTransferRecord::AckSection { qualifier, .. } => {
            Err(format_file_transfer_ack_error(context, qualifier))
        }
        _ => unreachable!("matcher only returns ack-section"),
    }
}

pub(crate) async fn wait_for_directory_listing_complete(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    expected_stage: MasterFileTransferStage,
    timeout: Duration,
    timeout_message: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut got_last = false;
        {
            let mut guard = sessions.write().await;
            let Some(session) = guard.get_mut(connection_id) else {
                return Err("文件传输会话不存在".to_string());
            };
            if session.cancel_requested || session.snapshot.state == MasterFileTransferState::Cancelled {
                return Err("用户取消".to_string());
            }
            if session.snapshot.state == MasterFileTransferState::Failed {
                return Err(session.snapshot.failure_reason.clone().unwrap_or_else(|| "文件传输失败".to_string()));
            }
            if session.snapshot.state != MasterFileTransferState::Running {
                return Err("文件传输会话已结束".to_string());
            }
            if session.stage != expected_stage {
                return Err("文件传输会话阶段已变化".to_string());
            }

            let stage_epoch = session.stage_epoch;
            while session
                .inbox
                .front()
                .is_some_and(|item| item.stage == expected_stage && item.stage_epoch == stage_epoch)
            {
                let item = session.inbox.pop_front().expect("front checked above");
                if let DecodedFileTransferRecord::Directory { is_last_file_of_directory, .. } = item.record {
                    got_last = got_last || is_last_file_of_directory;
                }
            }
            if got_last {
                session.snapshot.updated_at = Utc::now();
            }
        }

        if got_last {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(timeout_message.to_string());
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

pub(crate) async fn append_download_chunk(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
    data: &[u8],
) -> Result<(u64, u32), String> {
    let mut guard = sessions.write().await;
    let Some(session) = guard.get_mut(connection_id) else {
        return Err("文件传输会话不存在".to_string());
    };
    if session.snapshot.state != MasterFileTransferState::Running
        || session.stage != MasterFileTransferStage::ReceivingDownloadSection
    {
        return Err("文件传输会话不在下载数据阶段".to_string());
    }
    let next_len =
        session.download_buffer.len().checked_add(data.len()).ok_or_else(|| "下载缓冲区长度溢出".to_string())?;
    if next_len as u64 > session.snapshot.bytes_total {
        return Err(format!("下载数据超过配额: expected={} next={}", session.snapshot.bytes_total, next_len));
    }
    let next_segment = session
        .snapshot
        .current_segment
        .checked_add(1)
        .filter(|segment| *segment <= u8::MAX as u32)
        .ok_or_else(|| "下载段号超过单节可表达范围".to_string())?;
    session.download_buffer.extend_from_slice(data);
    session.snapshot.bytes_done = session.download_buffer.len() as u64;
    session.snapshot.current_segment = next_segment;
    session.snapshot.updated_at = Utc::now();
    Ok((session.snapshot.bytes_done, session.snapshot.current_segment))
}

pub(crate) async fn snapshot_download_result(
    sessions: &Arc<RwLock<HashMap<String, MasterFileTransferSessionRuntime>>>,
    connection_id: &str,
) -> Result<(Vec<u8>, String, u64), String> {
    let guard = sessions.read().await;
    let Some(session) = guard.get(connection_id) else {
        return Err("文件传输会话不存在".to_string());
    };
    if session.snapshot.state != MasterFileTransferState::Running
        || session.stage != MasterFileTransferStage::Finalizing
    {
        return Err("文件传输会话不在下载收尾阶段".to_string());
    }
    Ok((session.download_buffer.clone(), session.snapshot.session_id.clone(), session.snapshot.bytes_total))
}

fn should_store_directory_entry(file_name: u16, length: u32, is_file: bool) -> bool {
    is_file || file_name != 0 || length != 0
}

fn format_file_transfer_ack_error(context: &str, qualifier: u8) -> String {
    let detail = match qualifier {
        0x02 => "从站返回否定确认，文件不可用或长度异常",
        0x08 => "从站返回否定确认，节请求无效或会话状态异常",
        _ => "从站返回未知确认",
    };
    format!("{context}失败: {detail} (AFQ=0x{qualifier:02X})")
}
