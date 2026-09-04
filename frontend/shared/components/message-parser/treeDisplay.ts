import type { MessageParserTreeNode } from '@shared/api/types'
import { getCurrentLocale, t } from '@shared/i18n'

const exactLabelKeys: Record<string, string> = {
  frame: 'messageParser.tree.labels.frame',
  'frame.raw-input': 'messageParser.tree.labels.rawInput',
  apci: 'messageParser.tree.labels.apci',
  'apci.start': 'messageParser.tree.labels.start',
  'apci.length': 'messageParser.tree.labels.length',
  'apci.type': 'messageParser.tree.labels.frameType',
  'apci.send-seq': 'messageParser.tree.labels.sendSequence',
  'apci.recv-seq': 'messageParser.tree.labels.receiveSequence',
  'apci.u-control': 'messageParser.tree.labels.control',
  asdu: 'messageParser.tree.labels.asdu',
  'asdu.header': 'messageParser.tree.labels.header',
  'asdu.header.type-id': 'messageParser.tree.labels.typeId',
  'asdu.header.vsq': 'messageParser.tree.labels.vsq',
  'asdu.header.cot': 'messageParser.tree.labels.cot',
  'asdu.header.ca': 'messageParser.tree.labels.commonAddress',
  'asdu.header.descriptor': 'messageParser.tree.labels.descriptor',
  'asdu.measurements': 'messageParser.tree.labels.measurements',
  'asdu.commands': 'messageParser.tree.labels.commands',
  'asdu.interrogation': 'messageParser.tree.labels.interrogation',
  'asdu.initialization': 'messageParser.tree.labels.initializationEnd',
  'asdu.test-command': 'messageParser.tree.labels.testCommand',
  'asdu.parameters': 'messageParser.tree.labels.parameters',
  'asdu.protection': 'messageParser.tree.labels.protectionEvents',
  'asdu.file-transfer': 'messageParser.tree.labels.fileTransfer',
  'asdu.security': 'messageParser.tree.labels.security',
}

const suffixLabelKeys: Array<[string, string]> = [
  ['embedded-asdu', 'messageParser.tree.labels.embeddedAsdu'],
  ['mac-algorithm', 'messageParser.tree.labels.macAlgorithm'],
  ['wrap-algorithm', 'messageParser.tree.labels.wrapAlgorithm'],
  ['wrapped-key', 'messageParser.tree.labels.wrappedKey'],
  ['error-code', 'messageParser.tree.labels.errorCode'],
  ['parse-error', 'messageParser.tree.labels.parseError'],
  ['summer-time', 'messageParser.tree.labels.summerTime'],
  ['send-seq', 'messageParser.tree.labels.sendSequence'],
  ['recv-seq', 'messageParser.tree.labels.receiveSequence'],
  ['u-control', 'messageParser.tree.labels.control'],
  ['timestamp', 'messageParser.tree.labels.timestamp'],
  ['elapsed-time', 'messageParser.tree.labels.elapsedTime'],
  ['clock', 'messageParser.tree.labels.clock'],
  ['coi', 'messageParser.tree.labels.coi'],
  ['descriptor', 'messageParser.tree.labels.descriptor'],
  ['qualifier', 'messageParser.tree.labels.qualifier'],
  ['challenge', 'messageParser.tree.labels.challenge'],
  ['sequence', 'messageParser.tree.labels.sequence'],
  ['formatted', 'messageParser.tree.labels.formatted'],
  ['fragment', 'messageParser.tree.labels.fragment'],
  ['quality', 'messageParser.tree.labels.quality'],
  ['payload', 'messageParser.tree.labels.payload'],
  ['minutes', 'messageParser.tree.labels.minutes'],
  ['seconds', 'messageParser.tree.labels.seconds'],
  ['weekday', 'messageParser.tree.labels.weekday'],
  ['invalid', 'messageParser.tree.labels.invalid'],
  ['milliseconds', 'messageParser.tree.labels.milliseconds'],
  ['association', 'messageParser.tree.labels.association'],
  ['length', 'messageParser.tree.labels.length'],
  ['detail', 'messageParser.tree.labels.command'],
  ['record', 'messageParser.tree.labels.detail'],
  ['value', 'messageParser.tree.labels.value'],
  ['quality', 'messageParser.tree.labels.quality'],
  ['reason', 'messageParser.tree.labels.reason'],
  ['status', 'messageParser.tree.labels.status'],
  ['hmac', 'messageParser.tree.labels.hmac'],
  ['user', 'messageParser.tree.labels.user'],
  ['text', 'messageParser.tree.labels.text'],
  ['type', 'messageParser.tree.labels.type'],
  ['year', 'messageParser.tree.labels.year'],
  ['month', 'messageParser.tree.labels.month'],
  ['hours', 'messageParser.tree.labels.hours'],
  ['raw', 'messageParser.tree.labels.raw'],
  ['hex', 'messageParser.tree.labels.hex'],
  ['day', 'messageParser.tree.labels.day'],
  ['fir', 'messageParser.tree.labels.fir'],
  ['fin', 'messageParser.tree.labels.fin'],
  ['fbp', 'messageParser.tree.labels.fbp'],
  ['ioa', 'messageParser.tree.labels.ioa'],
]

const staticTextKeys: Record<string, string> = {
  Frame: 'messageParser.tree.labels.frame',
  'Raw Input': 'messageParser.tree.labels.rawInput',
  'Raw Payload': 'messageParser.tree.labels.rawPayload',
  Start: 'messageParser.tree.labels.start',
  Length: 'messageParser.tree.labels.length',
  'Frame Type': 'messageParser.tree.labels.frameType',
  'Send Sequence': 'messageParser.tree.labels.sendSequence',
  'Receive Sequence': 'messageParser.tree.labels.receiveSequence',
  Control: 'messageParser.tree.labels.control',
  Header: 'messageParser.tree.labels.header',
  'Type ID': 'messageParser.tree.labels.typeId',
  'Common Address': 'messageParser.tree.labels.commonAddress',
  Descriptor: 'messageParser.tree.labels.descriptor',
  Measurements: 'messageParser.tree.labels.measurements',
  Commands: 'messageParser.tree.labels.commands',
  Interrogation: 'messageParser.tree.labels.interrogation',
  'Initialization End': 'messageParser.tree.labels.initializationEnd',
  'Test Command': 'messageParser.tree.labels.testCommand',
  Parameters: 'messageParser.tree.labels.parameters',
  'Protection Events': 'messageParser.tree.labels.protectionEvents',
  'File Transfer': 'messageParser.tree.labels.fileTransfer',
  Security: 'messageParser.tree.labels.security',
  Object: 'messageParser.tree.labels.object',
  Record: 'messageParser.tree.labels.record',
  Value: 'messageParser.tree.labels.value',
  Quality: 'messageParser.tree.labels.quality',
  Command: 'messageParser.tree.labels.command',
  Qualifier: 'messageParser.tree.labels.qualifier',
  Payload: 'messageParser.tree.labels.payload',
  Detail: 'messageParser.tree.labels.detail',
  Sequence: 'messageParser.tree.labels.sequence',
  User: 'messageParser.tree.labels.user',
  'MAC Algorithm': 'messageParser.tree.labels.macAlgorithm',
  Reason: 'messageParser.tree.labels.reason',
  Challenge: 'messageParser.tree.labels.challenge',
  HMAC: 'messageParser.tree.labels.hmac',
  'Embedded ASDU': 'messageParser.tree.labels.embeddedAsdu',
  'Wrap Algorithm': 'messageParser.tree.labels.wrapAlgorithm',
  Status: 'messageParser.tree.labels.status',
  'Wrapped Key': 'messageParser.tree.labels.wrappedKey',
  Association: 'messageParser.tree.labels.association',
  'Error Code': 'messageParser.tree.labels.errorCode',
  Timestamp: 'messageParser.tree.labels.timestamp',
  Text: 'messageParser.tree.labels.text',
  Fragment: 'messageParser.tree.labels.fragment',
  Hex: 'messageParser.tree.labels.hex',
  Type: 'messageParser.tree.labels.type',
  Formatted: 'messageParser.tree.labels.formatted',
  Raw: 'messageParser.tree.labels.raw',
  'Parse Error': 'messageParser.tree.labels.parseError',
  Minutes: 'messageParser.tree.labels.minutes',
  Seconds: 'messageParser.tree.labels.seconds',
  Milliseconds: 'messageParser.tree.labels.milliseconds',
  Invalid: 'messageParser.tree.labels.invalid',
  Year: 'messageParser.tree.labels.year',
  Month: 'messageParser.tree.labels.month',
  Day: 'messageParser.tree.labels.day',
  Weekday: 'messageParser.tree.labels.weekday',
  Hours: 'messageParser.tree.labels.hours',
  'Summer Time': 'messageParser.tree.labels.summerTime',
  Clock: 'messageParser.tree.labels.clock',
  COI: 'messageParser.tree.labels.coi',
  QDP: 'messageParser.tree.labels.qdp',
  'Event State': 'messageParser.tree.labels.eventState',
  'Start Flags': 'messageParser.tree.labels.startFlags',
  'Output Flags': 'messageParser.tree.labels.outputFlags',
  'Elapsed Time': 'messageParser.tree.labels.elapsedTime',
}

const staticSummaryKeys: Record<string, string> = {
  'IEC104 \u8d77\u59cb\u5b57\u8282': 'messageParser.tree.summary.startByte',
  'APDU \u957f\u5ea6\u5b57\u6bb5\uff0c\u4e0d\u5305\u542b\u8d77\u59cb\u5b57\u8282\u548c\u957f\u5ea6\u5b57\u8282':
    'messageParser.tree.summary.apduLength',
  '\u53d1\u9001\u5e8f\u53f7': 'messageParser.tree.summary.sendSequence',
  '\u786e\u8ba4\u63a5\u6536\u5e8f\u53f7': 'messageParser.tree.summary.receiveSequence',
  '\u76d1\u7763\u5e27\u786e\u8ba4\u63a5\u6536\u5e8f\u53f7':
    'messageParser.tree.summary.supervisoryReceiveSequence',
  'I \u5e27': 'messageParser.tree.frameTypes.i',
  'S \u5e27': 'messageParser.tree.frameTypes.s',
  'U \u5e27': 'messageParser.tree.frameTypes.u',
  '\u6807\u51c6\u6d4b\u8bd5\u7801 0x55AA': 'messageParser.tree.summary.standardTestCode',
  '\u975e\u6807\u51c6\u6d4b\u8bd5\u7801': 'messageParser.tree.summary.nonStandardTestCode',
  '\u672a\u63d0\u4f9b': 'messageParser.tree.weekdays.none',
  '\u5468\u4e00': 'messageParser.tree.weekdays.monday',
  '\u5468\u4e8c': 'messageParser.tree.weekdays.tuesday',
  '\u5468\u4e09': 'messageParser.tree.weekdays.wednesday',
  '\u5468\u56db': 'messageParser.tree.weekdays.thursday',
  '\u5468\u4e94': 'messageParser.tree.weekdays.friday',
  '\u5468\u516d': 'messageParser.tree.weekdays.saturday',
  '\u5468\u65e5': 'messageParser.tree.weekdays.sunday',
  '\u975e\u6cd5\u503c': 'messageParser.tree.weekdays.invalid',
}

const causeKeys: Record<number, string> = {
  1: 'periodic',
  2: 'backgroundScan',
  3: 'spontaneous',
  4: 'initialized',
  5: 'request',
  6: 'activation',
  7: 'activationConfirmation',
  8: 'deactivation',
  9: 'deactivationConfirmation',
  10: 'activationTermination',
  13: 'fileTransfer',
  20: 'interrogation',
  37: 'counterInterrogation',
  44: 'unknownTypeId',
  45: 'unknownCauseOfTransmission',
  46: 'unknownCommonAddress',
  47: 'unknownObjectAddress',
}

const frameTypeKeys: Record<string, string> = {
  I: 'i',
  S: 's',
  U: 'u',
}

function translateStaticText(value: string): string {
  const key = staticTextKeys[value] ?? staticSummaryKeys[value]
  return key ? t(key) : value
}

function joinSummaryParts(parts: Array<string | null | undefined>): string {
  return parts.filter(Boolean).join(t('messageParser.tree.summary.separator'))
}

function parseNumber(value?: string | null): number | null {
  if (!value) return null
  const trimmed = value.trim()
  const radix = trimmed.toLowerCase().startsWith('0x') ? 16 : 10
  const parsed = Number.parseInt(trimmed, radix)
  return Number.isFinite(parsed) ? parsed : null
}

function findNode(node: MessageParserTreeNode, id: string): MessageParserTreeNode | null {
  if (node.id === id) return node
  for (const child of node.children) {
    const found = findNode(child, id)
    if (found) return found
  }
  return null
}

function childValue(node: MessageParserTreeNode, id: string): string | null {
  return node.children.find((child) => child.id === id)?.value ?? null
}

function childValueBySuffix(node: MessageParserTreeNode, suffix: string): string | null {
  return node.children.find((child) => child.id.endsWith(`.${suffix}`))?.value ?? null
}

function nodeByteCount(node: MessageParserTreeNode): number | null {
  if (node.byte_range) return node.byte_range.end - node.byte_range.start
  const length = childValueBySuffix(node, 'length')
  return parseNumber(length)
}

function formatByteCount(count?: number | null): string | null {
  return count == null ? null : t('messageParser.tree.summary.bytes', { count })
}

function formatObjectCount(count?: number | null): string | null {
  return count == null ? null : t('messageParser.tree.summary.informationObjects', { count })
}

function getTypeName(node: MessageParserTreeNode): string | null {
  return findNode(node, 'asdu.header.type-id')?.summary ?? null
}

function getCauseCode(node: MessageParserTreeNode): number | null {
  return parseNumber(findNode(node, 'asdu.header.cot')?.value)
}

function getCommonAddress(node: MessageParserTreeNode): string | null {
  return findNode(node, 'asdu.header.ca')?.value ?? null
}

function getObjectCount(node: MessageParserTreeNode): number | null {
  const vsq = parseNumber(findNode(node, 'asdu.header.vsq')?.value)
  return vsq == null ? null : vsq & 0x7f
}

function formatFrameSummary(node: MessageParserTreeNode): string | null {
  const frameType = findNode(node, 'apci.type')?.value
  const typeName = getTypeName(node)
  const cause = formatCauseDescriptionLabel(
    getCauseCode(node),
    findNode(node, 'asdu.header.cot')?.summary,
  )
  return joinSummaryParts([
    formatFrameTypeLabel(frameType),
    typeName,
    formatByteCount(nodeByteCount(node)),
    cause,
  ])
}

function formatApciSummary(node: MessageParserTreeNode): string | null {
  const frameType = childValue(node, 'apci.type')
  const frameTypeLabel = formatFrameTypeLabel(frameType)
  if (!frameTypeLabel) return null

  if (frameType === 'I') {
    return t('messageParser.tree.summary.apciI', {
      frameType: frameTypeLabel,
      send: childValue(node, 'apci.send-seq') ?? '',
      receive: childValue(node, 'apci.recv-seq') ?? '',
    })
  }
  if (frameType === 'S') {
    return t('messageParser.tree.summary.apciS', {
      frameType: frameTypeLabel,
      receive: childValue(node, 'apci.recv-seq') ?? '',
    })
  }
  if (frameType === 'U') {
    return joinSummaryParts([frameTypeLabel, childValue(node, 'apci.u-control')])
  }
  return frameTypeLabel
}

function formatAsduSummary(node: MessageParserTreeNode): string | null {
  const typeName = getTypeName(node)
  const cause = formatCauseDescriptionLabel(
    getCauseCode(node),
    findNode(node, 'asdu.header.cot')?.summary,
  )
  const commonAddress = getCommonAddress(node)
  const objectCount = getObjectCount(node)
  return joinSummaryParts([
    typeName,
    cause,
    commonAddress ? `CA=${commonAddress}` : null,
    formatObjectCount(objectCount),
  ])
}

function formatHeaderSummary(node: MessageParserTreeNode): string | null {
  const typeName = childValue(node, 'asdu.header.type-id')
    ? node.children.find((child) => child.id === 'asdu.header.type-id')?.summary
    : null
  const cause = formatCauseDescriptionLabel(parseNumber(childValue(node, 'asdu.header.cot')))
  const count = parseNumber(childValue(node, 'asdu.header.vsq'))
  return joinSummaryParts([typeName, formatObjectCount(count == null ? null : count & 0x7f), cause])
}

function formatVsqSummary(node: MessageParserTreeNode): string | null {
  const raw = parseNumber(node.value)
  if (raw == null) return null
  const count = raw & 0x7f
  const sequenceKey = raw & 0x80 ? 'sequenceContinuous' : 'sequenceIndependent'
  return t('messageParser.tree.summary.vsq', {
    count,
    sequence: t(`messageParser.tree.summary.${sequenceKey}`),
  })
}

function formatCotSummary(node: MessageParserTreeNode): string | null {
  const cause = formatCauseDescriptionLabel(parseNumber(node.value), node.summary)
  const origin = /origin=(\d+)/.exec(node.summary ?? '')?.[1] ?? '0'
  const flags = [
    node.summary?.includes('test') ? t('messageParser.tree.summary.cotFlagTest') : null,
    node.summary?.includes('negative') ? t('messageParser.tree.summary.cotFlagNegative') : null,
  ]
  return joinSummaryParts([cause, `origin=${origin}`, ...flags])
}

function localizeDescriptorValue(value?: string | null): string {
  if (value === 'variable') return t('messageParser.tree.summary.variable')
  if (value === 'none') return t('messageParser.tree.summary.none')
  return value ?? ''
}

function formatDescriptorSummary(node: MessageParserTreeNode): string | null {
  const summary = node.summary ?? ''
  const ioa = /IOA=([^,\uff0c]+)/.exec(summary)?.[1]
  const objectLength = /(?:\u5bf9\u8c61\u957f\u5ea6|object length)=([^,\uff0c]+)/i.exec(
    summary,
  )?.[1]
  const timestamp = /(?:\u65f6\u95f4\u6233|timestamp)=([^,\uff0c]+)/i.exec(summary)?.[1]
  if (!ioa && !objectLength && !timestamp) return null
  return t('messageParser.tree.summary.descriptor', {
    ioa: ioa ?? '',
    objectLength: localizeDescriptorValue(objectLength),
    timestamp: localizeDescriptorValue(timestamp),
  })
}

function formatGroupCount(node: MessageParserTreeNode, key: string): string {
  return t(`messageParser.tree.summary.${key}`, { count: node.children.length })
}

function formatObjectSummary(node: MessageParserTreeNode): string | null {
  const ioa = childValueBySuffix(node, 'ioa')
  const value =
    childValueBySuffix(node, 'value') ??
    childValueBySuffix(node, 'detail') ??
    childValueBySuffix(node, 'payload')
  if (!ioa || !value) return null
  return t('messageParser.tree.summary.ioaValue', { ioa, value })
}

function formatQualifierObjectSummary(node: MessageParserTreeNode): string | null {
  const ioa = childValueBySuffix(node, 'ioa')
  const qualifier = childValueBySuffix(node, 'qualifier')
  if (!ioa || !qualifier) return null
  return t('messageParser.tree.summary.ioaQualifier', { ioa, qualifier })
}

function formatFbpObjectSummary(node: MessageParserTreeNode): string | null {
  const ioa = childValueBySuffix(node, 'ioa')
  const fbp = childValueBySuffix(node, 'fbp')
  if (!ioa || !fbp) return null
  return t('messageParser.tree.summary.ioaFbp', { ioa, fbp })
}

function formatTimestampSummary(node: MessageParserTreeNode): string | null {
  const type = childValueBySuffix(node, 'type')
  const formatted = childValueBySuffix(node, 'formatted')
  if (type && formatted) {
    return t('messageParser.tree.summary.timestampParsed', { type, value: formatted })
  }

  const parseError = childValueBySuffix(node, 'parse-error')
  const count = nodeByteCount(node)
  if (parseError && count != null) {
    return t('messageParser.tree.summary.timestampParseFailed', { count })
  }
  return null
}

function formatKnownFieldSummary(node: MessageParserTreeNode): string | null {
  if (node.id.endsWith('.weekday')) {
    return formatWeekdayLabel(parseNumber(node.value), node.summary)
  }

  if (node.id.endsWith('.fbp')) {
    return node.value === '0x55AA'
      ? t('messageParser.tree.summary.standardTestCode')
      : t('messageParser.tree.summary.nonStandardTestCode')
  }

  const countMatch =
    /^(\d+)\s*\u5b57\u8282(?:\u968f\u673a\u8d28\u8be2|\u5185\u5d4c ASDU|\u6587\u672c|\u7247\u6bb5)?$/.exec(
      node.summary ?? '',
    )
  if (countMatch) {
    const count = Number.parseInt(countMatch[1], 10)
    if (node.id.endsWith('.challenge'))
      return t('messageParser.tree.summary.randomChallengeBytes', { count })
    if (node.id.endsWith('.embedded-asdu'))
      return t('messageParser.tree.summary.embeddedAsduBytes', { count })
    if (node.id.endsWith('.text')) return t('messageParser.tree.summary.textBytes', { count })
    if (node.id.endsWith('.fragment'))
      return t('messageParser.tree.summary.fragmentBytes', { count })
    return formatByteCount(count)
  }

  return null
}

function translateFallbackSummary(summary?: string | null): string | null {
  if (!summary) return null

  const exact = staticSummaryKeys[summary]
  if (exact) return t(exact)

  if (getCurrentLocale() !== 'en-US') return summary

  return summary
    .replace(/I \u5e27/g, t('messageParser.tree.frameTypes.i'))
    .replace(/S \u5e27/g, t('messageParser.tree.frameTypes.s'))
    .replace(/U \u5e27/g, t('messageParser.tree.frameTypes.u'))
    .replace(
      /(\d+) \u4e2a\u4fe1\u606f\u5bf9\u8c61/g,
      t('messageParser.tree.summary.informationObjects', { count: '$1' }),
    )
    .replace(
      /(\d+) \u4e2a\u547d\u4ee4\u5bf9\u8c61/g,
      t('messageParser.tree.summary.commandObjects', { count: '$1' }),
    )
    .replace(
      /(\d+) \u4e2a\u53ec\u5524\u5bf9\u8c61/g,
      t('messageParser.tree.summary.interrogationObjects', { count: '$1' }),
    )
    .replace(
      /(\d+) \u4e2a\u6d4b\u8bd5\u5bf9\u8c61/g,
      t('messageParser.tree.summary.testObjects', { count: '$1' }),
    )
    .replace(
      /(\d+) \u4e2a\u53c2\u6570\u5bf9\u8c61/g,
      t('messageParser.tree.summary.parameterObjects', { count: '$1' }),
    )
    .replace(
      /(\d+) \u4e2a\u4fdd\u62a4\u4e8b\u4ef6/g,
      t('messageParser.tree.summary.protectionEvents', { count: '$1' }),
    )
    .replace(
      /(\d+) \u6761\u6587\u4ef6\u4f20\u8f93\u8bb0\u5f55/g,
      t('messageParser.tree.summary.fileRecords', { count: '$1' }),
    )
    .replace(/(\d+) \u5b57\u8282/g, t('messageParser.tree.summary.bytes', { count: '$1' }))
    .replace(/\uff0c/g, ', ')
}

function formatFrameTypeLabel(frameType?: string | null, fallback?: string | null): string | null {
  const key = frameType ? frameTypeKeys[frameType.toUpperCase()] : null
  if (key) return t(`messageParser.tree.frameTypes.${key}`)
  if (fallback) return translateStaticText(fallback)
  return null
}

function formatCauseDescriptionLabel(
  cause?: number | null,
  fallback?: string | null,
): string | null {
  if (cause != null && cause >= 21 && cause <= 36) {
    return t('messageParser.tree.causes.groupInterrogation', { group: cause - 20 })
  }
  if (cause != null && cause >= 38 && cause <= 41) {
    return t('messageParser.tree.causes.groupCounterInterrogation', { group: cause - 37 })
  }

  const key = cause == null ? null : causeKeys[cause]
  if (key) return t(`messageParser.tree.causes.${key}`)
  return translateFallbackSummary(fallback) ?? null
}

function formatWeekdayLabel(value?: number | null, fallback?: string | null): string | null {
  if (value != null && value >= 0 && value <= 7) {
    const keys = [
      'none',
      'monday',
      'tuesday',
      'wednesday',
      'thursday',
      'friday',
      'saturday',
      'sunday',
    ]
    return t(`messageParser.tree.weekdays.${keys[value]}`)
  }
  return translateFallbackSummary(fallback)
}

export function getParsedFrameTreeLabel(node: MessageParserTreeNode): string {
  const objectMatch =
    /^(?:measurement|command|interrogation|test-command|parameter|protection)\.(\d+)$/.exec(node.id)
  if (objectMatch) {
    return t('messageParser.tree.labels.object', { index: Number.parseInt(objectMatch[1], 10) + 1 })
  }

  const recordMatch = /^file-transfer\.(\d+)$/.exec(node.id)
  if (recordMatch) {
    return t('messageParser.tree.labels.record', { index: Number.parseInt(recordMatch[1], 10) + 1 })
  }

  const exact = exactLabelKeys[node.id]
  if (exact) return t(exact)

  if (
    node.children.length > 0 &&
    (node.id.includes('.raw') || node.id.endsWith('payload') || node.label === 'Raw Payload')
  ) {
    return t('messageParser.tree.labels.rawPayload')
  }

  const suffix = suffixLabelKeys.find(([candidate]) => node.id.endsWith(`.${candidate}`))
  if (suffix) return t(suffix[1])

  return translateStaticText(node.label)
}

export function getParsedFrameTreeSummary(node: MessageParserTreeNode): string | null {
  if (node.id === 'frame') return formatFrameSummary(node)
  if (node.id === 'apci') return formatApciSummary(node)
  if (node.id === 'asdu') return formatAsduSummary(node)
  if (node.id === 'asdu.header') return formatHeaderSummary(node)
  if (node.id === 'asdu.header.vsq') return formatVsqSummary(node)
  if (node.id === 'asdu.header.cot') return formatCotSummary(node)
  if (node.id === 'asdu.header.descriptor') return formatDescriptorSummary(node)
  if (node.id === 'asdu.measurements') return formatGroupCount(node, 'informationObjects')
  if (node.id === 'asdu.commands') return formatGroupCount(node, 'commandObjects')
  if (node.id === 'asdu.interrogation') return formatGroupCount(node, 'interrogationObjects')
  if (node.id === 'asdu.test-command') return formatGroupCount(node, 'testObjects')
  if (node.id === 'asdu.parameters') return formatGroupCount(node, 'parameterObjects')
  if (node.id === 'asdu.protection') return formatGroupCount(node, 'protectionEvents')
  if (node.id === 'asdu.file-transfer') return formatGroupCount(node, 'fileRecords')
  if (/^(?:measurement|command|parameter|protection)\.\d+$/.test(node.id))
    return formatObjectSummary(node)
  if (/^interrogation\.\d+$/.test(node.id)) return formatQualifierObjectSummary(node)
  if (/^test-command\.\d+$/.test(node.id)) return formatFbpObjectSummary(node)
  if (node.id.endsWith('.timestamp')) return formatTimestampSummary(node)
  if (
    node.children.length > 0 &&
    (node.id.includes('.raw') || node.id.endsWith('payload') || node.label === 'Raw Payload')
  ) {
    return formatByteCount(nodeByteCount(node))
  }

  return formatKnownFieldSummary(node) ?? translateFallbackSummary(node.summary)
}
