import { ElMessage } from 'element-plus'
import { watch, type Ref } from 'vue'
import type {
  BackendMasterMessage,
  BackendMasterRuntimeHintEvent,
  BackendSoeEvent,
  MasterFileTransferSession,
} from '@shared/api/types'
import { useRuntimeSyncLoop } from '@shared/composables/useRuntimeSyncLoop'
import { t } from '@shared/i18n'
import { hasReachedPointSyncCursor, type PointSyncCursor } from '@shared/api/pointSync'

export type MasterSyncTask = 'connections' | 'points' | 'messages' | 'soe' | 'file'

const FULL_BACKEND_SYNC_TASKS: MasterSyncTask[] = [
  'connections',
  'points',
  'messages',
  'soe',
  'file',
]
const LIGHT_POLL_SYNC_TASKS: MasterSyncTask[] = ['messages', 'soe', 'file']
const MANUAL_RUNTIME_HINT_TOAST_TTL_MS = 15_000

type UseMasterSyncOptions = {
  currentStationId: Ref<string>
  isMessagePanelHidden: Ref<boolean>
  isSoePanelVisible: Ref<boolean>
  showFileTransferDialog: Ref<boolean>
  fileTransferSession: Ref<MasterFileTransferSession | null>
  loadConnections: (isCurrent?: () => boolean) => Promise<void>
  loadDataPoints: (isCurrent?: () => boolean) => Promise<void>
  getPointCursor: () => PointSyncCursor | null
  loadMessages: (isCurrent?: () => boolean) => Promise<BackendMasterMessage[]>
  setMasterMessageTracking: (stationId: string, enabled: boolean) => Promise<void>
  loadMasterSoeIncrementally: (isCurrent?: () => boolean) => Promise<BackendSoeEvent[]>
  appendBackendMessages: (messages: BackendMasterMessage[]) => void
  appendMasterSoeEvents: (events: BackendSoeEvent[]) => void
  syncSelectedSlaveRuntimeState: () => void
  refreshFileTransferSession: () => Promise<void>
  resetMessageState: () => void
  addLog: (level: string, message: string) => void
}

export function useMasterSync(options: UseMasterSyncOptions) {
  const {
    currentStationId,
    isMessagePanelHidden,
    isSoePanelVisible,
    showFileTransferDialog,
    fileTransferSession,
    loadConnections,
    loadDataPoints,
    getPointCursor,
    loadMessages,
    setMasterMessageTracking,
    loadMasterSoeIncrementally,
    appendBackendMessages,
    appendMasterSoeEvents,
    syncSelectedSlaveRuntimeState,
    refreshFileTransferSession,
    resetMessageState,
    addLog,
  } = options

  const pendingManualRuntimeHintToasts = new Map<string, number[]>()
  let trackedStationId = ''

  const buildBackendSyncTaskSet = (
    tasks: MasterSyncTask[] = FULL_BACKEND_SYNC_TASKS,
  ): Set<MasterSyncTask> => new Set(tasks.length > 0 ? tasks : FULL_BACKEND_SYNC_TASKS)

  const buildManualRuntimeHintToastKey = (
    connectionId: string | null | undefined,
    reason: string,
  ): string => {
    return `${String(connectionId ?? '').trim()}::${String(reason ?? '').trim()}`
  }

  const pruneExpiredManualRuntimeHintToasts = (now = Date.now()) => {
    for (const [key, expirations] of pendingManualRuntimeHintToasts.entries()) {
      const activeExpirations = expirations.filter((expiresAt) => expiresAt > now)
      if (activeExpirations.length > 0) {
        pendingManualRuntimeHintToasts.set(key, activeExpirations)
      } else {
        pendingManualRuntimeHintToasts.delete(key)
      }
    }
  }

  const registerManualRuntimeHintToast = (
    connectionId: string | null | undefined,
    reason: string,
  ) => {
    const normalizedConnectionId = String(connectionId ?? '').trim()
    const normalizedReason = String(reason ?? '').trim()
    if (!normalizedConnectionId || !normalizedReason) return
    const now = Date.now()
    pruneExpiredManualRuntimeHintToasts(now)
    const key = buildManualRuntimeHintToastKey(normalizedConnectionId, normalizedReason)
    const expirations = pendingManualRuntimeHintToasts.get(key) ?? []
    expirations.push(now + MANUAL_RUNTIME_HINT_TOAST_TTL_MS)
    pendingManualRuntimeHintToasts.set(key, expirations)
  }

  const unregisterManualRuntimeHintToast = (
    connectionId: string | null | undefined,
    reason: string,
  ) => {
    const normalizedConnectionId = String(connectionId ?? '').trim()
    const normalizedReason = String(reason ?? '').trim()
    if (!normalizedConnectionId || !normalizedReason) return
    const key = buildManualRuntimeHintToastKey(normalizedConnectionId, normalizedReason)
    const expirations = pendingManualRuntimeHintToasts.get(key)
    if (!expirations || expirations.length === 0) return
    expirations.pop()
    if (expirations.length > 0) {
      pendingManualRuntimeHintToasts.set(key, expirations)
    } else {
      pendingManualRuntimeHintToasts.delete(key)
    }
  }

  const consumeManualRuntimeHintToast = (
    connectionId: string | null | undefined,
    reason: string,
  ): boolean => {
    const normalizedConnectionId = String(connectionId ?? '').trim()
    const normalizedReason = String(reason ?? '').trim()
    if (!normalizedConnectionId || !normalizedReason) return false
    const now = Date.now()
    pruneExpiredManualRuntimeHintToasts(now)
    const key = buildManualRuntimeHintToastKey(normalizedConnectionId, normalizedReason)
    const expirations = pendingManualRuntimeHintToasts.get(key)
    if (!expirations || expirations.length === 0) return false
    expirations.shift()
    if (expirations.length > 0) {
      pendingManualRuntimeHintToasts.set(key, expirations)
    } else {
      pendingManualRuntimeHintToasts.delete(key)
    }
    return true
  }

  const resolveRuntimeHintSyncTasks = (reason: string): MasterSyncTask[] => {
    switch (reason) {
      case 'points-changed':
        return ['points']
      case 'rx-data':
        return ['points', 'messages', 'soe']
      case 'soe-updated':
        return ['points', 'messages', 'soe']
      case 'file-transfer-updated':
        return ['messages', 'file']
      case 'test-frame-sent':
      case 'test-frame-confirmed':
      case 'command-confirmed':
        return ['messages']
      case 'connection-opened':
      case 'connected':
      case 'disconnected':
      case 'reconnect-attempt-failed':
      case 'reconnect-exhausted':
      case 'network-error':
      case 'frame-count-error':
      case 'connection-removed':
      case 'task-ended':
        return ['connections', 'messages', 'file']
      default:
        return FULL_BACKEND_SYNC_TASKS
    }
  }

  const shouldHandleRuntimeHint = (payload: BackendMasterRuntimeHintEvent) => {
    if (!currentStationId.value) return false
    return payload.station_id === currentStationId.value
  }

  const filterAutomaticMessageTasks = (tasks: Iterable<MasterSyncTask>): MasterSyncTask[] => {
    const set = new Set(tasks)
    if (isMessagePanelHidden.value) {
      set.delete('messages')
    }
    if (!isSoePanelVisible.value) set.delete('soe')
    return [...set]
  }

  const normalizeManualSyncTasks = (
    tasks: MasterSyncTask[] = FULL_BACKEND_SYNC_TASKS,
  ): Set<MasterSyncTask> => {
    const requestedTasks = buildBackendSyncTaskSet(tasks)
    const shouldForceMessageSync = requestedTasks.size === 1 && requestedTasks.has('messages')
    const shouldForceSoeSync = requestedTasks.size === 1 && requestedTasks.has('soe')
    if (isMessagePanelHidden.value && !shouldForceMessageSync) {
      requestedTasks.delete('messages')
    }
    if (!isSoePanelVisible.value && !shouldForceSoeSync) requestedTasks.delete('soe')
    return requestedTasks
  }

  const syncMessageTrackingState = async (stationIdOverride?: string) => {
    const targetStationId = String(stationIdOverride ?? currentStationId.value ?? '').trim()
    const shouldTrack = targetStationId.length > 0 && !isMessagePanelHidden.value

    if (!shouldTrack) {
      if (trackedStationId) {
        try {
          await setMasterMessageTracking(trackedStationId, false)
        } catch (error) {
          addLog('error', t('master.sync.logs.closeTrackingFailed', { error }))
        }
        trackedStationId = ''
      }
      resetMessageState()
      return
    }

    if (trackedStationId === targetStationId) {
      return
    }

    if (trackedStationId) {
      try {
        await setMasterMessageTracking(trackedStationId, false)
      } catch (error) {
        addLog('error', t('master.sync.logs.closeOldTrackingFailed', { error }))
      }
      trackedStationId = ''
    }

    resetMessageState()

    try {
      await setMasterMessageTracking(targetStationId, true)
      trackedStationId = targetStationId
    } catch (error) {
      addLog('error', t('master.sync.logs.openTrackingFailed', { error }))
      ElMessage.error(t('master.sync.messages.openTrackingFailed', { error }))
    }
  }

  const syncFromBackend = async (tasks: MasterSyncTask[] = FULL_BACKEND_SYNC_TASKS) => {
    const requestedTasks = normalizeManualSyncTasks(tasks)
    await runtimeSyncLoop.run([...requestedTasks])
  }

  const runtimeSyncLoop = useRuntimeSyncLoop<MasterSyncTask, BackendMasterRuntimeHintEvent>({
    eventName: 'master-runtime-hint',
    intervalMs: 2500,
    buildTaskSet: buildBackendSyncTaskSet,
    shouldHandleEvent: shouldHandleRuntimeHint,
    resolveEventTasks: (payload) => {
      const tasks = filterAutomaticMessageTasks(resolveRuntimeHintSyncTasks(payload.reason))
      if (
        payload.point_cursor &&
        hasReachedPointSyncCursor(getPointCursor(), payload.point_cursor)
      ) {
        return tasks.filter((task) => task !== 'points')
      }
      return tasks
    },
    resolvePollTasks: (tick) =>
      filterAutomaticMessageTasks(tick % 4 === 0 ? FULL_BACKEND_SYNC_TASKS : LIGHT_POLL_SYNC_TASKS),
    runSync: async (task, isCurrent) => {
      switch (task) {
        case 'connections': {
          await loadConnections(isCurrent)
          if (!isCurrent()) return
          syncSelectedSlaveRuntimeState()
          return
        }
        case 'points':
          await loadDataPoints(isCurrent)
          return
        case 'messages': {
          const messages = await loadMessages(isCurrent)
          if (isCurrent()) appendBackendMessages(messages)
          return
        }
        case 'soe': {
          const events = await loadMasterSoeIncrementally(isCurrent)
          if (isCurrent()) appendMasterSoeEvents(events)
          return
        }
        case 'file':
          if (showFileTransferDialog.value || fileTransferSession.value) {
            await refreshFileTransferSession()
          }
      }
    },
    onEvent: (payload) => {
      if (
        payload.reason === 'test-frame-confirmed' &&
        consumeManualRuntimeHintToast(payload.connection_id, payload.reason)
      ) {
        ElMessage.success(t('master.sync.messages.testFrameConfirmed'))
      } else if (
        payload.reason === 'command-confirmed' &&
        consumeManualRuntimeHintToast(payload.connection_id, payload.reason)
      ) {
        ElMessage.success(t('master.sync.messages.commandConfirmed'))
      } else if (payload.reason === 'reconnect-exhausted') {
        const attempt = Number(payload.attempt ?? 0)
        const maxAttempts = Number(payload.max_attempts ?? 0)
        if (attempt > 0 && maxAttempts > 0) {
          ElMessage.error(
            t('master.sync.messages.reconnectExhaustedWithAttempts', { attempt, maxAttempts }),
          )
        } else {
          ElMessage.error(t('master.sync.messages.reconnectExhausted'))
        }
      }
    },
    onListenError: (error) => {
      addLog('error', t('master.sync.logs.subscribeFailed', { error }))
    },
  })

  watch(currentStationId, () => runtimeSyncLoop.invalidate(), { flush: 'sync' })

  const startRuntimeSync = async () => {
    await syncMessageTrackingState()
    await runtimeSyncLoop.start()
  }

  const handleMessagePanelVisibilityChange = async (hidden: boolean) => {
    await syncMessageTrackingState()
    if (hidden) return
    await syncFromBackend(['messages'])
  }

  const handleSoePanelVisibilityChange = async (visible: boolean) => {
    if (visible) await syncFromBackend(['soe'])
  }

  const stopRuntimeSync = () => {
    runtimeSyncLoop.stop()
    if (trackedStationId) {
      void setMasterMessageTracking(trackedStationId, false).catch((error) => {
        console.error('disable master message tracking during shutdown failed:', error)
      })
      trackedStationId = ''
    }
    resetMessageState()
    pendingManualRuntimeHintToasts.clear()
  }

  return {
    handleMessagePanelVisibilityChange,
    handleSoePanelVisibilityChange,
    syncMessageTrackingState,
    syncFromBackend,
    startRuntimeSync,
    stopRuntimeSync,
    registerManualRuntimeHintToast,
    unregisterManualRuntimeHintToast,
  }
}
