//! TCP 连接抽象层。
//!
//! 本模块提供 TCP 连接的统一抽象,封装底层 tokio TcpStream,包括:
//!
//! - **读写分离**:使用 OwnedReadHalf 和 OwnedWriteHalf 实现并发读写
//! - **状态管理**:跟踪连接状态(连接中、已连接、断开、错误)
//! - **超时控制**:读写操作支持超时设置
//! - **事件发送**:连接状态变化自动发送事件
//!
//! # 设计特点
//!
//! - **线程安全**:使用 Mutex 保护共享状态
//! - **异步 I/O**:基于 tokio 异步运行时
//! - **错误处理**:统一的错误类型和传播机制
//!
//! # 使用示例
//!
//! ```rust
//! let connection = TcpConnection::new(stream, event_sender)?;
//! let data = connection.read(1024, Duration::from_secs(5)).await?;
//! connection.write(&data).await?;
//! ```

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::{Mutex, mpsc},
    time::{Duration, timeout},
};

use super::{ConnectionInfo, ConnectionState, NetworkError, NetworkEvent};

/// TCP连接包装器
pub struct TcpConnection {
    /// 连接信息
    info: Mutex<ConnectionInfo>,
    /// 读半部
    reader: Mutex<OwnedReadHalf>,
    /// 写半部
    writer: Mutex<OwnedWriteHalf>,
    /// 事件发送器
    event_sender: mpsc::Sender<NetworkEvent>,
}

impl TcpConnection {
    /// 创建新的TCP连接
    pub fn new(stream: TcpStream, event_sender: mpsc::Sender<NetworkEvent>) -> Result<Self, NetworkError> {
        let local_addr = stream.local_addr()?.to_string();
        let peer_addr = stream.peer_addr()?.to_string();
        let id = format!("{}_{}", peer_addr, chrono::Utc::now().timestamp());

        let (reader, writer) = stream.into_split();

        let mut info = ConnectionInfo::new(id, local_addr, peer_addr.clone());
        info.set_connected();

        Ok(Self { info: Mutex::new(info), reader: Mutex::new(reader), writer: Mutex::new(writer), event_sender })
    }

    /// 获取连接信息
    pub async fn get_info(&self) -> ConnectionInfo {
        self.info.lock().await.clone()
    }

    /// 发送数据
    pub async fn send(&self, data: &[u8]) -> Result<(), NetworkError> {
        let peer_addr = { self.info.lock().await.peer_addr.clone() };
        log::debug!("发送数据: peer={}, size={} bytes", peer_addr, data.len());

        let mut writer = self.writer.lock().await;

        // 写入数据并立即刷新缓冲区
        let write_result = timeout(Duration::from_millis(5000), async {
            writer.write_all(data).await?;
            writer.flush().await
        })
        .await;

        match write_result {
            Ok(Ok(())) => {
                {
                    let mut info = self.info.lock().await;
                    info.add_bytes_sent(data.len() as u64);
                    log::debug!(
                        "数据发送成功: peer={}, size={} bytes, total_sent={}",
                        info.peer_addr,
                        data.len(),
                        info.bytes_sent
                    );
                }

                Ok(())
            }
            Ok(Err(e)) => {
                {
                    let mut info = self.info.lock().await;
                    log::error!("数据发送失败: peer={}, error={}", info.peer_addr, e);
                    info.state = ConnectionState::Error(e.to_string());
                }
                Err(NetworkError::Io(e))
            }
            Err(_) => {
                {
                    let mut info = self.info.lock().await;
                    log::error!("数据发送超时: peer={}, timeout=5s", info.peer_addr);
                    info.state = ConnectionState::Error("Write timeout".to_string());
                }
                Err(NetworkError::Timeout)
            }
        }
    }

    /// 接收数据
    pub async fn receive(&self) -> Result<Option<Vec<u8>>, NetworkError> {
        let mut reader = self.reader.lock().await;

        // 准备缓冲区
        let mut buffer = [0u8; 1024];

        // 读取数据
        match timeout(Duration::from_millis(1000), reader.read(&mut buffer)).await {
            Ok(Ok(0)) => {
                // 连接关闭
                let peer_addr = {
                    let mut info = self.info.lock().await;
                    info.set_disconnected();
                    info.peer_addr.clone()
                };
                let _ = self
                    .event_sender
                    .send(NetworkEvent::Disconnected { peer_addr, reason: "Connection closed by peer".to_string() })
                    .await;
                Err(NetworkError::ConnectionClosed)
            }
            Ok(Ok(n)) => {
                // 收到数据
                let data = buffer[..n].to_vec();

                let peer_addr = {
                    let mut info = self.info.lock().await;
                    info.add_bytes_received(n as u64);
                    info.peer_addr.clone()
                };

                // 发送数据接收事件
                let _ = self.event_sender.send(NetworkEvent::DataReceived { peer_addr, data: data.clone() }).await;

                Ok(Some(data))
            }
            Ok(Err(e)) => {
                let reason = e.to_string();
                let peer_addr = {
                    let mut info = self.info.lock().await;
                    info.state = ConnectionState::Error(reason.clone());
                    info.peer_addr.clone()
                };

                let _ = self
                    .event_sender
                    .send(NetworkEvent::Disconnected { peer_addr, reason: format!("Receive error: {reason}") })
                    .await;
                Err(NetworkError::Io(e))
            }
            Err(_) => {
                // 超时，返回None表示没有数据
                Ok(None)
            }
        }
    }

    /// 关闭连接
    pub async fn close(&self) -> Result<(), NetworkError> {
        let mut writer = self.writer.lock().await;
        match writer.shutdown().await {
            Ok(()) => {
                {
                    let mut info = self.info.lock().await;
                    info.set_disconnected();
                }
                Ok(())
            }
            Err(e) => Err(NetworkError::Io(e)),
        }
    }

    /// 检查连接是否活跃
    pub async fn is_connected(&self) -> bool {
        let info = self.info.lock().await;
        matches!(info.state, ConnectionState::Connected)
    }

    /// 获取对端地址
    pub async fn peer_addr(&self) -> String {
        let info = self.info.lock().await;
        info.peer_addr.clone()
    }
}

/// 连接工厂
#[derive(Clone)]
pub struct ConnectionFactory {
    event_sender: mpsc::Sender<NetworkEvent>,
}

impl ConnectionFactory {
    pub fn new(event_sender: mpsc::Sender<NetworkEvent>) -> Self {
        Self { event_sender }
    }

    pub async fn emit_connected(&self, connection: &TcpConnection) {
        let info = connection.get_info().await;
        let _ = self
            .event_sender
            .send(NetworkEvent::Connected { local_addr: info.local_addr, peer_addr: info.peer_addr })
            .await;
    }

    pub async fn emit(&self, event: NetworkEvent) {
        let _ = self.event_sender.send(event).await;
    }

    /// 从TCP流创建连接
    pub fn create_connection(&self, stream: TcpStream) -> Result<TcpConnection, NetworkError> {
        TcpConnection::new(stream, self.event_sender.clone())
    }

    /// 连接到远程地址
    pub async fn connect(&self, addr: &str) -> Result<TcpConnection, NetworkError> {
        log::info!("尝试连接到远程地址: {}", addr);

        match timeout(Duration::from_millis(30000), TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                log::info!("TCP连接建立成功: {}", addr);
                self.create_connection(stream)
            }
            Ok(Err(e)) => {
                log::error!("TCP连接失败: addr={}, error={}", addr, e);
                Err(NetworkError::Io(e))
            }
            Err(_) => {
                log::error!("TCP连接超时: addr={}, timeout=30s", addr);
                Err(NetworkError::Timeout)
            }
        }
    }
}
