//! TCP 服务器实现(从站模式)。
//!
//! 本模块提供从站 TCP 服务器功能,包括:
//!
//! - **监听端口**:绑定指定地址和端口,等待主站连接
//! - **多连接管理**:支持多个主站同时连接
//! - **连接跟踪**:维护所有活动连接的状态和信息
//! - **事件通知**:新连接建立、连接断开等事件通知上层
//!
//! # 使用示例
//!
//! ```rust
//! let server = TcpServer::new("0.0.0.0:2404".to_string(), config);
//! server.start().await?;
//!
//! // 获取所有连接
//! let connections = server.get_connections().await;
//! ```

use std::{collections::HashMap, sync::Arc};

use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock, mpsc},
    task::{JoinHandle, JoinSet},
    time::{Duration, sleep},
};

use super::{
    ConnectionFactory, ConnectionInfo, NETWORK_EVENT_CHANNEL_CAPACITY, NetworkConfig, NetworkError, NetworkEvent,
    TcpConnection,
};

/// TCP服务器
pub struct TcpServer {
    /// 监听地址
    listen_addr: String,
    /// 活动连接
    connections: Arc<Mutex<HashMap<String, Arc<TcpConnection>>>>,
    /// 事件接收器
    event_receiver: Arc<Mutex<Option<mpsc::Receiver<NetworkEvent>>>>,
    /// 连接工厂
    factory: ConnectionFactory,
    /// 运行状态
    running: Arc<RwLock<bool>>,
    /// 服务器任务句柄
    server_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TcpServer {
    /// 创建新的TCP服务器
    pub fn new(listen_addr: String, _config: NetworkConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(NETWORK_EVENT_CHANNEL_CAPACITY);
        let factory = ConnectionFactory::new(event_tx.clone());

        Self {
            listen_addr,
            connections: Arc::new(Mutex::new(HashMap::new())),
            event_receiver: Arc::new(Mutex::new(Some(event_rx))),
            factory,
            running: Arc::new(RwLock::new(false)),
            server_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// 启动服务器
    pub async fn start(&self) -> Result<(), NetworkError> {
        // 检查是否已经在运行
        if *self.running.read().await {
            return Err(NetworkError::Protocol("Server already running".to_string()));
        }

        // 绑定监听地址
        let listener = TcpListener::bind(&self.listen_addr).await.map_err(NetworkError::Io)?;

        log::info!("TCP server listening on {}", self.listen_addr);

        // 设置运行状态
        *self.running.write().await = true;

        // 启动服务器任务
        self.start_server_task(listener).await;

        Ok(())
    }

    /// 停止服务器
    pub async fn stop(&self) -> Result<(), NetworkError> {
        // 设置停止状态
        *self.running.write().await = false;

        // 停止服务器任务
        if let Some(handle) = self.server_handle.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }

        // 关闭所有连接
        let connections = {
            let mut connections = self.connections.lock().await;
            connections.drain().map(|(_, connection)| connection).collect::<Vec<_>>()
        };
        for connection in connections {
            let _ = connection.close().await;
        }

        log::info!("TCP server stopped");
        Ok(())
    }

    /// 获取唯一的事件接收器。
    pub async fn get_event_receiver(&self) -> Result<mpsc::Receiver<NetworkEvent>, NetworkError> {
        self.event_receiver
            .lock()
            .await
            .take()
            .ok_or_else(|| NetworkError::Protocol("Event receiver already taken".to_string()))
    }

    /// 向所有连接广播数据
    pub async fn broadcast(&self, data: &[u8]) -> Result<usize, NetworkError> {
        let targets = {
            let connections = self.connections.lock().await;
            connections.iter().map(|(addr, connection)| (addr.clone(), Arc::clone(connection))).collect::<Vec<_>>()
        };

        let mut sent_count = 0;
        for (addr, connection) in targets {
            match connection.send(data).await {
                Ok(()) => {
                    sent_count += 1;
                }
                Err(e) => {
                    log::warn!("Failed to send to {}: {}", addr, e);
                }
            }
        }

        Ok(sent_count)
    }

    /// 向指定连接发送数据
    pub async fn send_to(&self, peer_addr: &str, data: &[u8]) -> Result<(), NetworkError> {
        let connection = {
            let connections = self.connections.lock().await;
            connections.get(peer_addr).cloned()
        };
        match connection {
            Some(connection) => connection.send(data).await,
            None => Err(NetworkError::Protocol(format!("Connection not found: {}", peer_addr))),
        }
    }

    /// 主动断开指定连接
    pub async fn disconnect(&self, peer_addr: &str) -> Result<(), NetworkError> {
        let connection = { self.connections.lock().await.remove(peer_addr) };
        match connection {
            Some(connection) => connection.close().await,
            None => Err(NetworkError::Protocol(format!("Connection not found: {}", peer_addr))),
        }
    }

    /// 获取活动连接信息
    pub async fn get_connections(&self) -> Vec<ConnectionInfo> {
        let connections = {
            let connections = self.connections.lock().await;
            connections.values().cloned().collect::<Vec<_>>()
        };

        let mut infos = Vec::with_capacity(connections.len());
        for connection in connections {
            infos.push(connection.get_info().await);
        }
        infos
    }

    /// 获取连接数量
    pub async fn connection_count(&self) -> usize {
        self.connections.lock().await.len()
    }

    /// 检查服务器是否运行
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// 启动服务器任务
    async fn start_server_task(&self, listener: TcpListener) {
        let connections = Arc::clone(&self.connections);
        let factory = self.factory.clone();
        let running = Arc::clone(&self.running);

        let handle = tokio::spawn(async move {
            let mut connection_tasks = JoinSet::new();
            while *running.read().await {
                tokio::select! {
                    accept_result = listener.accept() => match accept_result {
                    Ok((stream, peer_addr)) => {
                        log::info!("New connection from {}", peer_addr);

                        // 创建连接
                        match factory.create_connection(stream) {
                            Ok(connection) => {
                                let peer_addr_str = peer_addr.to_string();

                                // 保存连接
                                let connection = Arc::new(connection);
                                connections.lock().await.insert(peer_addr_str.clone(), Arc::clone(&connection));
                                factory.emit_connected(&connection).await;

                                // 启动连接处理任务
                                let conn_connections = Arc::clone(&connections);
                                let conn_running = Arc::clone(&running);
                                let conn_peer_addr = peer_addr_str.clone();

                                connection_tasks.spawn(async move {
                                    while *conn_running.read().await {
                                        let connection = {
                                            let connections_guard = conn_connections.lock().await;
                                            connections_guard.get(&conn_peer_addr).cloned()
                                        };

                                        let Some(connection) = connection else {
                                            break;
                                        };

                                        // 处理接收数据（避免持有全局锁跨 await）
                                        let receive_result = connection.receive().await;

                                        match receive_result {
                                            Ok(Some(_data)) => {
                                                // 数据已通过事件系统处理
                                            }
                                            Ok(None) => {
                                                // 没有数据，继续
                                            }
                                            Err(NetworkError::ConnectionClosed) => {
                                                log::info!("Connection {} closed by peer", conn_peer_addr);
                                                conn_connections.lock().await.remove(&conn_peer_addr);
                                                break;
                                            }
                                            Err(e) => {
                                                log::error!("Receive error from {}: {}", conn_peer_addr, e);
                                                conn_connections.lock().await.remove(&conn_peer_addr);
                                                break;
                                            }
                                        }

                                        // 短暂休眠
                                        sleep(Duration::from_millis(10)).await;
                                    }
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to create connection for {}: {}", peer_addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to accept connection: {}", e);
                        sleep(Duration::from_millis(100)).await;
                    }
                    },
                    _ = connection_tasks.join_next(), if !connection_tasks.is_empty() => {}
                }
            }
        });

        *self.server_handle.lock().await = Some(handle);
    }
}

impl Drop for TcpServer {
    fn drop(&mut self) {
        // 确保在析构时停止服务器
        let running = Arc::clone(&self.running);
        let server_handle = Arc::clone(&self.server_handle);

        tokio::spawn(async move {
            *running.write().await = false;

            if let Some(handle) = server_handle.lock().await.take() {
                handle.abort();
            }
        });
    }
}
