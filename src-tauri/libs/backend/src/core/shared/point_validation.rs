use std::collections::{BTreeMap, BTreeSet};

use crate::core::{
    iec104_registry::{PointRole, ValueModel, lookup_capability},
    types::PointDef,
};

const IEC104_IOA_MAX: u32 = 0x00FF_FFFF;

pub(crate) fn normalize_and_validate_point_defs(point_defs: &mut [PointDef]) -> Result<(), (&'static str, String)> {
    let mut addresses = BTreeSet::new();
    let mut point_types = BTreeMap::new();
    let mut control_addresses = BTreeSet::new();
    for (index, def) in point_defs.iter_mut().enumerate() {
        let row = index + 1;
        let Some(capability) = lookup_capability(def.type_id) else {
            return Err((
                "UNDEFINED_IEC104_TYPE",
                format!("第 {row} 条记录字段 type_id={} 不是已定义的 IEC104 Type", def.type_id),
            ));
        };
        if !capability.point_configurable {
            return Err((
                "UNSUPPORTED_IEC104_CAPABILITY",
                format!("第 {row} 条记录字段 type_id={} ({}) 不允许配置为点", def.type_id, capability.name),
            ));
        }
        if def.address == 0 || def.address > IEC104_IOA_MAX {
            return Err((
                "INVALID_IOA",
                format!("第 {row} 条记录字段 address={} 必须在 1..={IEC104_IOA_MAX}", def.address),
            ));
        }
        if !addresses.insert(def.address) {
            return Err(("CONFLICT_IOA_DUPLICATE", format!("第 {row} 条记录字段 address={} 重复", def.address)));
        }
        point_types.insert(def.address, capability.clone());
        if let Some(control_ioa) = def.control_ioa {
            if control_ioa == 0 || control_ioa > IEC104_IOA_MAX {
                return Err((
                    "INVALID_CONTROL_IOA",
                    format!("第 {row} 条记录字段 control_ioa={control_ioa} 必须在 1..={IEC104_IOA_MAX}"),
                ));
            }
            if !control_addresses.insert(control_ioa) {
                return Err((
                    "CONFLICT_CONTROL_IOA_DUPLICATE",
                    format!("第 {row} 条记录字段 control_ioa={control_ioa} 重复"),
                ));
            }
        }
        if def.gi_group.is_some_and(|group| !(1..=16).contains(&group)) {
            return Err(("INVALID_GI_GROUP", format!("第 {row} 条记录字段 gi_group 必须在 1..=16")));
        }
        if def.counter_group.is_some_and(|group| !(1..=4).contains(&group)) {
            return Err(("INVALID_COUNTER_GROUP", format!("第 {row} 条记录字段 counter_group 必须在 1..=4")));
        }
        if def.counter_group.is_some() && !matches!(def.type_id, 15 | 16 | 37) {
            return Err((
                "COUNTER_GROUP_TYPE_MISMATCH",
                format!("第 {row} 条记录只有累计量 Type 15/16/37 可以配置 counter_group"),
            ));
        }
        normalize_default_value(row, def, capability.value_model)?;
    }

    for (index, target) in point_defs.iter().enumerate() {
        let Some(control_ioa) = target.control_ioa else {
            continue;
        };
        let row = index + 1;
        let target_capability = point_types.get(&target.address).expect("validated target point capability must exist");
        if target_capability.point_role != PointRole::Monitor {
            return Err((
                "INVALID_MAPPING_TARGET_ROLE",
                format!("第 {row} 条记录 IOA={} 的 control_ioa 只能配置在监视点上", target.address),
            ));
        }
        let Some(control_capability) = point_types.get(&control_ioa) else {
            return Err((
                "MAPPING_CONTROL_POINT_NOT_FOUND",
                format!("第 {row} 条记录 control_ioa={control_ioa} 在当前点表中不存在"),
            ));
        };
        if control_capability.point_role != PointRole::Control {
            return Err((
                "INVALID_MAPPING_SOURCE_ROLE",
                format!("第 {row} 条记录 control_ioa={control_ioa} 不是控制点"),
            ));
        }
        if control_capability.family != target_capability.family {
            return Err((
                "INCOMPATIBLE_MAPPING_FAMILY",
                format!("第 {row} 条记录 IOA={} 与控制点 IOA={control_ioa} 的 IEC104 family 不兼容", target.address),
            ));
        }
    }
    Ok(())
}

fn normalize_default_value(row: usize, def: &mut PointDef, model: ValueModel) -> Result<(), (&'static str, String)> {
    let Some(value) = def.default_value.take() else {
        return Ok(());
    };
    let value = match value {
        serde_json::Value::Null => return Ok(()),
        serde_json::Value::Number(number) => serde_json::Value::Number(number),
        serde_json::Value::Bool(flag) if model == ValueModel::Boolean => {
            serde_json::Value::Number(serde_json::Number::from(u8::from(flag)))
        }
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            if model == ValueModel::Bitstring32 {
                if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
                    let parsed = u32::from_str_radix(hex, 16).map_err(|_| {
                        ("INVALID_DEFAULT_VALUE", format!("第 {row} 条记录字段 default_value 必须是32位位串"))
                    })?;
                    serde_json::Value::Number(serde_json::Number::from(parsed))
                } else {
                    let parsed = trimmed.parse::<u32>().map_err(|_| {
                        ("INVALID_DEFAULT_VALUE", format!("第 {row} 条记录字段 default_value 必须是32位位串"))
                    })?;
                    serde_json::Value::Number(serde_json::Number::from(parsed))
                }
            } else {
                let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
                    .map_err(|_| ("INVALID_DEFAULT_VALUE", format!("第 {row} 条记录字段 default_value 必须是数值")))?;
                match parsed {
                    serde_json::Value::Number(number) => serde_json::Value::Number(number),
                    serde_json::Value::Bool(flag) if model == ValueModel::Boolean => {
                        serde_json::Value::Number(serde_json::Number::from(u8::from(flag)))
                    }
                    _ => {
                        return Err(("INVALID_DEFAULT_VALUE", format!("第 {row} 条记录字段 default_value 必须是数值")));
                    }
                }
            }
        }
        _ => {
            return Err(("INVALID_DEFAULT_VALUE", format!("第 {row} 条记录字段 default_value 必须是数值")));
        }
    };
    let number = value.as_f64().expect("normalized default value must be numeric");
    if model.accepts(number) {
        def.default_value = Some(value);
        Ok(())
    } else {
        Err((
            "INVALID_DEFAULT_VALUE",
            format!("第 {row} 条记录字段 default_value={value} 不符合 Type {} 的值域", def.type_id),
        ))
    }
}
