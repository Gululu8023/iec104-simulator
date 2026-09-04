import { ElMessage } from 'element-plus'
import { nextTick, watch, type Ref } from 'vue'
import { t } from '@shared/i18n'
import type {
  BackendConnectionInfo,
  BackendSoeEvent,
  BackendSlaveMessage,
  BackendSlaveRuntimeHintEvent,
  StationSummary,
} from '@shared/api/types'
import type { PointSyncBatch, PointSyncCursor } from '@shared/api/pointSync'
import { hasReachedPointSyncCursor } from '@shared/api/pointSync'
import { useRuntimeSyncLoop } from '@shared/composables/useRuntimeSyncLoop'
import { deriveUnifiedConnStatus } from '@shared/ui/connectionStatus'
import type { DeviceNode } from '../types/slave'
import type { SlavePointData } from './useSlavePointData'
import type { BackendDataPoint } from '@shared/api/types'

type SlaveStatsState = {
  singlePointCount: number
  measuredCount: number
  pendingQueue: number
  txUnconfirmed: number | null
  rxPending: number | null
  connectingCount: number
  errorCount: number
}

type UseSlaveSyncOptions = {
  currentStationId: Ref<string>
  isMessagePanelHidden: Ref<boolean>
  isSoePanelVisible: Ref<boolean>
  stations: Ref<StationSummary[]>
  selectedSlaveId: Ref<number | null>
  setStationConnections: (stationId: string, connections: BackendConnectionInfo[]) => void
  stationRuntimeStatusMap: Ref<Record<string, DeviceNode['status']>>
  slaveStats: SlaveStatsState
  statusBarSyncRevision: Ref<number>
  pointData: SlavePointData
  backendMessages: Ref<BackendSlaveMessage[]>
  stationTimeOffsetMs: Ref<number>
  lastMessageId: Ref<number>
  soeEventsByStation: Ref<Record<string, BackendSoeEvent[]>>
  lastSoeIdByStation: Ref<Record<string, number>>
  messagePanelRef: Ref<any>
  getSlaveConnections: (stationId: string) => Promise<BackendConnectionInfo[]>
  getSlaveStationTimeOffset: (stationId: string) => Promise<number>
  syncSlaveDataPoints: (
    stationId: string,
    cursor: PointSyncCursor | null,
  ) => Promise<PointSyncBatch<BackendDataPoint>>
  getSlaveMessages: (
    stationId: string,
    sinceId: number | null,
    limit: number,
  ) => Promise<BackendSlaveMessage[]>
  setSlaveMessageTracking: (stationId: string, enabled: boolean) => Promise<void>
  getSlaveSoeEvents: (
    stationId: string,
    sinceId: number | null,
    limit: number,
  ) => Promise<BackendSoeEvent[]>
  resolveMessageUiKeys: (
    runtimeConnectionId: string,
    commonAddress: number | null,
  ) => {
    uiConnectionKey: string
    slaveUiKey: string
  }
  syncUiForCurrentStation: () => void
}

type StationConnectionRuntimeSummary = {
  status: DeviceNode['status']
  connectedCount: number
  connectingCount: number
  errorCount: number
}

type SlaveSyncTask = 'connections' | 'points' | 'messages' | 'soe' | 'clock'

const MESSAGE_SYNC_PAGE_SIZE = 200
const MAX_MESSAGE_SYNC_BATCHES = 25
const MAX_FRONTEND_MESSAGE_RECORDS = 1000
const SOE_SYNC_PAGE_SIZE = 200
const SIMULATION_VISUAL_SYNC_INTERVAL_MS = 100
const MAX_SOE_SYNC_BATCHES = 25
const FULL_BACKEND_SYNC_TASKS: SlaveSyncTask[] = [
  'connections',
  'points',
  'messages',
  'soe',
  'clock',
]
const LIGHT_POLL_SYNC_TASKS: SlaveSyncTask[] = ['connections', 'messages', 'soe', 'clock']

export function useSlaveSync(options: UseSlaveSyncOptions) {
  const {
    currentStationId,
    isMessagePanelHidden,
    isSoePanelVisible,
    stations,
    selectedSlaveId,
    setStationConnections,
    stationRuntimeStatusMap,
    slaveStats,
    statusBarSyncRevision,
    pointData,
    backendMessages,
    stationTimeOffsetMs,
    lastMessageId,
    soeEventsByStation,
    lastSoeIdByStation,
    messagePanelRef,
    getSlaveConnections,
    getSlaveStationTimeOffset,
    syncSlaveDataPoints,
    getSlaveMessages,
    setSlaveMessageTracking,
    getSlaveSoeEvents,
    resolveMessageUiKeys,
    syncUiForCurrentStation,
  } = options

  let syncErrorStreak = 0
  let lastSyncErrorToastAt = 0
  let trackedStationId = ''
  let pendingTrackingSessionRestartStationId = ''
  let simulationPointSyncTimer: ReturnType<typeof setTimeout> | null = null
  let lastSimulationPointSyncAt = 0
  const runtimeHintToastAt = new Map<string, number>()

  const appendSlaveSoeEvents = (stationId: string, events: BackendSoeEvent[]) => {
    if (!stationId || events.length === 0) return
    const existing = soeEventsByStation.value[stationId] ?? []
    const knownIds = new Set(existing.map((event) => event.id))
    const uniqueEvents = events.filter((event) => {
      if (knownIds.has(event.id)) return false
      knownIds.add(event.id)
      return true
    })
    if (uniqueEvents.length === 0) return
    soeEventsByStation.value = {
      ...soeEventsByStation.value,
      [stationId]: [...existing, ...uniqueEvents].slice(-10000),
    }
    lastSoeIdByStation.value = {
      ...lastSoeIdByStation.value,
      [stationId]: uniqueEvents[uniqueEvents.length - 1].id,
    }
  }

  const syncSelectedNodeAfterRuntimeRefresh = async () => {
    await nextTick()
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => resolve())
    })
    syncUiForCurrentStation()
  }

  const loadSlaveSoeIncrementally = async (
    stationId: string,
    isCurrent: () => boolean,
  ): Promise<BackendSoeEvent[]> => {
    const merged: BackendSoeEvent[] = []
    let cursor = lastSoeIdByStation.value[stationId] ?? null
    try {
      for (let batchIndex = 0; batchIndex < MAX_SOE_SYNC_BATCHES; batchIndex += 1) {
        const batch = await getSlaveSoeEvents(stationId, cursor, SOE_SYNC_PAGE_SIZE)
        if (!isCurrent() || currentStationId.value !== stationId) return []
        if (batch.length === 0) break
        merged.push(...batch)
        const lastId = batch[batch.length - 1]?.id
        if (!Number.isFinite(lastId)) break
        cursor = lastId
        if (batch.length < SOE_SYNC_PAGE_SIZE) break
      }
    } catch (error) {
      console.error('loadSlaveSoeIncrementally failed:', error)
      return []
    }
    return merged
  }

  const loadSlaveMessagesIncrementally = async (
    stationId: string,
    isCurrent: () => boolean,
  ): Promise<BackendSlaveMessage[]> => {
    const merged: BackendSlaveMessage[] = []
    let cursor = lastMessageId.value > 0 ? lastMessageId.value : null

    try {
      for (let batchIndex = 0; batchIndex < MAX_MESSAGE_SYNC_BATCHES; batchIndex += 1) {
        const batch = await getSlaveMessages(stationId, cursor, MESSAGE_SYNC_PAGE_SIZE)
        if (!isCurrent() || currentStationId.value !== stationId) return []
        if (batch.length === 0) break

        merged.push(...batch)

        const lastId = batch[batch.length - 1]?.id
        if (typeof lastId !== 'number' || !Number.isFinite(lastId)) break

        cursor = lastId
        if (batch.length < MESSAGE_SYNC_PAGE_SIZE) break
      }
    } catch (error) {
      console.error('loadSlaveMessagesIncrementally failed:', error)
      return []
    }

    return merged
  }

  const shouldHandleSlaveRuntimeHint = (payload: BackendSlaveRuntimeHintEvent) => {
    const stationId = String(payload.station_id || '').trim()
    if (!stationId) return false

    switch (payload.reason) {
      case 'station-started':
      case 'station-stopped':
      case 'simulation-step':
      case 'connection-changed':
      case 'points-changed':
      case 'soe-updated':
      case 'soe-cleared':
      case 'soe-overflow':
      case 'startdt-timeout':
        return (
          stationId === currentStationId.value ||
          stations.value.some((station) => station.station_id === stationId)
        )
      case 'rx-data':
      case 'tx-data':
        return (
          !isMessagePanelHidden.value &&
          (stationId === currentStationId.value ||
            stations.value.some((station) => station.station_id === stationId))
        )
      default:
        return false
    }
  }

  const notifySlaveRuntimeHint = (payload: BackendSlaveRuntimeHintEvent) => {
    scheduleTrackingSessionRestart(payload)

    const now = Date.now()
    const key = `${payload.reason}:${payload.connection_id ?? ''}`
    const lastAt = runtimeHintToastAt.get(key) ?? 0
    if (now - lastAt < 3000) return

    if (payload.reason === 'soe-overflow') {
      runtimeHintToastAt.set(key, now)
      ElMessage.warning(t('slave.sync.soeOverflow'))
      return
    }

    if (payload.reason === 'startdt-timeout') {
      runtimeHintToastAt.set(key, now)
      ElMessage.error(t('slave.sync.startdtTimeout'))
    }
  }

  const appendBackendMessages = (records: BackendSlaveMessage[]) => {
    if (records.length === 0) return

    const seenIds = new Set(backendMessages.value.map((item) => item.id))
    const freshRecords = records.filter((record) => {
      if (seenIds.has(record.id)) return false
      seenIds.add(record.id)
      return true
    })
    lastMessageId.value = Math.max(lastMessageId.value, records[records.length - 1].id)
    if (freshRecords.length === 0) return

    backendMessages.value = [...backendMessages.value, ...freshRecords].slice(
      -MAX_FRONTEND_MESSAGE_RECORDS,
    )

    if (!messagePanelRef.value) return
    const panelMessages = freshRecords.map((record) => {
      const runtimeConnectionId = String(record.connection_id ?? '').trim()
      const commonAddress =
        typeof record.common_address === 'number' && Number.isFinite(record.common_address)
          ? record.common_address
          : null
      const { uiConnectionKey, slaveUiKey } = resolveMessageUiKeys(
        runtimeConnectionId,
        commonAddress,
      )
      return {
        id: String(record.id),
        type: record.direction,
        content: record.content,
        hexData: record.hex_data ?? '',
        runtimeConnectionId,
        uiConnectionKey,
        slaveUiKey,
        timestamp: new Date(record.timestamp).getTime(),
        parsed: {
          typeId: record.type_id ?? undefined,
          cot: record.cause ?? undefined,
          commonAddress: record.common_address ?? undefined,
        },
      }
    })
    messagePanelRef.value.addMessages(panelMessages)
  }

  const pruneDisconnectedConnectionMessages = (connections: BackendConnectionInfo[]) => {
    const activeConnectionIds = new Set(
      connections
        .map((connection) => String(connection.id ?? '').trim())
        .filter((connectionId) => connectionId.length > 0),
    )
    const staleConnectionIds = Array.from(
      new Set(
        backendMessages.value
          .map((message) => String(message.connection_id ?? '').trim())
          .filter(
            (connectionId) => connectionId.length > 0 && !activeConnectionIds.has(connectionId),
          ),
      ),
    )

    if (staleConnectionIds.length === 0) return

    const staleSet = new Set(staleConnectionIds)
    backendMessages.value = backendMessages.value.filter((message) => {
      const connectionId = String(message.connection_id ?? '').trim()
      return !connectionId || !staleSet.has(connectionId)
    })
    messagePanelRef.value?.removeMessagesByConnectionIds?.(staleConnectionIds)
  }

  const clearMessageState = () => {
    backendMessages.value = []
    lastMessageId.value = 0
    messagePanelRef.value?.clearMessages?.()
  }

  const syncMessageTrackingState = async (stationIdOverride?: string) => {
    const targetStationId = String(stationIdOverride ?? currentStationId.value ?? '').trim()
    const shouldTrack = targetStationId.length > 0 && !isMessagePanelHidden.value

    if (!shouldTrack) {
      if (trackedStationId) {
        try {
          await setSlaveMessageTracking(trackedStationId, false)
        } catch (error) {
          console.error('disable slave message tracking failed:', error)
        }
        trackedStationId = ''
      }
      clearMessageState()
      return
    }

    if (trackedStationId === targetStationId) {
      return
    }

    if (trackedStationId) {
      try {
        await setSlaveMessageTracking(trackedStationId, false)
      } catch (error) {
        console.error('disable previous slave message tracking failed:', error)
      }
      trackedStationId = ''
    }

    clearMessageState()

    try {
      await setSlaveMessageTracking(targetStationId, true)
      if (currentStationId.value !== targetStationId || isMessagePanelHidden.value) {
        await setSlaveMessageTracking(targetStationId, false)
        return
      }
      trackedStationId = targetStationId
    } catch (error) {
      console.error('enable slave message tracking failed:', error)
      ElMessage.error(t('slave.sync.enableTrackingFailed', { error: String(error) }))
    }
  }

  const restartMessageTrackingSession = async (stationIdOverride?: string) => {
    const targetStationId = String(stationIdOverride ?? currentStationId.value ?? '').trim()
    if (!targetStationId || isMessagePanelHidden.value) {
      trackedStationId = ''
      clearMessageState()
      return
    }

    clearMessageState()

    try {
      await setSlaveMessageTracking(targetStationId, true)
      if (currentStationId.value !== targetStationId || isMessagePanelHidden.value) {
        await setSlaveMessageTracking(targetStationId, false)
        return
      }
      trackedStationId = targetStationId
    } catch (error) {
      console.error('restart slave message tracking session failed:', error)
      ElMessage.error(t('slave.sync.restartTrackingFailed', { error: String(error) }))
    }
  }

  const scheduleTrackingSessionRestart = (payload: BackendSlaveRuntimeHintEvent) => {
    const stationId = String(payload.station_id || '').trim()
    if (
      !stationId ||
      isMessagePanelHidden.value ||
      stationId !== currentStationId.value ||
      (payload.reason !== 'station-started' && payload.reason !== 'station-stopped')
    ) {
      return
    }

    pendingTrackingSessionRestartStationId = stationId
  }

  const normalizeOptionalCounter = (value: unknown): number | null => {
    const numeric = Number(value)
    if (!Number.isFinite(numeric)) return null
    return Math.max(0, Math.trunc(numeric))
  }

  const summarizePendingAckMetrics = (
    connections: BackendConnectionInfo[],
  ): { txUnconfirmed: number | null; rxPending: number | null } => {
    let txUnconfirmed = 0
    let rxPending = 0
    let txSeen = false
    let rxSeen = false

    for (const connection of connections) {
      const raw = connection as BackendConnectionInfo & Record<string, unknown>
      const txValue =
        normalizeOptionalCounter(raw.tx_unconfirmed) ??
        normalizeOptionalCounter(raw.tx_pending_i) ??
        normalizeOptionalCounter(raw.unconfirmed_tx)
      if (txValue != null) {
        txUnconfirmed += txValue
        txSeen = true
      }

      const rxValue =
        normalizeOptionalCounter(raw.rx_pending_ack) ??
        normalizeOptionalCounter(raw.rx_need_s_ack) ??
        normalizeOptionalCounter(raw.unconfirmed_rx)
      if (rxValue != null) {
        rxPending += rxValue
        rxSeen = true
      }
    }

    return {
      txUnconfirmed: txSeen ? txUnconfirmed : null,
      rxPending: rxSeen ? rxPending : null,
    }
  }

  const summarizeConnectionRuntime = (
    connections: BackendConnectionInfo[],
  ): StationConnectionRuntimeSummary => {
    let connectedCount = 0
    let processingCount = 0
    let unreadyCount = 0
    let errorCount = 0

    for (const connection of connections) {
      const status = deriveUnifiedConnStatus(
        connection.transport_state,
        connection.data_transfer_state,
      )
      if (status.severity === 'status-error') {
        errorCount += 1
        continue
      }
      if (status.severity === 'status-normal') {
        connectedCount += 1
        continue
      }
      if (status.severity === 'status-processing') {
        processingCount += 1
        continue
      }
      if (status.severity === 'status-unready') {
        unreadyCount += 1
      }
    }

    let status: DeviceNode['status'] = 'listening'
    if (connectedCount > 0) {
      status = 'running'
    } else if (processingCount > 0) {
      status = 'connecting'
    } else if (unreadyCount > 0) {
      status = 'connected'
    } else if (errorCount > 0) {
      status = 'error'
    }

    return {
      status,
      connectedCount,
      connectingCount: processingCount + unreadyCount,
      errorCount,
    }
  }

  const mapStationSummaryStatusToNodeStatus = (
    status: string | undefined,
  ): DeviceNode['status'] => {
    switch (String(status ?? '').trim()) {
      case 'Running':
        return 'running'
      case 'Starting':
        return 'starting'
      case 'Stopping':
        return 'stopping'
      case 'Error':
        return 'error'
      case 'Stopped':
      default:
        return 'stopped'
    }
  }

  const buildStationRuntimeStatusMap = async (
    prefetchedConnectionsByStation: Record<string, BackendConnectionInfo[]>,
  ): Promise<Record<string, DeviceNode['status']>> => {
    const stationRows = stations.value ?? []
    if (stationRows.length === 0) return {}

    const entries = await Promise.all(
      stationRows.map(async (station) => {
        const stationId = station.station_id
        const fallbackStatus = mapStationSummaryStatusToNodeStatus(station.status)
        if (fallbackStatus !== 'running' && fallbackStatus !== 'listening') {
          return [stationId, fallbackStatus] as const
        }

        let connections = prefetchedConnectionsByStation[stationId]
        if (!connections) {
          try {
            connections = await getSlaveConnections(stationId)
          } catch {
            connections = []
          }
        }

        const runtime = summarizeConnectionRuntime(connections)
        return [stationId, runtime.status] as const
      }),
    )

    return Object.fromEntries(entries)
  }

  const buildBackendSyncTaskSet = (
    tasks: SlaveSyncTask[] = FULL_BACKEND_SYNC_TASKS,
  ): Set<SlaveSyncTask> => new Set(tasks.length > 0 ? tasks : FULL_BACKEND_SYNC_TASKS)

  const filterAutomaticMessageTasks = (tasks: Iterable<SlaveSyncTask>): SlaveSyncTask[] => {
    const requestedTasks = new Set(tasks)
    if (isMessagePanelHidden.value) {
      requestedTasks.delete('messages')
    }
    if (!isSoePanelVisible.value) requestedTasks.delete('soe')
    return [...requestedTasks]
  }

  const resolveRuntimeHintSyncTasks = (
    reason: BackendSlaveRuntimeHintEvent['reason'],
  ): SlaveSyncTask[] => {
    switch (reason) {
      case 'rx-data':
      case 'tx-data':
        return ['messages']
      case 'simulation-step':
      case 'points-changed':
        return ['points']
      case 'soe-updated':
      case 'soe-cleared':
      case 'soe-overflow':
        return ['points', 'soe']
      case 'connection-changed':
      case 'startdt-timeout':
        return ['connections', 'messages']
      case 'station-started':
      case 'station-stopped':
      default:
        return FULL_BACKEND_SYNC_TASKS
    }
  }

  const syncFromBackend = async (tasks: SlaveSyncTask[] = FULL_BACKEND_SYNC_TASKS) => {
    const requestedTasks = buildBackendSyncTaskSet(tasks)
    const shouldForceMessageSync = requestedTasks.size === 1 && requestedTasks.has('messages')
    const shouldForceSoeSync = requestedTasks.size === 1 && requestedTasks.has('soe')
    if (isMessagePanelHidden.value && !shouldForceMessageSync) {
      requestedTasks.delete('messages')
    }
    if (!isSoePanelVisible.value && !shouldForceSoeSync) requestedTasks.delete('soe')
    await runtimeSyncLoop.run([...requestedTasks])
  }

  const resolveThrottledRuntimeHintTasks = (
    payload: BackendSlaveRuntimeHintEvent,
  ): SlaveSyncTask[] => {
    let tasks = filterAutomaticMessageTasks(resolveRuntimeHintSyncTasks(payload.reason))
    if (
      payload.point_cursor &&
      hasReachedPointSyncCursor(pointData.cursor(), payload.point_cursor)
    ) {
      tasks = tasks.filter((task) => task !== 'points')
    }
    if (payload.reason !== 'simulation-step' && payload.reason !== 'points-changed') return tasks
    if (!tasks.includes('points')) return tasks

    const elapsed = Date.now() - lastSimulationPointSyncAt
    if (elapsed >= SIMULATION_VISUAL_SYNC_INTERVAL_MS) {
      lastSimulationPointSyncAt = Date.now()
      return tasks
    }
    if (simulationPointSyncTimer == null) {
      simulationPointSyncTimer = setTimeout(() => {
        simulationPointSyncTimer = null
        lastSimulationPointSyncAt = Date.now()
        void syncFromBackend(['points'])
      }, SIMULATION_VISUAL_SYNC_INTERVAL_MS - elapsed)
    }
    return tasks.filter((task) => task !== 'points')
  }

  const runtimeSyncLoop = useRuntimeSyncLoop<SlaveSyncTask, BackendSlaveRuntimeHintEvent>({
    eventName: 'slave-runtime-hint',
    intervalMs: 3000,
    buildTaskSet: buildBackendSyncTaskSet,
    shouldHandleEvent: shouldHandleSlaveRuntimeHint,
    resolveEventTasks: resolveThrottledRuntimeHintTasks,
    resolvePollTasks: (tick) =>
      filterAutomaticMessageTasks(tick % 4 === 0 ? FULL_BACKEND_SYNC_TASKS : LIGHT_POLL_SYNC_TASKS),
    runSync: async (task, isCurrent) => {
      if (!currentStationId.value) {
        pointData.clear()
        stationRuntimeStatusMap.value = {}
        selectedSlaveId.value = null
        stationTimeOffsetMs.value = 0
        slaveStats.txUnconfirmed = null
        slaveStats.rxPending = null
        statusBarSyncRevision.value = 0
        return
      }

      try {
        const stationId = currentStationId.value
        switch (task) {
          case 'connections': {
            const connections = await getSlaveConnections(stationId)
            if (!isCurrent() || currentStationId.value !== stationId) return
            setStationConnections(stationId, connections)
            const runtimeSummary = summarizeConnectionRuntime(connections)
            const pendingAckMetrics = summarizePendingAckMetrics(connections)
            slaveStats.connectingCount = runtimeSummary.connectingCount
            slaveStats.errorCount = runtimeSummary.errorCount
            slaveStats.txUnconfirmed = pendingAckMetrics.txUnconfirmed
            slaveStats.rxPending = pendingAckMetrics.rxPending
            const statusMap = await buildStationRuntimeStatusMap({
              [stationId]: connections,
            })
            if (!isCurrent() || currentStationId.value !== stationId) return
            stationRuntimeStatusMap.value = statusMap
            await syncSelectedNodeAfterRuntimeRefresh()
            if (!isCurrent() || currentStationId.value !== stationId) return
            pruneDisconnectedConnectionMessages(connections)
            statusBarSyncRevision.value += 1
            break
          }
          case 'points': {
            const cursor = pointData.stationId.value === stationId ? pointData.cursor() : null
            const batch = await syncSlaveDataPoints(stationId, cursor)
            if (!isCurrent() || currentStationId.value !== stationId) return
            const delta = await pointData.applySyncBatch(stationId, batch)
            if (!isCurrent() || currentStationId.value !== stationId) return
            if (!delta) return
            slaveStats.singlePointCount = pointData.singlePointCount.value
            slaveStats.measuredCount = pointData.measuredCount.value
            if (delta.reset || delta.upserts.length > 0 || delta.removed.length > 0) {
              statusBarSyncRevision.value += 1
            }
            break
          }
          case 'messages':
            if (pendingTrackingSessionRestartStationId === stationId) {
              pendingTrackingSessionRestartStationId = ''
              await restartMessageTrackingSession(stationId)
              if (!isCurrent() || currentStationId.value !== stationId) return
            }
            appendBackendMessages(await loadSlaveMessagesIncrementally(stationId, isCurrent))
            break
          case 'soe':
            appendSlaveSoeEvents(stationId, await loadSlaveSoeIncrementally(stationId, isCurrent))
            break
          case 'clock': {
            const offset = await getSlaveStationTimeOffset(stationId)
            if (!isCurrent() || currentStationId.value !== stationId) return
            stationTimeOffsetMs.value = offset
            break
          }
        }

        syncErrorStreak = 0
      } catch (error) {
        syncErrorStreak += 1
        console.error('syncFromBackend failed:', error)

        const now = Date.now()
        if (syncErrorStreak >= 3 && now - lastSyncErrorToastAt > 5000) {
          lastSyncErrorToastAt = now
          const message = error instanceof Error ? error.message : String(error)
          ElMessage.error(t('slave.sync.syncFailed', { message }))
        }
      }
    },
    onEvent: notifySlaveRuntimeHint,
    onListenError: (error) => {
      ElMessage.error(t('slave.sync.subscribeFailed', { error: String(error) }))
    },
  })

  watch(
    currentStationId,
    () => {
      runtimeSyncLoop.invalidate()
      if (simulationPointSyncTimer != null) clearTimeout(simulationPointSyncTimer)
      simulationPointSyncTimer = null
      lastSimulationPointSyncAt = 0
    },
    { flush: 'sync' },
  )

  const startRuntimeSync = async () => {
    await syncMessageTrackingState()
    await runtimeSyncLoop.start()
  }

  const stopRuntimeSync = () => {
    runtimeSyncLoop.stop()
    if (simulationPointSyncTimer != null) clearTimeout(simulationPointSyncTimer)
    simulationPointSyncTimer = null
    lastSimulationPointSyncAt = 0
    if (trackedStationId) {
      void setSlaveMessageTracking(trackedStationId, false).catch((error) => {
        console.error('disable slave message tracking during shutdown failed:', error)
      })
      trackedStationId = ''
    }
    clearMessageState()
    syncErrorStreak = 0
    lastSyncErrorToastAt = 0
    pendingTrackingSessionRestartStationId = ''
    runtimeHintToastAt.clear()
  }

  const resetStationSoeState = (stationId: string) => {
    if (!stationId) return
    soeEventsByStation.value = {
      ...soeEventsByStation.value,
      [stationId]: [],
    }
    lastSoeIdByStation.value = {
      ...lastSoeIdByStation.value,
      [stationId]: 0,
    }
  }

  const handleMessagePanelVisibilityChange = async (hidden: boolean) => {
    await syncMessageTrackingState()
    if (hidden) return
    await syncFromBackend(['messages'])
  }

  const handleSoePanelVisibilityChange = async (visible: boolean) => {
    if (visible) await syncFromBackend(['soe'])
  }

  return {
    handleMessagePanelVisibilityChange,
    handleSoePanelVisibilityChange,
    syncMessageTrackingState,
    syncFromBackend,
    startRuntimeSync,
    stopRuntimeSync,
    resetStationSoeState,
  }
}
