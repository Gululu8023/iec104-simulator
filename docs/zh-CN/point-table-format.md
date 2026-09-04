# IEC104 点表导入格式

## 支持格式

当前支持 CSV、JSON、XML 文本点表，编码支持 UTF-8、UTF-8 BOM、UTF-16LE 和 UTF-16BE。不支持直接导入 `.xls` 或 `.xlsx`；请先另存为 UTF-8 CSV。

可在“帮助 → 下载点表模板”中下载与当前界面语言一致的 CSV 模板。模板使用 UTF-8 BOM 和 Windows CRLF 换行。

## CSV 正式表头

CSV 第一行必须是表头。可以使用完整英文表头、完整中文表头，也可以混用；同一语义不能同时出现中英文两列。

| 语义 | 英文表头 | 中文表头 | 说明 |
| --- | --- | --- | --- |
| 信息对象地址 | `address` | `信息对象地址` | 1..16777215 |
| 点名 | `name` | `点名` | 空白时生成默认名称 |
| 类型 ID | `type_id` | `类型ID` | 数字 IEC104 Type ID |
| 类型名称 | `data_type` | `类型名称` | 标准 Type Name，如 `M_SP_NA_1` |
| 描述 | `description` | `描述` | 可选 |
| 控制映射 IOA | `control_ioa` | `控制映射IOA` | 可选，填写在监视点上 |
| 总召组 | `gi_group` | `总召组` | 空白表示仅站总召，1..16表示同时属于对应组 |
| 电度组 | `counter_group` | `电度组` | 仅 Type 15/16/37 可用；空白表示仅参与全部累计量操作，1..4表示同时属于对应电度组 |
| 初始值 | `default_value` | `初始值` | 可选 |
| 启用 | `is_enabled` | `启用` | `true` 或 `false`，默认 `true` |

`type_id` 只填写数字，`data_type` 只填写标准 IEC104 Type Name。不要使用旧模板中的 `type`、DI、AI、DO、AO 或 `value` 字段。

位串 Type 7/51 的值可使用 `0x00000000..0xFFFFFFFF` 或等价十进制值。位串控制点不会因为出现在模板中自动建立映射。

## CSV 示例

```csv
address,name,type_id,data_type,description,control_ioa,gi_group,counter_group,default_value,is_enabled
1001,断路器合位,1,M_SP_NA_1,位置反馈,6001,1,,0,true
4001,正向有功总电度,15,M_IT_NA_1,累计电度量,,,1,0,true
6001,断路器遥控,45,C_SC_NA_1,单点遥控,,,,,true
3001,位串状态,7,M_BO_NA_1,32位状态,,,,0x00000000,true
6101,位串命令,51,C_BO_NA_1,32位命令,,,,0x00000000,true
```

## JSON 与 XML

JSON 支持数组根节点，或对象根节点中的 `points`、`point_defs`、`pointDefs`。字段可使用下划线或常见驼峰形式；对象根还可通过 `asdu_aliases` 或 `asduAliases` 配置“标准类型名 → 显示别名”。每个 JSON 点位可选填 `common_address` / `commonAddress`，该值必须与导入目标的公共地址一致。

```json
{
  "points": [
    {
      "address": 1001,
      "name": "断路器合位",
      "type_id": 1,
      "data_type": "M_SP_NA_1",
      "control_ioa": 6001,
      "default_value": false,
      "is_enabled": true
    },
    {
      "address": 6001,
      "name": "断路器遥控",
      "data_type": "C_SC_NA_1",
      "is_enabled": true
    }
  ],
  "asdu_aliases": {
    "M_SP_NA_1": "单点遥信",
    "C_SC_NA_1": "单点遥控"
  }
}
```

XML 根元素必须为 `<PointTable>`，点位可放在 `<Points>` 下或直接放在根节点下。字段既可写成属性，也可写成子元素。ASDU 别名使用 `<AsduAliases>`；下面展示推荐的紧凑属性写法：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PointTable>
  <Points>
    <Point ioa="1001" name="断路器合位" type="M_SP_NA_1"
      controlIoa="6001" defaultValue="false" enabled="true" />
    <Point ioa="6001" name="断路器遥控" type="C_SC_NA_1" enabled="true" />
  </Points>
  <AsduAliases>
    <Alias type="M_SP_NA_1" value="单点遥信" />
    <Alias type="C_SC_NA_1" value="单点遥控" />
  </AsduAliases>
</PointTable>
```

`control_ioa` / `controlIoa` 应配置在接收控制结果的监视点上，值指向对应控制点 IOA；不要反向把监视点 IOA 写到控制点上。下载模板只提供 CSV。

## 导入规则

- 文件内重复 IOA 会阻止应用。
- 默认“严格追加”模式下，任一 IOA 与现有点表冲突都会整体拒绝。
- “整表替换”必须在导入窗口显式选择。
- 整表替换只替换当前选择的逻辑从站点表，不影响同一 TCP Link / Listener 下的其他逻辑从站。
- 文件中的公共地址若存在，必须与所选导入目标一致。
- 错误和警告会保留文件行号与原字段内容。
- 预检窗口最多展示 200 条记录；这是界面预览上限，不是导入记录数上限。

## 相关文档

- [用户指南](./user-guide.md)
- [协议支持范围](./protocol-support.md)
