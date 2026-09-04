//! 从站文件仓辅助逻辑。

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use log::warn;
use serde::{Deserialize, Serialize};

/// 从站文件仓条目
///
/// 记录单个文件的元数据信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SlaveFileRepoEntry {
    /// 文件名称（NOF）
    pub(crate) nof: u16,
    /// 文件长度（字节）
    pub(crate) length: u32,
    /// 更新时间
    pub(crate) updated_at: chrono::DateTime<chrono::Utc>,
}

/// 从站文件仓索引
///
/// 包含文件仓中所有文件的元数据列表。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SlaveFileRepoIndex {
    #[serde(default)]
    pub(crate) entries: Vec<SlaveFileRepoEntry>,
}

/// 解析从站文件仓基础目录
///
/// 返回文件仓的根目录路径（可执行文件目录下的 files 子目录）。
pub(crate) fn resolve_slave_file_repo_base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("files")
}

/// 构建从站文件仓目录路径
///
/// 根据基础目录和从站 ID 构建文件仓目录路径。
pub(crate) fn build_slave_file_repo_dir(base_dir: &Path, station_id: &str) -> PathBuf {
    base_dir.join("slave").join(station_id)
}

/// 创建从站文件仓目录
///
/// 创建文件仓目录（如果不存在），返回目录路径。
pub(crate) fn create_slave_file_repo_dir(base_dir: &Path, station_id: &str) -> std::io::Result<PathBuf> {
    let repo_dir = build_slave_file_repo_dir(base_dir, station_id);
    std::fs::create_dir_all(&repo_dir)?;
    Ok(repo_dir)
}

/// 解析从站文件仓目录（异步版本）
///
/// 返回指定从站的文件仓目录路径。
pub(crate) async fn resolve_slave_file_repo_dir(
    _runtime_event_handle: &Arc<crate::services::RuntimeEventSink>,
    station_id: &str,
    _common_address: u16,
) -> PathBuf {
    build_slave_file_repo_dir(&resolve_slave_file_repo_base_dir(), station_id)
}

/// 构建文件仓文件路径
///
/// 根据文件仓目录和文件名称（NOF）构建文件路径。
pub(crate) fn slave_repo_file_path(repo_dir: &Path, nof: u16) -> PathBuf {
    repo_dir.join(format!("nof_{nof:04X}.bin"))
}

/// 构建文件仓索引文件路径
///
/// 返回文件仓索引文件（index.json）的路径。
pub(crate) fn slave_repo_index_path(repo_dir: &Path) -> PathBuf {
    repo_dir.join("index.json")
}

/// 加载文件仓索引
///
/// 从索引文件加载文件列表，如果索引不存在或过期则扫描目录重建。
pub(crate) fn load_slave_repo_index(repo_dir: &Path) -> SlaveFileRepoIndex {
    let index_path = slave_repo_index_path(repo_dir);
    let cached_index = std::fs::read_to_string(&index_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<SlaveFileRepoIndex>(&raw).ok())
        .unwrap_or_default();

    let Some(entries) = scan_slave_repo_entries(repo_dir) else {
        return cached_index;
    };
    let index = SlaveFileRepoIndex { entries };
    if index != cached_index {
        save_slave_repo_index(repo_dir, &index);
    }
    index
}

/// 扫描文件仓目录中的文件条目
///
/// 扫描目录中所有符合命名规则的文件，提取文件元数据。
fn scan_slave_repo_entries(repo_dir: &Path) -> Option<Vec<SlaveFileRepoEntry>> {
    let entries = std::fs::read_dir(repo_dir).ok()?;
    let mut repo_entries = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if !name.to_ascii_lowercase().starts_with("nof_") || !name.to_ascii_lowercase().ends_with(".bin") {
            continue;
        }
        let hex_part = &name[4..name.len() - 4];
        let Ok(nof) = u16::from_str_radix(hex_part, 16) else {
            continue;
        };
        let meta = entry.metadata().ok();
        let length = meta.as_ref().map(|item| item.len() as u32).unwrap_or(0);
        let updated_at = meta
            .and_then(|item| item.modified().ok())
            .map(chrono::DateTime::<chrono::Utc>::from)
            .unwrap_or_else(chrono::Utc::now);
        repo_entries.push(SlaveFileRepoEntry { nof, length, updated_at });
    }
    repo_entries.sort_by_key(|item| item.nof);
    Some(repo_entries)
}

/// 保存文件仓索引到磁盘
///
/// 将索引序列化为 JSON 并写入 index.json 文件。
pub(crate) fn save_slave_repo_index(repo_dir: &Path, index: &SlaveFileRepoIndex) {
    if let Err(err) = std::fs::create_dir_all(repo_dir) {
        warn!("[slave] create repo dir failed path={} error={}", repo_dir.display(), err);
        return;
    }
    let index_path = slave_repo_index_path(repo_dir);
    match serde_json::to_string_pretty(index) {
        Ok(raw) => {
            if let Err(err) = std::fs::write(&index_path, raw) {
                warn!("[slave] write repo index failed path={} error={}", index_path.display(), err);
            }
        }
        Err(err) => {
            warn!("[slave] encode repo index failed path={} error={}", index_path.display(), err);
        }
    }
}

/// 更新或插入文件仓条目
///
/// 如果条目已存在则更新长度和时间，否则插入新条目并排序。
pub(crate) fn upsert_slave_repo_entry(index: &mut SlaveFileRepoIndex, nof: u16, length: u32) {
    if let Some(entry) = index.entries.iter_mut().find(|item| item.nof == nof) {
        entry.length = length;
        entry.updated_at = chrono::Utc::now();
        return;
    }
    index.entries.push(SlaveFileRepoEntry { nof, length, updated_at: chrono::Utc::now() });
    index.entries.sort_by_key(|item| item.nof);
}

/// 解码 CP56Time2a 时间戳为 UTC 时间
///
/// 将 IEC104 标准的 7 字节时间格式转换为 chrono::DateTime。
pub(crate) fn decode_cp56_time2a_utc(raw: &[u8]) -> Option<chrono::DateTime<chrono::Utc>> {
    if raw.len() < 7 {
        return None;
    }
    if raw[..7].iter().all(|byte| *byte == 0) {
        return None;
    }

    let timestamp = iec60870_parser::parser::asdu::timestamp::CP56Time2a::parse(&raw[..7]).ok()?;
    let second = u32::from(timestamp.seconds());
    let millisecond = u32::from(timestamp.millis());
    let minute = u32::from(timestamp.minutes);
    let hour = u32::from(timestamp.hours);
    let day = u32::from(timestamp.day);
    let month = u32::from(timestamp.month);
    let year = i32::from(timestamp.year);

    let date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
    let time = chrono::NaiveTime::from_hms_milli_opt(hour, minute, second, millisecond)?;
    Some(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
        chrono::NaiveDateTime::new(date, time),
        chrono::Utc,
    ))
}

/// 根据查询日志参数过滤文件仓条目
///
/// 按文件名称（NOF）和时间范围过滤条目，用于文件目录查询（TI=126）。
pub(crate) fn filter_slave_repo_entries_by_query_log(
    entries: &[SlaveFileRepoEntry],
    file_name: u16,
    range_start_time: &[u8],
    range_end_time: &[u8],
) -> Vec<SlaveFileRepoEntry> {
    let nof_filter = if file_name == 0 { None } else { Some(file_name) };
    let start_time = decode_cp56_time2a_utc(range_start_time);
    let end_time = decode_cp56_time2a_utc(range_end_time);

    entries
        .iter()
        .filter(|entry| {
            if let Some(nof) = nof_filter {
                if entry.nof != nof {
                    return false;
                }
            }
            if let Some(start) = start_time {
                if entry.updated_at < start {
                    return false;
                }
            }
            if let Some(end) = end_time {
                if entry.updated_at > end {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}
