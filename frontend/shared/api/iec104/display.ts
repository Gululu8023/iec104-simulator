import type { BackendDataPoint } from '../types'
import {
  type AsduCategory,
  categoryByTypeId,
  getIec104Capability,
  iec104TypeNameToId,
  isDoublePointType,
  isIntegratedTotalType,
  isSinglePointType,
  resolveIec104TypeName,
} from './catalog'
import { t } from '@shared/i18n'

function resolveTypeId(typeIdOrName: number | string | null | undefined): number {
  if (typeof typeIdOrName === 'number') return typeIdOrName
  return iec104TypeNameToId(typeIdOrName as string) ?? 0
}

export type SemanticValueClass =
  | 'value-on'
  | 'value-off'
  | 'value-transient'
  | 'value-invalid'
  | 'value-normal'

// ─────────────────────────────────────────────────────────────────────────────
// 排序与显示
// ─────────────────────────────────────────────────────────────────────────────

/** ASDU 类别排序序号（用于设备树/Tab 排序） */
const CATEGORY_SORT_ORDER: Record<AsduCategory, number> = {
  single_point: 0,
  double_point: 1,
  step_position: 2,
  bitstring: 3,
  measurement: 4,
  integrated_total: 5,
  protection: 6,
  command: 7,
  security: 8,
  system: 9,
  parameter: 10,
  file_transfer: 11,
  unknown: 99,
}

export function compareIec104TypeForDisplay(leftType: string, rightType: string): number {
  const leftId = iec104TypeNameToId(leftType) ?? 999
  const rightId = iec104TypeNameToId(rightType) ?? 999
  const leftCat = CATEGORY_SORT_ORDER[categoryByTypeId(leftId)] ?? 99
  const rightCat = CATEGORY_SORT_ORDER[categoryByTypeId(rightId)] ?? 99
  if (leftCat !== rightCat) return leftCat - rightCat
  return leftId - rightId
}

export function normalizeIecObjectDisplayName(value: string | null | undefined): string {
  let normalized = String(value ?? '')
    .trim()
    .replace(/\s+/g, ' ')
  if (!normalized) return ''

  // Remove trailing count marker, e.g. "对象名 (20)"
  normalized = normalized.replace(/\s*\(\d+\)\s*$/, '').trim()
  // Remove trailing ASDU type marker, e.g. "Point M_SP_NA_1" / "Point_M_SP_NA_1"
  normalized = normalized.replace(/(?:[\s_-]+)[MC]_[A-Z]{2}_[A-Z]{2}_\d$/i, '').trim()

  return normalized
}

export function extractIecObjectNameFromPointName(
  pointName: string | null | undefined,
  address: number,
): string {
  let normalized = String(pointName ?? '').trim()
  if (!normalized) return ''

  const numericAddress = Math.max(1, Math.trunc(Number(address) || 0))
  if (numericAddress > 0) {
    const suffixes = [`_${numericAddress}`, `-${numericAddress}`, ` ${numericAddress}`]
    for (const suffix of suffixes) {
      if (normalized.length > suffix.length && normalized.endsWith(suffix)) {
        normalized = normalized.slice(0, -suffix.length).trim()
        break
      }
    }
  }

  return normalizeIecObjectDisplayName(normalized)
}

export function summarizeIecObjectNameByType(
  points: Array<Pick<BackendDataPoint, 'name' | 'address' | 'data_type' | 'type_id'>>,
): Record<string, string> {
  const candidates = new Map<string, Map<string, number>>()

  for (const point of points) {
    const dataType = resolveIec104TypeName(point)
    const objectName = extractIecObjectNameFromPointName(point.name, point.address)
    if (!objectName) continue

    const typeCandidates = candidates.get(dataType) ?? new Map<string, number>()
    typeCandidates.set(objectName, (typeCandidates.get(objectName) ?? 0) + 1)
    candidates.set(dataType, typeCandidates)
  }

  const displayNameByType: Record<string, string> = {}
  for (const [dataType, typeCandidates] of candidates.entries()) {
    const sortedCandidates = Array.from(typeCandidates.entries()).sort((left, right) => {
      if (right[1] !== left[1]) return right[1] - left[1]
      return left[0].localeCompare(right[0], 'zh-CN', { numeric: true })
    })
    const topCandidate = sortedCandidates[0]?.[0]
    if (topCandidate) {
      displayNameByType[dataType] = topCandidate
    }
  }

  return displayNameByType
}

// ─────────────────────────────────────────────────────────────────────────────
// 语义值格式化
// ─────────────────────────────────────────────────────────────────────────────

export function formatSemanticPointValue(
  typeIdOrName: number | string | null | undefined,
  value: unknown,
): string {
  const typeId = resolveTypeId(typeIdOrName)
  const capability = getIec104Capability(typeId)

  if (capability?.value_model === 'boolean') {
    return normalizeSinglePointState(value) === 1 ? '合(1)' : '分(0)'
  }

  if (capability?.value_model === 'double_point') {
    switch (normalizeDoublePointState(value)) {
      case 1:
        return '分(1)'
      case 2:
        return '合(2)'
      case 3:
        return '无效(3)'
      default:
        return '中间态(0)'
    }
  }

  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return '--'

  // ── 步位置 (M_ST): 有符号整数 -64~+63 ──
  if (capability?.value_model === 'step_position') {
    const intVal = Math.max(-64, Math.min(63, Math.round(numeric)))
    return String(intVal)
  }

  // ── 位串 (M_BO): 十六进制显示 ──
  if (capability?.value_model === 'bitstring32') {
    const u32 = Math.round(numeric) >>> 0
    return `0x${u32.toString(16).toUpperCase().padStart(8, '0')}`
  }

  if (capability?.family === 'packed_status') {
    const status = Math.round(numeric) & 0xffff
    return `0x${status.toString(16).toUpperCase().padStart(4, '0')}`
  }

  // ── 遥测-归一化 (M_ME_NA/ND): 4 位小数, 范围 [-1, 1] ──
  if (capability?.value_model === 'normalized') {
    return numeric.toFixed(4)
  }

  // ── 遥测-标度化 (M_ME_NB): 整数型, 范围 [-32768, 32767] ──
  if (capability?.value_model === 'scaled_i16') {
    return String(Math.round(numeric))
  }

  // ── 遥测-短浮点 (M_ME_NC): 2 位小数 ──
  if (capability?.value_model === 'float32') {
    return numeric.toFixed(2)
  }

  // ── 累计量 (M_IT): 整数 ──
  if (capability?.value_model === 'counter') {
    return String(Math.round(numeric))
  }

  // ── 保护 (M_EP): 整数状态 ──
  if (capability?.family === 'protection') {
    return String(Math.round(numeric))
  }

  // ── 其他 (命令/系统/参数等): 通用数值 ──
  return Number.isInteger(numeric) ? String(numeric) : numeric.toFixed(4).replace(/\.?0+$/, '')
}

export function formatSemanticPointValueLocalized(
  typeIdOrName: number | string | null | undefined,
  value: unknown,
): string {
  const typeId = resolveTypeId(typeIdOrName)
  const capability = getIec104Capability(typeId)

  if (capability?.value_model === 'boolean') {
    return normalizeSinglePointState(value) === 1
      ? t('iec104.values.singleOn')
      : t('iec104.values.singleOff')
  }

  if (capability?.value_model === 'double_point') {
    switch (normalizeDoublePointState(value)) {
      case 1:
        return t('iec104.values.doubleOff')
      case 2:
        return t('iec104.values.doubleOn')
      case 3:
        return t('iec104.values.doubleInvalid')
      default:
        return t('iec104.values.doubleIntermediate')
    }
  }

  return formatSemanticPointValue(typeIdOrName, value)
}

export function semanticPointValueClass(
  typeIdOrName: number | string | null | undefined,
  value: unknown,
): SemanticValueClass {
  const typeId = resolveTypeId(typeIdOrName)
  const capability = getIec104Capability(typeId)

  if (capability?.value_model === 'boolean') {
    return normalizeSinglePointState(value) === 1 ? 'value-on' : 'value-off'
  }

  if (capability?.value_model === 'double_point') {
    const state = normalizeDoublePointState(value)
    if (state === 1) return 'value-off'
    if (state === 2) return 'value-on'
    if (state === 0) return 'value-transient'
    return 'value-invalid'
  }

  return 'value-normal'
}

export function parseSemanticPointValue(
  dataType: string | null | undefined,
  value: unknown,
  fallback = 0,
): number {
  if (isSinglePointType(dataType)) {
    return parseSinglePointState(value)
  }

  if (isDoublePointType(dataType)) {
    return parseDoublePointState(value)
  }

  const numeric = Number(value)
  if (!Number.isFinite(numeric)) {
    return fallback
  }

  if (isIntegratedTotalType(dataType)) {
    return Math.round(numeric)
  }

  return numeric
}

// ─────────────────────────────────────────────────────────────────────────────
// 内部辅助函数
// ─────────────────────────────────────────────────────────────────────────────

function normalizeSinglePointState(value: unknown): 0 | 1 {
  return Number(value) > 0.5 ? 1 : 0
}

function normalizeDoublePointState(value: unknown): 0 | 1 | 2 | 3 {
  const numeric = Number(value)
  const state = Math.max(0, Math.min(3, Math.round(Number.isFinite(numeric) ? numeric : 0)))
  if (state === 1) return 1
  if (state === 2) return 2
  if (state === 3) return 3
  return 0
}

function parseSinglePointState(value: unknown): 0 | 1 {
  if (typeof value === 'string') {
    const raw = value.trim().toLowerCase()
    if (
      raw.includes('合') ||
      raw === 'on' ||
      raw === 'true' ||
      raw === 'close' ||
      raw === 'closed'
    ) {
      return 1
    }
    if (
      raw.includes('分') ||
      raw === 'off' ||
      raw === 'false' ||
      raw === 'open' ||
      raw === 'opened'
    ) {
      return 0
    }
  }

  return normalizeSinglePointState(value)
}

function parseDoublePointState(value: unknown): 0 | 1 | 2 | 3 {
  if (typeof value === 'string') {
    const raw = value.trim().toLowerCase()
    if (raw.includes('无效') || raw.includes('invalid')) return 3
    if (raw.includes('中间') || raw.includes('indeterminate')) return 0
    if (raw.includes('合') || raw === 'on' || raw === 'true') return 2
    if (raw.includes('分') || raw === 'off' || raw === 'false') return 1
  }

  return normalizeDoublePointState(value)
}
