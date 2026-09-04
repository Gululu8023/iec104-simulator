import type { BackendDataPointQualityCommon, BackendDataPointQualityDetail } from '../types'
import { t } from '@shared/i18n'
import { getIec104Capability } from './catalog'

export type CommonQualityFlagKey = 'IV' | 'NT' | 'SB' | 'BL'

export interface QualityFlagDetail {
  flag: string // 缩写: IV / NT / SB / BL
  label: string // 中文描述
  active: boolean // 是否置位
}

// 位布局(IEC104 线路语义):
// bit7 = IV, bit6 = NT, bit5 = SB, bit4 = BL
const QUALITY_BIT_IV = 0x80
const QUALITY_BIT_NT = 0x40
const QUALITY_BIT_SB = 0x20
const QUALITY_BIT_BL = 0x10

const requireQualityByte = (qualityRaw: number): number => {
  const raw = Number(qualityRaw)
  if (!Number.isInteger(raw) || raw < 0 || raw > 0xff) {
    throw new RangeError(`quality raw must be an integer in 0..255, got ${String(qualityRaw)}`)
  }
  return raw
}

export function normalizeQualityRaw(qualityRaw: number): number {
  const raw = requireQualityByte(qualityRaw)
  const wire = raw & 0xf0
  if (wire !== 0 || (raw & 0x0f) === 0) {
    return wire
  }

  return (
    (raw & 0x01 ? QUALITY_BIT_IV : 0) |
    (raw & 0x02 ? QUALITY_BIT_NT : 0) |
    (raw & 0x04 ? QUALITY_BIT_SB : 0) |
    (raw & 0x08 ? QUALITY_BIT_BL : 0)
  )
}

export function buildQualityCommonFromRaw(qualityRaw: number): BackendDataPointQualityCommon {
  const normalized = normalizeQualityRaw(qualityRaw)
  return {
    iv: (normalized & QUALITY_BIT_IV) !== 0,
    nt: (normalized & QUALITY_BIT_NT) !== 0,
    sb: (normalized & QUALITY_BIT_SB) !== 0,
    bl: (normalized & QUALITY_BIT_BL) !== 0,
  }
}

export function qualityRawToFlagKeys(qualityRaw: number): CommonQualityFlagKey[] {
  const normalized = normalizeQualityRaw(qualityRaw)
  const flags: CommonQualityFlagKey[] = []
  if ((normalized & QUALITY_BIT_IV) !== 0) flags.push('IV')
  if ((normalized & QUALITY_BIT_NT) !== 0) flags.push('NT')
  if ((normalized & QUALITY_BIT_SB) !== 0) flags.push('SB')
  if ((normalized & QUALITY_BIT_BL) !== 0) flags.push('BL')
  return flags
}

export function buildQualityRawFromFlagKeys(
  flags: Iterable<CommonQualityFlagKey | string>,
): number {
  let raw = 0
  for (const flag of flags) {
    switch (
      String(flag ?? '')
        .trim()
        .toUpperCase()
    ) {
      case 'IV':
        raw |= QUALITY_BIT_IV
        break
      case 'NT':
        raw |= QUALITY_BIT_NT
        break
      case 'SB':
        raw |= QUALITY_BIT_SB
        break
      case 'BL':
        raw |= QUALITY_BIT_BL
        break
      default:
        break
    }
  }
  return raw
}

export function encodeUiQualityToRaw(quality: unknown): number {
  const asNumber = Number(quality)
  if (Number.isFinite(asNumber)) {
    return normalizeQualityRaw(asNumber)
  }

  const normalized = String(quality ?? '')
    .trim()
    .toLowerCase()
  if (normalized === 'invalid') return QUALITY_BIT_IV
  if (normalized === 'questionable') return QUALITY_BIT_NT
  if (normalized === '' || normalized === 'good') return 0
  throw new RangeError(`unsupported quality value: ${String(quality)}`)
}

function normalizeQualityCommon(
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityRaw: number,
  qualityDetail?: BackendDataPointQualityDetail | null,
): BackendDataPointQualityCommon {
  if (
    qualityDetail?.kind === 'siq' ||
    qualityDetail?.kind === 'diq' ||
    qualityDetail?.kind === 'qds'
  ) {
    return {
      iv: Boolean(qualityDetail.iv),
      nt: Boolean(qualityDetail.nt),
      sb: Boolean(qualityDetail.sb),
      bl: Boolean(qualityDetail.bl),
    }
  }
  if (qualityDetail?.kind === 'bcr') {
    return {
      iv: Boolean(qualityDetail.iv),
      nt: false,
      sb: false,
      bl: false,
    }
  }

  const fromRaw = buildQualityCommonFromRaw(qualityRaw)
  if (qualityCommon) {
    const normalized = {
      iv: Boolean(qualityCommon.iv),
      nt: Boolean(qualityCommon.nt),
      sb: Boolean(qualityCommon.sb),
      bl: Boolean(qualityCommon.bl),
    }
    const normalizedHasFlags = normalized.iv || normalized.nt || normalized.sb || normalized.bl
    const rawHasFlags = fromRaw.iv || fromRaw.nt || fromRaw.sb || fromRaw.bl
    if (normalizedHasFlags || !rawHasFlags) {
      return normalized
    }
  }
  return fromRaw
}

function activeQualityFlags(
  common: BackendDataPointQualityCommon,
  qualityDetail?: BackendDataPointQualityDetail | null,
): string[] {
  if (qualityDetail?.kind === 'bcr') {
    const flags: string[] = []
    if (qualityDetail.iv) flags.push('IV')
    if (qualityDetail.cy) flags.push('CY')
    if (qualityDetail.ca) flags.push('CA')
    return flags
  }

  const active: string[] = []
  if (common.iv) active.push('IV')
  if (common.nt) active.push('NT')
  if (common.sb) active.push('SB')
  if (common.bl) active.push('BL')
  if (qualityDetail?.kind === 'qds' && qualityDetail.ov) active.push('OV')
  return active
}

export function formatCommonQualityLabel(
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityRaw = 0,
  qualityDetail?: BackendDataPointQualityDetail | null,
): string {
  const common = normalizeQualityCommon(qualityCommon, qualityRaw, qualityDetail)
  const flags = activeQualityFlags(common, qualityDetail)
  if (flags.length === 0) return 'Good'
  if (flags.length === 1) return flags[0]
  return `${flags[0]} +`
}

export function hasPointQuality(typeId: number | null | undefined): boolean {
  const model = getIec104Capability(typeId)?.quality_model
  return model != null && model !== 'none'
}

export function formatPointQualityLabelLocalized(
  typeId: number | null | undefined,
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityRaw = 0,
  qualityDetail?: BackendDataPointQualityDetail | null,
): string {
  if (!hasPointQuality(typeId)) return t('iec104.quality.states.notPresent')
  return formatCommonQualityLabel(qualityCommon, qualityRaw, qualityDetail)
}

export function qualityLevelFromCommon(
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityRaw = 0,
  qualityDetail?: BackendDataPointQualityDetail | null,
): 'good' | 'invalid' | 'questionable' {
  const common = normalizeQualityCommon(qualityCommon, qualityRaw, qualityDetail)
  if (qualityDetail?.kind === 'bcr') {
    if (qualityDetail.iv) return 'invalid'
    if (qualityDetail.cy || qualityDetail.ca) return 'questionable'
    return 'good'
  }

  const hasDescriptorFlag = qualityDetail?.kind === 'qds' ? Boolean(qualityDetail.ov) : false
  if (!common.iv && !common.nt && !common.sb && !common.bl && !hasDescriptorFlag) return 'good'
  if (common.iv) return 'invalid'
  return 'questionable'
}

function formatDpiStateLabel(dpi: number): string {
  switch (dpi & 0x03) {
    case 1:
      return '分(Off)'
    case 2:
      return '合(On)'
    case 3:
      return '不确定(Indeterminate)'
    default:
      return '中间态(Intermediate)'
  }
}

function formatDpiStateLabelLocalized(dpi: number): string {
  switch (dpi & 0x03) {
    case 1:
      return t('iec104.values.doubleOff')
    case 2:
      return t('iec104.values.doubleOn')
    case 3:
      return t('iec104.values.doubleIndeterminate')
    default:
      return t('iec104.values.doubleIntermediate')
  }
}

export function formatDescriptorDetail(
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): string[] {
  if (!qualityDetail) return []
  switch (qualityDetail.kind) {
    case 'siq':
      return [`SPI: ${qualityDetail.spi ? '合(On)' : '分(Off)'}`]
    case 'diq':
      return [`DPI: ${qualityDetail.dpi} (${formatDpiStateLabel(qualityDetail.dpi)})`]
    case 'qds':
      return [
        `OV: ${qualityDetail.ov ? '溢出(Overflow)' : '未溢出(No Overflow)'}`,
        ...(qualityDetail.transient == null
          ? []
          : [`T: ${qualityDetail.transient ? '瞬变(Transient)' : '稳定(Stable)'}`]),
        ...(qualityDetail.scd_status == null
          ? []
          : [`SCD Status: 0x${qualityDetail.scd_status.toString(16).padStart(4, '0')}`]),
        ...(qualityDetail.scd_change == null
          ? []
          : [`SCD Change: 0x${qualityDetail.scd_change.toString(16).padStart(4, '0')}`]),
      ]
    case 'bcr':
      return [
        `SQ: ${qualityDetail.sq} 顺序号(Sequence Number)`,
        `CY: ${qualityDetail.cy ? '进位(Carry)' : '无进位(No Carry)'}`,
        `CA: ${qualityDetail.ca ? '计数调整(Adjusted)' : '未调整(Not Adjusted)'}`,
        `IV: ${qualityDetail.iv ? '无效(Invalid)' : '有效(Valid)'}`,
      ]
    case 'none':
    default:
      return []
  }
}

export function formatDescriptorDetailLocalized(
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): string[] {
  if (!qualityDetail) return []
  switch (qualityDetail.kind) {
    case 'siq':
      return [
        t('iec104.quality.details.spi', {
          value: qualityDetail.spi ? t('iec104.values.singleOn') : t('iec104.values.singleOff'),
        }),
      ]
    case 'diq':
      return [
        t('iec104.quality.details.dpi', {
          value: qualityDetail.dpi,
          label: formatDpiStateLabelLocalized(qualityDetail.dpi),
        }),
      ]
    case 'qds':
      return [
        t('iec104.quality.details.ov', {
          value: qualityDetail.ov
            ? t('iec104.quality.states.overflow')
            : t('iec104.quality.states.notOverflow'),
        }),
        ...(qualityDetail.transient == null
          ? []
          : [`T: ${qualityDetail.transient ? 'Transient' : 'Stable'}`]),
        ...(qualityDetail.scd_status == null
          ? []
          : [`SCD Status: 0x${qualityDetail.scd_status.toString(16).padStart(4, '0')}`]),
        ...(qualityDetail.scd_change == null
          ? []
          : [`SCD Change: 0x${qualityDetail.scd_change.toString(16).padStart(4, '0')}`]),
      ]
    case 'bcr':
      return [
        t('iec104.quality.details.sq', { value: qualityDetail.sq }),
        t('iec104.quality.details.cy', {
          value: qualityDetail.cy
            ? t('iec104.quality.states.carry')
            : t('iec104.quality.states.noCarry'),
        }),
        t('iec104.quality.details.ca', {
          value: qualityDetail.ca
            ? t('iec104.quality.states.adjusted')
            : t('iec104.quality.states.notAdjusted'),
        }),
        t('iec104.quality.details.iv', {
          value: qualityDetail.iv
            ? t('iec104.quality.states.invalid')
            : t('iec104.quality.states.valid'),
        }),
      ]
    case 'none':
    default:
      return []
  }
}

export interface TooltipKeyValueRow {
  label: string
  value: string
}

interface QualityTooltipRow {
  label: string
  value: string
  active: boolean
}

const TOOLTIP_TABLE_STYLE = 'border-spacing:4px 2px'
const TOOLTIP_LABEL_STYLE = 'color:#a78bfa;white-space:nowrap;vertical-align:top'
const TOOLTIP_MARKER_BASE_STYLE = 'white-space:nowrap;vertical-align:top;padding:0 6px'
const TOOLTIP_MARKER_ACTIVE_STYLE = `${TOOLTIP_MARKER_BASE_STYLE};color:#22c55e;font-weight:700`
const TOOLTIP_MARKER_INACTIVE_STYLE = `${TOOLTIP_MARKER_BASE_STYLE};color:#a78bfa;font-weight:700`

function escapeTooltipHtml(value: string): string {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

export function formatKeyValueTooltip(rows: TooltipKeyValueRow[]): string {
  const normalizedRows = rows
    .map((row) => ({
      label: String(row.label || '').trim(),
      value: String(row.value ?? '').trim() || '--',
    }))
    .filter((row) => row.label.length > 0)

  if (normalizedRows.length === 0) return '--'

  return `<table style="${TOOLTIP_TABLE_STYLE}">${normalizedRows
    .map(
      (row) =>
        `<tr><td style="${TOOLTIP_LABEL_STYLE}">${escapeTooltipHtml(row.label)}</td><td style="${TOOLTIP_MARKER_INACTIVE_STYLE}">-</td><td>${escapeTooltipHtml(row.value)}</td></tr>`,
    )
    .join('')}</table>`
}

export function formatInformationValueSuffix(
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): string {
  return qualityDetail?.kind === 'qds' && qualityDetail.transient === true ? ' (T)' : ''
}

export function hasInformationValueDetail(
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): boolean {
  return (
    qualityDetail?.kind === 'qds' &&
    (qualityDetail.transient != null ||
      qualityDetail.scd_status != null ||
      qualityDetail.scd_change != null)
  )
}

export function formatInformationValueTooltipLocalized(
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): string {
  if (!hasInformationValueDetail(qualityDetail) || qualityDetail?.kind !== 'qds') return ''
  const rows: TooltipKeyValueRow[] = []
  if (qualityDetail.transient != null) {
    rows.push({
      label: 'T',
      value: t(
        qualityDetail.transient
          ? 'iec104.quality.states.transient'
          : 'iec104.quality.states.stable',
      ),
    })
  }
  if (qualityDetail.scd_status != null) {
    rows.push({
      label: t('iec104.quality.details.scdStatus'),
      value: `0x${qualityDetail.scd_status.toString(16).toUpperCase().padStart(4, '0')}`,
    })
  }
  if (qualityDetail.scd_change != null) {
    rows.push({
      label: t('iec104.quality.details.scdChange'),
      value: `0x${qualityDetail.scd_change.toString(16).toUpperCase().padStart(4, '0')}`,
    })
  }
  return rows.length > 0 ? formatKeyValueTooltip(rows) : ''
}

export function formatControlConfirmationText(negative: boolean | null | undefined): string {
  return negative ? '否定确认' : '肯定确认'
}

export function formatControlConfirmationTextLocalized(
  negative: boolean | null | undefined,
): string {
  return negative
    ? t('iec104.quality.confirmation.negative')
    : t('iec104.quality.confirmation.positive')
}

function formatQualityTooltipRows(rows: QualityTooltipRow[]): string {
  const normalizedRows = rows
    .map((row) => ({
      label: String(row.label || '').trim(),
      value: String(row.value ?? '').trim() || '--',
      active: Boolean(row.active),
    }))
    .filter((row) => row.label.length > 0)

  if (normalizedRows.length === 0) return '--'

  return `<table style="${TOOLTIP_TABLE_STYLE}">${normalizedRows
    .map(
      (row) =>
        `<tr><td style="${TOOLTIP_LABEL_STYLE}">${escapeTooltipHtml(row.label)}:</td><td style="${row.active ? TOOLTIP_MARKER_ACTIVE_STYLE : TOOLTIP_MARKER_INACTIVE_STYLE}">${row.active ? '&#10003;' : '-'}</td><td>${escapeTooltipHtml(row.value)}</td></tr>`,
    )
    .join('')}</table>`
}

function buildQualityTooltipRows(
  common: BackendDataPointQualityCommon,
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): QualityTooltipRow[] {
  if (qualityDetail?.kind === 'bcr') {
    return [
      { label: 'SQ', value: `${qualityDetail.sq} 顺序号(Sequence Number)`, active: false },
      {
        label: 'CY',
        value: qualityDetail.cy ? '进位(Carry)' : '无进位(No Carry)',
        active: qualityDetail.cy,
      },
      {
        label: 'CA',
        value: qualityDetail.ca ? '计数调整(Adjusted)' : '未调整(Not Adjusted)',
        active: qualityDetail.ca,
      },
      {
        label: 'IV',
        value: qualityDetail.iv ? '无效(Invalid)' : '有效(Valid)',
        active: qualityDetail.iv,
      },
    ]
  }

  const rows: QualityTooltipRow[] = [
    { label: 'IV', value: '无效(Invalid)', active: common.iv },
    { label: 'NT', value: '非当前(Not Topical)', active: common.nt },
    { label: 'SB', value: '取代(Substituted)', active: common.sb },
    { label: 'BL', value: '闭锁(Blocked)', active: common.bl },
  ]

  if (qualityDetail?.kind === 'qds') {
    rows.push({
      label: 'OV',
      value: qualityDetail.ov ? '溢出(Overflow)' : '未溢出(No Overflow)',
      active: qualityDetail.ov,
    })
  } else if (qualityDetail?.kind === 'siq') {
    rows.push({ label: 'SPI', value: qualityDetail.spi ? '合(On)' : '分(Off)', active: false })
  } else if (qualityDetail?.kind === 'diq') {
    rows.push({
      label: 'DPI',
      value: `${formatDpiStateLabel(qualityDetail.dpi)}`,
      active: false,
    })
  }

  return rows
}

function buildQualityTooltipRowsLocalized(
  common: BackendDataPointQualityCommon,
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
): QualityTooltipRow[] {
  if (qualityDetail?.kind === 'bcr') {
    return [
      {
        label: 'SQ',
        value: t('iec104.quality.details.sequenceNumber', { value: qualityDetail.sq }),
        active: false,
      },
      {
        label: 'CY',
        value: qualityDetail.cy
          ? t('iec104.quality.states.carry')
          : t('iec104.quality.states.noCarry'),
        active: qualityDetail.cy,
      },
      {
        label: 'CA',
        value: qualityDetail.ca
          ? t('iec104.quality.states.adjusted')
          : t('iec104.quality.states.notAdjusted'),
        active: qualityDetail.ca,
      },
      {
        label: 'IV',
        value: qualityDetail.iv
          ? t('iec104.quality.states.invalid')
          : t('iec104.quality.states.valid'),
        active: qualityDetail.iv,
      },
    ]
  }

  const rows: QualityTooltipRow[] = [
    { label: 'IV', value: t('iec104.quality.flagLabels.IV'), active: common.iv },
    { label: 'NT', value: t('iec104.quality.flagLabels.NT'), active: common.nt },
    { label: 'SB', value: t('iec104.quality.flagLabels.SB'), active: common.sb },
    { label: 'BL', value: t('iec104.quality.flagLabels.BL'), active: common.bl },
  ]

  if (qualityDetail?.kind === 'qds') {
    rows.push({
      label: 'OV',
      value: qualityDetail.ov
        ? t('iec104.quality.states.overflow')
        : t('iec104.quality.states.notOverflow'),
      active: qualityDetail.ov,
    })
  } else if (qualityDetail?.kind === 'siq') {
    rows.push({
      label: 'SPI',
      value: qualityDetail.spi ? t('iec104.values.singleOn') : t('iec104.values.singleOff'),
      active: false,
    })
  } else if (qualityDetail?.kind === 'diq') {
    rows.push({
      label: 'DPI',
      value: formatDpiStateLabelLocalized(qualityDetail.dpi),
      active: false,
    })
  }

  return rows
}

export function formatQualityTooltip(
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
  qualityRaw = 0,
): string {
  const common = normalizeQualityCommon(qualityCommon, qualityRaw, qualityDetail)
  return formatQualityTooltipRows(buildQualityTooltipRows(common, qualityDetail))
}

export function formatQualityTooltipLocalized(
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
  qualityRaw = 0,
): string {
  const common = normalizeQualityCommon(qualityCommon, qualityRaw, qualityDetail)
  return formatQualityTooltipRows(buildQualityTooltipRowsLocalized(common, qualityDetail))
}

export function formatPointQualityTooltipLocalized(
  typeId: number | null | undefined,
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  qualityDetail: BackendDataPointQualityDetail | null | undefined,
  qualityRaw = 0,
): string {
  if (!hasPointQuality(typeId)) return t('iec104.quality.states.notPresentDescription')
  return formatQualityTooltipLocalized(qualityCommon, qualityDetail, qualityRaw)
}

/**
 * 解析品质字节（u8）所有标志位的完整描述。
 * 与本地 iec60870-parser crate 的 QualityFlags::from_u8 对齐。
 */
export function formatQualityFlags(quality: number): QualityFlagDetail[] {
  const common = normalizeQualityCommon(null, quality)
  return [
    { flag: 'IV', label: '无效(Invalid)', active: common.iv },
    { flag: 'NT', label: '非当前(Not Topical)', active: common.nt },
    { flag: 'SB', label: '取代(Substituted)', active: common.sb },
    { flag: 'BL', label: '闭锁(Blocked)', active: common.bl },
  ]
}

export function formatQualityFlagsLocalized(quality: number): QualityFlagDetail[] {
  const common = normalizeQualityCommon(null, quality)
  return [
    { flag: 'IV', label: t('iec104.quality.flagLabels.IV'), active: common.iv },
    { flag: 'NT', label: t('iec104.quality.flagLabels.NT'), active: common.nt },
    { flag: 'SB', label: t('iec104.quality.flagLabels.SB'), active: common.sb },
    { flag: 'BL', label: t('iec104.quality.flagLabels.BL'), active: common.bl },
  ]
}

/**
 * 简洁品质标签：Good / IV / NT+SB 等。
 */
export function formatQualityLabel(quality: number): string {
  return formatCommonQualityLabel(null, quality)
}

/**
 * 品质等级分类（用于 Tag / 颜色）。
 * - good:         0 → 绿色
 * - invalid:      IV 置位 → 红色
 * - questionable: 其他标志位 → 黄色/警告
 */
export function qualityLevel(quality: number): 'good' | 'invalid' | 'questionable' {
  return qualityLevelFromCommon(null, quality)
}
