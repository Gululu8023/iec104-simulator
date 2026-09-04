import { t } from '@shared/i18n'

export type NormalizedTransportState =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'listening'
  | 'error'
export type NormalizedDataTransferState =
  | 'stopped'
  | 'starting'
  | 'started'
  | 'stopping'
  | 'timeout'
  | 'error'

export type ConnectionRole = 'client' | 'server'

export type UnifiedConnectionSeverity =
  | 'status-idle'
  | 'status-listening'
  | 'status-processing'
  | 'status-unready'
  | 'status-normal'
  | 'status-error'

export interface UnifiedConnectionStatus {
  text: string
  severity: UnifiedConnectionSeverity
  tooltip: string[]
  transport: NormalizedTransportState
  link: NormalizedDataTransferState
}

const normalize = (value: unknown): string =>
  String(value ?? '')
    .trim()
    .toLowerCase()

export const normalizeTransportState = (state: unknown): NormalizedTransportState => {
  const normalized = normalize(state)
  if (normalized === 'connected') return 'connected'
  if (normalized === 'connecting') return 'connecting'
  if (normalized === 'listening') return 'listening'
  if (normalized === 'error') return 'error'
  return 'disconnected'
}

export const normalizeDataTransferState = (state: unknown): NormalizedDataTransferState => {
  const normalized = normalize(state)
  if (normalized === 'started' || normalized === 'running') return 'started'
  if (normalized === 'starting') return 'starting'
  if (normalized === 'stopping') return 'stopping'
  if (normalized === 'timeout') return 'timeout'
  if (normalized === 'error') return 'error'
  return 'stopped'
}

export const isTransportConnectedState = (state: unknown): boolean =>
  normalizeTransportState(state) === 'connected'

export const isBusinessLinkReadyState = (state: unknown): boolean =>
  normalizeDataTransferState(state) === 'started'

export const isConnectionTransitioning = (
  transportState: unknown,
  dataTransferState: unknown,
): boolean => {
  const transport = normalizeTransportState(transportState)
  const link = normalizeDataTransferState(dataTransferState)
  return transport === 'connecting' || link === 'starting' || link === 'stopping'
}

// ---------------------------------------------------------------------------
// TCP 服务端（从站）tooltip 文案
// ---------------------------------------------------------------------------

const buildServerTooltip = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
): string[] => [
  `TCP：${
    transport === 'connecting'
      ? '启动中'
      : transport === 'connected'
        ? '已接入'
        : transport === 'listening'
          ? '监听中'
          : transport === 'error'
            ? '异常'
            : '未启动'
  }`,
  `IEC104：${
    link === 'started'
      ? '运行中'
      : link === 'starting'
        ? '建链中'
        : link === 'stopping'
          ? '停链中'
          : link === 'timeout'
            ? '超时'
            : link === 'error'
              ? '错误'
              : transport === 'connected'
                ? '待建链'
                : '未启动'
  }`,
]

// ---------------------------------------------------------------------------
// TCP 客户端（主站）tooltip 文案
// ---------------------------------------------------------------------------

const buildClientTooltip = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
): string[] => [
  `TCP：${
    transport === 'connected'
      ? '已连接'
      : transport === 'connecting'
        ? '连接中'
        : transport === 'error'
          ? '异常'
          : '未连接'
  }`,
  `IEC104：${
    link === 'started'
      ? '运行中'
      : link === 'starting'
        ? '建链中'
        : link === 'stopping'
          ? '停链中'
          : link === 'timeout'
            ? '超时'
            : link === 'error'
              ? '错误'
              : transport === 'connected'
                ? '未就绪'
                : '未启动'
  }`,
]

// ---------------------------------------------------------------------------
// 服务端状态推导
// ---------------------------------------------------------------------------

const deriveServerStatus = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
  reasonText: string,
): UnifiedConnectionStatus => {
  const tooltipBase = buildServerTooltip(transport, link)

  if (link === 'timeout') {
    return {
      text: '已超时',
      severity: 'status-error',
      tooltip: reasonText ? [...tooltipBase, `原因：${reasonText}`] : tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'error' || link === 'error') {
    return {
      text: '异常态',
      severity: 'status-error',
      tooltip: reasonText ? [...tooltipBase, `原因：${reasonText}`] : tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connecting') {
    return {
      text: '启动中',
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'stopping') {
    return {
      text: '停链中',
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'starting') {
    return {
      text: '建链中',
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'started') {
    return {
      text: '运行中',
      severity: 'status-normal',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected') {
    return {
      text: '待建链',
      severity: 'status-unready',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'listening') {
    return {
      text: '监听中',
      severity: 'status-listening',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  return {
    text: '未启动',
    severity: 'status-idle',
    tooltip: tooltipBase,
    transport,
    link,
  }
}

// ---------------------------------------------------------------------------
// 客户端状态推导（保持原有逻辑）
// ---------------------------------------------------------------------------

const deriveClientStatus = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
  reasonText: string,
): UnifiedConnectionStatus => {
  const tooltipBase = buildClientTooltip(transport, link)

  if (link === 'timeout') {
    return {
      text: '已超时',
      severity: 'status-error',
      tooltip: reasonText ? [...tooltipBase, `原因：${reasonText}`] : tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'error' || link === 'error') {
    return {
      text: '异常态',
      severity: 'status-error',
      tooltip: reasonText ? [...tooltipBase, `原因：${reasonText}`] : tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connecting') {
    return {
      text: '连接中',
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'stopping') {
    return {
      text: '停链中',
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'starting') {
    return {
      text: '建链中',
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'started') {
    return {
      text: '运行中',
      severity: 'status-normal',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected') {
    return {
      text: '未就绪',
      severity: 'status-unready',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  return {
    text: '未连接',
    severity: 'status-idle',
    tooltip: tooltipBase,
    transport,
    link,
  }
}

// ---------------------------------------------------------------------------
// 统一入口
// ---------------------------------------------------------------------------

export const deriveUnifiedConnStatus = (
  transportState: unknown,
  dataTransferState: unknown,
  reason?: string | null,
  role?: ConnectionRole,
): UnifiedConnectionStatus => {
  const transport = normalizeTransportState(transportState)
  const link = normalizeDataTransferState(dataTransferState)
  const reasonText = String(reason ?? '').trim()

  if (role === 'server') {
    return deriveServerStatus(transport, link, reasonText)
  }
  return deriveClientStatus(transport, link, reasonText)
}

const resolveLocalizedTransportText = (
  transport: NormalizedTransportState,
  role: ConnectionRole,
): string => {
  if (role === 'server') {
    return t(`iec104.connection.server.transport.${transport}`)
  }
  return t(`iec104.connection.client.transport.${transport}`)
}

const resolveLocalizedLinkText = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
  role: ConnectionRole,
): string => {
  if (role === 'server' && link === 'stopped' && transport === 'connected') {
    return t('iec104.connection.server.link.waiting')
  }
  if (role === 'client' && link === 'stopped' && transport === 'connected') {
    return t('iec104.connection.client.link.unready')
  }
  return t(`iec104.connection.${role}.link.${link}`)
}

const buildLocalizedTooltip = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
  role: ConnectionRole,
): string[] => [
  t('iec104.connection.tooltip.tcp', {
    state: resolveLocalizedTransportText(transport, role),
  }),
  t('iec104.connection.tooltip.iec104', {
    state: resolveLocalizedLinkText(transport, link, role),
  }),
]

const withReasonTooltip = (tooltip: string[], reasonText: string): string[] =>
  reasonText ? [...tooltip, t('iec104.connection.tooltip.reason', { reason: reasonText })] : tooltip

const deriveServerStatusLocalized = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
  reasonText: string,
): UnifiedConnectionStatus => {
  const tooltipBase = buildLocalizedTooltip(transport, link, 'server')

  if (link === 'timeout') {
    return {
      text: t('iec104.connection.status.timeout'),
      severity: 'status-error',
      tooltip: withReasonTooltip(tooltipBase, reasonText),
      transport,
      link,
    }
  }

  if (transport === 'error' || link === 'error') {
    return {
      text: t('iec104.connection.status.error'),
      severity: 'status-error',
      tooltip: withReasonTooltip(tooltipBase, reasonText),
      transport,
      link,
    }
  }

  if (transport === 'connecting') {
    return {
      text: t('iec104.connection.status.starting'),
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'stopping') {
    return {
      text: t('iec104.connection.status.stoppingLink'),
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'starting') {
    return {
      text: t('iec104.connection.status.startingLink'),
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'started') {
    return {
      text: t('iec104.connection.status.running'),
      severity: 'status-normal',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected') {
    return {
      text: t('iec104.connection.status.waitingLink'),
      severity: 'status-unready',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'listening') {
    return {
      text: t('iec104.connection.status.listening'),
      severity: 'status-listening',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  return {
    text: t('iec104.connection.status.stopped'),
    severity: 'status-idle',
    tooltip: tooltipBase,
    transport,
    link,
  }
}

const deriveClientStatusLocalized = (
  transport: NormalizedTransportState,
  link: NormalizedDataTransferState,
  reasonText: string,
): UnifiedConnectionStatus => {
  const tooltipBase = buildLocalizedTooltip(transport, link, 'client')

  if (link === 'timeout') {
    return {
      text: t('iec104.connection.status.timeout'),
      severity: 'status-error',
      tooltip: withReasonTooltip(tooltipBase, reasonText),
      transport,
      link,
    }
  }

  if (transport === 'error' || link === 'error') {
    return {
      text: t('iec104.connection.status.error'),
      severity: 'status-error',
      tooltip: withReasonTooltip(tooltipBase, reasonText),
      transport,
      link,
    }
  }

  if (transport === 'connecting') {
    return {
      text: t('iec104.connection.status.connecting'),
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'stopping') {
    return {
      text: t('iec104.connection.status.stoppingLink'),
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'starting') {
    return {
      text: t('iec104.connection.status.startingLink'),
      severity: 'status-processing',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected' && link === 'started') {
    return {
      text: t('iec104.connection.status.running'),
      severity: 'status-normal',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  if (transport === 'connected') {
    return {
      text: t('iec104.connection.status.unready'),
      severity: 'status-unready',
      tooltip: tooltipBase,
      transport,
      link,
    }
  }

  return {
    text: t('iec104.connection.status.disconnected'),
    severity: 'status-idle',
    tooltip: tooltipBase,
    transport,
    link,
  }
}

export const deriveUnifiedConnStatusLocalized = (
  transportState: unknown,
  dataTransferState: unknown,
  reason?: string | null,
  role: ConnectionRole = 'client',
): UnifiedConnectionStatus => {
  const transport = normalizeTransportState(transportState)
  const link = normalizeDataTransferState(dataTransferState)
  const reasonText = String(reason ?? '').trim()

  if (role === 'server') {
    return deriveServerStatusLocalized(transport, link, reasonText)
  }
  return deriveClientStatusLocalized(transport, link, reasonText)
}
