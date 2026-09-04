//! 主站命令管理模块
//!
//! 本模块负责 IEC104 主站命令的分发、跟踪和超时管理。
//!
//! # 子模块
//!
//! - `dispatch` - 命令分发逻辑,负责编码、发送和响应跟踪
//! - `store` - 待完成命令存储,维护命令队列和状态
//! - `support` - 辅助函数,如错误映射、命令验证等
//! - `timeout` - 命令超时检测和处理
//!
//! # 命令流程
//!
//! 1. 用户调用 `dispatch_command()` 发送命令
//! 2. 命令被编码为 ASDU 并通过 TCP 发送
//! 3. 命令加入待完成队列（`PendingCommand`）
//! 4. 等待从站响应（激活确认/否定）
//! 5. 根据响应更新命令状态
//! 6. 超时检测器定期检查并清理超时命令

pub(crate) mod dispatch;
pub(crate) mod store;
pub(crate) mod support;
pub(crate) mod timeout;
