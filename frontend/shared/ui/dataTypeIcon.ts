import type { Component } from 'vue'
import {
  CircleCheck,
  DataLine,
  Document,
  Grid,
  Odometer,
  Operation,
  Setting,
  Switch,
  WarningFilled,
} from '@element-plus/icons-vue'
import {
  categoryByTypeId,
  iec104TypeNameToLocalizedName,
  iec104TypeNameToLocalizedShortName,
  iec104TypeNameToId,
  type AsduCategory,
} from '@shared/api/iec104'
import { t } from '@shared/i18n'

/** 将 typeName 字符串转为 typeId，再查分类 */
function resolveCategory(dataType: string | null | undefined): AsduCategory {
  const id = iec104TypeNameToId(dataType)
  return categoryByTypeId(id)
}

// 通用图标（保持细分类别）
const CATEGORY_ICON: Record<AsduCategory, Component> = {
  single_point: CircleCheck,
  double_point: Switch,
  step_position: DataLine,
  bitstring: Grid,
  measurement: DataLine,
  integrated_total: Odometer,
  protection: WarningFilled,
  command: Operation,
  security: Setting,
  system: Setting,
  parameter: Setting,
  file_transfer: Document,
  unknown: Document,
}

// 设备树图标（按大类收敛）
const CATEGORY_TREE_ICON: Record<AsduCategory, Component> = {
  // 遥信大类（合并单点/双点/步位置/位串/保护）
  single_point: CircleCheck,
  double_point: CircleCheck,
  step_position: CircleCheck,
  bitstring: CircleCheck,
  protection: CircleCheck,
  // 遥测大类
  measurement: DataLine,
  // 电度大类
  integrated_total: Odometer,
  // 命令大类（含系统命令）
  command: Operation,
  security: Operation,
  system: Operation,
  // 参数大类
  parameter: Setting,
  // 其他
  file_transfer: Document,
  unknown: Document,
}

export function getIecDataTypeIcon(dataType: string | null | undefined): Component {
  return CATEGORY_ICON[resolveCategory(dataType)] ?? Document
}

export function getIecDataTypeTreeIcon(dataType: string | null | undefined): Component {
  return CATEGORY_TREE_ICON[resolveCategory(dataType)] ?? Document
}

export function getIecDataTypeLabel(dataType: string | null | undefined): string {
  const localized = iec104TypeNameToLocalizedName(dataType)
  if (localized) return localized
  return t(`iec104.categories.name.${resolveCategory(dataType)}`)
}

export function getIecDataTypeTabLabel(dataType: string | null | undefined): string {
  const label = getIecDataTypeLabel(dataType)
  const typeId = iec104TypeNameToId(dataType)
  return typeId == null ? label : `${label} (${typeId})`
}

export function getIecDataTypeNodeLabel(dataType: string | null | undefined): string {
  const localizedShort = iec104TypeNameToLocalizedShortName(dataType)
  if (localizedShort) return localizedShort
  return t(`iec104.categories.nodeLabel.${resolveCategory(dataType)}`)
}

export function getIecDataTypeSimpleName(dataType: string | null | undefined): string {
  const localizedShort = iec104TypeNameToLocalizedShortName(dataType)
  if (localizedShort) return localizedShort
  return t(`iec104.categories.simpleName.${resolveCategory(dataType)}`)
}
