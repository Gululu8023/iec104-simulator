//! 网络通信层模块。
//!
//! 提供基于 TCP 的网络通信功能，支持主站（客户端）和从站（服务器）模式。
//!
//! - **TcpClient**: 主站 TCP 客户端，支持自动重连
//! - **TcpServer**: 从站 TCP 服务器，支持多客户端连接
//! - **TcpConnection**: TCP 连接抽象，封装读写操作
//! - **Protocol**: IEC 104 协议解析和编码

pub mod client;
pub mod connection;
pub mod protocol;
pub mod server;

// 重新导出主要类型
use std::fmt;

pub use client::*;
pub use connection::*;
pub use protocol::*;
use serde::{Deserialize, Serialize};
pub use server::*;

/// 连接状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// 断开连接
    Disconnected,
    /// 正在连接
    Connecting,
    /// 已连接
    Connected,
    /// 连接错误
    Error(String),
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Connected => write!(f, "Connected"),
            ConnectionState::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

/// 网络事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkEvent {
    /// 连接建立
    Connected { local_addr: String, peer_addr: String },
    /// 连接断开
    Disconnected { peer_addr: String, reason: String },
    /// 自动重连失败一次
    ReconnectAttemptFailed { addr: String, attempt: u32, max_attempts: u32, error: String },
    /// 自动重连已耗尽
    ReconnectExhausted { addr: String, attempts: u32, max_attempts: u32, last_error: String },
    /// 数据接收
    DataReceived { peer_addr: String, data: Vec<u8> },
    /// 错误事件
    Error { peer_addr: Option<String>, error: String },
    /// 站点连接成功
    StationConnected { station_id: i64, connection_id: String, station_type: String },
    /// 站点连接断开
    StationDisconnected { station_id: i64, connection_id: String, station_type: String, reason: String },
}

pub(crate) const NETWORK_EVENT_CHANNEL_CAPACITY: usize = 1024;

// 重新导出统一错误系统中的网络相关类型
pub use crate::errors::{NetworkError, NetworkResult};

/// 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// 连接超时时间（毫秒）
    pub connect_timeout: u64,
    /// 读取超时时间（毫秒）
    pub read_timeout: u64,
    /// 写入超时时间（毫秒）
    pub write_timeout: u64,
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 重连间隔（毫秒）
    pub reconnect_interval: u64,
    /// 发送队列大小
    pub send_queue_size: usize,
    /// 接收缓冲区大小
    pub receive_buffer_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connect_timeout: 30000,
            read_timeout: 30000,
            write_timeout: 30000,
            max_reconnect_attempts: 3,
            reconnect_interval: 5000,
            send_queue_size: 1000,
            receive_buffer_size: 8192,
        }
    }
}

/// 连接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// 连接ID
    pub id: String,
    /// 本地地址
    pub local_addr: String,
    /// 远程地址
    pub peer_addr: String,
    /// 连接状态
    pub state: ConnectionState,
    /// 连接时间
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 最后活动时间
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// 发送字节数
    pub bytes_sent: u64,
    /// 接收字节数
    pub bytes_received: u64,
}

impl ConnectionInfo {
    pub fn new(id: String, local_addr: String, peer_addr: String) -> Self {
        Self {
            id,
            local_addr,
            peer_addr,
            state: ConnectionState::Disconnected,
            connected_at: None,
            last_activity: chrono::Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    pub fn set_connected(&mut self) {
        self.state = ConnectionState::Connected;
        self.connected_at = Some(chrono::Utc::now());
        self.last_activity = chrono::Utc::now();
    }

    pub fn set_disconnected(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.connected_at = None;
    }

    pub fn update_activity(&mut self) {
        self.last_activity = chrono::Utc::now();
    }

    pub fn add_bytes_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.update_activity();
    }

    pub fn add_bytes_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.update_activity();
    }
}

/// 网络统计信息
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    /// 总连接数
    pub total_connections: u64,
    /// 当前活动连接数
    pub active_connections: u64,
    /// 总发送字节数
    pub total_bytes_sent: u64,
    /// 总接收字节数
    pub total_bytes_received: u64,
    /// 连接错误数
    pub connection_errors: u64,
    /// 协议错误数
    pub protocol_errors: u64,
}
