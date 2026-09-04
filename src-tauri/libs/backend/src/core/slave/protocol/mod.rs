//! 从站协议处理模块
//!
//! 本模块包含 IEC104 从站协议处理的核心逻辑，负责处理主站发送的各类 ASDU 命令和请求。
//!
//! # 模块结构
//!
//! ## asdu_dispatch
//!
//! ASDU 分发处理模块，是从站协议处理的核心入口。负责：
//! - 根据类型标识（Type ID）将 ASDU 分发到对应的处理函数
//! - 处理总召唤、计数量召唤、控制命令、时钟同步等
//! - 公共地址验证和策略控制
//! - 文件传输请求路由
//!
//! ## control_target
//!
//! 控制目标验证模块。负责：
//! - 验证控制命令类型与目标数据点类型的兼容性
//! - 检查控制映射是否存在
//! - 支持严格模式、兼容模式和调试模式
//!
//! ## file_transfer_runtime
//!
//! 文件传输运行时处理模块。负责：
//! - 处理文件上传（主站→从站）和下载（从站→主站）
//! - 目录列表查询和文件查询
//! - 分段传输和会话管理
//! - 文件仓库管理
//!
//! ## interrogation
//!
//! 召唤命令处理模块。负责：
//! - 总召唤（General Interrogation）响应
//! - 计数量召唤（Counter Interrogation）响应
//! - 批量数据点上报和 SQ=1 优化
//!
//! ## point_type
//!
//! 数据点类型分类与兼容性判断模块。负责：
//! - 类型标识分类（单点、双点、测量值等）
//! - 控制命令与监视点类型兼容性判断
//! - 可模拟类型判定
//!
//! ## quality
//!
//! 质量描述符处理模块。负责：
//! - 质量位（IV、NT、SB、BL、OV）的解析和设置
//! - 质量状态判定（有效性、阻塞状态等）
//! - 质量描述符构建
//!
//! ## response_cause
//!
//! 响应原因处理模块。负责：
//! - 响应原因（COT）与源地址（OA）的合并
//! - 负确认原因生成
//! - 负确认判定逻辑
//! - 控制请求识别
//!
//! # 处理流程
//!
//! ```text
//! 主站 ASDU
//!   ↓
//! asdu_dispatch (分发)
//!   ├─ 公共地址验证
//!   ├─ 文件传输 → file_transfer_runtime
//!   ├─ 总召唤 → interrogation
//!   ├─ 计数量召唤 → interrogation
//!   ├─ 控制命令
//!   │   ├─ control_target (验证)
//!   │   ├─ point_type (兼容性)
//!   │   ├─ quality (阻塞检查)
//!   │   └─ response_cause (响应)
//!   ├─ 时钟同步
//!   └─ 其他命令
//! ```
//!
//! # 线程安全
//!
//! 所有模块都通过 Arc<RwLock<T>> 或 Arc<Mutex<T>> 保护共享状态，支持多连接并发访问。

pub(crate) mod asdu_dispatch;
pub(crate) mod control_target;
pub(crate) mod file_transfer_runtime;
pub(crate) mod handlers;
pub(crate) mod interrogation;
pub(crate) mod point_type;
pub(crate) mod quality;
pub(crate) mod response_cause;
