//! # iec60870-parser
//!
//! 高性能、零拷贝的 IEC 60870-5-104 协议解析库，专注于工业 SCADA 系统的报文解析、验证与转换。
//!
//! ## 概览
//!
//! 本库围绕 **解析 → 验证 → 转换** 的流水线组织，将协议处理分为清晰的层次：
//!
//! 1. **字节流管理** ([`stream`]): 缓冲 TCP 输入，维护滑动窗口
//! 2. **帧提取** ([`decoder`]): 从字节流中识别并提取完整的 IEC 104 帧
//! 3. **协议解析** ([`parser`]): 解析 APCI 和 ASDU 结构，零拷贝访问字段
//! 4. **语义验证** ([`validator`]): 检查传输原因、信息体数量等语义规则
//! 5. **类型转换** ([`types`]): 将原始帧转换为类型化的领域对象
//!
//! ## 核心概念
//!
//! ### 数据流
//!
//! ```text
//! TCP字节流 → ByteStream → StreamDecoder → FrameView
//!                                              ↓
//!                                           Parser (APCI/ASDU)
//!                                              ↓
//!                                          Validator (可选)
//!                                              ↓
//!                                         TypeConverter
//!                                              ↓
//!                                          领域对象
//! ```
//!
//! ### 关键类型
//!
//! - [`FrameView`] - 零拷贝的帧引用，提供对 APCI 和 ASDU 的访问
//! - [`Frame`] - 拥有所有权的帧副本
//! - [`TypeId`] - 类型标识枚举，编译期类型安全
//! - [`TypeDescriptor`] - 编译期构建的解析策略表，描述每种 ASDU 类型的结构
//! - [`AsduFrame`] - 类型化的 ASDU 表示（`Typed` 或 `Raw`）
//!
//! ### 特性开关 (Features)
//!
//! - **`simd`**: 使用 SIMD 指令加速帧起始字节扫描（基于 `memchr`）
//! - **`link-manager`**: 提供链路层状态机（link 模块），支持窗口管理和确认机制
//! - **`pcap`**: 启用 PCAP 文件解析支持，用于离线分析
//! - **`serde`**: 为主要类型提供序列化/反序列化支持
//!
//! 默认情况下所有特性均关闭，遵循最小化原则。
//!
//! ## 错误处理
//!
//! 库采用分层错误设计，精确区分不同的故障场景：
//!
//! - [`ParseError`]: 二进制格式错误（帧结构非法、长度不足、字段无效）
//!   - 致命错误：`Incomplete`, `InvalidStart`, `InvalidApci`, `InvalidAsduHeader`,
//!     `InsufficientAsduHeader`
//!   - 可恢复错误：`InvalidInfoObject`, `UnsupportedTypeId`, `InsufficientInfoObject`
//!
//! - [`ValidationError`]: 语义规则违反（VSQ、质量字段、传输原因、命令限定词）
//!   - 每个错误变体携带具体字段值（如 `InvalidVsq { reason, count, sq }`）
//!
//! - [`StreamError`]: 流处理错误（缓冲区溢出、解析失败包装）
//!
//! - [`ConvertError`]: 类型转换失败（不支持的 ASDU 类型、字段格式非法）
//!
//! 所有错误都实现了标准的 `Error` trait，并提供丰富的上下文信息（偏移量、TypeId、IOA
//! 等），便于诊断和日志记录。
//!
//! ## 使用示例
//!
//! ### 基础解析流程
//!
//! ```rust,no_run
//! use iec60870_parser::{
//!     StreamDecoder,
//!     stream::BytesStream,
//!     types::{AsduConverter, TypeConverter},
//! };
//!
//! fn process_stream(tcp_data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//!     let mut decoder = StreamDecoder::new(BytesStream::default());
//!     let converter = AsduConverter;
//!
//!     decoder.push(tcp_data);
//!
//!     while let Some(frame) = decoder.poll_frame()? {
//!         // 尝试转换为类型化对象
//!         match converter.convert(&frame) {
//!             Ok(asdu_frame) => {
//!                 // 成功解析 ASDU（可能是 Measurements, Commands 等）
//!                 println!("解析成功: {:?}", asdu_frame);
//!             }
//!             Err(e) => {
//!                 // 转换失败
//!                 println!("转换失败: {}", e);
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### 自定义验证规则
//!
//! ```rust
//! use iec60870_parser::{FrameRule, FrameView, ValidationError};
//!
//! struct CustomRule;
//!
//! impl FrameRule for CustomRule {
//!     fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
//!         // 自定义验证逻辑
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## 性能考量
//!
//! - **零拷贝**: [`FrameView`] 直接引用输入缓冲区，避免不必要的内存分配
//! - **流式处理**: [`StreamDecoder`] 使用滑动窗口，支持增量解析
//! - **SIMD 加速**: 启用 `simd` feature 可显著加速帧起始定位
//! - **编译期优化**: [`TypeDescriptor`] 在编译期构建查找表，运行时无需哈希查找
//!
//! ## 进一步阅读
//!
//! - [架构文档](../docs/ARCHITECTURE.md): 整体设计和模块划分
//! - [错误处理分析](../docs/error_handling_analysis.md): 错误分层策略详解
//! - [TypeId 使用指南](../docs/type_id_usage.md): 类型系统设计说明
//! - 示例代码: `examples/` 目录下的可运行示例

#![deny(unused_must_use)]

pub mod decoder;
pub mod error;
pub mod parser;
pub mod profile;
pub mod profiles;
pub mod stream;
pub mod types;
pub mod validator;

#[cfg(feature = "link-manager")]
pub mod link;

pub use crate::{
    decoder::StreamDecoder,
    error::{ConvertError, ParseError, StreamError, ValidationError},
    parser::{
        apci::{ApciHeader, FrameType},
        asdu::{Asdu, AsduHeader, ParsedAsduResult, TypeDescriptor, TypeId, parse_asdu_with_profile},
    },
    profile::{DefaultProfile, ProfileAsdu, ProtocolProfile},
    stream::{ByteStream, BytesStream},
    types::{AsduConverter, AsduFrame, Frame, FrameView, TypeConverter},
    validator::FrameRule,
};
