# IEC104 Point Table Import Format

## Supported Formats

The importer supports CSV, JSON, and XML text files encoded as UTF-8, UTF-8 with BOM, UTF-16LE, or UTF-16BE. Direct `.xls` and `.xlsx` import is not supported; save the spreadsheet as UTF-8 CSV first.

Use **Help → Download Point Table Template** to save a CSV template in the current UI language. The template uses UTF-8 BOM and Windows CRLF line endings.

## Official CSV Headers

The first row must contain headers. English and Chinese official headers may be mixed, but the same semantic field cannot appear twice.

| Meaning | English | Chinese | Notes |
| --- | --- | --- | --- |
| Information object address | `address` | `信息对象地址` | 1..16777215 |
| Point name | `name` | `点名` | A default name is generated when blank |
| Type ID | `type_id` | `类型ID` | Numeric IEC104 Type ID |
| Type name | `data_type` | `类型名称` | Standard Type Name such as `M_SP_NA_1` |
| Description | `description` | `描述` | Optional |
| Control mapping IOA | `control_ioa` | `控制映射IOA` | Optional; stored on the monitor point |
| GI group | `gi_group` | `总召组` | Blank means station GI only; 1..16 also joins that group |
| Counter group | `counter_group` | `电度组` | Type 15/16/37 only; blank means all-counters operations only, while 1..4 also joins that counter group |
| Initial value | `default_value` | `初始值` | Optional |
| Enabled | `is_enabled` | `启用` | `true` or `false`; defaults to `true` |

Use numeric values in `type_id` and standard IEC104 Type Names in `data_type`. Do not use legacy `type`, DI, AI, DO, AO, or `value` fields.

Bit-string Type 7/51 values may use `0x00000000..0xFFFFFFFF` or the equivalent decimal value. Template bit-string rows are not mapped automatically.

## CSV Example

```csv
address,name,type_id,data_type,description,control_ioa,gi_group,counter_group,default_value,is_enabled
1001,Breaker closed,1,M_SP_NA_1,Position feedback,6001,1,,0,true
4001,Energy total,15,M_IT_NA_1,Integrated total,,,1,0,true
6001,Breaker command,45,C_SC_NA_1,Single command,,,,,true
3001,Bitstring status,7,M_BO_NA_1,32-bit status,,,,0x00000000,true
6101,Bitstring command,51,C_BO_NA_1,32-bit command,,,,0x00000000,true
```

## JSON and XML

JSON supports an array root or object roots containing `points`, `point_defs`, or `pointDefs`. Fields accept snake_case and common camelCase forms. An object root may define display aliases as a “standard type name → display alias” map under `asdu_aliases` or `asduAliases`. Each JSON point may include `common_address` / `commonAddress`; when present, it must match the selected target.

```json
{
  "points": [
    {
      "address": 1001,
      "name": "Breaker closed",
      "type_id": 1,
      "data_type": "M_SP_NA_1",
      "control_ioa": 6001,
      "default_value": false,
      "is_enabled": true
    },
    {
      "address": 6001,
      "name": "Breaker command",
      "data_type": "C_SC_NA_1",
      "is_enabled": true
    }
  ],
  "asdu_aliases": {
    "M_SP_NA_1": "Single-point status",
    "C_SC_NA_1": "Single command"
  }
}
```

XML requires a `<PointTable>` root, with points under `<Points>` or directly under the root. Fields may be attributes or child elements. ASDU aliases use `<AsduAliases>`; this is the recommended compact attribute form:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PointTable>
  <Points>
    <Point ioa="1001" name="Breaker closed" type="M_SP_NA_1"
      controlIoa="6001" defaultValue="false" enabled="true" />
    <Point ioa="6001" name="Breaker command" type="C_SC_NA_1" enabled="true" />
  </Points>
  <AsduAliases>
    <Alias type="M_SP_NA_1" value="Single-point status" />
    <Alias type="C_SC_NA_1" value="Single command" />
  </AsduAliases>
</PointTable>
```

Set `control_ioa` / `controlIoa` on the monitor point that receives the control result, pointing to its control-point IOA. Do not put the monitor IOA on the control point. The downloadable template is CSV only.

## Import Rules

- Duplicate IOAs inside the file prevent applying it.
- The default append-only mode atomically rejects any conflict with the existing table.
- Replace-all must be selected explicitly in the import dialog.
- Replace-all changes only the selected logical slave, not other slaves under the same TCP Link / Listener.
- A source common address, when present, must match the selected target.
- Errors and warnings retain the original row and field context.
- The precheck displays at most 200 records. This is a UI preview limit, not an import-size limit.

## Related Documents

- [User Guide](./user-guide.md)
- [Protocol Support](./protocol-support.md)
