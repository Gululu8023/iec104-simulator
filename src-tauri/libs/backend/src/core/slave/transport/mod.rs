//! 从站传输层子模块。
//!
//! 本模块负责从站的传输层功能，包括：
//!
//! - **出站调度器**：管理出站数据的调度和发送
//! - **出站帧**：构建和编码 IEC104 帧
//! - **出站队列**：管理待发送数据的优先级队列
//! - **载荷批处理**：优化多个 ASDU 的批量发送

pub(crate) mod outbound_dispatcher;
pub(crate) mod outbound_frame;
pub(crate) mod outbound_queue;
pub(crate) mod payload_batching;
