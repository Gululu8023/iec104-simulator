//! 抓包数据导出模块。
//!
//! 提供将抓包数据导出为 PCAP/PCAPNG 格式的功能。

use std::{collections::VecDeque, sync::Arc};

use tokio::sync::RwLock;

use crate::utils::pcap::{CaptureFormat, CapturePacket, build_pcap, build_pcapng};

/// 导出抓包数据
///
/// 根据指定格式导出抓包数据，支持按连接 ID 过滤。
pub(crate) async fn export_capture_packets(
    records: &Arc<RwLock<VecDeque<CapturePacket>>>,
    connection_id: Option<&str>,
    format: CaptureFormat,
) -> Vec<u8> {
    let packets: Vec<CapturePacket> = {
        let guard = records.read().await;
        guard
            .iter()
            .filter(|packet| connection_id.map(|id| packet.connection_id == id).unwrap_or(true))
            .cloned()
            .collect()
    };

    match format {
        CaptureFormat::Pcap => build_pcap(&packets),
        CaptureFormat::Pcapng => build_pcapng(&packets),
    }
}
