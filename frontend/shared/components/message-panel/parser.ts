import {
  getIec104Capability,
  iec104TypeIdToLocalizedName,
  iec104TypeIdToLocalizedShortName,
} from '@shared/api/iec104'
import type { MessageFrameParseResult, MessageParserTreeNode } from '@shared/api/types'
import { t } from '@shared/i18n'

import type { FrameSummary, Message, MessageFrameMeta, ParsedRow } from './types'

export function hexToBytes(hex: string): number[] {
  const clean = hex.replace(/\s/g, '')
  if (!clean || clean.length % 2 !== 0 || /[^0-9a-f]/i.test(clean)) return []
  const bytes: number[] = []
  for (let index = 0; index < clean.length; index += 2) {
    bytes.push(Number.parseInt(clean.slice(index, index + 2), 16))
  }
  return bytes
}

export function formatHighlightedHex(hexData?: string): string {
  const bytes = hexToBytes(hexData ?? '')
  if (bytes.length === 0) return ''
  const typeId = bytes.length > 6 ? bytes[6] : null
  const ioaLength = typeId == null ? 0 : (getIec104Capability(typeId)?.ioa_length ?? 0)
  return bytes
    .map((value, index) => {
      let className = 'hex-byte'
      let title = `Byte ${index}`
      if (index === 0 && value === 0x68) {
        className += ' hex-start'
        title = t('frame.byteStart')
      } else if (index >= 2 && index <= 5) {
        className += ' hex-ctrl'
        title = t('frame.controlField')
      } else if (index === 6) {
        className += ' hex-type'
        title = t('frame.typeId')
      } else if (index >= 8 && index <= 9) {
        className += ' hex-cot'
        title = t('frame.cot')
      } else if (index >= 10 && index <= 11) {
        className += ' hex-coa'
        title = t('frame.commonAddress')
      } else if (ioaLength > 0 && index >= 12 && index < 12 + ioaLength) {
        className += ' hex-ioa'
        title = t('frame.informationObjectAddress')
      }
      const byte = value.toString(16).padStart(2, '0').toUpperCase()
      return `<span class="${className}" title="${title}">${byte}</span>`
    })
    .join(' ')
}

export function formatHexData(hexData: string): string {
  return hexToBytes(hexData)
    .map((value) => value.toString(16).padStart(2, '0').toUpperCase())
    .join(' ')
}

function getCotDescription(cot: number): string {
  const descriptions: Record<number, string> = {
    1: t('frame.cotDescriptions.periodic'),
    2: t('frame.cotDescriptions.background'),
    3: t('frame.cotDescriptions.spontaneous'),
    4: t('frame.cotDescriptions.initialized'),
    5: t('frame.cotDescriptions.request'),
    6: t('frame.cotDescriptions.activation'),
    7: t('frame.cotDescriptions.activationConfirm'),
    8: t('frame.cotDescriptions.deactivation'),
    9: t('frame.cotDescriptions.deactivationConfirm'),
    10: t('frame.cotDescriptions.activationTermination'),
    11: t('frame.cotDescriptions.remoteCommand'),
    12: t('frame.cotDescriptions.localCommand'),
    13: t('frame.cotDescriptions.fileTransfer'),
    20: t('frame.cotDescriptions.interrogation'),
    37: t('frame.cotDescriptions.counterInterrogation'),
    44: t('frame.cotDescriptions.unknownTypeId'),
    45: t('frame.cotDescriptions.unknownCot'),
    46: t('frame.cotDescriptions.unknownCommonAddress'),
    47: t('frame.cotDescriptions.unknownObjectAddress'),
  }
  if (cot >= 21 && cot <= 36) return `${t('frame.cotDescriptions.groupInterrogation')} ${cot - 20}`
  if (cot >= 38 && cot <= 41)
    return `${t('frame.cotDescriptions.counterInterrogation')} ${cot - 37}`
  return descriptions[cot] ?? ''
}

function getCotSummaryText(cotLow: number): string {
  const cot = cotLow & 0x3f
  const base = getCotDescription(cot) || `COT=${cot}`
  const flags: string[] = []
  if ((cotLow & 0x40) !== 0) flags.push(t('frame.negative'))
  if ((cotLow & 0x80) !== 0) flags.push(t('frame.test'))
  return flags.length > 0 ? `${base}(${flags.join('/')})` : base
}

export function parseIoaFromBytes(bytes: number[], offset: number): number | null {
  if (bytes.length < offset + 3) return null
  return bytes[offset] | (bytes[offset + 1] << 8) | (bytes[offset + 2] << 16)
}

function buildCommandSummaryHint(bytes: number[], typeId: number): string {
  if ((getIec104Capability(typeId)?.ioa_length ?? 0) === 0) return ''
  const ioa = parseIoaFromBytes(bytes, 12)
  if (ioa == null) return ''
  const ioaText = `IOA=${ioa}`
  const read = (index: number) => (index < bytes.length ? bytes[index] : null)
  if ([45, 58].includes(typeId)) {
    const raw = read(15)
    return raw == null
      ? ioaText
      : `${ioaText} ${(raw & 0x80) !== 0 ? t('frame.select') : t('frame.execute')}/${(raw & 1) !== 0 ? t('frame.close') : t('frame.open')}`
  }
  if ([46, 47, 59, 60].includes(typeId)) {
    const raw = read(15)
    return raw == null
      ? ioaText
      : `${ioaText} ${(raw & 0x80) !== 0 ? t('frame.select') : t('frame.execute')}/${t('frame.status')}=${raw & 3}`
  }
  if ([48, 49, 61, 62].includes(typeId)) {
    const qos = read(17)
    return qos == null
      ? ioaText
      : `${ioaText} ${(qos & 0x80) !== 0 ? t('frame.select') : t('frame.execute')} QL=${qos & 0x7f}`
  }
  if ([50, 63].includes(typeId)) {
    const qos = read(19)
    return qos == null
      ? ioaText
      : `${ioaText} ${(qos & 0x80) !== 0 ? t('frame.select') : t('frame.execute')} QL=${qos & 0x7f}`
  }
  if ([51, 64].includes(typeId)) return `${ioaText} ${t('frame.writeBitstring')}`
  if (typeId === 100) return read(15) == null ? ioaText : `${ioaText} QOI=${read(15)}`
  if (typeId === 101) return read(15) == null ? ioaText : `${ioaText} QCC=${read(15)}`
  if (typeId === 102) return `${ioaText} ${t('frame.readCommand')}`
  if (typeId === 105) return read(15) == null ? ioaText : `${ioaText} QRP=${read(15)}`
  if (typeId === 107) return `${ioaText} ${t('frame.testCommand')}`
  return ''
}

function decodeUFrameType(control: number): string {
  if (control & 0x0c)
    return control & 0x08 ? t('frame.uFrameTypes.startConfirm') : t('frame.uFrameTypes.start')
  if (control & 0x30)
    return control & 0x20 ? t('frame.uFrameTypes.stopConfirm') : t('frame.uFrameTypes.stop')
  if (control & 0xc0)
    return control & 0x80 ? t('frame.uFrameTypes.testConfirm') : t('frame.uFrameTypes.test')
  return t('frame.unknown')
}

export function getFrameSummary(hexData: string | undefined): FrameSummary {
  const bytes = hexToBytes(hexData ?? '')
  if (bytes.length === 0) return { tag: '', label: '', color: '', title: '' }
  if (bytes.length < 6 || bytes[0] !== 0x68) {
    const label = t('frame.unknownFragment')
    return { tag: '[?]', label, color: 'frame-unknown', title: label }
  }
  const control = bytes[2]
  if ((control & 0x03) === 0x03) {
    const label = decodeUFrameType(control)
    return { tag: '[U]', label, color: 'frame-u', title: label }
  }
  if ((control & 0x01) === 0x01) {
    const nr = (bytes[4] | (bytes[5] << 8)) >> 1
    const label = `N(R)=${nr}`
    return { tag: '[S]', label, color: 'frame-s', title: label }
  }
  if (bytes.length >= 12) {
    const typeId = bytes[6]
    const lifecycle = getCotSummaryText(bytes[8])
    const hint = buildCommandSummaryHint(bytes, typeId)
    const businessName = iec104TypeIdToLocalizedShortName(typeId)
    const label = [`${businessName || t('frame.unknown')} (${typeId})`, lifecycle, hint]
      .filter(Boolean)
      .join(' ')
    const capability = getIec104Capability(typeId)
    const fullTypeName = capability
      ? `${capability.name} (Type ID: ${typeId}) ${iec104TypeIdToLocalizedName(typeId)}`
      : `${t('frame.unknown')} (Type ID: ${typeId})`
    const title = [fullTypeName, lifecycle, hint].filter(Boolean).join(' ')
    return { tag: '[I]', label, color: 'frame-i', title }
  }
  const label = `N(S)=${(control | (bytes[3] << 8)) >> 1}`
  return { tag: '[I]', label, color: 'frame-i', title: label }
}

function flattenNode(node: MessageParserTreeNode, parents: string[], rows: ParsedRow[]): void {
  if (node.children.length === 0) {
    rows.push({
      field: [...parents, node.label].filter(Boolean).join(' / '),
      value: node.value ?? '—',
      description: node.summary ?? '',
    })
    return
  }
  const nextParents = node.id === 'frame' ? parents : [...parents, node.label]
  for (const child of node.children) flattenNode(child, nextParents, rows)
}

export function getSmartParsedData(
  message: Message,
  parseResult?: MessageFrameParseResult | null,
): ParsedRow[] {
  if (parseResult?.tree.length) {
    const rows: ParsedRow[] = []
    for (const node of parseResult.tree) flattenNode(node, [], rows)
    if (rows.length > 0) return rows
  }
  if (message.parsed) {
    return Object.entries(message.parsed)
      .filter(([, value]) => value != null)
      .map(([field, value]) => ({
        field,
        value: typeof value === 'object' ? JSON.stringify(value) : String(value),
        description: '',
      }))
  }
  return [{ field: t('frame.hint'), value: t('frame.noParsedData'), description: '' }]
}

const normalizeOptionalNumber = (value: unknown): number | null => {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? Math.trunc(numeric) : null
}

export function extractMessageFrameMeta(message: Message): MessageFrameMeta {
  let typeId = normalizeOptionalNumber(message.parsed?.typeId)
  let commonAddress = normalizeOptionalNumber(message.parsed?.commonAddress)
  let ioa = normalizeOptionalNumber(message.parsed?.ioa)
  if ((typeId == null || commonAddress == null || ioa == null) && message.hexData) {
    const bytes = hexToBytes(message.hexData)
    if (bytes.length >= 12 && bytes[0] === 0x68 && (bytes[2] & 0x01) === 0) {
      typeId ??= bytes[6]
      commonAddress ??= bytes[10] | (bytes[11] << 8)
      if ((getIec104Capability(typeId)?.ioa_length ?? 0) > 0) ioa ??= parseIoaFromBytes(bytes, 12)
    }
  }
  return { typeId, commonAddress, ioa }
}
