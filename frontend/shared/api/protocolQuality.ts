/**
 * 协议品质属性分组定义与编辑工具函数。
 *
 * 按 capability registry 动态渲染协议属性分组（SIQ / DIQ / QDS / BCR / QDP），
 * 替代原来硬编码的「质量 + 描述词 + SQ」固定布局。
 */

import { getIec104Capability } from './iec104/catalog'
import type { CommonQualityFlagKey } from './iec104/quality'
import { buildQualityRawFromFlagKeys } from './iec104/quality'
import { t } from '@shared/i18n'
import type { BackendDataPointQualityCommon, BackendDataPointQualityDetail } from './types'

// ─────────────────────────────────────────────────────────────────────────────
// 协议品质分组类型
// ─────────────────────────────────────────────────────────────────────────────

/** 品质分组类型 */
export type ProtocolQualityKind = 'siq' | 'diq' | 'qds' | 'bcr' | 'qdp' | 'none'

/** 单个协议标志位定义 */
export interface ProtocolFlagDef {
  /** 标志位缩写 (IV / NT / SB / BL / OV / CA / CY) */
  key: string
  /** 中文标签 */
  label: string
  /** 所属编辑分组: 'common' = 通用品质位, 'descriptor' = 描述词位 */
  group: 'common' | 'descriptor'
}

/** 判断是否为 BCR 品质类型 */
export function isBcrQualityType(typeId: number): boolean {
  return resolveProtocolQualityKind(typeId) === 'bcr'
}

/** 判断是否为 QDS 品质类型 */
export function isQdsQualityType(typeId: number): boolean {
  return resolveProtocolQualityKind(typeId) === 'qds'
}

/** 按 TI 解析品质分组类型 */
export function resolveProtocolQualityKind(typeId: number): ProtocolQualityKind {
  return getIec104Capability(typeId)?.quality_model ?? 'none'
}

/** 分组中文标题 */
export function getProtocolQualityGroupLabel(kind: ProtocolQualityKind): string {
  switch (kind) {
    case 'siq':
      return '品质描述 (SIQ)'
    case 'diq':
      return '品质描述 (DIQ)'
    case 'qds':
      return '品质描述 (QDS)'
    case 'bcr':
      return '计数量描述 (BCR)'
    case 'qdp':
      return '保护事件品质 (QDP)'
    case 'none':
      return '品质'
  }
}

export function getProtocolQualityGroupLabelLocalized(kind: ProtocolQualityKind): string {
  return t(`iec104.quality.groups.${kind}`)
}

// ── 各分组的标志位定义 ──

const COMMON_FLAGS: ProtocolFlagDef[] = [
  { key: 'IV', label: '无效 (IV)', group: 'common' },
  { key: 'NT', label: '非当前 (NT)', group: 'common' },
  { key: 'SB', label: '取代 (SB)', group: 'common' },
  { key: 'BL', label: '闭锁 (BL)', group: 'common' },
]

const QDS_DESCRIPTOR_FLAGS: ProtocolFlagDef[] = [
  { key: 'OV', label: '溢出 (OV)', group: 'descriptor' },
]

const BCR_DESCRIPTOR_FLAGS: ProtocolFlagDef[] = [
  { key: 'CA', label: '计数器调整 (CA)', group: 'descriptor' },
  { key: 'CY', label: '进位/溢出 (CY)', group: 'descriptor' },
]

const BCR_COMMON_FLAGS: ProtocolFlagDef[] = [{ key: 'IV', label: '无效 (IV)', group: 'common' }]

/** 按分组类型返回可编辑的标志位定义 */
export function getProtocolQualityFields(kind: ProtocolQualityKind): ProtocolFlagDef[] {
  switch (kind) {
    case 'siq':
    case 'diq':
      return [...COMMON_FLAGS]
    case 'qds':
      return [...QDS_DESCRIPTOR_FLAGS, ...COMMON_FLAGS]
    case 'bcr':
      return [...BCR_COMMON_FLAGS, ...BCR_DESCRIPTOR_FLAGS]
    case 'qdp':
    case 'none':
      return []
  }
}

const localizeProtocolFlag = (flag: ProtocolFlagDef): ProtocolFlagDef => ({
  ...flag,
  label: t(`iec104.quality.flagDefs.${flag.key}`),
})

export function getProtocolQualityFieldsLocalized(kind: ProtocolQualityKind): ProtocolFlagDef[] {
  return getProtocolQualityFields(kind).map(localizeProtocolFlag)
}

// ─────────────────────────────────────────────────────────────────────────────
// 编辑工具函数（从 SlaveContentPanel.vue 迁移）
// ─────────────────────────────────────────────────────────────────────────────

export type QualityDescriptorFlagKey = 'OV' | 'CY' | 'CA'

const COMMON_QUALITY_FLAG_ORDER: CommonQualityFlagKey[] = ['IV', 'NT', 'SB', 'BL']

function normalizeSinglePointValue(value: unknown): boolean {
  return Number(value) > 0.5
}

function normalizeDoublePointValue(value: unknown): number {
  const numeric = Math.trunc(Number(value))
  if (!Number.isFinite(numeric)) return 0
  if (numeric >= 1 && numeric <= 3) return numeric
  return 0
}

/** 按 TI 归一化通用品质标志 */
export function normalizeCommonQualityFlags(
  typeId: number,
  flags: readonly CommonQualityFlagKey[],
): CommonQualityFlagKey[] {
  const selected = new Set(flags)
  const qualityKind = resolveProtocolQualityKind(typeId)
  const allowed =
    qualityKind === 'bcr'
      ? (['IV'] as const)
      : qualityKind === 'siq' || qualityKind === 'diq' || qualityKind === 'qds'
        ? COMMON_QUALITY_FLAG_ORDER
        : []
  return allowed.filter((flag) =>
    selected.has(flag as CommonQualityFlagKey),
  ) as CommonQualityFlagKey[]
}

/** 按 TI 归一化描述词品质标志 */
export function normalizeDescriptorQualityFlags(
  typeId: number,
  flags: readonly QualityDescriptorFlagKey[],
): QualityDescriptorFlagKey[] {
  const selected = new Set(flags)
  if (isQdsQualityType(typeId))
    return (['OV'] as const).filter((f) =>
      selected.has(f as QualityDescriptorFlagKey),
    ) as QualityDescriptorFlagKey[]
  if (isBcrQualityType(typeId))
    return (['CY', 'CA'] as const).filter((f) =>
      selected.has(f as QualityDescriptorFlagKey),
    ) as QualityDescriptorFlagKey[]
  return []
}

/** 归一化 BCR 序列号 (0-31) */
export function normalizeBcrSequence(value: unknown): number {
  return Math.max(0, Math.min(31, Math.trunc(Number(value) || 0)))
}

/** 从 qualityRaw 提取可编辑的通用品质标志 */
export function extractEditableCommonQualityFlags(
  typeId: number,
  qualityRaw: number,
): CommonQualityFlagKey[] {
  const qualityKind = resolveProtocolQualityKind(typeId)
  if (!['siq', 'diq', 'qds', 'bcr'].includes(qualityKind)) return []
  const flags: CommonQualityFlagKey[] = []
  if ((qualityRaw & 0x80) !== 0) flags.push('IV')
  if (!isBcrQualityType(typeId) && (qualityRaw & 0x40) !== 0) flags.push('NT')
  if (!isBcrQualityType(typeId) && (qualityRaw & 0x20) !== 0) flags.push('SB')
  if (!isBcrQualityType(typeId) && (qualityRaw & 0x10) !== 0) flags.push('BL')
  return flags
}

/** 从 qualityRaw 提取可编辑的描述词品质标志 */
export function extractEditableDescriptorQualityFlags(
  typeId: number,
  qualityRaw: number,
): QualityDescriptorFlagKey[] {
  const flags: QualityDescriptorFlagKey[] = []
  if (isQdsQualityType(typeId) && (qualityRaw & 0x01) !== 0) flags.push('OV')
  if (isBcrQualityType(typeId) && (qualityRaw & 0x20) !== 0) flags.push('CY')
  if (isBcrQualityType(typeId) && (qualityRaw & 0x40) !== 0) flags.push('CA')
  return flags
}

/** 编辑态类型 */
export interface EditableQualityFields {
  commonQualityFlags: CommonQualityFlagKey[]
  descriptorQualityFlags: QualityDescriptorFlagKey[]
  bcrSequence: number
  typeId: number
}

/** 从编辑态构建 qualityRaw */
export function buildEditableQualityRaw(point: EditableQualityFields): number {
  if (isBcrQualityType(point.typeId)) {
    let raw = normalizeBcrSequence(point.bcrSequence) & 0x1f
    if (normalizeCommonQualityFlags(point.typeId, point.commonQualityFlags).includes('IV'))
      raw |= 0x80
    for (const flag of normalizeDescriptorQualityFlags(
      point.typeId,
      point.descriptorQualityFlags,
    )) {
      if (flag === 'CY') raw |= 0x20
      if (flag === 'CA') raw |= 0x40
    }
    return raw
  }

  let raw = buildQualityRawFromFlagKeys(
    normalizeCommonQualityFlags(point.typeId, point.commonQualityFlags),
  )
  if (
    isQdsQualityType(point.typeId) &&
    normalizeDescriptorQualityFlags(point.typeId, point.descriptorQualityFlags).includes('OV')
  ) {
    raw |= 0x01
  }
  return raw
}

/** 构建本地 qualityCommon */
export function buildLocalQualityCommon(
  typeId: number,
  qualityRaw: number,
): BackendDataPointQualityCommon {
  const qualityKind = resolveProtocolQualityKind(typeId)
  if (!['siq', 'diq', 'qds', 'bcr'].includes(qualityKind)) {
    return { iv: false, nt: false, sb: false, bl: false }
  }
  return {
    iv: (qualityRaw & 0x80) !== 0,
    nt: isBcrQualityType(typeId) ? false : (qualityRaw & 0x40) !== 0,
    sb: isBcrQualityType(typeId) ? false : (qualityRaw & 0x20) !== 0,
    bl: isBcrQualityType(typeId) ? false : (qualityRaw & 0x10) !== 0,
  }
}

/** 从 qualityRaw 构建本地 qualityDetail */
export function buildLocalQualityDetailFromRaw(
  typeId: number,
  qualityRaw: number,
  pointValue?: unknown,
): BackendDataPointQualityDetail {
  const qualityKind = resolveProtocolQualityKind(typeId)
  if (qualityKind === 'siq') {
    return {
      kind: 'siq',
      raw: qualityRaw,
      spi: normalizeSinglePointValue(pointValue),
      iv: (qualityRaw & 0x80) !== 0,
      nt: (qualityRaw & 0x40) !== 0,
      sb: (qualityRaw & 0x20) !== 0,
      bl: (qualityRaw & 0x10) !== 0,
    }
  }
  if (qualityKind === 'diq') {
    return {
      kind: 'diq',
      raw: qualityRaw,
      dpi: normalizeDoublePointValue(pointValue),
      iv: (qualityRaw & 0x80) !== 0,
      nt: (qualityRaw & 0x40) !== 0,
      sb: (qualityRaw & 0x20) !== 0,
      bl: (qualityRaw & 0x10) !== 0,
    }
  }
  if (isBcrQualityType(typeId)) {
    return {
      kind: 'bcr',
      raw: qualityRaw,
      sq: qualityRaw & 0x1f,
      cy: (qualityRaw & 0x20) !== 0,
      ca: (qualityRaw & 0x40) !== 0,
      iv: (qualityRaw & 0x80) !== 0,
    }
  }
  if (isQdsQualityType(typeId)) {
    return {
      kind: 'qds',
      raw: qualityRaw,
      ov: (qualityRaw & 0x01) !== 0,
      iv: (qualityRaw & 0x80) !== 0,
      nt: (qualityRaw & 0x40) !== 0,
      sb: (qualityRaw & 0x20) !== 0,
      bl: (qualityRaw & 0x10) !== 0,
    }
  }
  return { kind: 'none' }
}

/** 优先使用后端明细；若缺失或为 none，则按 TI + qualityRaw 本地回推 */
export function resolveEffectiveQualityDetail(
  typeId: number,
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
  qualityRaw: number,
  pointValue?: unknown,
): BackendDataPointQualityDetail {
  const qualityKind = resolveProtocolQualityKind(typeId)
  if (qualityDetail?.kind === qualityKind) return qualityDetail
  return buildLocalQualityDetailFromRaw(typeId, qualityRaw, pointValue)
}
