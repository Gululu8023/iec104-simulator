import type { BackendDataPoint, Iec104Capability } from '../types'
import { t } from '@shared/i18n'

const IEC104_TYPE_ID_TO_NAME: Record<number, string> = {}
const IEC104_TYPE_NAME_TO_ID: Record<string, number> = {}
const IEC104_CAPABILITY_BY_TYPE_ID: Record<number, Iec104Capability> = {}

export function installIec104CapabilityCatalog(
  capabilities: ReadonlyArray<Iec104Capability>,
): void {
  for (const key of Object.keys(IEC104_TYPE_ID_TO_NAME)) delete IEC104_TYPE_ID_TO_NAME[Number(key)]
  for (const key of Object.keys(IEC104_TYPE_NAME_TO_ID)) delete IEC104_TYPE_NAME_TO_ID[key]
  for (const key of Object.keys(IEC104_CAPABILITY_BY_TYPE_ID)) {
    delete IEC104_CAPABILITY_BY_TYPE_ID[Number(key)]
  }
  for (const capability of capabilities) {
    IEC104_TYPE_ID_TO_NAME[capability.type_id] = capability.name
    IEC104_TYPE_NAME_TO_ID[capability.name] = capability.type_id
    IEC104_CAPABILITY_BY_TYPE_ID[capability.type_id] = capability
  }
}

export function getIec104Capability(typeId: number | null | undefined): Iec104Capability | null {
  const numeric = Math.trunc(Number(typeId))
  if (!Number.isFinite(numeric) || numeric <= 0) return null
  return IEC104_CAPABILITY_BY_TYPE_ID[numeric] ?? null
}

export function getInstalledIec104Capabilities(): readonly Iec104Capability[] {
  return Object.values(IEC104_CAPABILITY_BY_TYPE_ID).sort(
    (left, right) => left.type_id - right.type_id,
  )
}

export function formatIec104TypeLabel(typeId: number | null | undefined): string {
  const capability = getIec104Capability(typeId)
  if (!capability) return `UNKNOWN (${Math.trunc(Number(typeId) || 0)})`
  const description = iec104TypeIdToLocalizedName(capability.type_id)
  return `${formatIec104TypeCompactLabel(capability.type_id)}${description ? ` · ${description}` : ''}`
}

export function formatIec104TypeCompactLabel(typeId: number | null | undefined): string {
  const capability = getIec104Capability(typeId)
  const numeric = Math.trunc(Number(typeId) || 0)
  return capability ? `${capability.name} (${capability.type_id})` : `UNKNOWN (${numeric})`
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeId ↔ TypeName 转换（纯查表，无模糊匹配）
// ─────────────────────────────────────────────────────────────────────────────

export function iec104TypeIdToName(typeId: number | null | undefined): string {
  const numeric = Math.trunc(Number(typeId))
  if (!Number.isFinite(numeric) || numeric <= 0) return 'UNKNOWN'
  return IEC104_TYPE_ID_TO_NAME[numeric] ?? `TYPE_${numeric}`
}

const translateWithFallback = (key: string, fallback: string): string => {
  const translated = t(key)
  return translated === key ? fallback : translated
}

export function iec104TypeIdToLocalizedName(typeId: number | null | undefined): string {
  const numeric = Math.trunc(Number(typeId))
  if (!Number.isFinite(numeric) || numeric <= 0) return ''
  const typeName = iec104TypeIdToName(numeric)
  if (typeName === 'UNKNOWN' || typeName.startsWith('TYPE_')) return typeName
  return translateWithFallback(`iec104.types.name.${typeName}`, typeName)
}

export function iec104TypeIdToLocalizedShortName(typeId: number | null | undefined): string {
  const numeric = Math.trunc(Number(typeId))
  if (!Number.isFinite(numeric) || numeric <= 0) return ''
  const typeName = iec104TypeIdToName(numeric)
  if (typeName === 'UNKNOWN' || typeName.startsWith('TYPE_')) return typeName
  return translateWithFallback(
    `iec104.types.short.${typeName}`,
    iec104TypeIdToLocalizedName(numeric),
  )
}

export function iec104TypeNameToId(typeName: string | null | undefined): number | null {
  const normalized = String(typeName ?? '')
    .trim()
    .toUpperCase()
  if (!normalized) return null

  const direct = IEC104_TYPE_NAME_TO_ID[normalized]
  if (direct != null) return direct

  const matched = normalized.match(/^TYPE_(\d{1,3})$/)
  if (!matched) return null
  const parsed = Number(matched[1])
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 127) return null
  return parsed
}

export function iec104TypeNameToLocalizedName(typeName: string | null | undefined): string {
  const typeId = iec104TypeNameToId(typeName)
  if (typeId == null) return ''
  return iec104TypeIdToLocalizedName(typeId)
}

export function iec104TypeNameToLocalizedShortName(typeName: string | null | undefined): string {
  const typeId = iec104TypeNameToId(typeName)
  if (typeId == null) return ''
  return iec104TypeIdToLocalizedShortName(typeId)
}

// ─────────────────────────────────────────────────────────────────────────────
// ASDU 分类（纯 typeId，无字符串前缀匹配）
// ─────────────────────────────────────────────────────────────────────────────

export type AsduCategory =
  | 'single_point' // 单点 (1,2,30)
  | 'double_point' // 双点 (3,4,31)
  | 'step_position' // 步位置 (5,6,32)
  | 'bitstring' // 位串 (7,8,20,33)
  | 'measurement' // 遥测 (9-14,21,34-36)
  | 'integrated_total' // 累计量 (15,16,37)
  | 'protection' // 保护 (17-19,38-40)
  | 'command' // 控制命令 (45-51,58-64)
  | 'security' // 安全类型 (81-95)
  | 'system' // 系统命令 (70,100-107)
  | 'parameter' // 参数 (110-113)
  | 'file_transfer' // 文件传输 (120-127)
  | 'unknown'

/** 按 typeId 判定 ASDU 所属类别 */
export function categoryByTypeId(typeId: number | null | undefined): AsduCategory {
  const id = Math.trunc(Number(typeId))
  if (!Number.isFinite(id) || id <= 0) return 'unknown'
  const capability = getIec104Capability(id)
  if (!capability) return 'unknown'
  if (capability.point_role === 'control') return 'command'
  switch (capability.family) {
    case 'single_point':
      return 'single_point'
    case 'double_point':
      return 'double_point'
    case 'step_position':
      return 'step_position'
    case 'bitstring':
    case 'packed_status':
      return 'bitstring'
    case 'normalized':
    case 'scaled':
    case 'short_float':
      return 'measurement'
    case 'integrated_total':
      return 'integrated_total'
    case 'protection':
      return 'protection'
    case 'security':
      return 'security'
    case 'parameter':
      return 'parameter'
    case 'file_transfer':
      return 'file_transfer'
    case 'initialization':
    case 'system_command':
    case 'control':
      return 'system'
    default:
      return 'unknown'
  }
}

// 快捷判定函数（接受 typeId 或 typeName）
function resolveTypeId(typeIdOrName: number | string | null | undefined): number {
  if (typeof typeIdOrName === 'number') return typeIdOrName
  return iec104TypeNameToId(typeIdOrName as string) ?? 0
}

export function isSinglePointType(v: number | string | null | undefined): boolean {
  const capability = getIec104Capability(resolveTypeId(v))
  return capability?.family === 'single_point' && capability.point_role === 'monitor'
}
export function isDoublePointType(v: number | string | null | undefined): boolean {
  const capability = getIec104Capability(resolveTypeId(v))
  return capability?.family === 'double_point' && capability.point_role === 'monitor'
}
export function isIntegratedTotalType(v: number | string | null | undefined): boolean {
  return getIec104Capability(resolveTypeId(v))?.family === 'integrated_total'
}
export function isMeasurementType(v: number | string | null | undefined): boolean {
  const capability = getIec104Capability(resolveTypeId(v))
  return (
    capability?.point_role === 'monitor' &&
    ['normalized', 'scaled', 'short_float'].includes(capability.family)
  )
}
export function isStepPositionType(v: number | string | null | undefined): boolean {
  const capability = getIec104Capability(resolveTypeId(v))
  return capability?.family === 'step_position' && capability.point_role === 'monitor'
}
export function isBitstringType(v: number | string | null | undefined): boolean {
  const capability = getIec104Capability(resolveTypeId(v))
  return (
    capability?.point_role === 'monitor' &&
    ['bitstring', 'packed_status'].includes(capability.family)
  )
}
export function isProtectionType(v: number | string | null | undefined): boolean {
  return getIec104Capability(resolveTypeId(v))?.family === 'protection'
}
export function isParameterType(v: number | string | null | undefined): boolean {
  return getIec104Capability(resolveTypeId(v))?.family === 'parameter'
}
export function isRegulatingType(v: number | string | null | undefined): boolean {
  const capability = getIec104Capability(resolveTypeId(v))
  return capability?.family === 'step_position' && capability.point_role === 'control'
}
export function isCommandType(v: number | string | null | undefined): boolean {
  return getIec104Capability(resolveTypeId(v))?.point_role === 'control'
}

export function iec104DefaultPointNamePrefix(
  typeIdOrName: number | string | null | undefined,
): string {
  switch (resolveTypeId(typeIdOrName)) {
    case 1:
    case 2:
    case 30:
      return '单点'
    case 3:
    case 4:
    case 31:
      return '双点'
    case 5:
    case 6:
    case 32:
      return '步位'
    case 7:
    case 8:
    case 33:
      return '位串'
    case 9:
    case 10:
    case 21:
    case 34:
      return '归一化测量'
    case 11:
    case 12:
    case 35:
      return '标度化测量'
    case 13:
    case 14:
    case 36:
      return '短浮点测量'
    case 15:
    case 16:
    case 37:
      return '累计量'
    case 17:
    case 38:
      return '保护事件'
    case 18:
    case 39:
      return '保护启动'
    case 19:
    case 40:
      return '保护输出'
    case 20:
      return '成组单点'
    case 45:
    case 58:
      return '单命令'
    case 46:
    case 59:
      return '双命令'
    case 47:
    case 60:
      return '升降命令'
    case 48:
    case 61:
      return '归一化设点'
    case 49:
    case 62:
      return '标度化设点'
    case 50:
    case 63:
      return '短浮点设点'
    case 51:
    case 64:
      return '位串命令'
    case 110:
      return '归一化参数'
    case 111:
      return '标度化参数'
    case 112:
      return '短浮点参数'
    case 113:
      return '参数激活'
    default:
      return '数据点'
  }
}

export function buildDefaultIec104PointName(
  typeIdOrName: number | string | null | undefined,
  address: number | null | undefined,
): string {
  const normalizedAddress = Math.max(0, Math.trunc(Number(address) || 0))
  return `${iec104DefaultPointNamePrefix(typeIdOrName)}_${normalizedAddress}`
}

// ─────────────────────────────────────────────────────────────────────────────
// 数据点类型解析（typeId 优先）
// ─────────────────────────────────────────────────────────────────────────────

/** 解析数据点的精确 ASDU typeName（如 "M_ME_NC_1"），typeId 为权威来源 */
export function resolveIec104TypeName(
  point: Pick<BackendDataPoint, 'data_type' | 'type_id' | 'name'>,
): string {
  const byTypeId = iec104TypeIdToName(point.type_id)
  if (byTypeId !== 'UNKNOWN') return byTypeId

  const normalized = String(point.data_type ?? '')
    .trim()
    .toUpperCase()
  if (normalized && IEC104_TYPE_NAME_TO_ID[normalized] != null) return normalized

  return 'UNKNOWN'
}

/** 从数据点解析 typeId（typeId 字段优先，data_type 字段备选） */
export function resolveIec104TypeId(
  point: Pick<BackendDataPoint, 'data_type' | 'type_id'>,
): number {
  const id = Math.trunc(Number(point.type_id))
  if (Number.isFinite(id) && id > 0) return id
  return iec104TypeNameToId(point.data_type) ?? 0
}

/**
 * 返回符合 IEC 104 协议语义的默认值。
 *
 * - M_SP (单点):  0 = 分(OFF)        ← 正常默认
 * - M_DP (双点):  1 = 分(OFF)        ← 注意：0 = 中间态(indeterminate)，不是合法静态状态
 * - M_ST (步位置): 0 = 中间位
 * - M_BO (位串):  0
 * - M_ME (遥测):  0.0
 * - M_IT (累计):  0
 * - M_EP (保护):  0
 * - C_*  (命令):  0
 */
export function iec104ProtocolDefaultValue(typeId: number | null | undefined): number {
  return getIec104Capability(typeId)?.value_model === 'double_point' ? 1 : 0
}

// ─────────────────────────────────────────────────────────────────────────────
// IOA 默认起始地址（国内电力行业惯例，十六进制分段）
// ─────────────────────────────────────────────────────────────────────────────

/**
 * 按 ASDU 类型返回国内电力行业惯例的 IOA 起始地址（十六进制分段）。
 *
 * | 类别 | 起始 IOA | 十六进制 |
 * |---|---|---|
 * | 遥信-单点 (M_SP) | 1 | 0x0001 |
 * | 遥信-双点 (M_DP) | 4097 | 0x1001 |
 * | 步位置 (M_ST) | 8193 | 0x2001 |
 * | 位串 (M_BO) | 12289 | 0x3001 |
 * | 遥测 (M_ME) | 16385 | 0x4001 |
 * | 遥控 (C_*) | 24577 | 0x6001 |
 * | 遥调 (C_SE_*) | 25089 | 0x6201 |
 * | 位串命令 (C_BO_*) | 25217 | 0x6281 |
 * | 遥脉 (M_IT) | 25601 | 0x6401 |
 * | 保护 (M_EP) | 28673 | 0x7001 |
 */
export function iec104DefaultStartAddress(typeId: number): number {
  const cat = categoryByTypeId(typeId)
  const id = Math.trunc(Number(typeId))
  const isSetpointCommand = [48, 49, 50, 61, 62, 63].includes(id)
  const isBitstringCommand = [51, 64].includes(id)
  switch (cat) {
    case 'single_point':
      return 0x0001 // 1
    case 'double_point':
      return 0x1001 // 4097
    case 'step_position':
      return 0x2001 // 8193
    case 'bitstring':
      return 0x3001 // 12289
    case 'measurement':
      return 0x4001 // 16385
    case 'command':
      if (isBitstringCommand) return 0x6281 // 25217
      return isSetpointCommand ? 0x6201 : 0x6001 // 25089 / 24577
    case 'integrated_total':
      return 0x6401 // 25601
    case 'protection':
      return 0x7001 // 28673
    default:
      return 0x0001
  }
}
