//! 主站连接管理模块
//!
//! 本模块负责管理主站与从站之间的 TCP 连接,包括连接的创建、断开、
//! 状态监控和数据传输。
//!
//! # 子模块
//!
//! - `lifecycle` - 连接生命周期管理,负责连接的创建、断开和监控任务
//! - `registry` - 连接注册表,维护连接 ID 到配置文件 ID 的映射关系
//! - `transport_runtime` - 传输运行时,负责 TCP 数据收发和协议帧解析
//!
//! # 连接生命周期
//!
//! 1. **创建连接**：通过 `connect_to_slave_via_transport()` 建立 TCP 连接
//! 2. **启动监控**：为每个连接启动后台任务,处理数据收发和超时检测
//! 3. **数据传输**：通过传输运行时处理 I-frame、U-frame、S-frame
//! 4. **断开连接**：通过 `disconnect_connection()` 关闭连接并清理资源
//!
//! # 连接状态
//!
//! - **TransportState**：TCP 连接状态（Disconnected/Connecting/Connected/Error）
//! - **DataTransferState**：数据传输状态（Stopped/Starting/Started/Stopping/Error）
//!
//! # 自动重连
//!
//! 支持连接断开后自动重连,可配置重试次数和重试间隔。

pub(crate) mod lifecycle;
pub(crate) mod registry;
pub(crate) mod transport_runtime;
