//! 主站数据管理模块
//!
//! 本模块负责管理主站接收和处理的数据,包括数据点存储、SOE 事件管理、
//! 控制命令快照和数据摄取处理。
//!
//! # 子模块
//!
//! - `ingest` - 数据摄取运行时,负责处理接收到的 ASDU 数据并提取数据点和 SOE 事件
//! - `point_store` - 数据点存储,维护数据点的内存缓存和数据库持久化
//! - `point_scope` - 数据点作用域,提供数据点键生成和查询辅助函数
//! - `control_store` - 控制命令快照存储,记录控制命令的执行状态
//! - `soe_store` - SOE 事件存储,管理带时标的事件序列
//!
//! # 数据流
//!
//! 1. **接收 ASDU**：传输运行时接收到 I-frame 后调用数据摄取运行时
//! 2. **解析数据点**：从 ASDU 中提取数据点信息（地址、值、质量、时标等）
//! 3. **更新缓存**：将数据点更新到内存缓存（data_points）
//! 4. **持久化**：可选地将数据点持久化到 SQLite 数据库
//! 5. **提取 SOE**：如果数据点带时标且值发生变化,生成 SOE 事件
//! 6. **记录报文**：将接收到的报文记录到报文缓存
//!
//! # 数据点键格式
//!
//! 数据点在内存中使用复合键标识：`"{connection_key}:{ioa}"`
//! - connection_key 格式：`"{profile_id}:{slave_id}"` 或 `"{connection_id}"`
//! - ioa：信息对象地址（Information Object Address）

pub(crate) mod control_store;
pub(crate) mod ingest;
pub(crate) mod point_scope;
pub(crate) mod point_store;
pub(crate) mod soe_store;
