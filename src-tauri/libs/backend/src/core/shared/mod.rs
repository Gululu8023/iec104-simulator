//! 共享工具模块。
//!
//! 本模块包含主站和从站共用的工具函数和辅助逻辑，包括：
//!
//! - **抓包导出**：PCAP 数据导出功能
//! - **抓包存储**：抓包数据的容量限制存储
//! - **消息解析**：ASDU 消息的解析和格式化
//! - **点表导入**：从 CSV/Excel 导入数据点配置
//! - **运行时提示**：向前端发送运行时事件通知

pub(crate) mod capture_export;
pub(crate) mod capture_store;
pub(crate) mod message_parser;
pub(crate) mod point_table_import;
pub(crate) mod point_validation;
pub(crate) mod runtime_hint;
pub(crate) mod versioned_point_store;
