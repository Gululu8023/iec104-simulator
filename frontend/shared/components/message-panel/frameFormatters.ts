import { extractMessageFrameMeta } from './parser'
import { getCurrentLocale, t } from '@shared/i18n'

import type { Message } from './types'

export const formatDetailTime = (timestamp: number) => {
  const date = new Date(timestamp)
  const pad = (value: number, length = 2) => String(value).padStart(length, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`
}

export const getDirectionLabel = (type: string) => {
  switch (type) {
    case 'sent':
      return 'TX'
    case 'received':
      return 'RX'
    case 'error':
      return 'ERR'
    default:
      return 'UNK'
  }
}

type BuildOptimizedDescriptionOptions = {
  stationKind: 'master' | 'slave'
  getStationName: (message: Message) => string
  getFrameSummary: (hexData: string) => { tag?: string | null; label?: string | null } | null
}

const normalizePeerLabel = (stationName: string) => {
  const normalized = String(stationName || '').trim()
  if (!normalized || normalized === 'Unknown' || normalized === t('messagePanel.unknownStation')) {
    return t('frame.description.unknownPeer')
  }
  return `[${normalized}]`
}

const resolveEndpoints = (
  stationKind: 'master' | 'slave',
  messageType: Message['type'],
  stationName: string,
) => {
  const peerLabel = normalizePeerLabel(stationName)

  if (stationKind === 'master') {
    if (messageType === 'sent') {
      return { sender: t('frame.description.master'), receiver: peerLabel, peerLabel }
    }
    if (messageType === 'received') {
      return { sender: peerLabel, receiver: t('frame.description.master'), peerLabel }
    }
    return { sender: t('frame.description.master'), receiver: peerLabel, peerLabel }
  }

  if (messageType === 'sent') {
    return { sender: peerLabel, receiver: t('frame.description.master'), peerLabel }
  }
  if (messageType === 'received') {
    return { sender: t('frame.description.master'), receiver: peerLabel, peerLabel }
  }
  return { sender: t('frame.description.master'), receiver: peerLabel, peerLabel }
}

const splitSummaryLabel = (label: string) => {
  const parts = label
    .split(/\s+/)
    .map((part) => part.trim())
    .filter(Boolean)
  const rest = parts.slice(1).join(' ')
  const cotCandidates = [
    t('frame.cotDescriptions.activationConfirm'),
    t('frame.cotDescriptions.deactivationConfirm'),
    t('frame.cotDescriptions.activationTermination'),
    t('frame.cotDescriptions.groupInterrogation'),
    t('frame.cotDescriptions.counterInterrogation'),
    t('frame.cotDescriptions.unknownCommonAddress'),
    t('frame.cotDescriptions.unknownObjectAddress'),
    t('frame.cotDescriptions.unknownTypeId'),
    t('frame.cotDescriptions.unknownCot'),
    t('frame.cotDescriptions.interrogation'),
    t('frame.cotDescriptions.spontaneous'),
    t('frame.cotDescriptions.initialized'),
    t('frame.cotDescriptions.fileTransfer'),
    t('frame.cotDescriptions.deactivation'),
    t('frame.cotDescriptions.activation'),
    t('frame.cotDescriptions.request'),
  ].sort((left, right) => right.length - left.length)
  const cotLabel = cotCandidates.find(
    (candidate) => rest === candidate || rest.startsWith(`${candidate} `),
  )

  return {
    typeLabel: parts[0] || '',
    cotLabel: cotLabel || parts[1] || '',
    hint: cotLabel ? rest.slice(cotLabel.length).trim() : parts.slice(2).join(' '),
  }
}

const splitCotLabel = (cotLabel: string) => {
  const normalized = String(cotLabel || '').trim()
  if (!normalized) {
    return { baseCot: '', flags: [] as string[] }
  }

  const escapeRegex = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const negative = escapeRegex(t('frame.negative'))
  const test = escapeRegex(t('frame.test'))
  const flagMatch = normalized.match(
    new RegExp(`\\((?:${negative}|${test})(?:/(?:${negative}|${test}))*\\)$`),
  )
  if (!flagMatch) {
    return { baseCot: normalized, flags: [] as string[] }
  }

  return {
    baseCot: normalized.slice(0, -flagMatch[0].length),
    flags: flagMatch[0].slice(1, -1).split('/').filter(Boolean),
  }
}

const describeCotFlag = (flag: string) => {
  if (flag === t('frame.negative')) return t('frame.description.negativeAck')
  if (flag === t('frame.test')) return t('frame.description.testFrame')
  return flag
}

const humanizeHintSegments = (hint: string, ioa: number | null) => {
  const details: string[] = []
  let hasIoa = false
  const escapeRegex = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const select = escapeRegex(t('frame.select'))
  const execute = escapeRegex(t('frame.execute'))
  const close = escapeRegex(t('frame.close'))
  const open = escapeRegex(t('frame.open'))
  const raise = escapeRegex(t('frame.raise'))
  const lower = escapeRegex(t('frame.lower'))
  const status = escapeRegex(t('frame.status'))

  for (const segment of String(hint || '')
    .split(/\s+/)
    .filter(Boolean)) {
    const ioaMatch = segment.match(/^IOA=(\d+)$/i)
    if (ioaMatch) {
      hasIoa = true
      details.push(`IOA=${ioaMatch[1]}`)
      continue
    }

    const commandMatch = segment.match(
      new RegExp(`^(${select}|${execute})\\/(${close}|${open}|${raise}|${lower})$`),
    )
    if (commandMatch) {
      const stage = commandMatch[1] === t('frame.select') ? t('frame.select') : t('frame.execute')
      details.push(t('frame.description.stage', { value: stage }))
      const suffix = commandMatch[2]
      if (suffix === t('frame.close')) details.push(t('frame.description.actionClose'))
      else if (suffix === t('frame.open')) details.push(t('frame.description.actionOpen'))
      else details.push(t('frame.description.direction', { value: suffix }))
      continue
    }

    const stateMatch = segment.match(new RegExp(`^(${select}|${execute})\\/${status}=(\\d+)$`))
    if (stateMatch) {
      const stage = stateMatch[1] === t('frame.select') ? t('frame.select') : t('frame.execute')
      details.push(t('frame.description.stage', { value: stage }))
      details.push(t('frame.description.status', { value: stateMatch[2] }))
      continue
    }

    const qlMatch = segment.match(/^QL=(\d+)$/i)
    if (qlMatch) {
      details.push(t('frame.description.qualifier', { value: qlMatch[1] }))
      continue
    }

    const qoiMatch = segment.match(/^QOI=(\d+)$/i)
    if (qoiMatch) {
      details.push(t('frame.description.interrogationRange', { value: qoiMatch[1] }))
      continue
    }

    const qccMatch = segment.match(/^QCC=(\d+)$/i)
    if (qccMatch) {
      details.push(t('frame.description.counterInterrogation', { value: qccMatch[1] }))
      continue
    }

    const qrpMatch = segment.match(/^QRP=(\d+)$/i)
    if (qrpMatch) {
      details.push(t('frame.description.resetReason', { value: qrpMatch[1] }))
      continue
    }

    if (segment === t('frame.writeBitstring')) {
      details.push(t('frame.description.writeBitstring'))
      continue
    }
    if (segment === t('frame.readCommand')) {
      details.push(t('frame.description.read'))
      continue
    }
    if (segment === t('frame.testCommand')) {
      details.push(t('frame.description.test'))
      continue
    }

    details.push(segment)
  }

  if (!hasIoa && ioa != null) {
    details.unshift(`IOA=${ioa}`)
  }

  return details
}

const buildIFrameSentence = (sender: string, receiver: string, label: string, message: Message) => {
  const { typeLabel, cotLabel, hint } = splitSummaryLabel(label)
  const meta = extractMessageFrameMeta(message)
  const { baseCot, flags } = splitCotLabel(cotLabel)
  const details = [...humanizeHintSegments(hint, meta.ioa), ...flags.map(describeCotFlag)].filter(
    Boolean,
  )
  const subject = typeLabel || t('frame.description.iData')
  const withDetails = (sentence: string) => {
    const separator = getCurrentLocale() === 'en-US' ? ', ' : '，'
    const detailText =
      details.length > 0
        ? t('frame.description.detailJoin', { details: details.join(separator) })
        : ''
    return `${sentence}${detailText}${t('frame.description.period')}`
  }

  let sentence = ''
  switch (baseCot) {
    case t('frame.cotDescriptions.activation'):
      sentence = t('frame.description.sentActivation', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.activationConfirm'):
      sentence = t('frame.description.sentActivationConfirm', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.deactivation'):
      sentence = t('frame.description.sentDeactivation', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.deactivationConfirm'):
      sentence = t('frame.description.sentDeactivationConfirm', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.activationTermination'):
      sentence = t('frame.description.sentActivationTermination', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.spontaneous'):
      sentence = t('frame.description.sentSpontaneous', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.request'):
      sentence = t('frame.description.sentRequest', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.interrogation'):
      sentence = t('frame.description.sentInterrogation', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.groupInterrogation'):
      sentence = t('frame.description.sentGroupInterrogation', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.counterInterrogation'):
      sentence = t('frame.description.sentCounterInterrogation', { sender, receiver, subject })
      break
    case t('frame.cotDescriptions.initialized'):
      sentence = t('frame.description.sentInitialized', { sender, receiver })
      break
    case t('frame.cotDescriptions.fileTransfer'):
      sentence = t('frame.description.sentFileTransfer', { sender, receiver })
      break
    case t('frame.cotDescriptions.unknownTypeId'):
      sentence = t('frame.description.sentUnknownTypeId', { sender, receiver })
      break
    case t('frame.cotDescriptions.unknownCot'):
      sentence = t('frame.description.sentUnknownCot', { sender, receiver })
      break
    case t('frame.cotDescriptions.unknownCommonAddress'):
      sentence = t('frame.description.sentUnknownCommonAddress', { sender, receiver })
      break
    case t('frame.cotDescriptions.unknownObjectAddress'):
      sentence = t('frame.description.sentUnknownObjectAddress', { sender, receiver })
      break
    default:
      sentence = t('frame.description.sentDefault', {
        sender,
        receiver,
        subject,
        cot: baseCot ? t('frame.description.cotParen', { cot: baseCot }) : '',
      })
      break
  }

  return withDetails(sentence)
}

const buildSFrameSentence = (sender: string, receiver: string, label: string) => {
  const nrMatch = String(label || '').match(/N\(R\)=(\d+)/)
  const detail = nrMatch ? t('frame.description.sFrameDetail', { nr: nrMatch[1] }) : ''
  return t('frame.description.sFrame', { sender, receiver, detail })
}

const buildUFrameSentence = (sender: string, receiver: string, label: string) => {
  switch (label) {
    case t('frame.uFrameTypes.start'):
      return t('frame.description.uFrameStart', { sender, receiver })
    case t('frame.uFrameTypes.startConfirm'):
      return t('frame.description.uFrameStartConfirm', { sender, receiver })
    case t('frame.uFrameTypes.stop'):
      return t('frame.description.uFrameStop', { sender, receiver })
    case t('frame.uFrameTypes.stopConfirm'):
      return t('frame.description.uFrameStopConfirm', { sender, receiver })
    case t('frame.uFrameTypes.test'):
      return t('frame.description.uFrameTest', { sender, receiver })
    case t('frame.uFrameTypes.testConfirm'):
      return t('frame.description.uFrameTestConfirm', { sender, receiver })
    default:
      return t('frame.description.uFrameDefault', {
        sender,
        receiver,
        label: label ? t('frame.description.labelParen', { label }) : '',
      })
  }
}

export const buildOptimizedDescriptionFormatter = (options: BuildOptimizedDescriptionOptions) => {
  return (message: Message) => {
    const stationName = options.getStationName(message)
    const { sender, receiver, peerLabel } = resolveEndpoints(
      options.stationKind,
      message.type,
      stationName,
    )

    if (message.type === 'error') {
      const detail = String(message.content || '').trim()
      return t('frame.description.communicationError', {
        peer: peerLabel,
        detail: detail
          ? t('frame.description.detailSuffix', { detail })
          : t('frame.description.period'),
      })
    }

    if (message.hexData) {
      const summary = options.getFrameSummary(message.hexData)
      const tag = summary?.tag || ''
      const label = summary?.label || ''

      if (tag === '[I]') {
        return buildIFrameSentence(sender, receiver, label, message)
      }
      if (tag === '[S]') {
        return buildSFrameSentence(sender, receiver, label)
      }
      if (tag === '[U]') {
        return buildUFrameSentence(sender, receiver, label)
      }
      if (tag) {
        const frameTag = tag.replace('[', '').replace(']', '')
        return t('frame.description.frameDefault', {
          sender,
          receiver,
          frameTag,
          label: label ? t('frame.description.labelParen', { label }) : '',
        })
      }
    }

    const fallbackContent = String(message.content || '').trim()
    if (fallbackContent) {
      return t('frame.description.fallbackMessage', { sender, receiver, content: fallbackContent })
    }
    return t('frame.description.unparsedMessage', { sender, receiver })
  }
}
