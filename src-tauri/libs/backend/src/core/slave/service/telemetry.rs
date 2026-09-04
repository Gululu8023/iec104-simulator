use super::SlaveService;
use crate::{
    core::{
        shared::capture_export::export_capture_packets,
        slave::data::message_store::reset_slave_message_tracking_state,
        types::{BackendSoeEvent, ConnectionInfo, DataPoint, SlaveMessageRecord},
    },
    errors::AppResult,
    utils::pcap::CaptureFormat,
};

impl SlaveService {
    /// 启用或禁用报文与抓包记录；禁用时同时清空当前跟踪会话。
    pub async fn set_message_tracking_enabled(&self, enabled: bool) {
        reset_slave_message_tracking_state(
            &self.telemetry.message_tracking_guard,
            &self.telemetry.message_tracking_enabled,
            enabled,
            &self.telemetry.message_records,
            &self.telemetry.capture_packets,
            &self.telemetry.next_message_id,
        )
        .await;
    }

    /// 获取所有连接信息
    ///
    /// 返回当前所有已连接主站的连接信息列表。
    ///
    /// # 返回
    ///
    /// 返回 ConnectionInfo 列表，每个元素包含：
    /// - `peer_addr` - 对端地址
    /// - `transport_state` - 传输状态
    /// - `data_transfer_state` - 数据传输状态
    /// - `connected_at` - 连接时间
    /// - `rx_bytes` - 接收字节数
    /// - `tx_bytes` - 发送字节数
    /// - `rx_apdu_count` - 接收 APDU 计数
    /// - `tx_apdu_count` - 发送 APDU 计数
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_connections(&self) -> Vec<ConnectionInfo> {
        self.transport.connections.read().await.values().cloned().collect()
    }

    pub async fn get_station_time_offset_ms(&self) -> i64 {
        *self.transport.station_time_offset_ms.read().await
    }

    /// 获取所有数据点
    ///
    /// 返回当前所有数据点的列表。
    ///
    /// # 返回
    ///
    /// 返回 DataPoint 列表，每个元素包含：
    /// - `address` - 信息对象地址（IOA）
    /// - `common_address` - 公共地址
    /// - `type_id` - 类型标识
    /// - `value` - 当前值
    /// - `quality` - 质量描述符
    /// - `timestamp` - 时间戳
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub(crate) async fn sync_data_points(
        &self,
        cursor: Option<crate::core::types::PointSyncCursor>,
    ) -> crate::core::shared::versioned_point_store::PointStoreSync<
        crate::core::slave::simulation::engine::PointKey,
        DataPoint,
    > {
        self.points.data_points.sync_since(cursor).await
    }

    pub(crate) async fn point_cursor(&self) -> crate::core::types::PointSyncCursor {
        self.points.data_points.cursor().await
    }

    /// 获取消息记录
    ///
    /// 返回从站的消息记录列表，支持增量查询和分页。
    ///
    /// # 参数
    ///
    /// * `since_id` - 起始消息 ID（可选），只返回 ID 大于此值的消息
    /// * `limit` - 最大返回数量（1-1000，超出范围会被限制）
    ///
    /// # 返回
    ///
    /// 返回 SlaveMessageRecord 列表，每个元素包含：
    /// - `id` - 消息 ID
    /// - `timestamp` - 时间戳
    /// - `direction` - 消息方向（Sent/Received/Error）
    /// - `peer_addr` - 对端地址
    /// - `content` - 消息内容
    /// - `raw_hex` - 原始十六进制数据
    ///
    /// # 使用场景
    ///
    /// 前端轮询调用，获取最新的消息记录用于显示报文日志。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_messages(&self, since_id: Option<u64>, limit: usize) -> Vec<SlaveMessageRecord> {
        let bounded_limit = limit.clamp(1, 1000);
        let messages = self.telemetry.message_records.read().await;
        messages
            .iter()
            .filter(|item| since_id.map(|id| item.id > id).unwrap_or(true))
            .take(bounded_limit)
            .cloned()
            .collect()
    }

    /// 获取 SOE 事件记录
    ///
    /// 返回从站的 SOE（Sequence of Events）事件记录列表，支持增量查询和分页。
    ///
    /// # 参数
    ///
    /// * `since_id` - 起始事件 ID（可选），只返回 ID 大于此值的事件
    /// * `limit` - 最大返回数量（1-1000，超出范围会被限制）
    ///
    /// # 返回
    ///
    /// 返回 BackendSoeEvent 列表，每个元素包含：
    /// - `id` - 事件 ID
    /// - `logged_at` - 记录时间
    /// - `event_timestamp` - 事件时标
    /// - `point_name` - 数据点名称
    /// - `common_address` - 公共地址
    /// - `ioa` - 信息对象地址
    /// - `type_id` - 类型标识
    /// - `value` - 事件值
    /// - `quality` - 质量描述符
    ///
    /// # 返回策略
    ///
    /// 如果过滤后的事件数量超过 limit，返回最新的 limit 条记录（删除最旧的记录）。
    ///
    /// # 使用场景
    ///
    /// 前端轮询调用，获取最新的 SOE 事件用于显示事件序列。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn get_soe_events(&self, since_id: Option<u64>, limit: usize) -> Vec<BackendSoeEvent> {
        let bounded_limit = limit.clamp(1, 1000);
        let records = self.telemetry.soe_records.read().await;
        records
            .iter()
            .filter(|item| since_id.map(|id| item.id > id).unwrap_or(true))
            .take(bounded_limit)
            .cloned()
            .collect()
    }

    /// 清空 SOE 事件记录
    ///
    /// 清空所有已记录的 SOE 事件。
    ///
    /// # 使用场景
    ///
    /// 用户手动清空 SOE 事件记录，或在从站重启前清理旧数据。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 写锁访问共享状态，支持并发调用。
    pub async fn clear_soe_events(&self) {
        self.telemetry.soe_records.write().await.clear();
    }

    /// 导出 PCAP 抓包数据
    ///
    /// 将已记录的 PCAP 抓包数据导出为指定格式。
    ///
    /// # 参数
    ///
    /// * `connection_id` - 连接 ID（可选），如果指定则只导出该连接的抓包数据
    /// * `format` - 导出格式（Pcap/PcapNg）
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<u8>)` - 导出成功，返回二进制数据
    /// - `Err(AppError)` - 导出失败
    ///
    /// # 使用场景
    ///
    /// 用户导出抓包数据用于 Wireshark 分析或存档。
    ///
    /// # 线程安全
    ///
    /// 本方法通过 RwLock 读锁访问共享状态，支持并发调用。
    pub async fn export_capture(&self, connection_id: Option<&str>, format: CaptureFormat) -> AppResult<Vec<u8>> {
        Ok(export_capture_packets(&self.telemetry.capture_packets, connection_id, format).await)
    }
}
