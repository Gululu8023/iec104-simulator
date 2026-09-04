//! 召唤命令处理
//!
//! 本模块负责处理 IEC104 协议中的召唤命令，包括总召唤和计数量召唤。
//!
//! # 核心功能
//!
//! - **总召唤（General Interrogation）**：响应主站的总召唤命令，上报所有数据点
//! - **计数量召唤（Counter Interrogation）**：响应主站的计数量召唤命令，上报累计量数据点
//! - **异步任务管理**：在独立的 tokio 任务中处理召唤响应，避免阻塞主线程
//! - **批量上报**：支持批量打包数据点，减少网络开销
//! - **SQ=1 优化**：支持序列传输模式，进一步压缩数据
//!
//! # 总召唤（C_IC_NA_1, Type ID 100）
//!
//! 总召唤是 IEC104 协议中最常用的命令之一，主站通过总召唤获取从站的所有数据点状态。
//!
//! ## 处理流程
//!
//! ```text
//! 接收总召唤命令
//!   ↓
//! 启动异步任务
//!   ↓
//! 发送激活确认（ACT_CON）
//!   ↓
//! 读取所有数据点
//!   ↓
//! 按类型和公共地址分组
//!   ↓
//! 批量打包 ASDU
//!   ├─ SQ=0：每个数据点单独编码（带 IOA）
//!   └─ SQ=1：序列传输（共享 IOA，节省空间）
//!   ↓
//! 发送数据点 ASDU（COT=20）
//!   ↓
//! 发送激活终止（ACT_TERM）
//! ```
//!
//! ## 传输原因
//!
//! - **ACT_CON (7)**：激活确认，表示从站开始处理总召唤
//! - **INTERROGATION (20)**：总召唤响应，用于数据点上报
//! - **ACT_TERM (10)**：激活终止，表示总召唤完成
//!
//! ## 限定词（Qualifier）
//!
//! - **20**：站召唤（Station Interrogation）- 召唤所有数据点
//! - **21-36**：组召唤（Group Interrogation）- 召唤特定组的数据点
//!
//! # 计数量召唤（C_CI_NA_1, Type ID 101）
//!
//! 计数量召唤用于读取累计量（Integrated Total）类型的数据点，支持冻结和复位操作。
//!
//! ## 冻结限定词（Freeze Qualifier）
//!
//! - **Read (0)**：读取当前值，不冻结
//! - **Freeze (1)**：冻结当前值（保存快照）
//! - **FreezeAndReset (2)**：冻结当前值并复位为 0
//! - **Reset (3)**：复位为 0
//!
//! ## 处理流程
//!
//! ```text
//! 接收计数量召唤命令
//!   ↓
//! 启动异步任务
//!   ↓
//! 发送激活确认（ACT_CON）
//!   ↓
//! 根据冻结限定词处理
//!   ├─ Read/Freeze → 读取累计量数据点
//!   ├─ FreezeAndReset → 读取并复位为 0
//!   └─ Reset → 复位为 0 并读取
//!   ↓
//! 批量打包 ASDU
//!   ↓
//! 发送累计量 ASDU（COT=37）
//!   ↓
//! 发送激活终止（ACT_TERM）
//! ```
//!
//! ## 累计量类型
//!
//! - **15 (M_IT_NA_1)**：累计量（无时标）
//! - **16 (M_IT_TA_1)**：累计量（带时标）
//! - **37 (M_IT_TB_1)**：累计量（带 CP56Time2a 时标）
//!
//! # 批量打包策略
//!
//! ## SQ=0 模式（非序列传输）
//!
//! 每个数据点单独编码，包含完整的 IOA（信息对象地址）：
//!
//! ```text
//! ASDU Header
//!   ├─ Type ID
//!   ├─ VSQ (数量 + SQ=0)
//!   ├─ COT
//!   └─ Common Address
//! Object 1: [IOA(3字节)] [Value] [Quality]
//! Object 2: [IOA(3字节)] [Value] [Quality]
//! ...
//! ```
//!
//! ## SQ=1 模式（序列传输）
//!
//! 多个连续地址的数据点共享起始 IOA，节省空间：
//!
//! ```text
//! ASDU Header
//!   ├─ Type ID
//!   ├─ VSQ (数量 + SQ=1)
//!   ├─ COT
//!   └─ Common Address
//! Start IOA: [IOA(3字节)]
//! Object 1: [Value] [Quality]
//! Object 2: [Value] [Quality]  // IOA = Start IOA + 1
//! Object 3: [Value] [Quality]  // IOA = Start IOA + 2
//! ...
//! ```
//!
//! ## 批量限制
//!
//! - **max_asdu_bytes**：单个 ASDU 的最大字节数（默认 249）
//! - **max_objects_per_asdu**：单个 ASDU 的最大对象数量（根据类型计算）
//!
//! # 性能优化
//!
//! - **异步任务**：召唤响应在独立任务中执行，不阻塞主线程
//! - **批量发送**：多个数据点打包到一个 ASDU 中，减少网络开销
//! - **SQ=1 优化**：对于连续地址的数据点，使用序列传输模式
//! - **按类型分组**：相同类型的数据点打包到同一个 ASDU 中
//!
//! # 线程安全
//!
//! 所有共享状态通过 Arc<RwLock<T>> 或 Arc<Mutex<T>> 保护，支持多连接并发访问。

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, atomic::AtomicU64},
};

use log::warn;
use tokio::sync::{Mutex, RwLock};

use super::response_cause::merge_response_cause_with_request_origin;
use crate::{
    core::{
        protocol_adapter::build_ioa_qualifier_payload,
        slave::{
            reporting::soe_support::{
                build_counter_measurement_payload_with_offset, build_point_measurement_payload_with_offset,
            },
            transport::{
                outbound_dispatcher::enqueue_interrogation_sequence,
                outbound_frame::{encode_batched_outbound_frame, encode_single_outbound_frame},
                payload_batching::{combine_payloads, max_objects_per_asdu, try_compose_sq1_payload},
            },
        },
        types::{AsduInfo, ConnectionInfo, DataPoint, SlaveMessageRecord},
    },
    errors::AppResult,
    network::{Iec104Protocol, TcpServer},
    utils::pcap::CapturePacket,
};

// Extracted from service.rs to isolate interrogation response flow.

/// 启动总召唤响应任务
///
/// 在独立的 tokio 异步任务中处理总召唤命令，避免阻塞主线程。
/// 本函数是总召唤处理的入口点，负责启动异步任务并处理错误。
///
/// # 参数
///
/// * `station_id` - 从站标识符（用于日志记录）
/// * `peer_addr` - 对端地址（主站的 IP:Port）
/// * `request_cause` - 请求的传输原因（用于响应时保留原始地址）
/// * `target_common_address` - 目标公共地址（用于过滤数据点）
/// * `ioa` - 信息对象地址（总召唤命令的 IOA，通常为 0）
/// * `qualifier_raw` - 限定词原始值（20=站召唤，21-36=组召唤）
/// * `enable_sq1_upload` - 是否启用 SQ=1 序列传输模式
/// * `max_asdu_bytes` - 单个 ASDU 的最大字节数（用于批量限制）
/// * `server` - TCP 服务器实例（用于发送响应报文）
/// * `protocol` - IEC104 协议实例（用于编码和发送 ASDU）
/// * `connections` - 连接信息映射（用于更新连接状态）
/// * `peer_local_addrs` - 对端本地地址映射（用于获取本地绑定地址）
/// * `message_records` - 消息记录队列（用于记录收发的 ASDU 消息）
/// * `next_message_id` - 下一个消息 ID（用于生成唯一的消息标识）
/// * `capture_packets` - 抓包数据队列（用于记录原始报文数据）
/// * `data_points` - 数据点映射（用于读取所有数据点状态）
///
/// # 执行流程
///
/// 1. **克隆参数**：将字符串参数克隆为 `String`，以便在异步任务中使用
/// 2. **启动异步任务**：调用 `tokio::spawn` 启动独立的异步任务
/// 3. **调用响应函数**：在异步任务中调用 `send_general_interrogation_response_sequence`
/// 4. **错误处理**：如果响应失败，记录警告日志
///
/// # 响应流程
///
/// 异步任务中的响应流程：
///
/// 1. 发送激活确认（ACT_CON, COT=7）
/// 2. 读取所有数据点（按公共地址过滤）
/// 3. 按类型分组数据点
/// 4. 批量打包 ASDU（支持 SQ=0 和 SQ=1 模式）
/// 5. 发送数据点 ASDU（COT=20, INTERROGATION）
/// 6. 发送激活终止（ACT_TERM, COT=10）
///
/// # 使用场景
///
/// 在 ASDU 分发处理中，当接收到总召唤命令（Type ID 100）时调用此函数：
///
/// ```rust
/// SlaveCommandEvent::GeneralInterrogation { ioa, qualifier } => {
///     spawn_general_interrogation_response_task(
///         station_id,
///         peer_addr,
///         asdu.cause,
///         target_common_address,
///         ioa,
///         qualifier.as_u8(),
///         enable_sq1_upload,
///         max_asdu_bytes,
///         Arc::clone(server),
///         Arc::clone(protocol),
///         // ... 其他参数
///     );
/// }
/// ```
///
/// # 错误处理
///
/// 如果响应序列发送失败，会记录警告日志，但不会影响主线程的执行。
/// 错误日志包含从站 ID、对端地址、公共地址和错误信息。
///
/// # 性能考虑
///
/// - **异步执行**：在独立任务中执行，不阻塞主线程
/// - **批量发送**：多个数据点打包到一个 ASDU 中，减少网络开销
/// - **SQ=1 优化**：对于连续地址的数据点，使用序列传输模式节省空间
///
/// # 线程安全
///
/// 本函数通过 Arc 克隆共享状态，异步任务独立运行，不会产生数据竞争。
pub(crate) fn spawn_general_interrogation_response_task(
    station_id: &str,
    peer_addr: &str,
    request_cause: u16,
    target_common_address: u16,
    ioa: u32,
    qualifier_raw: u8,
    enable_sq1_upload: bool,
    max_asdu_bytes: u16,
    station_time_offset_ms: i64,
    server: Arc<TcpServer>,
    protocol: Arc<Mutex<Iec104Protocol>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: Arc<AtomicU64>,
    capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
    data_points: Arc<crate::core::slave::data::point_store::SlavePointStore>,
) {
    let station_id = station_id.to_string();
    let peer_addr = peer_addr.to_string();
    tokio::spawn(async move {
        if let Err(err) = send_general_interrogation_response_sequence(
            &peer_addr,
            request_cause,
            target_common_address,
            ioa,
            qualifier_raw,
            enable_sq1_upload,
            max_asdu_bytes,
            station_time_offset_ms,
            &server,
            &protocol,
            &connections,
            &peer_local_addrs,
            &message_records,
            &next_message_id,
            &capture_packets,
            &data_points,
        )
        .await
        {
            warn!(
                "[slave] gi_response_failed station_id={} peer_addr={} ca={} error={}",
                station_id, peer_addr, target_common_address, err
            );
        }
    });
}

/// 启动计数量召唤响应任务
///
/// 在独立的 tokio 异步任务中处理计数量召唤命令，避免阻塞主线程。
/// 本函数是计数量召唤处理的入口点，负责启动异步任务并处理错误。
///
/// # 参数
///
/// * `station_id` - 从站标识符（用于日志记录）
/// * `peer_addr` - 对端地址（主站的 IP:Port）
/// * `request_cause` - 请求的传输原因（用于响应时保留原始地址）
/// * `target_common_address` - 目标公共地址（用于过滤数据点）
/// * `ioa` - 信息对象地址（计数量召唤命令的 IOA，通常为 0）
/// * `qualifier_raw` - 限定词原始值（包含冻结限定词和请求限定词）
/// * `enable_sq1_upload` - 是否启用 SQ=1 序列传输模式
/// * `max_asdu_bytes` - 单个 ASDU 的最大字节数（用于批量限制）
/// * `snapshot` - 累计量数据点快照（已经根据冻结限定词处理过）
/// * `server` - TCP 服务器实例（用于发送响应报文）
/// * `protocol` - IEC104 协议实例（用于编码和发送 ASDU）
/// * `connections` - 连接信息映射（用于更新连接状态）
/// * `peer_local_addrs` - 对端本地地址映射（用于获取本地绑定地址）
/// * `message_records` - 消息记录队列（用于记录收发的 ASDU 消息）
/// * `next_message_id` - 下一个消息 ID（用于生成唯一的消息标识）
/// * `capture_packets` - 抓包数据队列（用于记录原始报文数据）
///
/// # 执行流程
///
/// 1. **克隆参数**：将字符串参数克隆为 `String`，以便在异步任务中使用
/// 2. **启动异步任务**：调用 `tokio::spawn` 启动独立的异步任务
/// 3. **调用响应函数**：在异步任务中调用 `send_counter_interrogation_response_sequence`
/// 4. **错误处理**：如果响应失败，记录警告日志
///
/// # 响应流程
///
/// 异步任务中的响应流程：
///
/// 1. 发送激活确认（ACT_CON, COT=7）
/// 2. 使用传入的快照数据（已经根据冻结限定词处理过）
/// 3. 按类型分组累计量数据点
/// 4. 批量打包 ASDU（支持 SQ=0 和 SQ=1 模式）
/// 5. 发送累计量 ASDU（COT=20, INTERROGATION）
/// 6. 发送激活终止（ACT_TERM, COT=10）
///
/// # 冻结限定词处理
///
/// 在调用本函数之前，调用者已经根据冻结限定词处理了数据点：
///
/// - **Read (0)**：读取当前值，不冻结
/// - **Freeze (1)**：冻结当前值（保存快照）
/// - **FreezeAndReset (2)**：冻结当前值并复位为 0
/// - **Reset (3)**：复位为 0
///
/// 本函数接收的 `snapshot` 参数是已经处理过的数据点快照。
///
/// # 使用场景
///
/// 在 ASDU 分发处理中，当接收到计数量召唤命令（Type ID 101）时调用此函数：
///
/// ```rust
/// SlaveCommandEvent::CounterInterrogation { ioa, qualifier } => {
///     // 根据冻结限定词处理数据点
///     let snapshot = match qualifier.freeze {
///         FreezeQualifier::Read | FreezeQualifier::Freeze => {
///             // 读取累计量数据点
///         }
///         FreezeQualifier::FreezeAndReset => {
///             // 读取并复位为 0
///         }
///         FreezeQualifier::Reset => {
///             // 复位为 0 并读取
///         }
///     };
///
///     spawn_counter_interrogation_response_task(
///         station_id,
///         peer_addr,
///         asdu.cause,
///         target_common_address,
///         ioa,
///         qualifier.as_u8(),
///         enable_sq1_upload,
///         max_asdu_bytes,
///         snapshot,
///         Arc::clone(server),
///         Arc::clone(protocol),
///         // ... 其他参数
///     );
/// }
/// ```
///
/// # 错误处理
///
/// 如果响应序列发送失败，会记录警告日志，但不会影响主线程的执行。
/// 错误日志包含从站 ID、对端地址、公共地址和错误信息。
///
/// # 性能考虑
///
/// - **异步执行**：在独立任务中执行，不阻塞主线程
/// - **批量发送**：多个数据点打包到一个 ASDU 中，减少网络开销
/// - **SQ=1 优化**：对于连续地址的数据点，使用序列传输模式节省空间
/// - **快照传递**：接收已处理的快照，避免在异步任务中重复处理
///
/// # 线程安全
///
/// 本函数通过 Arc 克隆共享状态，异步任务独立运行，不会产生数据竞争。
/// 快照数据通过值传递，避免了对原始数据点映射的并发访问。
pub(crate) fn spawn_counter_interrogation_response_task(
    station_id: &str,
    peer_addr: &str,
    request_cause: u16,
    target_common_address: u16,
    ioa: u32,
    qualifier_raw: u8,
    enable_sq1_upload: bool,
    max_asdu_bytes: u16,
    snapshot: Vec<DataPoint>,
    station_time_offset_ms: i64,
    server: Arc<TcpServer>,
    protocol: Arc<Mutex<Iec104Protocol>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: Arc<AtomicU64>,
    capture_packets: Arc<RwLock<VecDeque<CapturePacket>>>,
) {
    let station_id = station_id.to_string();
    let peer_addr = peer_addr.to_string();
    tokio::spawn(async move {
        if let Err(err) = send_counter_interrogation_response_sequence(
            &peer_addr,
            request_cause,
            target_common_address,
            ioa,
            qualifier_raw,
            enable_sq1_upload,
            max_asdu_bytes,
            &snapshot,
            station_time_offset_ms,
            &server,
            &protocol,
            &connections,
            &peer_local_addrs,
            &message_records,
            &next_message_id,
            &capture_packets,
        )
        .await
        {
            warn!(
                "[slave] counter_interrogation_response_failed station_id={} peer_addr={} ca={} error={}",
                station_id, peer_addr, target_common_address, err
            );
        }
    });
}

/// 发送总召唤响应序列
///
/// 实际执行总召唤响应的核心函数，负责读取数据点、分组打包、生成 ASDU 帧序列。
/// 本函数在异步任务中执行，不会阻塞主线程。
///
/// # 参数
///
/// * `peer_addr` - 对端地址（主站的 IP:Port）
/// * `request_cause` - 请求的传输原因（用于响应时保留原始地址）
/// * `target_common_address` - 目标公共地址（用于过滤数据点）
/// * `ioa` - 信息对象地址（总召唤命令的 IOA，通常为 0）
/// * `qualifier_raw` - 限定词原始值（20=站召唤，21-36=组召唤）
/// * `enable_sq1_upload` - 是否启用 SQ=1 序列传输模式
/// * `max_asdu_bytes` - 单个 ASDU 的最大字节数（用于批量限制）
/// * `server` - TCP 服务器实例（用于发送响应报文）
/// * `protocol` - IEC104 协议实例（用于编码和发送 ASDU）
/// * `connections` - 连接信息映射（用于更新连接状态）
/// * `peer_local_addrs` - 对端本地地址映射（用于获取本地绑定地址）
/// * `message_records` - 消息记录队列（用于记录收发的 ASDU 消息）
/// * `next_message_id` - 下一个消息 ID（用于生成唯一的消息标识）
/// * `capture_packets` - 抓包数据队列（用于记录原始报文数据）
/// * `data_points` - 数据点映射（用于读取所有数据点状态）
///
/// # 返回
///
/// - `Ok(())` - 响应序列发送成功
/// - `Err(AppError)` - 响应序列发送失败，返回错误信息
///
/// # 执行流程
///
/// ## 1. 构建请求 payload
///
/// 使用 `build_ioa_qualifier_payload` 构建包含 IOA 和限定词的 payload，
/// 用于激活确认和激活终止响应。
///
/// ## 2. 发送激活确认（ACT_CON）
///
/// 构建并添加激活确认帧到帧序列：
/// - Type ID: 100 (C_IC_NA_1)
/// - COT: 7 (ACT_CON) | 原始地址
/// - Common Address: target_common_address
/// - Data: request_payload
///
/// ## 3. 读取并过滤数据点
///
/// - 获取数据点映射的读锁
/// - 过滤出目标公共地址的所有数据点
/// - 按 IOA 排序（确保顺序一致性）
///
/// ## 4. 按类型分组数据点
///
/// - 遍历所有数据点
/// - 调用 `build_point_measurement_payload` 构建每个数据点的 payload
/// - 按 type_id 分组存储到 BTreeMap 中（保证类型顺序）
///
/// ## 5. 批量打包 ASDU
///
/// 对每个类型的数据点：
///
/// a. **计算批量限制**：
///    - 调用 `max_objects_per_asdu` 计算单个 ASDU 最多可容纳的对象数量
///    - 基于 payload 大小和 max_asdu_bytes 限制
///
/// b. **分块处理**：
///    - 将数据点 payload 按批量限制分块
///    - 每个块生成一个 ASDU 帧
///
/// c. **SQ=1 优化（可选）**：
///    - 如果启用 `enable_sq1_upload`，尝试使用序列传输模式
///    - 调用 `try_compose_sq1_payload` 检查是否可以使用 SQ=1
///    - 如果成功，使用 SQ=1 payload；否则回退到 SQ=0
///
/// d. **编码 ASDU 帧**：
///    - 调用 `encode_batched_outbound_frame` 编码批量 ASDU 帧
///    - Type ID: 数据点类型
///    - COT: 20 (INTERROGATION) | 原始地址
///    - Common Address: target_common_address
///    - Data: 组合后的 payload
///    - VSQ: 对象数量 + SQ 标志
///
/// ## 6. 发送激活终止（ACT_TERM）
///
/// 构建并添加激活终止帧到帧序列：
/// - Type ID: 100 (C_IC_NA_1)
/// - COT: 10 (ACT_TERM) | 原始地址
/// - Common Address: target_common_address
/// - Data: request_payload
///
/// ## 7. 入队帧序列
///
/// 调用 `enqueue_interrogation_sequence` 将所有帧入队发送：
/// - 使用唯一的序列 ID：`gi:{common_address}:{ioa}`
/// - 按顺序发送所有帧
///
/// # 批量打包示例
///
/// 假设有 100 个单点信息数据点（Type ID 1），max_asdu_bytes=249：
///
/// 1. 每个单点信息 payload 大小：3 字节（IOA）+ 1 字节（SIQ）= 4 字节
/// 2. ASDU 头部大小：约 6 字节
/// 3. 最大对象数量：(249 - 6) / 4 ≈ 60 个
/// 4. 分块结果：第一个 ASDU 包含 60 个数据点，第二个 ASDU 包含 40 个数据点
///
/// # SQ=1 优化示例
///
/// 如果数据点地址连续（100, 101, 102, ...），使用 SQ=1 模式：
///
/// - SQ=0 模式：每个对象 4 字节（3 字节 IOA + 1 字节 SIQ）
/// - SQ=1 模式：起始 IOA 3 字节 + 每个对象 1 字节（SIQ）
/// - 节省空间：对于 60 个对象，节省 60 * 3 = 180 字节
///
/// # 错误处理
///
/// 如果 `enqueue_interrogation_sequence` 失败，返回错误信息。
/// 调用者会记录警告日志。
///
/// # 性能考虑
///
/// - **读锁持有时间**：读取数据点时持有读锁，克隆数据后立即释放
/// - **批量发送**：多个数据点打包到一个 ASDU 中，减少网络开销
/// - **SQ=1 优化**：对于连续地址的数据点，使用序列传输模式节省空间
/// - **按类型分组**：相同类型的数据点打包到同一个 ASDU 中，提高编码效率
///
/// # 线程安全
///
/// 本函数通过不可变引用访问共享状态，使用读锁保护数据点映射，可以安全地在多线程环境中调用。
async fn send_general_interrogation_response_sequence(
    peer_addr: &str,
    request_cause: u16,
    target_common_address: u16,
    ioa: u32,
    qualifier_raw: u8,
    enable_sq1_upload: bool,
    max_asdu_bytes: u16,
    station_time_offset_ms: i64,
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
    data_points: &Arc<crate::core::slave::data::point_store::SlavePointStore>,
) -> AppResult<()> {
    let request_payload = build_ioa_qualifier_payload(ioa, qualifier_raw);
    let _ = (server, connections, peer_local_addrs, message_records, next_message_id, capture_packets);
    let mut frames = Vec::new();
    frames.push(encode_single_outbound_frame(&AsduInfo {
        type_id: 100,
        cause: merge_response_cause_with_request_origin(request_cause, 7),
        common_address: target_common_address,
        data: request_payload.clone(),
        timestamp: chrono::Utc::now(),
    }));

    let mut snapshot: Vec<DataPoint> = data_points
        .read()
        .await
        .iter()
        .filter_map(|((common_address, _), point)| {
            let in_group = qualifier_raw == 20 || point.gi_group == Some(qualifier_raw - 20);
            if *common_address == target_common_address && in_group { Some(point.clone()) } else { None }
        })
        .collect();
    snapshot.sort_by_key(|point| point.address);

    let mut groups: std::collections::BTreeMap<u8, Vec<Vec<u8>>> = std::collections::BTreeMap::new();
    for point in &snapshot {
        let type_id = point.type_id;
        let Some(payload) = build_point_measurement_payload_with_offset(type_id, point, station_time_offset_ms) else {
            continue;
        };
        groups.entry(type_id).or_default().push(payload);
    }

    for (type_id, payloads) in groups {
        let max_batch_count =
            payloads.first().map(|payload| max_objects_per_asdu(payload.len(), max_asdu_bytes)).unwrap_or(1);
        for chunk in payloads.chunks(max_batch_count) {
            let batch_count = chunk.len() as u8;
            let (combined_data, sq_sequence) = if enable_sq1_upload {
                if let Some(sq1_payload) = try_compose_sq1_payload(chunk) {
                    (sq1_payload, true)
                } else {
                    (combine_payloads(chunk), false)
                }
            } else {
                (combine_payloads(chunk), false)
            };
            frames.push(encode_batched_outbound_frame(
                &AsduInfo {
                    type_id,
                    cause: merge_response_cause_with_request_origin(request_cause, u16::from(qualifier_raw)),
                    common_address: target_common_address,
                    data: combined_data,
                    timestamp: chrono::Utc::now(),
                },
                batch_count,
                sq_sequence,
            ));
        }
    }

    frames.push(encode_single_outbound_frame(&AsduInfo {
        type_id: 100,
        cause: merge_response_cause_with_request_origin(request_cause, 10),
        common_address: target_common_address,
        data: request_payload,
        timestamp: chrono::Utc::now(),
    }));

    enqueue_interrogation_sequence(peer_addr, protocol, format!("gi:{}:{}", target_common_address, ioa), frames).await
}

/// 发送计数量召唤响应序列
///
/// 实际执行计数量召唤响应的核心函数，负责处理累计量数据点快照、分组打包、生成 ASDU 帧序列。
/// 本函数在异步任务中执行，不会阻塞主线程。
///
/// # 参数
///
/// * `peer_addr` - 对端地址（主站的 IP:Port）
/// * `request_cause` - 请求的传输原因（用于响应时保留原始地址）
/// * `target_common_address` - 目标公共地址（用于过滤数据点）
/// * `ioa` - 信息对象地址（计数量召唤命令的 IOA，通常为 0）
/// * `qualifier_raw` - 限定词原始值（包含冻结限定词和请求限定词）
/// * `enable_sq1_upload` - 是否启用 SQ=1 序列传输模式
/// * `max_asdu_bytes` - 单个 ASDU 的最大字节数（用于批量限制）
/// * `snapshot` - 累计量数据点快照（已经根据冻结限定词处理过）
/// * `server` - TCP 服务器实例（用于发送响应报文）
/// * `protocol` - IEC104 协议实例（用于编码和发送 ASDU）
/// * `connections` - 连接信息映射（用于更新连接状态）
/// * `peer_local_addrs` - 对端本地地址映射（用于获取本地绑定地址）
/// * `message_records` - 消息记录队列（用于记录收发的 ASDU 消息）
/// * `next_message_id` - 下一个消息 ID（用于生成唯一的消息标识）
/// * `capture_packets` - 抓包数据队列（用于记录原始报文数据）
///
/// # 返回
///
/// - `Ok(())` - 响应序列发送成功
/// - `Err(AppError)` - 响应序列发送失败，返回错误信息
///
/// # 执行流程
///
/// ## 1. 构建请求 payload
///
/// 使用 `build_ioa_qualifier_payload` 构建包含 IOA 和限定词的 payload，
/// 用于激活确认和激活终止响应。
///
/// ## 2. 发送激活确认（ACT_CON）
///
/// 构建并添加激活确认帧到帧序列：
/// - Type ID: 101 (C_CI_NA_1)
/// - COT: 7 (ACT_CON) | 原始地址
/// - Common Address: target_common_address
/// - Data: request_payload
///
/// ## 3. 处理累计量数据点快照
///
/// 使用传入的快照数据（已经根据冻结限定词处理过）：
/// - **Read/Freeze**：快照包含当前值
/// - **FreezeAndReset**：快照包含冻结前的值（数据点已复位为 0）
/// - **Reset**：快照包含复位后的值（0）
///
/// ## 4. 按类型分组累计量数据点
///
/// - 遍历快照中的所有数据点
/// - 确定响应类型：如果数据点类型是累计量类型（15, 16, 37），使用原类型；否则使用 15
/// - 调用 `build_counter_measurement_payload` 构建每个数据点的 payload
/// - 按 response_type_id 分组存储到 BTreeMap 中（保证类型顺序）
///
/// ## 5. 批量打包 ASDU
///
/// 对每个类型的数据点：
///
/// a. **计算批量限制**：
///    - 调用 `max_objects_per_asdu` 计算单个 ASDU 最多可容纳的对象数量
///    - 基于 payload 大小和 max_asdu_bytes 限制
///
/// b. **分块处理**：
///    - 将数据点 payload 按批量限制分块
///    - 每个块生成一个 ASDU 帧
///
/// c. **SQ=1 优化（可选）**：
///    - 如果启用 `enable_sq1_upload`，尝试使用序列传输模式
///    - 调用 `try_compose_sq1_payload` 检查是否可以使用 SQ=1
///    - 如果成功，使用 SQ=1 payload；否则回退到 SQ=0
///
/// d. **编码 ASDU 帧**：
///    - 调用 `encode_batched_outbound_frame` 编码批量 ASDU 帧
///    - Type ID: 累计量类型（15, 16, 或 37）
///    - COT: 20 (INTERROGATION) | 原始地址
///    - Common Address: target_common_address
///    - Data: 组合后的 payload
///    - VSQ: 对象数量 + SQ 标志
///
/// ## 6. 发送激活终止（ACT_TERM）
///
/// 构建并添加激活终止帧到帧序列：
/// - Type ID: 101 (C_CI_NA_1)
/// - COT: 10 (ACT_TERM) | 原始地址
/// - Common Address: target_common_address
/// - Data: request_payload
///
/// ## 7. 入队帧序列
///
/// 调用 `enqueue_interrogation_sequence` 将所有帧入队发送：
/// - 使用唯一的序列 ID：`ci:{common_address}:{ioa}`
/// - 按顺序发送所有帧
///
/// # 累计量类型处理
///
/// 累计量数据点可能有不同的类型标识：
///
/// - **15 (M_IT_NA_1)**：累计量（无时标）
/// - **16 (M_IT_TA_1)**：累计量（带 CP24Time2a 时标）
/// - **37 (M_IT_TB_1)**：累计量（带 CP56Time2a 时标）
///
/// 本函数会根据数据点的原始类型选择合适的响应类型：
/// - 如果数据点类型是 15, 16, 或 37，使用原类型
/// - 否则使用默认类型 15（无时标）
///
/// # 批量打包示例
///
/// 假设有 50 个累计量数据点（Type ID 15），max_asdu_bytes=249：
///
/// 1. 每个累计量 payload 大小：3 字节（IOA）+ 4 字节（计数值）+ 1 字节（BCR 质量）= 8 字节
/// 2. ASDU 头部大小：约 6 字节
/// 3. 最大对象数量：(249 - 6) / 8 ≈ 30 个
/// 4. 分块结果：第一个 ASDU 包含 30 个数据点，第二个 ASDU 包含 20 个数据点
///
/// # SQ=1 优化示例
///
/// 如果累计量地址连续（1000, 1001, 1002, ...），使用 SQ=1 模式：
///
/// - SQ=0 模式：每个对象 8 字节（3 字节 IOA + 5 字节 BCR）
/// - SQ=1 模式：起始 IOA 3 字节 + 每个对象 5 字节（BCR）
/// - 节省空间：对于 30 个对象，节省 30 * 3 = 90 字节
///
/// # 冻结限定词处理
///
/// 本函数接收的 `snapshot` 参数是已经根据冻结限定词处理过的数据点快照：
///
/// - **Read (0)**：快照包含当前值，数据点未修改
/// - **Freeze (1)**：快照包含当前值，活动点数值不变并递增 BCR SQ
/// - **FreezeAndReset (2)**：快照包含冻结前的值，数据点已复位为 0
/// - **Reset (3)**：快照包含复位后的值（0），数据点已复位为 0
///
/// 调用者在调用本函数之前已经完成了数据点的冻结和复位操作。
///
/// # 错误处理
///
/// 如果 `enqueue_interrogation_sequence` 失败，返回错误信息。
/// 调用者会记录警告日志。
///
/// # 性能考虑
///
/// - **快照传递**：接收已处理的快照，避免在异步任务中重复处理
/// - **批量发送**：多个数据点打包到一个 ASDU 中，减少网络开销
/// - **SQ=1 优化**：对于连续地址的数据点，使用序列传输模式节省空间
/// - **按类型分组**：相同类型的数据点打包到同一个 ASDU 中，提高编码效率
///
/// # 线程安全
///
/// 本函数通过不可变引用访问共享状态，快照数据通过值传递，可以安全地在多线程环境中调用。
async fn send_counter_interrogation_response_sequence(
    peer_addr: &str,
    request_cause: u16,
    target_common_address: u16,
    ioa: u32,
    qualifier_raw: u8,
    enable_sq1_upload: bool,
    max_asdu_bytes: u16,
    snapshot: &[DataPoint],
    station_time_offset_ms: i64,
    server: &Arc<TcpServer>,
    protocol: &Arc<Mutex<Iec104Protocol>>,
    connections: &Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    peer_local_addrs: &Arc<RwLock<HashMap<String, std::net::SocketAddr>>>,
    message_records: &Arc<RwLock<VecDeque<SlaveMessageRecord>>>,
    next_message_id: &Arc<AtomicU64>,
    capture_packets: &Arc<RwLock<VecDeque<CapturePacket>>>,
) -> AppResult<()> {
    let request_payload = build_ioa_qualifier_payload(ioa, qualifier_raw);
    let _ = (server, connections, peer_local_addrs, message_records, next_message_id, capture_packets);
    let mut frames = Vec::new();
    frames.push(encode_single_outbound_frame(&AsduInfo {
        type_id: 101,
        cause: merge_response_cause_with_request_origin(request_cause, 7),
        common_address: target_common_address,
        data: request_payload.clone(),
        timestamp: chrono::Utc::now(),
    }));

    let mut groups: std::collections::BTreeMap<u8, Vec<Vec<u8>>> = std::collections::BTreeMap::new();
    for point in snapshot {
        let preferred_type = if matches!(point.type_id, 15 | 16 | 37) { point.type_id } else { 15 };
        let Some((response_type_id, payload)) =
            build_counter_measurement_payload_with_offset(preferred_type, point, station_time_offset_ms)
        else {
            continue;
        };
        groups.entry(response_type_id).or_default().push(payload);
    }

    for (type_id, payloads) in groups {
        let max_batch_count =
            payloads.first().map(|payload| max_objects_per_asdu(payload.len(), max_asdu_bytes)).unwrap_or(1);
        for chunk in payloads.chunks(max_batch_count) {
            let batch_count = chunk.len() as u8;
            let (combined_data, sq_sequence) = if enable_sq1_upload {
                if let Some(sq1_payload) = try_compose_sq1_payload(chunk) {
                    (sq1_payload, true)
                } else {
                    (combine_payloads(chunk), false)
                }
            } else {
                (combine_payloads(chunk), false)
            };
            frames.push(encode_batched_outbound_frame(
                &AsduInfo {
                    type_id,
                    cause: merge_response_cause_with_request_origin(
                        request_cause,
                        36 + u16::from(qualifier_raw & 0x3F),
                    ),
                    common_address: target_common_address,
                    data: combined_data,
                    timestamp: chrono::Utc::now(),
                },
                batch_count,
                sq_sequence,
            ));
        }
    }

    frames.push(encode_single_outbound_frame(&AsduInfo {
        type_id: 101,
        cause: merge_response_cause_with_request_origin(request_cause, 10),
        common_address: target_common_address,
        data: request_payload,
        timestamp: chrono::Utc::now(),
    }));

    enqueue_interrogation_sequence(peer_addr, protocol, format!("ci:{}:{}", target_common_address, ioa), frames).await
}
