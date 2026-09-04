//! ASDU 消息解析模块。
//!
//! 本模块提供 IEC104 协议帧的解析功能,支持多种输入格式并生成结构化的解析树,包括:
//!
//! - **多格式输入**:支持连续 hex、按字节 hex、0x 列表、bytes 转义串、十进制字节数组
//! - **完整解析**:解析 APCI 头部、ASDU 头部和载荷数据
//! - **结构化输出**:生成树形结构的解析结果,包含字节范围和详细信息
//! - **错误诊断**:提供详细的解析错误和警告信息

use bytes::Bytes;
use iec60870_parser::{
    AsduConverter, AsduFrame, ConvertError, FrameView, ParseError, TypeConverter,
    parser::{
        apci::{FrameType, UControl, parse_apci},
        asdu::{
            TypeId,
            commands::{Command, CommandDetail, CommandQualifier, CommandSet},
            cot::CotReason,
            events::{
                ProtectionEvent, ProtectionEventData, ProtectionEventDetails, ProtectionEvents, ProtectionQuality,
            },
            file_transfer::{FileTransferRecord, FileTransferSet},
            initialization::InitializationFrame,
            interrogation::{Interrogation, InterrogationFrame},
            lookup_type_descriptor,
            measurements::{Measurement, MeasurementDescriptor, MeasurementSet, MeasurementValue},
            parameters::{Parameter, ParameterSet, ParameterValue},
            parse_asdu_header,
            qualifiers::QualityFlags,
            security::SecurityFrame,
            test_command::{TestCommand, TestCommandFrame},
            timestamp::Timestamp,
        },
    },
};

use crate::api::types::{
    MessageFrameParseResult, MessageFrameParseSummary, MessageParserByteRange, MessageParserInputKind,
    MessageParserIssue, MessageParserStatus, MessageParserTreeNode,
};

/// 规范化后的输入数据
#[derive(Debug, Clone)]
struct NormalizedInput {
    kind: MessageParserInputKind,
    bytes: Vec<u8>,
}

/// 帧选择结果
struct FrameSelection<'a> {
    frame_bytes: &'a [u8],
    declared_frame_byte_count: Option<usize>,
    extra_byte_count: usize,
}

const MAX_MESSAGE_INPUT_BYTES: usize = 16 * 1024;
const MAX_DECODED_MESSAGE_BYTES: usize = 4096;

/// 解析消息帧输入
///
/// 支持多种输入格式并生成完整的解析树结构。
pub fn parse_message_frame_input(input_text: &str) -> MessageFrameParseResult {
    if input_text.len() > MAX_MESSAGE_INPUT_BYTES {
        return MessageFrameParseResult {
            status: MessageParserStatus::Error,
            summary: build_summary(None, &[], &[], None, 0),
            warnings: Vec::new(),
            error: Some(issue("INPUT_TOO_LARGE", format!("输入文本不能超过 {MAX_MESSAGE_INPUT_BYTES} 字节"))),
            tree: Vec::new(),
        };
    }
    let normalized = match normalize_message_input(input_text) {
        Ok(value) => value,
        Err(error) => {
            return MessageFrameParseResult {
                status: MessageParserStatus::Error,
                summary: build_summary(None, &[], &[], None, 0),
                warnings: Vec::new(),
                error: Some(error),
                tree: Vec::new(),
            };
        }
    };

    if normalized.bytes.len() > MAX_DECODED_MESSAGE_BYTES {
        return MessageFrameParseResult {
            status: MessageParserStatus::Error,
            summary: build_summary(Some(normalized.kind), &normalized.bytes, &[], None, 0),
            warnings: Vec::new(),
            error: Some(issue(
                "DECODED_INPUT_TOO_LARGE",
                format!("解码后的报文不能超过 {MAX_DECODED_MESSAGE_BYTES} 字节"),
            )),
            tree: Vec::new(),
        };
    }

    let mut warnings = Vec::new();
    let selection = select_frame_bytes(&normalized.bytes, &mut warnings);
    let mut summary = build_summary(
        Some(normalized.kind),
        &normalized.bytes,
        selection.frame_bytes,
        selection.declared_frame_byte_count,
        selection.extra_byte_count,
    );
    populate_header_guess(&mut summary, selection.frame_bytes);

    if selection.frame_bytes.is_empty() {
        return error_result(summary, warnings, issue("EMPTY_FRAME", "未识别到可解析的帧字节"));
    }

    if selection.frame_bytes[0] != 0x68 {
        return error_result(
            summary,
            warnings,
            issue_with_range(
                "INVALID_START_BYTE",
                format!("首字节不是 0x68，当前为 0x{:02X}", selection.frame_bytes[0]),
                Some(range(0, 1)),
            ),
        );
    }

    if selection.frame_bytes.len() < 6 {
        return error_result(
            summary,
            warnings,
            issue_with_range(
                "FRAME_TOO_SHORT",
                format!("IEC104 单帧至少需要 6 字节，当前仅有 {} 字节", selection.frame_bytes.len()),
                Some(range(0, selection.frame_bytes.len())),
            ),
        );
    }

    let apci = match parse_apci(&selection.frame_bytes[..6]) {
        Ok(value) => value,
        Err(error) => {
            return error_result(summary, warnings, map_parse_error("APCI_PARSE_FAILED", error, Some(range(0, 6))));
        }
    };

    apply_apci_to_summary(&mut summary, apci.frame_type, apci.send_seq, apci.recv_seq);
    let mut root_children = vec![build_apci_node(selection.frame_bytes, &apci)];

    if apci.frame_type != FrameType::I {
        return finalize_result(summary, warnings, None, root_children);
    }

    if selection.frame_bytes.len() < 12 {
        warnings.push(issue_with_range(
            "ASDU_HEADER_INCOMPLETE",
            format!(
                "I 帧 APCI 已识别，但 ASDU 头部不足 6 字节，当前仅剩 {} 字节",
                selection.frame_bytes.len().saturating_sub(6)
            ),
            Some(range(6, selection.frame_bytes.len())),
        ));
        root_children.push(build_raw_payload_group("asdu", selection.frame_bytes.get(6..).unwrap_or_default(), 6));
        return finalize_result(summary, warnings, None, root_children);
    }

    let asdu_header = match parse_asdu_header(&selection.frame_bytes[6..]) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(map_parse_error(
                "ASDU_HEADER_PARSE_FAILED",
                error,
                Some(range(6, selection.frame_bytes.len().min(12))),
            ));
            root_children.push(build_raw_payload_group("asdu", &selection.frame_bytes[6..], 6));
            return finalize_result(summary, warnings, None, root_children);
        }
    };

    apply_asdu_to_summary(&mut summary, asdu_header);
    let mut asdu_children = vec![build_asdu_header_group(&asdu_header)];

    let frame = FrameView::new(Bytes::copy_from_slice(selection.frame_bytes), apci, asdu_header);
    match AsduConverter.convert(&frame) {
        Ok(asdu_frame) => {
            let (detail_nodes, detail_warnings) = build_asdu_detail_nodes(&asdu_frame);
            warnings.extend(detail_warnings);
            asdu_children.extend(detail_nodes);
            asdu_children.push(build_raw_payload_group(
                "asdu-payload.raw",
                selection.frame_bytes.get(12..).unwrap_or_default(),
                12,
            ));
        }
        Err(error) => {
            warnings.push(map_convert_error(error));
            asdu_children.push(build_raw_payload_group(
                "asdu-payload",
                selection.frame_bytes.get(12..).unwrap_or_default(),
                12,
            ));
        }
    }

    root_children.push(group_node(
        "asdu",
        "ASDU",
        Some(compose_asdu_summary(&summary)),
        Some(range(6, selection.frame_bytes.len())),
        true,
        asdu_children,
    ));

    finalize_result(summary, warnings, None, root_children)
}

fn normalize_message_input(input_text: &str) -> Result<NormalizedInput, MessageParserIssue> {
    let trimmed = input_text.trim();
    if trimmed.is_empty() {
        return Err(issue("EMPTY_INPUT", "请输入一帧十六进制或 bytes 字节流报文"));
    }

    if let Some(bytes) = try_parse_escaped_bytes(trimmed)? {
        return Ok(NormalizedInput { kind: MessageParserInputKind::EscapedBytes, bytes });
    }

    if let Some(bytes) = try_parse_prefixed_hex_tokens(trimmed)? {
        return Ok(NormalizedInput { kind: MessageParserInputKind::HexPrefixedTokens, bytes });
    }

    if let Some(bytes) = try_parse_byte_array(trimmed)? {
        return Ok(NormalizedInput { kind: MessageParserInputKind::ByteArray, bytes });
    }

    if let Some((bytes, kind)) = try_parse_plain_hex(trimmed)? {
        return Ok(NormalizedInput { kind, bytes });
    }

    Err(issue("UNSUPPORTED_INPUT_FORMAT", "仅支持连续 hex、按字节 hex、0x 列表、\\xNN bytes 或十进制字节数组"))
}

fn try_parse_escaped_bytes(input: &str) -> Result<Option<Vec<u8>>, MessageParserIssue> {
    let source = strip_matching_quotes(strip_optional_byte_string_prefix(input.trim()));
    let source = source.replace("\\\\x", "\\x");
    if !source.contains("\\x") && !source.contains("\\X") {
        return Ok(None);
    }

    let bytes = source.as_bytes();
    let mut cursor = 0usize;
    let mut output = Vec::new();
    let mut found_escape = false;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b' ' | b'\t' | b'\r' | b'\n' | b',' | b';' => {
                cursor += 1;
            }
            b'\\' => {
                found_escape = true;
                if cursor + 3 >= bytes.len() {
                    return Err(issue("INVALID_ESCAPE_SEQUENCE", "bytes 文本中的 \\xNN 转义不完整"));
                }
                let marker = bytes[cursor + 1];
                if marker != b'x' && marker != b'X' {
                    return Err(issue("INVALID_ESCAPE_SEQUENCE", "bytes 文本仅支持 \\xNN 形式的十六进制转义"));
                }
                let pair = std::str::from_utf8(&bytes[cursor + 2..cursor + 4]).unwrap_or_default();
                let value = u8::from_str_radix(pair, 16)
                    .map_err(|_| issue("INVALID_ESCAPE_SEQUENCE", format!("非法的 bytes 转义字节: \\x{pair}")))?;
                output.push(value);
                cursor += 4;
            }
            _ => {
                return Err(issue(
                    "INVALID_ESCAPE_SEQUENCE",
                    "bytes 文本中存在无法识别的字符，请仅保留 \\xNN 序列和分隔符",
                ));
            }
        }
    }

    if found_escape { Ok(Some(output)) } else { Ok(None) }
}

fn try_parse_prefixed_hex_tokens(input: &str) -> Result<Option<Vec<u8>>, MessageParserIssue> {
    let candidate = strip_matching_quotes(strip_optional_byte_string_prefix(input.trim()));
    if !candidate.to_ascii_lowercase().contains("0x") {
        return Ok(None);
    }

    let tokens = candidate
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | '[' | ']'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return Ok(None);
    }

    let mut output = Vec::with_capacity(tokens.len());
    for token in tokens {
        let Some(hex_digits) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) else {
            return Err(issue("INVALID_HEX_TOKEN", format!("0x 列表中存在非法字节 token: {token}")));
        };
        if hex_digits.is_empty() || hex_digits.len() > 2 || !hex_digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(issue("INVALID_HEX_TOKEN", format!("0x 列表中存在非法字节 token: {token}")));
        }
        output.push(
            u8::from_str_radix(hex_digits, 16)
                .map_err(|_| issue("INVALID_HEX_TOKEN", format!("无法解析十六进制字节: {token}")))?,
        );
    }

    Ok(Some(output))
}

fn try_parse_byte_array(input: &str) -> Result<Option<Vec<u8>>, MessageParserIssue> {
    let candidate = strip_matching_quotes(strip_optional_byte_string_prefix(input.trim()));
    if !(candidate.starts_with('[') && candidate.ends_with(']')) {
        return Ok(None);
    }

    let inner = candidate[1..candidate.len() - 1].trim();
    if inner.is_empty() {
        return Err(issue("EMPTY_BYTE_ARRAY", "十进制字节数组不能为空"));
    }

    let tokens = inner
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return Err(issue("EMPTY_BYTE_ARRAY", "十进制字节数组不能为空"));
    }

    if !tokens.iter().all(|token| token.chars().all(|ch| ch.is_ascii_digit())) {
        return Ok(None);
    }

    let mut output = Vec::with_capacity(tokens.len());
    for token in tokens {
        let value =
            token.parse::<u16>().map_err(|_| issue("INVALID_BYTE_DECIMAL", format!("非法的十进制字节: {token}")))?;
        if value > 255 {
            return Err(issue("BYTE_OUT_OF_RANGE", format!("十进制字节超出 0..255 范围: {token}")));
        }
        output.push(value as u8);
    }

    Ok(Some(output))
}

fn try_parse_plain_hex(input: &str) -> Result<Option<(Vec<u8>, MessageParserInputKind)>, MessageParserIssue> {
    let candidate = strip_matching_quotes(strip_optional_byte_string_prefix(input.trim()));
    let has_separators = candidate.chars().any(|ch| ch.is_whitespace() || matches!(ch, ',' | ';' | '[' | ']'));
    let cleaned = candidate
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, ',' | ';' | '[' | ']' | '_'))
        .collect::<String>();

    if cleaned.is_empty() {
        return Ok(None);
    }

    if !cleaned.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Ok(None);
    }

    if cleaned.len() % 2 != 0 {
        return Err(issue("ODD_HEX_LENGTH", "十六进制字符数必须为偶数，请检查是否缺少半个字节"));
    }

    let mut output = Vec::with_capacity(cleaned.len() / 2);
    let mut cursor = 0usize;
    while cursor < cleaned.len() {
        let pair = &cleaned[cursor..cursor + 2];
        output.push(
            u8::from_str_radix(pair, 16)
                .map_err(|_| issue("INVALID_HEX_TOKEN", format!("非法的十六进制字节: {pair}")))?,
        );
        cursor += 2;
    }

    let kind =
        if has_separators { MessageParserInputKind::HexByteTokens } else { MessageParserInputKind::HexContinuous };
    Ok(Some((output, kind)))
}

fn select_frame_bytes<'a>(bytes: &'a [u8], warnings: &mut Vec<MessageParserIssue>) -> FrameSelection<'a> {
    if bytes.len() < 2 || bytes.first().copied() != Some(0x68) {
        return FrameSelection { frame_bytes: bytes, declared_frame_byte_count: None, extra_byte_count: 0 };
    }

    let declared = bytes[1] as usize + 2;
    if declared < 6 {
        warnings.push(issue(
            "INVALID_APDU_LENGTH",
            format!("长度字节声明的总帧长为 {declared} 字节，小于 IEC104 最小帧长 6 字节"),
        ));
        return FrameSelection { frame_bytes: bytes, declared_frame_byte_count: Some(declared), extra_byte_count: 0 };
    }

    if bytes.len() > declared {
        warnings.push(issue_with_range(
            "SUSPECT_MULTI_FRAME",
            format!("检测到超出首帧长度的 {} 字节，当前仅解析第一帧", bytes.len() - declared),
            Some(range(declared, bytes.len())),
        ));
        return FrameSelection {
            frame_bytes: &bytes[..declared],
            declared_frame_byte_count: Some(declared),
            extra_byte_count: bytes.len() - declared,
        };
    }

    if bytes.len() < declared {
        warnings.push(issue(
            "TRUNCATED_FRAME",
            format!("长度字节声明总帧长为 {declared} 字节，但当前仅提供 {} 字节，已按残帧尽力解析", bytes.len()),
        ));
    }

    FrameSelection { frame_bytes: bytes, declared_frame_byte_count: Some(declared), extra_byte_count: 0 }
}

fn build_summary(
    input_kind: Option<MessageParserInputKind>,
    input_bytes: &[u8],
    frame_bytes: &[u8],
    declared_frame_byte_count: Option<usize>,
    extra_byte_count: usize,
) -> MessageFrameParseSummary {
    MessageFrameParseSummary {
        input_kind,
        input_kind_label: input_kind.map(|kind| input_kind_label(kind).to_string()),
        input_byte_count: input_bytes.len(),
        frame_byte_count: frame_bytes.len(),
        extra_byte_count,
        declared_frame_byte_count,
        input_hex: bytes_to_hex(input_bytes),
        frame_hex: bytes_to_hex(frame_bytes),
        normalized_bytes: input_bytes.to_vec(),
        frame_type: None,
        frame_type_label: None,
        apdu_length: frame_bytes.get(1).copied(),
        send_seq: None,
        recv_seq: None,
        type_id: None,
        type_name: None,
        cause_of_transmission: None,
        cause_description: None,
        common_address: None,
        object_count: None,
    }
}

fn populate_header_guess(summary: &mut MessageFrameParseSummary, frame_bytes: &[u8]) {
    if frame_bytes.len() >= 12 {
        let raw_type_id = frame_bytes[6];
        let cot = frame_bytes[8] & 0x3F;
        summary.type_id = Some(raw_type_id);
        summary.type_name = Some(type_name_from_raw(raw_type_id));
        summary.cause_of_transmission = Some(cot);
        summary.cause_description = Some(CotReason::from_u8(cot).description().to_string());
        summary.common_address = Some(u16::from_le_bytes([frame_bytes[10], frame_bytes[11]]));
        summary.object_count = Some((frame_bytes[7] & 0x7F) as usize);
    }
}

fn apply_apci_to_summary(summary: &mut MessageFrameParseSummary, frame_type: FrameType, send_seq: u16, recv_seq: u16) {
    summary.frame_type = Some(frame_type_code(frame_type).to_string());
    summary.frame_type_label = Some(frame_type_label(frame_type).to_string());
    match frame_type {
        FrameType::I => {
            summary.send_seq = Some(send_seq);
            summary.recv_seq = Some(recv_seq);
        }
        FrameType::S => {
            summary.recv_seq = Some(recv_seq);
        }
        FrameType::U => {}
    }
}

fn apply_asdu_to_summary(summary: &mut MessageFrameParseSummary, header: iec60870_parser::parser::asdu::AsduHeader) {
    summary.type_id = Some(header.type_id.raw());
    summary.type_name = Some(type_name(header.type_id));
    summary.cause_of_transmission = Some(header.cause_of_transmission());
    summary.cause_description = Some(header.cot.reason.description().to_string());
    summary.common_address = Some(header.common_addr);
    summary.object_count = Some(header.obj_count() as usize);
}

fn build_apci_node(frame_bytes: &[u8], apci: &iec60870_parser::parser::apci::ApciHeader) -> MessageParserTreeNode {
    let mut children = vec![
        field_node(
            "apci.start",
            "Start",
            format!("0x{:02X}", frame_bytes.first().copied().unwrap_or_default()),
            Some("IEC104 起始字节".to_string()),
            Some(range(0, 1)),
        ),
        field_node(
            "apci.length",
            "Length",
            frame_bytes.get(1).copied().unwrap_or_default().to_string(),
            Some("APDU 长度字段，不包含起始字节和长度字节".to_string()),
            Some(range(1, 2)),
        ),
        field_node(
            "apci.type",
            "Frame Type",
            frame_type_code(apci.frame_type),
            Some(frame_type_label(apci.frame_type).to_string()),
            Some(range(2, 6)),
        ),
    ];

    match apci.frame_type {
        FrameType::I => {
            children.push(field_node(
                "apci.send-seq",
                "Send Sequence",
                apci.send_seq.to_string(),
                Some("发送序号".to_string()),
                Some(range(2, 4)),
            ));
            children.push(field_node(
                "apci.recv-seq",
                "Receive Sequence",
                apci.recv_seq.to_string(),
                Some("确认接收序号".to_string()),
                Some(range(4, 6)),
            ));
        }
        FrameType::S => {
            children.push(field_node(
                "apci.recv-seq",
                "Receive Sequence",
                apci.recv_seq.to_string(),
                Some("监督帧确认接收序号".to_string()),
                Some(range(4, 6)),
            ));
        }
        FrameType::U => {
            let raw = apci.u_control_byte().unwrap_or_default();
            children.push(field_node(
                "apci.u-control",
                "Control",
                format!("0x{raw:02X}"),
                apci.u_control().map(|control| u_control_label(control).to_string()),
                Some(range(2, 3)),
            ));
        }
    }

    let summary = match apci.frame_type {
        FrameType::I => {
            format!("{}，发送 {}，接收 {}", frame_type_label(apci.frame_type), apci.send_seq, apci.recv_seq)
        }
        FrameType::S => format!("{}，接收 {}", frame_type_label(apci.frame_type), apci.recv_seq),
        FrameType::U => apci
            .u_control()
            .map(|control| format!("{}，{}", frame_type_label(apci.frame_type), u_control_label(control)))
            .unwrap_or_else(|| frame_type_label(apci.frame_type).to_string()),
    };

    group_node("apci", "APCI", Some(summary), Some(range(0, frame_bytes.len().min(6))), true, children)
}

fn build_asdu_header_group(header: &iec60870_parser::parser::asdu::AsduHeader) -> MessageParserTreeNode {
    let descriptor = lookup_type_descriptor(header.type_id);
    let mut children = vec![
        field_node(
            "asdu.header.type-id",
            "Type ID",
            header.type_id.raw().to_string(),
            Some(type_name(header.type_id)),
            Some(range(6, 7)),
        ),
        field_node(
            "asdu.header.vsq",
            "VSQ",
            format!("0x{:02X}", header.vsq),
            Some(format!(
                "对象数 {}，{}",
                header.obj_count(),
                if header.is_sequence() { "SQ=1 连续地址" } else { "SQ=0 独立地址" }
            )),
            Some(range(7, 8)),
        ),
        field_node(
            "asdu.header.cot",
            "COT",
            header.cause_of_transmission().to_string(),
            Some(format!(
                "{}，origin={}{}{}",
                header.cot.reason.description(),
                header.cot.origin,
                if header.is_test() { "，test" } else { "" },
                if header.is_negative_confirm() { "，negative" } else { "" }
            )),
            Some(range(8, 10)),
        ),
        field_node("asdu.header.ca", "Common Address", header.common_addr.to_string(), None, Some(range(10, 12))),
    ];

    if let Some(descriptor) = descriptor {
        children.push(field_node(
            "asdu.header.descriptor",
            "Descriptor",
            descriptor.name,
            Some(format!(
                "IOA={}，对象长度={}，时间戳={}",
                descriptor.ioa_len,
                descriptor.entry_len().map(|value| value.to_string()).unwrap_or_else(|| "variable".to_string()),
                descriptor.timestamp_len.map(|value| value.to_string()).unwrap_or_else(|| "none".to_string())
            )),
            None,
        ));
    }

    group_node(
        "asdu.header",
        "Header",
        Some(format!(
            "{}，{} 个对象，{}",
            type_name(header.type_id),
            header.obj_count(),
            header.cot.reason.description()
        )),
        Some(range(6, 12)),
        true,
        children,
    )
}

fn build_asdu_detail_nodes(asdu_frame: &AsduFrame<'_>) -> (Vec<MessageParserTreeNode>, Vec<MessageParserIssue>) {
    match asdu_frame {
        AsduFrame::Measurements(set) => (vec![build_measurements_group(set)], Vec::new()),
        AsduFrame::Commands(set) => (vec![build_commands_group(set)], Vec::new()),
        AsduFrame::Interrogation(frame) => (vec![build_interrogation_group(frame)], Vec::new()),
        AsduFrame::Initialization(frame) => (vec![build_initialization_group(frame)], Vec::new()),
        AsduFrame::TestCommand(frame) => (vec![build_test_command_group(frame)], Vec::new()),
        AsduFrame::Parameters(set) => (vec![build_parameters_group(set)], Vec::new()),
        AsduFrame::Protection(events) => (vec![build_protection_group(events)], Vec::new()),
        AsduFrame::File(set) => (vec![build_file_transfer_group(set)], Vec::new()),
        AsduFrame::Security(frame) => build_security_group(frame),
        AsduFrame::Raw(raw) => (vec![build_raw_payload_group("asdu.raw", raw.payload, 12)], vec![issue(
            "RAW_ASDU_PAYLOAD",
            "当前类型暂未结构化拆解，已展示 ASDU 原始载荷",
        )]),
    }
}

fn build_measurements_group(set: &MeasurementSet<'_>) -> MessageParserTreeNode {
    let children =
        set.items.iter().enumerate().map(|(index, item)| build_measurement_node(index, item)).collect::<Vec<_>>();
    group_node(
        "asdu.measurements",
        "Measurements",
        Some(format!("{} 个信息对象", set.items.len())),
        None,
        true,
        children,
    )
}

fn build_measurement_node(index: usize, item: &Measurement<'_>) -> MessageParserTreeNode {
    let mut children = vec![
        field_node(format!("measurement.{index}.ioa"), "IOA", item.ioa.to_string(), None, None),
        field_node(format!("measurement.{index}.value"), "Value", format_measurement_value(&item.value), None, None),
        field_node(format!("measurement.{index}.quality"), "Quality", format_quality_flags(item.quality), None, None),
    ];

    if item.descriptor != MeasurementDescriptor::None {
        children.push(field_node(
            format!("measurement.{index}.descriptor"),
            "Descriptor",
            format_measurement_descriptor(item.descriptor),
            None,
            None,
        ));
    }

    if let Some(timestamp) = item.timestamp {
        children.push(build_timestamp_node(format!("measurement.{index}.timestamp"), "Timestamp", timestamp));
    }

    group_node(
        format!("measurement.{index}"),
        format!("Object {}", index + 1),
        Some(format!("IOA {}，{}", item.ioa, format_measurement_value(&item.value))),
        None,
        index == 0,
        children,
    )
}

fn build_commands_group(set: &CommandSet<'_>) -> MessageParserTreeNode {
    let children = set
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| build_command_node(set.header.type_id, index, item))
        .collect::<Vec<_>>();
    group_node("asdu.commands", "Commands", Some(format!("{} 个命令对象", set.items.len())), None, true, children)
}

fn build_command_node(type_id: TypeId, index: usize, item: &Command<'_>) -> MessageParserTreeNode {
    let descriptor = lookup_type_descriptor(type_id);
    let has_ioa = descriptor.is_some_and(|value| value.ioa_len > 0);
    let mut children = Vec::new();
    if has_ioa {
        children.push(field_node(format!("command.{index}.ioa"), "IOA", item.ioa.to_string(), None, None));
    }
    children.extend([
        field_node(format!("command.{index}.detail"), "Command", format_command_detail(&item.detail), None, None),
        field_node(
            format!("command.{index}.qualifier"),
            "Qualifier",
            format!("0x{:02X}", item.qualifier.as_u8()),
            Some(format_command_qualifier(item.qualifier)),
            None,
        ),
    ]);

    if let CommandDetail::ClockSync { raw, .. } = &item.detail {
        children.push(build_timestamp_node(format!("command.{index}.clock"), "Clock", raw));
    }

    if let Some(timestamp) = item.timestamp {
        children.push(build_timestamp_node(format!("command.{index}.timestamp"), "Timestamp", timestamp));
    }

    group_node(
        format!("command.{index}"),
        format!("Object {}", index + 1),
        Some(if has_ioa {
            format!("IOA {}，{}", item.ioa, format_command_detail(&item.detail))
        } else {
            format_command_detail(&item.detail)
        }),
        None,
        index == 0,
        children,
    )
}

fn build_initialization_group(frame: &InitializationFrame) -> MessageParserTreeNode {
    let children = frame
        .causes
        .iter()
        .enumerate()
        .map(|(index, cause)| {
            group_node(
                format!("initialization.{index}"),
                format!("Object {}", index + 1),
                Some(format!("COI={}，LPC={}", cause.cause, cause.local_parameter_change)),
                None,
                index == 0,
                vec![field_node(
                    format!("initialization.{index}.coi"),
                    "COI",
                    format!("0x{:02X}", cause.raw),
                    Some(format!("cause={}，local-parameter-change={}", cause.cause, cause.local_parameter_change)),
                    None,
                )],
            )
        })
        .collect();
    group_node(
        "asdu.initialization",
        "Initialization End",
        Some(format!("{} 个初始化原因", frame.causes.len())),
        None,
        true,
        children,
    )
}

fn build_interrogation_group(frame: &InterrogationFrame) -> MessageParserTreeNode {
    let children =
        frame.items.iter().enumerate().map(|(index, item)| build_interrogation_node(index, item)).collect::<Vec<_>>();
    group_node(
        "asdu.interrogation",
        "Interrogation",
        Some(format!("{} 个召唤对象", frame.items.len())),
        None,
        true,
        children,
    )
}

fn build_interrogation_node(index: usize, item: &Interrogation) -> MessageParserTreeNode {
    group_node(
        format!("interrogation.{index}"),
        format!("Object {}", index + 1),
        Some(format!("IOA {}，Q=0x{:02X}", item.ioa, item.qualifier)),
        None,
        index == 0,
        vec![
            field_node(format!("interrogation.{index}.ioa"), "IOA", item.ioa.to_string(), None, None),
            field_node(
                format!("interrogation.{index}.qualifier"),
                "Qualifier",
                format!("0x{:02X}", item.qualifier),
                None,
                None,
            ),
        ],
    )
}

fn build_test_command_group(frame: &TestCommandFrame<'_>) -> MessageParserTreeNode {
    let children =
        frame.items.iter().enumerate().map(|(index, item)| build_test_command_node(index, item)).collect::<Vec<_>>();
    group_node(
        "asdu.test-command",
        "Test Command",
        Some(format!("{} 个测试对象", frame.items.len())),
        None,
        true,
        children,
    )
}

fn build_test_command_node(index: usize, item: &TestCommand<'_>) -> MessageParserTreeNode {
    let mut children = vec![
        field_node(format!("test-command.{index}.ioa"), "IOA", item.ioa.to_string(), None, None),
        field_node(
            format!("test-command.{index}.fbp"),
            "FBP",
            format!("0x{:04X}", item.fbp),
            Some(if item.is_valid_fbp() {
                "标准测试码 0x55AA".to_string()
            } else {
                "非标准测试码".to_string()
            }),
            None,
        ),
    ];
    if let Some(timestamp) = item.timestamp {
        children.push(build_timestamp_node(format!("test-command.{index}.timestamp"), "Timestamp", timestamp));
    }
    group_node(
        format!("test-command.{index}"),
        format!("Object {}", index + 1),
        Some(format!("IOA {}，FBP=0x{:04X}", item.ioa, item.fbp)),
        None,
        index == 0,
        children,
    )
}

fn build_parameters_group(set: &ParameterSet<'_>) -> MessageParserTreeNode {
    let children =
        set.items.iter().enumerate().map(|(index, item)| build_parameter_node(index, item)).collect::<Vec<_>>();
    group_node("asdu.parameters", "Parameters", Some(format!("{} 个参数对象", set.items.len())), None, true, children)
}

fn build_parameter_node(index: usize, item: &Parameter<'_>) -> MessageParserTreeNode {
    group_node(
        format!("parameter.{index}"),
        format!("Object {}", index + 1),
        Some(format!("IOA {}，{}", item.ioa, format_parameter_value(&item.value))),
        None,
        index == 0,
        vec![
            field_node(format!("parameter.{index}.ioa"), "IOA", item.ioa.to_string(), None, None),
            field_node(format!("parameter.{index}.value"), "Value", format_parameter_value(&item.value), None, None),
        ],
    )
}

fn build_protection_group(events: &ProtectionEvents<'_>) -> MessageParserTreeNode {
    let children =
        events.items.iter().enumerate().map(|(index, item)| build_protection_node(index, item)).collect::<Vec<_>>();
    group_node(
        "asdu.protection",
        "Protection Events",
        Some(format!("{} 个保护事件", events.items.len())),
        None,
        true,
        children,
    )
}

fn build_protection_node(index: usize, item: &ProtectionEvent<'_>) -> MessageParserTreeNode {
    let mut children = vec![
        field_node(format!("protection.{index}.ioa"), "IOA", item.ioa.to_string(), None, None),
        field_node(
            format!("protection.{index}.payload"),
            "Payload",
            format_protection_payload(&item.payload),
            None,
            None,
        ),
    ];

    if let Some(details) = item.payload.details() {
        children.extend(build_protection_detail_nodes(index, details));
    }

    if let Some(timestamp) = item.timestamp {
        children.push(build_timestamp_node(format!("protection.{index}.timestamp"), "Timestamp", timestamp));
    }

    group_node(
        format!("protection.{index}"),
        format!("Object {}", index + 1),
        Some(format!("IOA {}，{}", item.ioa, format_protection_payload(&item.payload))),
        None,
        index == 0,
        children,
    )
}

fn build_protection_detail_nodes(index: usize, details: ProtectionEventDetails) -> Vec<MessageParserTreeNode> {
    let (kind, state_label, state, quality, elapsed_ms) = match details {
        ProtectionEventDetails::Single { state, quality, elapsed_ms } => {
            ("SEP", "Event State", state, quality, elapsed_ms)
        }
        ProtectionEventDetails::PackedStart { flags, quality, elapsed_ms } => {
            ("SPE", "Start Flags", flags, quality, elapsed_ms)
        }
        ProtectionEventDetails::PackedOutput { flags, quality, elapsed_ms } => {
            ("OCI", "Output Flags", flags, quality, elapsed_ms)
        }
    };
    vec![
        field_node(
            format!("protection.{index}.state"),
            state_label,
            format!("0x{state:02X}"),
            Some(kind.to_string()),
            None,
        ),
        field_node(
            format!("protection.{index}.quality"),
            "QDP",
            format!("0x{:02X}", quality.raw),
            Some(format_protection_quality(quality)),
            None,
        ),
        field_node(
            format!("protection.{index}.elapsed-time"),
            "Elapsed Time",
            elapsed_ms.to_string(),
            Some("CP16Time2a milliseconds".to_string()),
            None,
        ),
    ]
}

fn build_file_transfer_group(set: &FileTransferSet<'_>) -> MessageParserTreeNode {
    let children =
        set.items.iter().enumerate().map(|(index, item)| build_file_transfer_node(index, item)).collect::<Vec<_>>();
    group_node(
        "asdu.file-transfer",
        "File Transfer",
        Some(format!("{} 条文件传输记录", set.items.len())),
        None,
        true,
        children,
    )
}

fn build_file_transfer_node(index: usize, item: &FileTransferRecord<'_>) -> MessageParserTreeNode {
    group_node(
        format!("file-transfer.{index}"),
        format!("Record {}", index + 1),
        Some(item.to_string()),
        None,
        index == 0,
        vec![field_node(format!("file-transfer.{index}.record"), "Detail", item.to_string(), None, None)],
    )
}

fn build_security_group(frame: &SecurityFrame<'_>) -> (Vec<MessageParserTreeNode>, Vec<MessageParserIssue>) {
    let mut warnings = Vec::new();
    let (summary, children) = match frame {
        SecurityFrame::Challenge(challenge) => {
            (format!("Challenge，sequence={}，user={}", challenge.sequence, challenge.user), vec![
                field_node("security.challenge.sequence", "Sequence", challenge.sequence.to_string(), None, None),
                field_node("security.challenge.user", "User", challenge.user.to_string(), None, None),
                field_node(
                    "security.challenge.mac-algorithm",
                    "MAC Algorithm",
                    challenge.mac_algorithm.to_string(),
                    None,
                    None,
                ),
                field_node("security.challenge.reason", "Reason", challenge.reason.to_string(), None, None),
                field_node(
                    "security.challenge.challenge",
                    "Challenge",
                    bytes_to_spaced_hex(challenge.challenge),
                    Some(format!("{} 字节随机质询", challenge.challenge.len())),
                    None,
                ),
            ])
        }
        SecurityFrame::Reply(reply) => (format!("Reply，sequence={}，user={}", reply.sequence, reply.user), vec![
            field_node("security.reply.sequence", "Sequence", reply.sequence.to_string(), None, None),
            field_node("security.reply.user", "User", reply.user.to_string(), None, None),
            field_node(
                "security.reply.hmac",
                "HMAC",
                bytes_to_spaced_hex(reply.hmac),
                Some(format!("{} 字节", reply.hmac.len())),
                None,
            ),
        ]),
        SecurityFrame::AggressiveRequest(request) => {
            (format!("Aggressive Request，sequence={}，user={}", request.sequence, request.user), vec![
                field_node("security.aggressive.sequence", "Sequence", request.sequence.to_string(), None, None),
                field_node("security.aggressive.user", "User", request.user.to_string(), None, None),
                field_node(
                    "security.aggressive.embedded-asdu",
                    "Embedded ASDU",
                    bytes_to_spaced_hex(request.embedded_asdu),
                    Some(format!("{} 字节内嵌 ASDU", request.embedded_asdu.len())),
                    None,
                ),
                field_node(
                    "security.aggressive.hmac",
                    "HMAC",
                    bytes_to_spaced_hex(request.hmac),
                    Some(format!("{} 字节", request.hmac.len())),
                    None,
                ),
            ])
        }
        SecurityFrame::StatusRequest { user, .. } => (format!("Status Request，user={user}"), vec![field_node(
            "security.status-request.user",
            "User",
            user.to_string(),
            None,
            None,
        )]),
        SecurityFrame::SessionKeyStatus(status) => {
            (format!("Session Key Status，sequence={}，user={}", status.sequence, status.user), vec![
                field_node("security.key-status.sequence", "Sequence", status.sequence.to_string(), None, None),
                field_node("security.key-status.user", "User", status.user.to_string(), None, None),
                field_node(
                    "security.key-status.wrap-algorithm",
                    "Wrap Algorithm",
                    status.wrap_algorithm.to_string(),
                    None,
                    None,
                ),
                field_node("security.key-status.status", "Status", status.status.to_string(), None, None),
                field_node(
                    "security.key-status.challenge",
                    "Challenge",
                    bytes_to_spaced_hex(status.challenge),
                    Some(format!("{} 字节", status.challenge.len())),
                    None,
                ),
                field_node(
                    "security.key-status.hmac",
                    "HMAC",
                    bytes_to_spaced_hex(status.hmac),
                    Some(format!("{} 字节", status.hmac.len())),
                    None,
                ),
            ])
        }
        SecurityFrame::SessionKeyChange(change) => {
            (format!("Session Key Change，sequence={}，user={}", change.sequence, change.user), vec![
                field_node("security.key-change.sequence", "Sequence", change.sequence.to_string(), None, None),
                field_node("security.key-change.user", "User", change.user.to_string(), None, None),
                field_node(
                    "security.key-change.wrapped-key",
                    "Wrapped Key",
                    bytes_to_spaced_hex(change.wrapped_key),
                    Some(format!("{} 字节", change.wrapped_key.len())),
                    None,
                ),
            ])
        }
        SecurityFrame::AuthenticationError(error) => {
            (format!("Authentication Error，code={}，user={}", error.error_code, error.user), vec![
                field_node("security.auth-error.sequence", "Sequence", error.sequence.to_string(), None, None),
                field_node("security.auth-error.user", "User", error.user.to_string(), None, None),
                field_node("security.auth-error.association", "Association", error.association.to_string(), None, None),
                field_node("security.auth-error.error-code", "Error Code", error.error_code.to_string(), None, None),
                field_node(
                    "security.auth-error.timestamp",
                    "Timestamp",
                    format_timestamp_summary(error.timestamp),
                    Some(format!("{} 字节", error.timestamp.len())),
                    None,
                ),
                field_node(
                    "security.auth-error.text",
                    "Text",
                    bytes_to_spaced_hex(error.text),
                    Some(format!("{} 字节文本", error.text.len())),
                    None,
                ),
            ])
        }
        SecurityFrame::Management { payload, .. } => {
            (format!("Management，{} 字节载荷", payload.len()), vec![build_raw_payload_group(
                "security.management",
                payload,
                12,
            )])
        }
        SecurityFrame::Segment(segment) => {
            warnings.push(issue("SECURITY_SEGMENT", "当前安全报文为分段片段，已展示片段内容但未执行跨帧重组"));
            (format!("Segment，FIR={}，FIN={}", segment.fir, segment.fin), vec![
                field_node("security.segment.fir", "FIR", segment.fir.to_string(), None, None),
                field_node("security.segment.fin", "FIN", segment.fin.to_string(), None, None),
                field_node(
                    "security.segment.fragment",
                    "Fragment",
                    bytes_to_spaced_hex(segment.fragment),
                    Some(format!("{} 字节片段", segment.fragment.len())),
                    None,
                ),
            ])
        }
        SecurityFrame::Raw { payload, .. } => {
            warnings.push(issue("SECURITY_RAW_PAYLOAD", "当前安全报文未进一步结构化拆解，已展示原始载荷"));
            (format!("Raw Security Payload，{} 字节", payload.len()), vec![build_raw_payload_group(
                "security.raw",
                payload,
                12,
            )])
        }
    };

    (vec![group_node("asdu.security", "Security", Some(summary), None, true, children)], warnings)
}

fn build_raw_payload_group(id: impl Into<String>, payload: &[u8], offset: usize) -> MessageParserTreeNode {
    let id = id.into();
    group_node(
        id.clone(),
        "Raw Payload",
        Some(format!("{} 字节", payload.len())),
        Some(range(offset, offset + payload.len())),
        true,
        vec![
            field_node(
                format!("{id}.length"),
                "Length",
                payload.len().to_string(),
                None,
                Some(range(offset, offset + payload.len())),
            ),
            field_node(
                format!("{id}.hex"),
                "Hex",
                bytes_to_spaced_hex(payload),
                None,
                Some(range(offset, offset + payload.len())),
            ),
        ],
    )
}

fn build_timestamp_node(
    id: impl Into<String>,
    label: impl Into<String>,
    raw_timestamp: &[u8],
) -> MessageParserTreeNode {
    let id = id.into();
    let label = label.into();
    let mut children = vec![
        field_node(format!("{id}.length"), "Length", raw_timestamp.len().to_string(), None, None),
        field_node(
            format!("{id}.raw"),
            "Raw",
            bytes_to_spaced_hex(raw_timestamp),
            Some(format!("{} 字节", raw_timestamp.len())),
            None,
        ),
    ];

    let summary = match Timestamp::parse(raw_timestamp) {
        Ok(parsed) => {
            children.push(field_node(format!("{id}.type"), "Type", timestamp_type_label(&parsed), None, None));
            children.push(field_node(format!("{id}.formatted"), "Formatted", parsed.to_string(), None, None));
            extend_timestamp_detail_nodes(&id, &parsed, &mut children);
            format!("{}，{}", timestamp_type_label(&parsed), parsed)
        }
        Err(error) => {
            children.push(field_node(format!("{id}.parse-error"), "Parse Error", error.to_string(), None, None));
            format!("{} 字节时间戳（解析失败）", raw_timestamp.len())
        }
    };

    group_node(id, label, Some(summary), None, false, children)
}

fn extend_timestamp_detail_nodes(id: &str, parsed: &Timestamp, children: &mut Vec<MessageParserTreeNode>) {
    match parsed {
        Timestamp::CP24(timestamp) => {
            children.push(field_node(format!("{id}.minutes"), "Minutes", timestamp.minutes.to_string(), None, None));
            children.push(field_node(format!("{id}.seconds"), "Seconds", timestamp.seconds().to_string(), None, None));
            children.push(field_node(
                format!("{id}.milliseconds"),
                "Milliseconds",
                timestamp.millis().to_string(),
                None,
                None,
            ));
            children.push(field_node(format!("{id}.invalid"), "Invalid", timestamp.invalid.to_string(), None, None));
        }
        Timestamp::CP56(timestamp) => {
            children.push(field_node(format!("{id}.year"), "Year", timestamp.year.to_string(), None, None));
            children.push(field_node(format!("{id}.month"), "Month", timestamp.month.to_string(), None, None));
            children.push(field_node(format!("{id}.day"), "Day", timestamp.day.to_string(), None, None));
            children.push(field_node(
                format!("{id}.weekday"),
                "Weekday",
                timestamp.weekday.to_string(),
                Some(cp56_weekday_label(timestamp.weekday).to_string()),
                None,
            ));
            children.push(field_node(format!("{id}.hours"), "Hours", timestamp.hours.to_string(), None, None));
            children.push(field_node(format!("{id}.minutes"), "Minutes", timestamp.minutes.to_string(), None, None));
            children.push(field_node(format!("{id}.seconds"), "Seconds", timestamp.seconds().to_string(), None, None));
            children.push(field_node(
                format!("{id}.milliseconds"),
                "Milliseconds",
                timestamp.millis().to_string(),
                None,
                None,
            ));
            children.push(field_node(
                format!("{id}.summer-time"),
                "Summer Time",
                timestamp.summer_time.to_string(),
                None,
                None,
            ));
            children.push(field_node(format!("{id}.invalid"), "Invalid", timestamp.invalid.to_string(), None, None));
        }
    }
}

fn format_timestamp_summary(raw_timestamp: &[u8]) -> String {
    Timestamp::parse(raw_timestamp)
        .map(|parsed| format!("{} ({})", parsed, timestamp_type_label(&parsed)))
        .unwrap_or_else(|_| bytes_to_spaced_hex(raw_timestamp))
}

fn timestamp_type_label(parsed: &Timestamp) -> &'static str {
    match parsed {
        Timestamp::CP24(_) => "CP24Time2a",
        Timestamp::CP56(_) => "CP56Time2a",
    }
}

fn cp56_weekday_label(weekday: u8) -> &'static str {
    match weekday {
        0 => "未提供",
        1 => "周一",
        2 => "周二",
        3 => "周三",
        4 => "周四",
        5 => "周五",
        6 => "周六",
        7 => "周日",
        _ => "非法值",
    }
}

fn finalize_result(
    summary: MessageFrameParseSummary,
    warnings: Vec<MessageParserIssue>,
    error: Option<MessageParserIssue>,
    children: Vec<MessageParserTreeNode>,
) -> MessageFrameParseResult {
    let status = if error.is_some() {
        MessageParserStatus::Error
    } else if warnings.is_empty() {
        MessageParserStatus::Success
    } else {
        MessageParserStatus::Partial
    };

    MessageFrameParseResult {
        status,
        summary: summary.clone(),
        warnings,
        error,
        tree: vec![group_node(
            "frame",
            "Frame",
            Some(compose_root_summary(&summary)),
            Some(range(0, summary.frame_byte_count)),
            true,
            children,
        )],
    }
}

fn error_result(
    summary: MessageFrameParseSummary,
    warnings: Vec<MessageParserIssue>,
    error: MessageParserIssue,
) -> MessageFrameParseResult {
    let mut children = Vec::new();
    if !summary.normalized_bytes.is_empty() {
        children.push(build_raw_payload_group("frame.raw-input", &summary.normalized_bytes, 0));
    }
    finalize_result(summary, warnings, Some(error), children)
}

fn compose_root_summary(summary: &MessageFrameParseSummary) -> String {
    let mut parts = Vec::new();
    if let Some(frame_type) = &summary.frame_type_label {
        parts.push(frame_type.clone());
    }
    if let Some(type_name) = &summary.type_name {
        parts.push(type_name.clone());
    }
    parts.push(format!("{} 字节", summary.frame_byte_count));
    if let Some(cause) = &summary.cause_description {
        parts.push(cause.clone());
    }
    parts.join("，")
}

fn compose_asdu_summary(summary: &MessageFrameParseSummary) -> String {
    let mut parts = Vec::new();
    if let Some(type_name) = &summary.type_name {
        parts.push(type_name.clone());
    }
    if let Some(cause) = &summary.cause_description {
        parts.push(cause.clone());
    }
    if let Some(common_address) = summary.common_address {
        parts.push(format!("CA={common_address}"));
    }
    if let Some(object_count) = summary.object_count {
        parts.push(format!("{object_count} 个对象"));
    }
    parts.join("，")
}

fn field_node(
    id: impl Into<String>,
    label: impl Into<String>,
    value: impl Into<String>,
    summary: Option<String>,
    byte_range: Option<MessageParserByteRange>,
) -> MessageParserTreeNode {
    MessageParserTreeNode {
        id: id.into(),
        label: label.into(),
        value: Some(value.into()),
        summary,
        byte_range,
        tone: None,
        default_expanded: false,
        children: Vec::new(),
    }
}

fn group_node(
    id: impl Into<String>,
    label: impl Into<String>,
    summary: Option<String>,
    byte_range: Option<MessageParserByteRange>,
    default_expanded: bool,
    children: Vec<MessageParserTreeNode>,
) -> MessageParserTreeNode {
    MessageParserTreeNode {
        id: id.into(),
        label: label.into(),
        value: None,
        summary,
        byte_range,
        tone: None,
        default_expanded,
        children,
    }
}

fn range(start: usize, end: usize) -> MessageParserByteRange {
    MessageParserByteRange { start, end }
}

fn issue(code: impl Into<String>, message: impl Into<String>) -> MessageParserIssue {
    MessageParserIssue { code: code.into(), message: message.into(), detail: None, byte_range: None }
}

fn issue_with_range(
    code: impl Into<String>,
    message: impl Into<String>,
    byte_range: Option<MessageParserByteRange>,
) -> MessageParserIssue {
    MessageParserIssue { code: code.into(), message: message.into(), detail: None, byte_range }
}

fn map_parse_error(
    code: impl Into<String>,
    error: ParseError,
    byte_range: Option<MessageParserByteRange>,
) -> MessageParserIssue {
    MessageParserIssue { code: code.into(), message: error.to_string(), detail: Some(format!("{error:?}")), byte_range }
}

fn map_convert_error(error: ConvertError) -> MessageParserIssue {
    match error {
        ConvertError::Unsupported(type_id) => MessageParserIssue {
            code: "UNSUPPORTED_TYPE".to_string(),
            message: format!("暂不支持对 {} 做结构化拆解", type_name(type_id)),
            detail: None,
            byte_range: Some(range(12, 12)),
        },
        ConvertError::UnsupportedFrame(frame_type) => MessageParserIssue {
            code: "UNSUPPORTED_FRAME".to_string(),
            message: format!("{} 不包含可转换的 ASDU 载荷", frame_type_label(frame_type)),
            detail: None,
            byte_range: Some(range(0, 6)),
        },
        ConvertError::InvalidField { ctx, source } => MessageParserIssue {
            code: "ASDU_CONVERT_FAILED".to_string(),
            message: format!("{ctx} 解析失败：{source}"),
            detail: Some(format!("{source:?}")),
            byte_range: Some(range(12, 12)),
        },
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join("")
}

fn bytes_to_spaced_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ")
}

fn type_name(type_id: TypeId) -> String {
    lookup_type_descriptor(type_id)
        .map(|descriptor| descriptor.name.to_string())
        .unwrap_or_else(|| type_name_from_raw(type_id.raw()))
}

fn type_name_from_raw(raw: u8) -> String {
    TypeId::from_raw(raw).map(type_name).unwrap_or_else(|| format!("Unknown({raw})"))
}

fn frame_type_code(frame_type: FrameType) -> &'static str {
    match frame_type {
        FrameType::I => "I",
        FrameType::S => "S",
        FrameType::U => "U",
    }
}

fn frame_type_label(frame_type: FrameType) -> &'static str {
    match frame_type {
        FrameType::I => "I 帧",
        FrameType::S => "S 帧",
        FrameType::U => "U 帧",
    }
}

fn input_kind_label(kind: MessageParserInputKind) -> &'static str {
    match kind {
        MessageParserInputKind::HexContinuous => "连续 Hex",
        MessageParserInputKind::HexByteTokens => "按字节 Hex",
        MessageParserInputKind::HexPrefixedTokens => "0x 列表",
        MessageParserInputKind::EscapedBytes => "Bytes 转义串",
        MessageParserInputKind::ByteArray => "十进制字节数组",
    }
}

fn u_control_label(control: UControl) -> &'static str {
    match control {
        UControl::STARTDT_ACT => "STARTDT act",
        UControl::STARTDT_CON => "STARTDT con",
        UControl::STOPDT_ACT => "STOPDT act",
        UControl::STOPDT_CON => "STOPDT con",
        UControl::TESTFR_ACT => "TESTFR act",
        UControl::TESTFR_CON => "TESTFR con",
    }
}

fn format_measurement_value(value: &MeasurementValue<'_>) -> String {
    match value {
        MeasurementValue::SinglePoint { state } => format!("SinglePoint({state})"),
        MeasurementValue::DoublePoint { state } => format!("DoublePoint({state})"),
        MeasurementValue::StepPosition { value, transient } => {
            format!("StepPosition(value={value}, transient={transient})")
        }
        MeasurementValue::Normalized(raw) => format!("Normalized({raw}, {:.6})", *raw as f32 / 32768.0),
        MeasurementValue::Scaled(raw) => format!("Scaled({raw})"),
        MeasurementValue::NormalizedNoQuality(raw) => {
            format!("NormalizedNoQuality({raw}, {:.6})", *raw as f32 / 32768.0)
        }
        MeasurementValue::ShortFloat(raw) => format!("ShortFloat({raw:.6})"),
        MeasurementValue::BitString(bits) => {
            let value =
                bits.get(..4).map(|raw| u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])).unwrap_or_default();
            format!("BitString(0x{value:08X}, {value})")
        }
        MeasurementValue::IntegratedTotal { value, sequence } => {
            format!("IntegratedTotal(value={value}, sequence={sequence})")
        }
        MeasurementValue::PackedStatus { status, change } => {
            format!("PackedStatus(status=0x{status:04X}, change=0x{change:04X})")
        }
        MeasurementValue::Raw(bits) => format!("Raw({})", bytes_to_spaced_hex(bits)),
    }
}

fn format_measurement_descriptor(descriptor: MeasurementDescriptor) -> String {
    match descriptor {
        MeasurementDescriptor::Siq { raw, spi, iv, nt, sb, bl } => {
            format!("SIQ(raw=0x{raw:02X}, spi={spi}, iv={iv}, nt={nt}, sb={sb}, bl={bl})")
        }
        MeasurementDescriptor::Diq { raw, dpi, iv, nt, sb, bl } => {
            format!("DIQ(raw=0x{raw:02X}, dpi={dpi}, iv={iv}, nt={nt}, sb={sb}, bl={bl})")
        }
        MeasurementDescriptor::Qds { raw, ov, iv, nt, sb, bl, transient, scd_status, scd_change } => {
            format!(
                "QDS(raw=0x{raw:02X}, ov={ov}, iv={iv}, nt={nt}, sb={sb}, bl={bl}, transient={transient:?}, scd_status={scd_status:?}, scd_change={scd_change:?})"
            )
        }
        MeasurementDescriptor::Bcr { raw, sq, cy, ca, iv } => {
            format!("BCR(raw=0x{raw:02X}, sq={sq}, cy={cy}, ca={ca}, iv={iv})")
        }
        MeasurementDescriptor::None => "None".to_string(),
    }
}

fn format_quality_flags(flags: QualityFlags) -> String {
    if flags.is_good() { "GOOD".to_string() } else { flags.to_string() }
}

fn format_command_detail(detail: &CommandDetail<'_>) -> String {
    match detail {
        CommandDetail::Single { state, select } => {
            format!("SingleCommand({state}, {})", if *select { "SELECT" } else { "EXECUTE" })
        }
        CommandDetail::Double { state, select } => {
            format!("DoubleCommand({state}, {})", if *select { "SELECT" } else { "EXECUTE" })
        }
        CommandDetail::Step { position } => format!("StepCommand(position={position})"),
        CommandDetail::SetPointNormalized(value) => {
            format!("SetPointNormalized({value}, {:.6})", *value as f32 / 32768.0)
        }
        CommandDetail::SetPointScaled(value) => format!("SetPointScaled({value})"),
        CommandDetail::SetPointFloat(value) => format!("SetPointFloat({value:.6})"),
        CommandDetail::BitString(bits) => {
            let value =
                bits.get(..4).map(|raw| u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])).unwrap_or_default();
            format!("BitString(0x{value:08X}, {value})")
        }
        CommandDetail::Read => "Read".to_string(),
        CommandDetail::ClockSync { timestamp, .. } => format!("ClockSync({timestamp})"),
        CommandDetail::ResetProcess => "ResetProcess".to_string(),
        CommandDetail::Raw(bits) => format!("Raw({})", bytes_to_spaced_hex(bits)),
    }
}

fn format_command_qualifier(qualifier: CommandQualifier) -> String {
    match qualifier {
        CommandQualifier::Sco { raw, select } => {
            format!(
                "SCO(raw=0x{raw:02X}, state={}, QU={}, mode={})",
                raw & 0x01,
                (raw >> 2) & 0x1F,
                if select { "SELECT" } else { "EXECUTE" }
            )
        }
        CommandQualifier::Dco { raw, select } => {
            format!(
                "DCO(raw=0x{raw:02X}, state={}, QU={}, mode={})",
                raw & 0x03,
                (raw >> 2) & 0x1F,
                if select { "SELECT" } else { "EXECUTE" }
            )
        }
        CommandQualifier::Rco { raw, select } => {
            format!(
                "RCO(raw=0x{raw:02X}, state={}, QU={}, mode={})",
                raw & 0x03,
                (raw >> 2) & 0x1F,
                if select { "SELECT" } else { "EXECUTE" }
            )
        }
        CommandQualifier::Qos(value) => value.to_string(),
        CommandQualifier::Qoi(value) => value.to_string(),
        CommandQualifier::Qcc(value) => value.to_string(),
        CommandQualifier::Qrp(value) => value.to_string(),
        CommandQualifier::None => "None".to_string(),
        CommandQualifier::Raw(value) => format!("Raw(0x{value:02X})"),
    }
}

fn format_parameter_value(value: &ParameterValue<'_>) -> String {
    match value {
        ParameterValue::Normalized { value, qualifier } => {
            format!("Normalized({value}, qpm=0x{qualifier:02X})")
        }
        ParameterValue::Scaled { value, qualifier } => format!("Scaled({value}, qpm=0x{qualifier:02X})"),
        ParameterValue::ShortFloat { value, qualifier } => {
            format!("ShortFloat({value:.6}, qpm=0x{qualifier:02X})")
        }
        ParameterValue::Activation(value) => format!("Activation(0x{value:02X})"),
        ParameterValue::Raw(bits) => format!("Raw({})", bytes_to_spaced_hex(bits)),
    }
}

fn format_protection_payload(payload: &ProtectionEventData<'_>) -> String {
    match payload {
        ProtectionEventData::Single(bytes) => format!("Single({})", bytes_to_spaced_hex(bytes)),
        ProtectionEventData::PackedStart(bytes) => format!("PackedStart({})", bytes_to_spaced_hex(bytes)),
        ProtectionEventData::PackedOutput(bytes) => format!("PackedOutput({})", bytes_to_spaced_hex(bytes)),
        ProtectionEventData::Raw(bytes) => format!("Raw({})", bytes_to_spaced_hex(bytes)),
    }
}

fn format_protection_quality(quality: ProtectionQuality) -> String {
    format!(
        "EI={} BL={} SB={} NT={} IV={}",
        quality.elapsed_time_invalid, quality.blocked, quality.substituted, quality.not_topical, quality.invalid
    )
}

fn strip_optional_byte_string_prefix(input: &str) -> &str {
    if input.len() >= 2 {
        let bytes = input.as_bytes();
        if (bytes[0] == b'b' || bytes[0] == b'B') && (bytes[1] == b'"' || bytes[1] == b'\'') {
            return &input[1..];
        }
    }
    input
}

fn strip_matching_quotes(input: &str) -> &str {
    let trimmed = input.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &trimmed[1..trimmed.len() - 1];
        }
    }
    trimmed
}
