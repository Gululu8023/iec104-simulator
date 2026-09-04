//! 点表导入模块。
//!
//! 本模块提供从外部文件导入数据点配置的功能,支持多种格式和灵活的字段映射,包括:
//!
//! - **多格式支持**:支持 XML、CSV、JSON 三种点表文件格式
//! - **编码检测**:自动检测 UTF-8、UTF-16LE、UTF-16BE 等编码格式
//! - **字段映射**:支持多种字段名称变体(驼峰、下划线、中文等)
//! - **数据验证**:检测重复地址、非法类型、缺失字段等问题
//! - **ASDU 别名**:支持自定义 ASDU 类型的显示名称

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use quick_xml::de::from_str as from_xml_str;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value};

use crate::{
    api::types::{
        PointTableImportFormat, PointTableImportIssue, PointTableImportNormalizedPayload,
        PointTableImportSourceCommonAddress,
    },
    core::{
        iec104_registry::{iec104_default_point_name, iec104_type_id, iec104_type_name},
        types::{PointDef, PointSource},
    },
};

/// 点表导入预览限制(最多显示 200 个点位)
pub(crate) const POINT_TABLE_IMPORT_PREVIEW_LIMIT: usize = 200;

struct CsvFieldDefinition {
    key: &'static str,
    zh_cn: &'static str,
}

const POINT_TABLE_CSV_FIELDS: [CsvFieldDefinition; 10] = [
    CsvFieldDefinition { key: "address", zh_cn: "信息对象地址" },
    CsvFieldDefinition { key: "name", zh_cn: "点名" },
    CsvFieldDefinition { key: "type_id", zh_cn: "类型ID" },
    CsvFieldDefinition { key: "data_type", zh_cn: "类型名称" },
    CsvFieldDefinition { key: "description", zh_cn: "描述" },
    CsvFieldDefinition { key: "control_ioa", zh_cn: "控制映射IOA" },
    CsvFieldDefinition { key: "gi_group", zh_cn: "总召组" },
    CsvFieldDefinition { key: "counter_group", zh_cn: "电度组" },
    CsvFieldDefinition { key: "default_value", zh_cn: "初始值" },
    CsvFieldDefinition { key: "is_enabled", zh_cn: "启用" },
];

pub(crate) fn build_point_table_csv_template(locale: &str) -> (String, Vec<u8>) {
    let english = locale.eq_ignore_ascii_case("en-US");
    let headers = POINT_TABLE_CSV_FIELDS
        .iter()
        .map(|field| if english { field.key } else { field.zh_cn })
        .collect::<Vec<_>>()
        .join(",");
    let rows = if english {
        [
            "1001,Breaker closed,1,M_SP_NA_1,Position feedback,6001,1,,0,true",
            "1002,Breaker position,3,M_DP_NA_1,Double-point position,,2,,1,true",
            "2001,Bus voltage,13,M_ME_NC_1,Bus voltage in kV,,1,,110.5,true",
            "4001,Energy total,15,M_IT_NA_1,Integrated total,,,1,0,true",
            "6001,Breaker command,45,C_SC_NA_1,Single command,,,,,true",
            "3001,Bitstring status,7,M_BO_NA_1,32-bit status,,,,0x00000000,true",
            "6101,Bitstring command,51,C_BO_NA_1,32-bit command,,,,0x00000000,true",
        ]
    } else {
        [
            "1001,断路器合位,1,M_SP_NA_1,位置反馈,6001,1,,0,true",
            "1002,断路器位置,3,M_DP_NA_1,双点位置状态,,2,,1,true",
            "2001,母线电压,13,M_ME_NC_1,母线电压kV,,1,,110.5,true",
            "4001,正向有功总电度,15,M_IT_NA_1,累计电度量,,,1,0,true",
            "6001,断路器遥控,45,C_SC_NA_1,单点遥控,,,,,true",
            "3001,位串状态,7,M_BO_NA_1,32位状态,,,,0x00000000,true",
            "6101,位串命令,51,C_BO_NA_1,32位命令,,,,0x00000000,true",
        ]
    };
    let mut content = String::from("\u{feff}");
    content.push_str(&headers);
    content.push_str("\r\n");
    content.push_str(&rows.join("\r\n"));
    content.push_str("\r\n");
    let file_name = if english { "IEC104-point-table-template.csv" } else { "IEC104-点表模板.csv" };
    (file_name.to_string(), content.into_bytes())
}

/// 解码后的点表导入文件
#[derive(Debug, Clone)]
pub(crate) struct DecodedPointTableImportFile {
    pub(crate) file_path: String,
    pub(crate) file_name: String,
    pub(crate) file_size: u64,
    pub(crate) format: PointTableImportFormat,
    pub(crate) encoding: String,
    pub(crate) total_points: usize,
    pub(crate) duplicate_address_count: usize,
    pub(crate) payload: PointTableImportNormalizedPayload,
    pub(crate) points_preview: Vec<PointDef>,
    pub(crate) errors: Vec<PointTableImportIssue>,
    pub(crate) warnings: Vec<PointTableImportIssue>,
}

#[derive(Debug, Clone)]
struct ParsedPointRecord {
    def: PointDef,
    location: String,
    common_address: Option<u16>,
}

#[derive(Debug, Clone)]
struct RawPointRecord {
    location: String,
    address: Option<Value>,
    name: Option<Value>,
    type_id: Option<Value>,
    data_type: Option<Value>,
    description: Option<Value>,
    control_ioa: Option<Value>,
    gi_group: Option<Value>,
    counter_group: Option<Value>,
    default_value: Option<Value>,
    is_enabled: Option<Value>,
    is_double_point: Option<Value>,
    common_address: Option<Value>,
}

#[derive(Debug, Clone)]
struct ParsePointTableFileResult {
    points: Vec<ParsedPointRecord>,
    asdu_aliases: HashMap<String, String>,
    errors: Vec<PointTableImportIssue>,
    warnings: Vec<PointTableImportIssue>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename = "PointTable")]
struct XmlPointTableDocument {
    #[serde(rename = "Points", alias = "points", default)]
    points: Option<XmlPointList>,
    #[serde(rename = "Point", alias = "point", default)]
    point: Vec<XmlPointEntry>,
    #[serde(rename = "AsduAliases", alias = "asdu_aliases", alias = "asduAliases", alias = "Aliases", default)]
    asdu_aliases: Option<XmlAliasList>,
}

#[derive(Debug, Deserialize, Default)]
struct XmlPointList {
    #[serde(rename = "Point", alias = "point", default)]
    point: Vec<XmlPointEntry>,
}

#[derive(Debug, Deserialize, Default)]
struct XmlAliasList {
    #[serde(rename = "Alias", alias = "alias", default)]
    alias: Vec<XmlAliasEntry>,
}

#[derive(Debug, Deserialize, Default)]
struct XmlPointEntry {
    #[serde(rename = "@ioa", default)]
    attr_ioa: Option<String>,
    #[serde(rename = "@IOA", default)]
    attr_ioa_upper: Option<String>,
    #[serde(rename = "@address", default)]
    attr_address: Option<String>,
    #[serde(rename = "@Address", default)]
    attr_address_upper: Option<String>,
    #[serde(rename = "ioa", default)]
    ioa: Option<String>,
    #[serde(rename = "IOA", default)]
    ioa_upper: Option<String>,
    #[serde(rename = "address", default)]
    address: Option<String>,
    #[serde(rename = "Address", default)]
    address_upper: Option<String>,
    #[serde(rename = "@name", default)]
    attr_name: Option<String>,
    #[serde(rename = "@Name", default)]
    attr_name_upper: Option<String>,
    #[serde(rename = "name", default)]
    name: Option<String>,
    #[serde(rename = "Name", default)]
    name_upper: Option<String>,
    #[serde(rename = "@description", default)]
    attr_description: Option<String>,
    #[serde(rename = "@Description", default)]
    attr_description_upper: Option<String>,
    #[serde(rename = "@desc", default)]
    attr_desc: Option<String>,
    #[serde(rename = "description", default)]
    description: Option<String>,
    #[serde(rename = "Description", default)]
    description_upper: Option<String>,
    #[serde(rename = "desc", default)]
    desc: Option<String>,
    #[serde(rename = "@type_id", default)]
    attr_type_id: Option<String>,
    #[serde(rename = "@typeId", default)]
    attr_type_id_camel: Option<String>,
    #[serde(rename = "@TypeId", default)]
    attr_type_id_upper: Option<String>,
    #[serde(rename = "@type", default)]
    attr_type_name: Option<String>,
    #[serde(rename = "@data_type", default)]
    attr_data_type: Option<String>,
    #[serde(rename = "@dataType", default)]
    attr_data_type_camel: Option<String>,
    #[serde(rename = "@typeName", default)]
    attr_type_name_camel: Option<String>,
    #[serde(rename = "type_id", default)]
    type_id: Option<String>,
    #[serde(rename = "typeId", default)]
    type_id_camel: Option<String>,
    #[serde(rename = "TypeId", default)]
    type_id_upper: Option<String>,
    #[serde(rename = "type", default)]
    type_name: Option<String>,
    #[serde(rename = "data_type", default)]
    data_type: Option<String>,
    #[serde(rename = "dataType", default)]
    data_type_camel: Option<String>,
    #[serde(rename = "typeName", default)]
    type_name_camel: Option<String>,
    #[serde(rename = "@control_ioa", default)]
    attr_control_ioa: Option<String>,
    #[serde(rename = "@controlIoa", default)]
    attr_control_ioa_camel: Option<String>,
    #[serde(rename = "@ControlIOA", default)]
    attr_control_ioa_upper: Option<String>,
    #[serde(rename = "control_ioa", default)]
    control_ioa: Option<String>,
    #[serde(rename = "controlIoa", default)]
    control_ioa_camel: Option<String>,
    #[serde(rename = "ControlIOA", default)]
    control_ioa_upper: Option<String>,
    #[serde(rename = "@gi_group", default)]
    attr_gi_group: Option<String>,
    #[serde(rename = "@giGroup", default)]
    attr_gi_group_camel: Option<String>,
    #[serde(rename = "gi_group", default)]
    gi_group: Option<String>,
    #[serde(rename = "giGroup", default)]
    gi_group_camel: Option<String>,
    #[serde(rename = "@counter_group", default)]
    attr_counter_group: Option<String>,
    #[serde(rename = "@counterGroup", default)]
    attr_counter_group_camel: Option<String>,
    #[serde(rename = "counter_group", default)]
    counter_group: Option<String>,
    #[serde(rename = "counterGroup", default)]
    counter_group_camel: Option<String>,
    #[serde(rename = "@default_value", default)]
    attr_default_value: Option<String>,
    #[serde(rename = "@defaultValue", default)]
    attr_default_value_camel: Option<String>,
    #[serde(rename = "default_value", default)]
    default_value: Option<String>,
    #[serde(rename = "defaultValue", default)]
    default_value_camel: Option<String>,
    #[serde(rename = "@enabled", default)]
    attr_enabled: Option<String>,
    #[serde(rename = "@isEnabled", default)]
    attr_is_enabled: Option<String>,
    #[serde(rename = "@is_enabled", default)]
    attr_is_enabled_snake: Option<String>,
    #[serde(rename = "enabled", default)]
    enabled: Option<String>,
    #[serde(rename = "isEnabled", default)]
    is_enabled: Option<String>,
    #[serde(rename = "is_enabled", default)]
    is_enabled_snake: Option<String>,
    #[serde(rename = "@is_double_point", default)]
    attr_is_double_point: Option<String>,
    #[serde(rename = "@isDoublePoint", default)]
    attr_is_double_point_camel: Option<String>,
    #[serde(rename = "@double_point", default)]
    attr_double_point: Option<String>,
    #[serde(rename = "is_double_point", default)]
    is_double_point: Option<String>,
    #[serde(rename = "isDoublePoint", default)]
    is_double_point_camel: Option<String>,
    #[serde(rename = "double_point", default)]
    double_point: Option<String>,
    #[serde(rename = "双点", default)]
    double_point_cn: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct XmlAliasEntry {
    #[serde(rename = "@key", default)]
    attr_key: Option<String>,
    #[serde(rename = "@type", default)]
    attr_type: Option<String>,
    #[serde(rename = "@asdu_type", default)]
    attr_asdu_type: Option<String>,
    #[serde(rename = "@value", default)]
    attr_value: Option<String>,
    #[serde(rename = "key", default)]
    key: Option<String>,
    #[serde(rename = "Key", default)]
    key_upper: Option<String>,
    #[serde(rename = "type", default)]
    type_name: Option<String>,
    #[serde(rename = "Type", default)]
    type_name_upper: Option<String>,
    #[serde(rename = "asdu_type", default)]
    asdu_type: Option<String>,
    #[serde(rename = "AsduType", default)]
    asdu_type_upper: Option<String>,
    #[serde(rename = "value", default)]
    value: Option<String>,
    #[serde(rename = "Value", default)]
    value_upper: Option<String>,
    #[serde(rename = "$text", default)]
    body_text: Option<String>,
}

pub(crate) fn decode_point_table_import_file(file_path: &str) -> Result<DecodedPointTableImportFile, String> {
    let bytes = fs::read(file_path).map_err(|err| format!("读取点表文件失败: {err}"))?;
    if bytes.is_empty() {
        return Err("点表文件为空".to_string());
    }

    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| file_path.to_string());

    let (text, encoding, mut decode_warnings) = decode_text(&bytes);
    let format = detect_import_format(file_path, &text)?;
    let mut parsed = match format {
        PointTableImportFormat::Xml => parse_xml_point_table(&text)?,
        PointTableImportFormat::Csv => parse_csv_point_table(&text),
        PointTableImportFormat::Json => parse_json_point_table(&text)?,
    };

    parsed.warnings.append(&mut decode_warnings);
    let mut retained_warnings = Vec::new();
    for item in parsed.warnings.drain(..) {
        if matches!(item.code.as_str(), "INVALID_TYPE_ID" | "INVALID_COMMON_ADDRESS") {
            parsed.errors.push(item);
        } else {
            retained_warnings.push(item);
        }
    }
    parsed.warnings = retained_warnings;
    let payload = build_normalized_payload(&parsed.points, parsed.asdu_aliases);
    let points_preview = payload.point_defs.iter().take(POINT_TABLE_IMPORT_PREVIEW_LIMIT).cloned().collect::<Vec<_>>();

    if payload.point_defs.is_empty() {
        parsed.errors.push(issue("EMPTY_POINT_TABLE", "未解析到可导入的点表定义", None::<String>));
    }

    let duplicate_address_count = parsed.errors.iter().filter(|item| item.code == "CONFLICT_IOA_DUPLICATE").count();

    Ok(DecodedPointTableImportFile {
        file_path: file_path.to_string(),
        file_name,
        file_size: bytes.len() as u64,
        format,
        encoding,
        total_points: parsed.points.len(),
        duplicate_address_count,
        payload,
        points_preview,
        errors: parsed.errors,
        warnings: parsed.warnings,
    })
}

fn decode_text(bytes: &[u8]) -> (String, String, Vec<PointTableImportIssue>) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(rest).into_owned(), "utf-8-bom".to_string(), Vec::new());
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let words = rest.chunks_exact(2).map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]])).collect::<Vec<_>>();
        return (String::from_utf16_lossy(&words), "utf-16le".to_string(), Vec::new());
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let words = rest.chunks_exact(2).map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]])).collect::<Vec<_>>();
        return (String::from_utf16_lossy(&words), "utf-16be".to_string(), Vec::new());
    }
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => (text, "utf-8".to_string(), Vec::new()),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), "utf-8-lossy".to_string(), vec![issue(
            "FILE_ENCODING_LOSSY",
            "文件不是标准 UTF-8 文本，已按容错模式解码，个别字符可能失真",
            None::<String>,
        )]),
    }
}

fn detect_import_format(file_path: &str, text: &str) -> Result<PointTableImportFormat, String> {
    let extension = Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_ascii_lowercase());

    match extension.as_deref() {
        Some("xml") => return Ok(PointTableImportFormat::Xml),
        Some("csv") => return Ok(PointTableImportFormat::Csv),
        Some("json") => return Ok(PointTableImportFormat::Json),
        _ => {}
    }

    let trimmed = text.trim_start();
    if trimmed.starts_with('<') {
        return Ok(PointTableImportFormat::Xml);
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok(PointTableImportFormat::Json);
    }
    if trimmed.contains(',') {
        return Ok(PointTableImportFormat::Csv);
    }

    Err("无法识别点表文件格式，仅支持 XML / CSV / JSON".to_string())
}

fn parse_json_point_table(text: &str) -> Result<ParsePointTableFileResult, String> {
    let root: Value = serde_json::from_str(text).map_err(|err| format!("解析 JSON 点表失败: {err}"))?;
    let mut result = ParsePointTableFileResult {
        points: Vec::new(),
        asdu_aliases: HashMap::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    match root {
        Value::Array(items) => {
            for (index, item) in items.into_iter().enumerate() {
                let location = format!("JSON root[{index}]");
                let Some(record) = item.as_object() else {
                    result.errors.push(issue("INVALID_POINT_RECORD", "点表记录必须是对象", Some(location)));
                    continue;
                };
                if let Some(point) = normalize_raw_point_record(
                    build_json_raw_point_record(record, location.clone()),
                    &mut result.warnings,
                ) {
                    result.points.push(point);
                }
            }
        }
        Value::Object(map) => {
            let points_value = map.get("points").or_else(|| map.get("point_defs")).or_else(|| map.get("pointDefs"));
            let Some(points_value) = points_value else {
                return Err("JSON 点表必须是数组，或包含 points / point_defs 字段".to_string());
            };
            let Some(items) = points_value.as_array() else {
                return Err("JSON points / point_defs 必须是数组".to_string());
            };
            for (index, item) in items.iter().enumerate() {
                let location = format!("JSON points[{index}]");
                let Some(record) = item.as_object() else {
                    result.errors.push(issue("INVALID_POINT_RECORD", "点表记录必须是对象", Some(location)));
                    continue;
                };
                if let Some(point) = normalize_raw_point_record(
                    build_json_raw_point_record(record, location.clone()),
                    &mut result.warnings,
                ) {
                    result.points.push(point);
                }
            }

            if let Some(alias_value) = map.get("asdu_aliases").or_else(|| map.get("asduAliases")) {
                result.asdu_aliases = parse_json_aliases(alias_value, &mut result.warnings);
            }
        }
        _ => {
            return Err("JSON 点表必须是数组，或包含 points / point_defs 字段的对象".to_string());
        }
    }

    append_duplicate_point_errors(&result.points, &mut result.errors);
    Ok(result)
}

fn parse_json_aliases(value: &Value, warnings: &mut Vec<PointTableImportIssue>) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    match value {
        Value::Object(map) => {
            for (raw_type, raw_alias) in map {
                let alias = value_to_string(Some(raw_alias));
                match normalize_alias_entry(
                    Some(raw_type.as_str()),
                    alias.as_deref(),
                    format!("JSON asdu_aliases.{raw_type}"),
                ) {
                    Some((type_name, normalized_alias, warning)) => {
                        if let Some(warning) = warning {
                            warnings.push(warning);
                        }
                        if aliases.insert(type_name.clone(), normalized_alias).is_some() {
                            warnings.push(issue(
                                "DUPLICATE_ASDU_ALIAS",
                                format!("ASDU 别名重复，后者覆盖前者: {type_name}"),
                                Some(format!("JSON asdu_aliases.{raw_type}")),
                            ));
                        }
                    }
                    None => warnings.push(issue(
                        "INVALID_ASDU_ALIAS",
                        "ASDU 别名条目无效，已忽略",
                        Some(format!("JSON asdu_aliases.{raw_type}")),
                    )),
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let location = format!("JSON asdu_aliases[{index}]");
                let Some(record) = item.as_object() else {
                    warnings.push(issue("INVALID_ASDU_ALIAS", "ASDU 别名条目必须是对象", Some(location)));
                    continue;
                };
                let raw_key = get_json_alias_value(record, &["key", "type", "asdu_type", "asduType"]);
                let raw_value = get_json_alias_value(record, &["value", "alias", "name"]);
                match normalize_alias_entry(raw_key.as_deref(), raw_value.as_deref(), location.clone()) {
                    Some((type_name, normalized_alias, warning)) => {
                        if let Some(warning) = warning {
                            warnings.push(warning);
                        }
                        if aliases.insert(type_name.clone(), normalized_alias).is_some() {
                            warnings.push(issue(
                                "DUPLICATE_ASDU_ALIAS",
                                format!("ASDU 别名重复，后者覆盖前者: {type_name}"),
                                Some(location),
                            ));
                        }
                    }
                    None => warnings.push(issue("INVALID_ASDU_ALIAS", "ASDU 别名条目无效，已忽略", Some(location))),
                }
            }
        }
        _ => warnings.push(issue(
            "INVALID_ASDU_ALIAS",
            "asdu_aliases 必须是对象或数组，已忽略",
            Some("JSON asdu_aliases".to_string()),
        )),
    }
    aliases
}

fn build_json_raw_point_record(record: &JsonMap<String, Value>, location: String) -> RawPointRecord {
    RawPointRecord {
        location,
        address: get_json_value(record, &["address", "ioa", "地址"]),
        name: get_json_value(record, &["name", "名称"]),
        type_id: get_json_value(record, &["type_id", "typeId"]),
        data_type: get_json_value(record, &["data_type", "dataType", "type", "typeName", "类型", "数据类型"]),
        description: get_json_value(record, &["description", "desc", "描述"]),
        control_ioa: get_json_value(record, &["control_ioa", "controlIoa", "ControlIOA"]),
        gi_group: get_json_value(record, &["gi_group", "giGroup"]),
        counter_group: get_json_value(record, &["counter_group", "counterGroup"]),
        default_value: get_json_value(record, &["default_value", "defaultValue"]),
        is_enabled: get_json_value(record, &["is_enabled", "isEnabled", "enabled", "启用"]),
        is_double_point: get_json_value(record, &[
            "is_double_point",
            "isDoublePoint",
            "double_point",
            "is_dp",
            "是否双点",
            "双点",
        ]),
        common_address: get_json_value(record, &["common_address", "commonAddress"]),
    }
}

fn csv_field_index(
    header: &[String],
    field: &CsvFieldDefinition,
    errors: &mut Vec<PointTableImportIssue>,
) -> Option<usize> {
    let indexes = header
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value == field.key || value == field.zh_cn).then_some(index))
        .collect::<Vec<_>>();
    if indexes.len() > 1 {
        errors.push(issue(
            "DUPLICATE_CSV_HEADER",
            format!("CSV 表头字段 {} 重复", field.key),
            Some(format!("CSV header: {} / {}", field.key, field.zh_cn)),
        ));
    }
    indexes.first().copied()
}

fn parse_csv_point_table(text: &str) -> ParsePointTableFileResult {
    let mut result = ParsePointTableFileResult {
        points: Vec::new(),
        asdu_aliases: HashMap::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    let lines = text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let normalized = line.trim().replace('\u{feff}', "");
            if normalized.is_empty() {
                return None;
            }
            Some((index + 1, normalized))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        result.errors.push(issue("EMPTY_POINT_TABLE", "CSV 点表为空", Some("CSV".to_string())));
        return result;
    }

    let header =
        parse_csv_row(&lines[0].1).into_iter().map(|value| value.trim().to_ascii_lowercase()).collect::<Vec<_>>();
    let mut field_indexes = POINT_TABLE_CSV_FIELDS
        .iter()
        .map(|field| (field.key, csv_field_index(&header, field, &mut result.errors)))
        .collect::<HashMap<_, _>>();
    let mut take_index = |key: &str| field_indexes.remove(key).flatten();
    let idx_address = take_index("address");
    let idx_name = take_index("name");
    let idx_type_id = take_index("type_id");
    let idx_data_type = take_index("data_type");
    let idx_description = take_index("description");
    let idx_control_ioa = take_index("control_ioa");
    let idx_gi_group = take_index("gi_group");
    let idx_counter_group = take_index("counter_group");
    let idx_default_value = take_index("default_value");
    let idx_enabled = take_index("is_enabled");
    let idx_common_address = header.iter().position(|value| value == "common_address" || value == "公共地址");
    let idx_is_double_point = None;

    if idx_address.is_none() {
        result.errors.push(issue(
            "MISSING_CSV_ADDRESS_HEADER",
            "CSV 缺少 address / 信息对象地址表头".to_string(),
            Some("CSV header".to_string()),
        ));
    }
    if idx_type_id.is_none() && idx_data_type.is_none() {
        result.errors.push(issue(
            "MISSING_CSV_TYPE_HEADER",
            "CSV 缺少 type_id / 类型ID 或 data_type / 类型名称表头".to_string(),
            Some("CSV header".to_string()),
        ));
    }

    for (line_no, line) in lines.into_iter().skip(1) {
        let columns = parse_csv_row(&line);
        let raw = RawPointRecord {
            location: format!("CSV line {line_no}"),
            address: value_from_csv_column(&columns, idx_address),
            name: value_from_csv_column(&columns, idx_name),
            type_id: value_from_csv_column(&columns, idx_type_id),
            data_type: value_from_csv_column(&columns, idx_data_type),
            description: value_from_csv_column(&columns, idx_description),
            control_ioa: value_from_csv_column(&columns, idx_control_ioa),
            gi_group: value_from_csv_column(&columns, idx_gi_group),
            counter_group: value_from_csv_column(&columns, idx_counter_group),
            default_value: value_from_csv_column(&columns, idx_default_value),
            is_enabled: value_from_csv_column(&columns, idx_enabled),
            is_double_point: value_from_csv_column(&columns, idx_is_double_point),
            common_address: value_from_csv_column(&columns, idx_common_address),
        };
        if let Some(point) = normalize_raw_point_record(raw, &mut result.warnings) {
            result.points.push(point);
        }
    }

    append_duplicate_point_errors(&result.points, &mut result.errors);
    result
}

fn parse_xml_point_table(text: &str) -> Result<ParsePointTableFileResult, String> {
    let document: XmlPointTableDocument = from_xml_str(text).map_err(|err| format!("解析 XML 点表失败: {err}"))?;
    let mut result = ParsePointTableFileResult {
        points: Vec::new(),
        asdu_aliases: HashMap::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    let point_entries = document
        .points
        .map(|value| value.point)
        .unwrap_or_default()
        .into_iter()
        .chain(document.point)
        .collect::<Vec<_>>();
    for (index, item) in point_entries.into_iter().enumerate() {
        let raw = RawPointRecord {
            location: format!("XML Point[{index}]"),
            address: string_to_value(first_non_empty([
                item.attr_ioa,
                item.attr_ioa_upper,
                item.attr_address,
                item.attr_address_upper,
                item.ioa,
                item.ioa_upper,
                item.address,
                item.address_upper,
            ])),
            name: string_to_value(first_non_empty([item.attr_name, item.attr_name_upper, item.name, item.name_upper])),
            type_id: string_to_value(first_non_empty([
                item.attr_type_id,
                item.attr_type_id_camel,
                item.attr_type_id_upper,
                item.type_id,
                item.type_id_camel,
                item.type_id_upper,
            ])),
            data_type: string_to_value(first_non_empty([
                item.attr_data_type,
                item.attr_data_type_camel,
                item.attr_type_name,
                item.attr_type_name_camel,
                item.data_type,
                item.data_type_camel,
                item.type_name,
                item.type_name_camel,
            ])),
            description: string_to_value(first_non_empty([
                item.attr_description,
                item.attr_description_upper,
                item.attr_desc,
                item.description,
                item.description_upper,
                item.desc,
            ])),
            control_ioa: string_to_value(first_non_empty([
                item.attr_control_ioa,
                item.attr_control_ioa_camel,
                item.attr_control_ioa_upper,
                item.control_ioa,
                item.control_ioa_camel,
                item.control_ioa_upper,
            ])),
            gi_group: string_to_value(first_non_empty([
                item.attr_gi_group,
                item.attr_gi_group_camel,
                item.gi_group,
                item.gi_group_camel,
            ])),
            counter_group: string_to_value(first_non_empty([
                item.attr_counter_group,
                item.attr_counter_group_camel,
                item.counter_group,
                item.counter_group_camel,
            ])),
            default_value: string_to_value(first_non_empty([
                item.attr_default_value,
                item.attr_default_value_camel,
                item.default_value,
                item.default_value_camel,
            ])),
            is_enabled: string_to_value(first_non_empty([
                item.attr_enabled,
                item.attr_is_enabled,
                item.attr_is_enabled_snake,
                item.enabled,
                item.is_enabled,
                item.is_enabled_snake,
            ])),
            is_double_point: string_to_value(first_non_empty([
                item.attr_is_double_point,
                item.attr_is_double_point_camel,
                item.attr_double_point,
                item.is_double_point,
                item.is_double_point_camel,
                item.double_point,
                item.double_point_cn,
            ])),
            common_address: None,
        };
        if let Some(point) = normalize_raw_point_record(raw, &mut result.warnings) {
            result.points.push(point);
        }
    }

    if let Some(alias_list) = document.asdu_aliases {
        for (index, item) in alias_list.alias.into_iter().enumerate() {
            let location = format!("XML Alias[{index}]");
            let raw_key = first_non_empty([
                item.attr_key,
                item.attr_type,
                item.attr_asdu_type,
                item.key,
                item.key_upper,
                item.type_name,
                item.type_name_upper,
                item.asdu_type,
                item.asdu_type_upper,
            ]);
            let raw_value = first_non_empty([item.attr_value, item.value, item.value_upper, item.body_text]);
            match normalize_alias_entry(raw_key.as_deref(), raw_value.as_deref(), location.clone()) {
                Some((type_name, alias, warning)) => {
                    if let Some(warning) = warning {
                        result.warnings.push(warning);
                    }
                    if result.asdu_aliases.insert(type_name.clone(), alias).is_some() {
                        result.warnings.push(issue(
                            "DUPLICATE_ASDU_ALIAS",
                            format!("ASDU 别名重复，后者覆盖前者: {type_name}"),
                            Some(location),
                        ));
                    }
                }
                None => result.warnings.push(issue("INVALID_ASDU_ALIAS", "ASDU 别名条目无效，已忽略", Some(location))),
            }
        }
    }

    append_duplicate_point_errors(&result.points, &mut result.errors);
    Ok(result)
}

fn build_normalized_payload(
    points: &[ParsedPointRecord],
    asdu_aliases: HashMap<String, String>,
) -> PointTableImportNormalizedPayload {
    let mut dedup_by_address = BTreeMap::<u32, PointDef>::new();
    for item in points {
        dedup_by_address.insert(item.def.address, item.def.clone());
    }
    PointTableImportNormalizedPayload {
        point_defs: dedup_by_address.into_values().collect(),
        asdu_aliases,
        source_common_addresses: points
            .iter()
            .filter_map(|point| {
                point.common_address.map(|common_address| PointTableImportSourceCommonAddress {
                    common_address,
                    location: point.location.clone(),
                })
            })
            .collect(),
    }
}

fn append_duplicate_point_errors(points: &[ParsedPointRecord], errors: &mut Vec<PointTableImportIssue>) {
    let mut locations_by_address = HashMap::<u32, Vec<String>>::new();
    for item in points {
        locations_by_address.entry(item.def.address).or_default().push(item.location.clone());
    }
    for (address, locations) in locations_by_address {
        if locations.len() <= 1 {
            continue;
        }
        errors.push(issue(
            "CONFLICT_IOA_DUPLICATE",
            format!("信息对象地址重复: IOA={address}"),
            Some(locations.join(", ")),
        ));
    }
}

fn normalize_raw_point_record(
    raw: RawPointRecord,
    warnings: &mut Vec<PointTableImportIssue>,
) -> Option<ParsedPointRecord> {
    let location = raw.location.clone();
    let address = match parse_u32_field(raw.address.as_ref()) {
        Ok(Some(value)) => value,
        Ok(None) => {
            warnings.push(issue("MISSING_ADDRESS", "缺少 address / ioa，已忽略该点位", Some(location)));
            return None;
        }
        Err(message) => {
            warnings.push(issue("INVALID_ADDRESS", message, Some(location)));
            return None;
        }
    };

    let inferred_type_id = match parse_bool_like(raw.is_double_point.as_ref()) {
        Some(true) => 3,
        Some(false) => 1,
        None => 13,
    };
    let (type_id, data_type) = match resolve_point_type(raw.type_id.as_ref(), raw.data_type.as_ref(), inferred_type_id)
    {
        Ok(value) => value,
        Err(message) => {
            warnings.push(issue("INVALID_TYPE_ID", message, Some(location)));
            return None;
        }
    };

    let name = match value_to_string(raw.name.as_ref()) {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => {
            warnings.push(issue(
                "MISSING_POINT_NAME",
                format!("点位缺少名称，已自动生成默认名称: IOA={address}"),
                Some(location.clone()),
            ));
            iec104_default_point_name(type_id, address)
        }
    };

    let description = value_to_string(raw.description.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let control_ioa = match parse_u32_field(raw.control_ioa.as_ref()) {
        Ok(value) => value,
        Err(message) => {
            warnings.push(issue("INVALID_CONTROL_IOA", message, Some(location.clone())));
            None
        }
    };
    let gi_group = match parse_u32_field(raw.gi_group.as_ref()) {
        Ok(Some(value)) if (1..=16).contains(&value) => Some(value as u8),
        Ok(None) => None,
        Ok(Some(_)) => {
            warnings.push(issue("INVALID_GI_GROUP", "gi_group 必须在 1..=16", Some(location.clone())));
            None
        }
        Err(message) => {
            warnings.push(issue("INVALID_GI_GROUP", message, Some(location.clone())));
            None
        }
    };
    let counter_group = match parse_u32_field(raw.counter_group.as_ref()) {
        Ok(Some(value)) if (1..=4).contains(&value) => Some(value as u8),
        Ok(None) => None,
        Ok(Some(_)) => {
            warnings.push(issue("INVALID_COUNTER_GROUP", "counter_group 必须在 1..=4", Some(location.clone())));
            None
        }
        Err(message) => {
            warnings.push(issue("INVALID_COUNTER_GROUP", message, Some(location.clone())));
            None
        }
    };
    let default_value = match normalize_default_value(raw.default_value) {
        Ok(value) => value,
        Err(message) => {
            warnings.push(issue("INVALID_DEFAULT_VALUE", message, Some(location.clone())));
            None
        }
    };
    let is_enabled = parse_bool_like(raw.is_enabled.as_ref()).unwrap_or(true);
    let common_address = match parse_u32_field(raw.common_address.as_ref()) {
        Ok(Some(value)) if (1..=u16::MAX as u32).contains(&value) => Some(value as u16),
        Ok(None) => None,
        Ok(Some(value)) => {
            warnings.push(issue(
                "INVALID_COMMON_ADDRESS",
                format!("common_address={value} 必须在 1..={}", u16::MAX),
                Some(location.clone()),
            ));
            None
        }
        Err(message) => {
            warnings.push(issue("INVALID_COMMON_ADDRESS", message, Some(location.clone())));
            None
        }
    };

    Some(ParsedPointRecord {
        location,
        common_address,
        def: PointDef {
            address,
            name,
            type_id,
            data_type,
            description,
            control_ioa,
            gi_group,
            counter_group,
            default_value,
            is_enabled,
            source: PointSource::Imported,
        },
    })
}

fn resolve_point_type(
    raw_type_id: Option<&Value>,
    raw_data_type: Option<&Value>,
    fallback_type_id: u8,
) -> Result<(u8, String), String> {
    if let Some(value) = raw_type_id {
        let Some(text) = value_to_string(Some(value)) else {
            return Err("type_id 为空".to_string());
        };
        let parsed = text.trim().parse::<u16>().map_err(|_| format!("非法 type_id: {text}"))?;
        if parsed == 0 || parsed > 127 {
            return Err(format!("非法 type_id: {parsed}"));
        }
        let type_id = parsed as u8;
        return Ok((type_id, iec104_type_name(type_id).to_string()));
    }

    if let Some(value) = raw_data_type {
        let Some(text) = value_to_string(Some(value)) else {
            return Err("data_type 为空".to_string());
        };
        if let Some(type_id) = resolve_type_id_from_name_or_alias(&text) {
            return Ok((type_id, iec104_type_name(type_id).to_string()));
        }
        return Err(format!("不支持的 ASDU 类型: {text}"));
    }

    Ok((fallback_type_id, iec104_type_name(fallback_type_id).to_string()))
}

fn resolve_type_id_from_name_or_alias(raw: &str) -> Option<u8> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return None;
    }
    let upper = normalized.to_ascii_uppercase();
    if let Some(type_id) = iec104_type_id(&upper) {
        return Some(type_id);
    }

    match normalized.trim().to_ascii_lowercase().as_str() {
        "single_point" | "single" => Some(1),
        "double_point" | "double" => Some(3),
        "integrated_total" | "integrated_totals" => Some(15),
        "normalized" => Some(9),
        "scaled" => Some(11),
        "float" => Some(13),
        _ => None,
    }
}

fn normalize_default_value(value: Option<Value>) -> Result<Option<Value>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::Null => Ok(None),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            match serde_json::from_str::<Value>(trimmed) {
                Ok(parsed) => Ok(Some(parsed)),
                Err(_) => Ok(Some(Value::String(trimmed.to_string()))),
            }
        }
        other => Ok(Some(other)),
    }
}

fn parse_u32_field(value: Option<&Value>) -> Result<Option<u32>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => {
            if let Some(parsed) = number.as_u64() {
                if parsed <= 0x00FF_FFFF {
                    return Ok(Some(parsed as u32));
                }
            }
            if let Some(parsed) = number.as_i64() {
                if (0..=0x00FF_FFFF).contains(&parsed) {
                    return Ok(Some(parsed as u32));
                }
            }
            Err(format!("数值超出 24 位 IOA 范围: {number}"))
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let parsed = trimmed.parse::<i64>().map_err(|_| format!("非法数值: {trimmed}"))?;
            if !(0..=0x00FF_FFFF).contains(&parsed) {
                return Err(format!("数值超出 24 位 IOA 范围: {parsed}"));
            }
            Ok(Some(parsed as u32))
        }
        other => Err(format!("字段必须是整数: {other}")),
    }
}

fn parse_bool_like(value: Option<&Value>) -> Option<bool> {
    let value = value?;
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_i64().map(|item| item != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" | "on" | "是" | "双点" | "double" | "double_point" => Some(true),
            "0" | "false" | "no" | "n" | "off" | "否" | "单点" | "single" | "single_point" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn parse_csv_row(line: &str) -> Vec<String> {
    let normalized = line.trim().trim_end_matches('\r');
    if normalized.is_empty() {
        return Vec::new();
    }
    if normalized.starts_with('"') && normalized.ends_with('"') {
        return normalized.trim_matches('"').split("\",\"").map(str::to_string).collect();
    }
    normalized.split(',').map(str::to_string).collect()
}

fn value_from_csv_column(columns: &[String], index: Option<usize>) -> Option<Value> {
    let index = index?;
    let value = columns.get(index)?.trim();
    if value.is_empty() {
        return None;
    }
    Some(Value::String(value.to_string()))
}

fn string_to_value(value: Option<String>) -> Option<Value> {
    value.map(Value::String)
}

fn first_non_empty<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    for value in values.into_iter().flatten() {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn value_to_string(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        other => Some(other.to_string()),
    }
}

fn get_json_value(record: &JsonMap<String, Value>, aliases: &[&str]) -> Option<Value> {
    aliases.iter().find_map(|alias| record.get(*alias).cloned())
}

fn get_json_alias_value(record: &JsonMap<String, Value>, aliases: &[&str]) -> Option<String> {
    aliases.iter().find_map(|alias| value_to_string(record.get(*alias)))
}

fn normalize_alias_entry(
    raw_type: Option<&str>,
    raw_alias: Option<&str>,
    location: String,
) -> Option<(String, String, Option<PointTableImportIssue>)> {
    let type_name = raw_type?.trim();
    let alias = raw_alias?.trim();
    if type_name.is_empty() || alias.is_empty() {
        return None;
    }
    let type_id = iec104_type_id(&type_name.to_ascii_uppercase())?;
    let canonical_type_name = iec104_type_name(type_id).to_string();
    let warning = if canonical_type_name != type_name {
        Some(issue("NORMALIZED_ASDU_ALIAS", format!("ASDU 类型已规范化为 {canonical_type_name}"), Some(location)))
    } else {
        None
    };
    Some((canonical_type_name, alias.to_string(), warning))
}

fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
    location: Option<impl Into<String>>,
) -> PointTableImportIssue {
    PointTableImportIssue { code: code.into(), message: message.into(), location: location.map(Into::into) }
}
