//! TCP 客户端实现(主站模式)。
//!
//! 本模块提供主站 TCP 客户端功能,包括:
//!
//! - **连接管理**:主动连接从站,维护连接状态
//! - **自动重连**:连接断开后自动重连,支持重连间隔和最大次数配置
//! - **事件通知**:连接状态变化通过事件通道通知上层
//! - **数据收发**:异步读写数据,支持超时控制
//!
//! # 使用示例
//!
//! ```rust
//! let config = NetworkConfig {
//!     remote_addr: "127.0.0.1:2404".to_string(),
//!     auto_reconnect: true,
//!     reconnect_interval_ms: 5000,
//!     max_reconnect_attempts: 10,
//!     ..Default::default()
//! };
//!
//! let client = TcpClient::new(config);
//! client.connect().await?;
//! ```

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::{
    sync::{Mutex, RwLock, mpsc},
    task::JoinHandle,
    time::{Duration, sleep},
};

use super::{
    ConnectionFactory, NETWORK_EVENT_CHANNEL_CAPACITY, NetworkConfig, NetworkError, NetworkEvent, TcpConnection,
};

/// TCP客户端
pub struct TcpClient {
    /// 配置
    config: NetworkConfig,
    /// 连接
    connection: Arc<Mutex<Option<Arc<TcpConnection>>>>,
    /// 事件接收器
    event_receiver: Arc<Mutex<Option<mpsc::Receiver<NetworkEvent>>>>,
    /// 连接工厂
    factory: ConnectionFactory,
    /// 运行状态
    running: Arc<RwLock<bool>>,
    /// 后台任务句柄
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    sent_frames: Arc<Mutex<VecDeque<(String, Vec<u8>)>>>,
    send_activity: Arc<AtomicU64>,
}

impl TcpClient {
    /// 创建新的TCP客户端
    pub fn new(config: NetworkConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(NETWORK_EVENT_CHANNEL_CAPACITY);
        let factory = ConnectionFactory::new(event_tx.clone());

        Self {
            config,
            connection: Arc::new(Mutex::new(None)),
            event_receiver: Arc::new(Mutex::new(Some(event_rx))),
            factory,
            running: Arc::new(RwLock::new(false)),
            task_handle: Arc::new(Mutex::new(None)),
            sent_frames: Arc::new(Mutex::new(VecDeque::new())),
            send_activity: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 连接到服务器
    pub async fn connect(&self, addr: &str) -> Result<(), NetworkError> {
        log::info!("客户端开始连接: addr={}", addr);

        // 检查是否已经在运行
        if *self.running.read().await {
            log::warn!("客户端已在运行，拒绝重复连接: addr={}", addr);
            return Err(NetworkError::Protocol("Client already running".to_string()));
        }

        // 尝试连接
        log::debug!("建立TCP连接: addr={}", addr);
        let connection = self.factory.connect(addr).await.map_err(|e| {
            log::error!("TCP连接失败: addr={}, error={}", addr, e);
            e
        })?;

        // 保存连接
        let connection = Arc::new(connection);
        *self.connection.lock().await = Some(Arc::clone(&connection));

        // 设置运行状态
        *self.running.write().await = true;

        // 启动后台任务
        log::debug!("启动后台任务: addr={}", addr);
        self.start_background_tasks(addr.to_string()).await;
        self.factory.emit_connected(&connection).await;

        log::info!("客户端连接成功: addr={}", addr);
        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<(), NetworkError> {
        log::info!("客户端开始断开连接");

        // 设置停止状态
        *self.running.write().await = false;
        log::debug!("设置客户端停止状态");

        // 停止后台任务
        if let Some(handle) = self.task_handle.lock().await.take() {
            log::debug!("终止后台任务");
            handle.abort();
            let _ = handle.await;
        }
        // 关闭连接
        let conn = { self.connection.lock().await.take() };
        if let Some(conn) = conn {
            log::debug!("关闭TCP连接");
            conn.close().await.map_err(|e| {
                log::error!("关闭连接失败: {}", e);
                e
            })?;
        }

        log::info!("客户端断开连接完成");
        Ok(())
    }

    /// 发送数据
    pub async fn send(&self, data: &[u8]) -> Result<(), NetworkError> {
        let connection = { self.connection.lock().await.clone() };
        match connection {
            Some(connection) => {
                let peer_addr = connection.peer_addr().await;
                connection.send(data).await?;
                self.send_activity.fetch_add(1, Ordering::Relaxed);
                let mut sent_frames = self.sent_frames.lock().await;
                sent_frames.push_back((peer_addr, data.to_vec()));
                while sent_frames.len() > 10_000 {
                    sent_frames.pop_front();
                }
                Ok(())
            }
            None => Err(NetworkError::Protocol("Not connected".to_string())),
        }
    }

    pub(crate) fn send_activity(&self) -> u64 {
        self.send_activity.load(Ordering::Relaxed)
    }

    pub(crate) fn reconnect_enabled(&self) -> bool {
        self.config.max_reconnect_attempts > 0
    }

    pub(crate) async fn take_sent_frames(&self) -> Vec<(String, Vec<u8>)> {
        self.sent_frames.lock().await.drain(..).collect()
    }

    /// 检查连接状态
    pub async fn is_connected(&self) -> bool {
        let connection = { self.connection.lock().await.clone() };
        match connection {
            Some(connection) => connection.is_connected().await,
            None => false,
        }
    }

    /// 获取唯一的事件接收器。
    pub async fn get_event_receiver(&self) -> Result<mpsc::Receiver<NetworkEvent>, NetworkError> {
        self.event_receiver
            .lock()
            .await
            .take()
            .ok_or_else(|| NetworkError::Protocol("Event receiver already taken".to_string()))
    }

    /// 使当前传输失效，由后台任务按既有策略重新连接。
    pub async fn request_reconnect(&self) -> Result<(), NetworkError> {
        let connection = self.connection.lock().await.take();
        if let Some(connection) = connection {
            connection.close().await?;
        }
        Ok(())
    }

    /// 启动后台任务
    async fn start_background_tasks(&self, addr: String) {
        let connection = Arc::clone(&self.connection);
        let running = Arc::clone(&self.running);
        let config = self.config.clone();
        let factory = self.factory.clone();

        let handle = tokio::spawn(async move {
            let mut reconnect_attempts = 0;

            while *running.read().await {
                // 检查连接状态
                let current = { connection.lock().await.clone() };
                let is_connected = match current {
                    Some(conn) => conn.is_connected().await,
                    None => false,
                };

                if !is_connected && reconnect_attempts < config.max_reconnect_attempts {
                    // 尝试重连
                    log::info!("Attempting to reconnect to {}", addr);

                    match factory.connect(&addr).await {
                        Ok(new_connection) => {
                            let new_connection = Arc::new(new_connection);
                            *connection.lock().await = Some(Arc::clone(&new_connection));
                            reconnect_attempts = 0;
                            factory.emit_connected(&new_connection).await;
                            log::info!("Reconnected to {}", addr);
                        }
                        Err(e) => {
                            reconnect_attempts += 1;
                            log::warn!("Reconnection attempt {} failed: {}", reconnect_attempts, e);
                            let event = NetworkEvent::ReconnectAttemptFailed {
                                addr: addr.clone(),
                                attempt: reconnect_attempts,
                                max_attempts: config.max_reconnect_attempts,
                                error: e.to_string(),
                            };
                            factory.emit(event).await;
                            if reconnect_attempts >= config.max_reconnect_attempts {
                                let exhausted_event = NetworkEvent::ReconnectExhausted {
                                    addr: addr.clone(),
                                    attempts: reconnect_attempts,
                                    max_attempts: config.max_reconnect_attempts,
                                    last_error: e.to_string(),
                                };
                                factory.emit(exhausted_event).await;
                                break;
                            }
                            sleep(Duration::from_millis(config.reconnect_interval)).await;
                        }
                    }
                } else if !is_connected {
                    log::error!("Max reconnection attempts reached for {}", addr);
                    break;
                }

                // 处理接收数据
                let current = { connection.lock().await.clone() };
                if let Some(conn) = current {
                    match conn.receive().await {
                        Ok(Some(_data)) => {
                            // 数据已通过事件系统处理
                        }
                        Ok(None) => {
                            // 没有数据，继续
                        }
                        Err(NetworkError::ConnectionClosed) => {
                            log::info!("Connection closed by peer");
                            *connection.lock().await = None;
                        }
                        Err(e) => {
                            log::error!("Receive error: {}", e);
                            *connection.lock().await = None;
                        }
                    }
                }

                // 短暂休眠
                sleep(Duration::from_millis(100)).await;
            }
        });

        *self.task_handle.lock().await = Some(handle);
    }
}

impl Drop for TcpClient {
    fn drop(&mut self) {
        // 确保在析构时停止后台任务
        let running = Arc::clone(&self.running);
        let task_handle = Arc::clone(&self.task_handle);

        tokio::spawn(async move {
            *running.write().await = false;
            if let Some(handle) = task_handle.lock().await.take() {
                handle.abort();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn event_receiver_can_only_be_taken_once() {
        let client = TcpClient::new(NetworkConfig::default());
        assert!(client.get_event_receiver().await.is_ok());
        assert!(client.get_event_receiver().await.is_err());
    }

    #[tokio::test]
    async fn bounded_event_delivery_applies_backpressure_without_reordering() {
        let (sender, mut receiver) = mpsc::channel(1);
        let factory = ConnectionFactory::new(sender);
        factory.emit(NetworkEvent::Error { peer_addr: None, error: "first".to_string() }).await;

        let second_factory = factory.clone();
        let blocked = tokio::spawn(async move {
            second_factory.emit(NetworkEvent::Error { peer_addr: None, error: "second".to_string() }).await;
        });
        tokio::task::yield_now().await;
        assert!(!blocked.is_finished());

        let first = receiver.recv().await.expect("first event");
        blocked.await.expect("second event sender");
        let second = receiver.recv().await.expect("second event");
        assert!(matches!(first, NetworkEvent::Error { error, .. } if error == "first"));
        assert!(matches!(second, NetworkEvent::Error { error, .. } if error == "second"));
    }
}
