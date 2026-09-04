//! IEC104 消息解析器 API 命令模块
//!
//! 提供 IEC104 消息帧解析的 Tauri 命令接口，支持多种输入格式的报文解析。

use crate::{
    api::types::{CommandResult, MessageFrameParseResult, ParseMessageFrameRequest},
    core::shared::message_parser::parse_message_frame_input,
};

/// 解析 IEC104 消息帧
#[tauri::command]
pub async fn parse_message_frame(request: ParseMessageFrameRequest) -> CommandResult<MessageFrameParseResult> {
    Ok(parse_message_frame_input(&request.input_text))
}
