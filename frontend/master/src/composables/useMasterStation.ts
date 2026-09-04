import { reactive, ref } from 'vue'
import {
  cancelMasterFileTransfer as apiCancelMasterFileTransfer,
  connectLinkProfile as apiConnectLinkProfile,
  connectToSlave as apiConnectToSlave,
  createMasterStation,
  disconnectFromSlave as apiDisconnectFromSlave,
  getMasterFileTransferSession as apiGetMasterFileTransferSession,
  getMasterConnections,
  getMasterMessages,
  sendIec104Command as apiSendIec104Command,
  sendTestFrame as apiSendTestFrame,
  startMasterFileTransfer as apiStartMasterFileTransfer,
  startMasterDataTransfer as apiStartMasterDataTransfer,
  stopMasterDataTransfer as apiStopMasterDataTransfer,
  startMasterStation,
  syncMasterDataPoints,
} from '../api/master'
import type {
  BackendConnectionInfo,
  BackendMasterMessage,
  CounterInterrogationQualifier,
  Iec104CommandPayload,
  InterrogationQualifier,
  MasterFileTransferRequest,
  MasterFileTransferSession,
  ResetProcessMode,
  SelectExecuteMode,
  SetpointQos,
} from '@shared/api/types'
import type { ControlCommandType } from '../types/master'
import { isBusinessLinkReadyState, isTransportConnectedState } from '@shared/ui/connectionStatus'
import { t } from '@shared/i18n'
import { normalizeApiError } from '@shared/i18n/errors'
import { useMasterPointData } from './useMasterPointData'

type LogRecord = {
  id: number
  level: string
  message: string
  timestamp: string
}

type CommandDispatchResult = {
  accepted: boolean
  traceId?: string | null
  errorCode?: string | null
  errorMessage?: string | null
}

export function useMasterStation() {
  const MESSAGE_SYNC_PAGE_SIZE = 200
  const MAX_MESSAGE_SYNC_BATCHES = 25
  const isConnected = ref(false)
  const showConnectDialog = ref(false)
  const connectionCount = ref(0)
  const pointData = useMasterPointData()
  const dataPointCount = pointData.size
  const commandCount = ref(0)

  const connections = ref<BackendConnectionInfo[]>([])
  const messages = ref<BackendMasterMessage[]>([])
  const logs = ref<LogRecord[]>([])
  const lastMessageId = ref<number>(0)
  const lastConnectionError = ref<{ code: string | null; message: string | null }>({
    code: null,
    message: null,
  })
  const lastFileTransferError = ref<{ code: string | null; message: string | null }>({
    code: null,
    message: null,
  })

  const connectForm = reactive({
    host: '127.0.0.1',
    port: 2404,
  })

  const currentStationId = ref('')
  const activeConnectionId = ref('')
  let pointSyncStationId = ''
  let pointSyncRequestRevision = 0

  const clearMessageState = () => {
    lastMessageId.value = 0
    messages.value = []
  }

  const isDataTransferStarted = (conn: BackendConnectionInfo) =>
    isBusinessLinkReadyState(conn.data_transfer_state)
  const isTransportConnected = (conn: BackendConnectionInfo) =>
    isTransportConnectedState(conn.transport_state)

  const addLog = (level: string, message: string) => {
    logs.value.unshift({
      id: Date.now(),
      level,
      message,
      timestamp: new Date().toISOString(),
    })

    if (logs.value.length > 100) {
      logs.value = logs.value.slice(0, 100)
    }
  }

  const initializeMasterStation = async () => {
    try {
      // createMasterStation 后端有 get-or-create 逻辑：
      // DB 中已有主站记录则复用稳定的站点 ID，否则新建。
      // 同时会触发 load_persisted_master_settings 恢复连接配置。
      const stationId = await createMasterStation({
        name: t('master.station.masterName'),
        protocol: 'iec104',
        station_type: 'Master',
        host: '0.0.0.0',
        port: 2404,
        common_address: 1,
        enabled: true,
      })

      currentStationId.value = stationId
      pointData.clear()
      pointSyncStationId = stationId
      activeConnectionId.value = ''
      clearMessageState()
      await startMasterStation(stationId)
      addLog('info', t('master.station.logs.initialized'))
    } catch (error) {
      addLog('error', t('master.station.logs.initializeFailed', { error }))
    }
  }

  const loadConnections = async (isCurrent: () => boolean = () => true) => {
    const stationId = currentStationId.value
    if (!stationId) return
    try {
      const list = await getMasterConnections(stationId)
      if (!isCurrent() || currentStationId.value !== stationId) return
      connections.value = list
      connectionCount.value = list.length
      isConnected.value = list.some(isDataTransferStarted)

      if (list.length === 0) {
        activeConnectionId.value = ''
        return
      }

      const stillExists = list.some((conn) => conn.id === activeConnectionId.value)
      if (!stillExists) {
        const preferred =
          list.find(isDataTransferStarted) ?? list.find(isTransportConnected) ?? list[0]
        activeConnectionId.value = preferred.id
      }
    } catch (error) {
      addLog('error', t('master.station.logs.loadConnectionsFailed', { error }))
    }
  }

  const loadDataPoints = async (isCurrent: () => boolean = () => true) => {
    const stationId = currentStationId.value
    if (!stationId) return
    if (pointSyncStationId !== stationId) {
      pointData.clear()
      pointSyncStationId = stationId
    }
    const requestRevision = ++pointSyncRequestRevision
    try {
      const batch = await syncMasterDataPoints(stationId, pointData.cursor())
      if (
        requestRevision !== pointSyncRequestRevision ||
        !isCurrent() ||
        currentStationId.value !== stationId
      ) {
        return
      }
      await pointData.applySyncBatch(batch)
    } catch (error) {
      addLog('error', t('master.station.logs.loadDataPointsFailed', { error }))
    }
  }

  const loadData = async () => {
    await loadConnections()
    await loadDataPoints()
  }

  const loadMessages = async (isCurrent: () => boolean = () => true) => {
    const stationId = currentStationId.value
    if (!stationId) return [] as BackendMasterMessage[]
    try {
      const list: BackendMasterMessage[] = []
      let cursor = lastMessageId.value > 0 ? lastMessageId.value : null

      for (let batchIndex = 0; batchIndex < MAX_MESSAGE_SYNC_BATCHES; batchIndex += 1) {
        const batch = await getMasterMessages(stationId, cursor, MESSAGE_SYNC_PAGE_SIZE)
        if (!isCurrent() || currentStationId.value !== stationId) return []
        if (batch.length === 0) break

        list.push(...batch)

        const lastId = batch[batch.length - 1]?.id
        if (typeof lastId !== 'number' || !Number.isFinite(lastId)) break

        cursor = lastId
        if (batch.length < MESSAGE_SYNC_PAGE_SIZE) break
      }

      if (list.length > 0 && isCurrent() && currentStationId.value === stationId) {
        lastMessageId.value = list[list.length - 1].id
        messages.value.push(...list)
        if (messages.value.length > 2000) {
          messages.value = messages.value.slice(-2000)
        }
      }

      return list
    } catch (error) {
      addLog('error', t('master.station.logs.loadMessagesFailed', { error }))
      return [] as BackendMasterMessage[]
    }
  }

  const connectToSlave = async (request?: {
    host: string
    port: number
    profileId?: number | null
    autoStartDataTransfer?: boolean | null
  }) => {
    if (!currentStationId.value) return null
    const host = request?.host ?? connectForm.host
    const port = request?.port ?? connectForm.port
    lastConnectionError.value = { code: null, message: null }
    try {
      const req: any = { host, port }
      if (request?.profileId != null) {
        req.profile_id = request.profileId
      }
      if (request?.autoStartDataTransfer != null) {
        req.auto_start_data_transfer = request.autoStartDataTransfer
      }
      const connectionId = await apiConnectToSlave(currentStationId.value, req)
      activeConnectionId.value = connectionId
      addLog('info', t('master.station.logs.connected', { target: `${host}:${port}` }))
      showConnectDialog.value = false
      await loadConnections()
      return connectionId
    } catch (error) {
      const normalized = normalizeApiError(error) ?? { code: null, message: String(error) }
      lastConnectionError.value = normalized
      addLog(
        'error',
        t('master.station.logs.connectFailed', {
          code: normalized.code ? t('master.station.codeSuffix', { code: normalized.code }) : '',
          message: normalized.message,
        }),
      )
      return null
    }
  }

  const disconnectFromSlave = async (connectionId?: string | null) => {
    if (!currentStationId.value) {
      addLog('warning', t('master.station.logs.initFirst'))
      return false
    }

    const targetConnectionId = connectionId ?? activeConnectionId.value
    if (!targetConnectionId) {
      addLog('warning', t('master.station.logs.noConnectionToDisconnect'))
      return false
    }

    try {
      await apiDisconnectFromSlave(currentStationId.value, targetConnectionId)
      if (activeConnectionId.value === targetConnectionId) {
        activeConnectionId.value = ''
      }
      addLog('info', t('master.station.logs.disconnected', { connectionId: targetConnectionId }))
      await loadConnections()
      return true
    } catch (error) {
      addLog('error', t('master.station.logs.disconnectFailed', { error }))
      return false
    }
  }

  const setActiveConnection = (connectionId: string | null) => {
    activeConnectionId.value = connectionId ?? ''
  }

  const sendGeneralInterrogation = async (
    qoi: InterrogationQualifier,
    connectionId?: string,
    slaveId?: number | null,
  ) => {
    return sendIec104Command(
      {
        type: 'general-interrogation',
        qoi,
      },
      qoi.kind === 'station'
        ? t('master.station.commands.generalInterrogationStation')
        : t('master.station.commands.generalInterrogationGroup', {
            group: qoi.group,
            qoi: qoi.group + 20,
          }),
      connectionId,
      slaveId,
    )
  }

  const getActiveConnectionId = (preferredConnectionId?: string | null) => {
    if (!currentStationId.value || connections.value.length === 0) {
      return null
    }

    if (preferredConnectionId) {
      const matched = connections.value.find((conn) => conn.id === preferredConnectionId)
      if (matched) return matched.id
    }

    if (activeConnectionId.value) {
      const selected = connections.value.find((conn) => conn.id === activeConnectionId.value)
      if (selected) return selected.id
    }

    const started =
      connections.value.find(isDataTransferStarted) ?? connections.value.find(isTransportConnected)
    return started?.id ?? connections.value[0].id
  }

  const dispatchIec104Command = async (
    command: Iec104CommandPayload,
    commandName: string,
    preferredConnectionId?: string | null,
    slaveId?: number | null,
    qualifierMode: 'standard' | 'raw_test' = 'standard',
  ): Promise<CommandDispatchResult> => {
    if (!currentStationId.value) {
      addLog('warning', t('master.station.logs.initFirst'))
      return {
        accepted: false,
        errorCode: 'INTERNAL_ERROR',
        errorMessage: t('master.station.errors.initFirst'),
      }
    }

    const connectionId = getActiveConnectionId(preferredConnectionId)
    if (!connectionId) {
      addLog('warning', t('master.station.errors.connectFirst'))
      return {
        accepted: false,
        errorCode: 'NOT_CONNECTED',
        errorMessage: t('master.station.errors.connectFirst'),
      }
    }

    const conn = connections.value.find((item) => item.id === connectionId)
    if (!conn || !isDataTransferStarted(conn)) {
      addLog('warning', t('master.station.logs.commandIntercepted'))
      return {
        accepted: false,
        errorCode: 'PROTOCOL_STATE',
        errorMessage: t('master.station.errors.protocolState'),
      }
    }

    try {
      const result = await apiSendIec104Command(currentStationId.value, {
        connection_id: connectionId,
        slave_id: slaveId ?? 0,
        qualifier_mode: qualifierMode,
        command,
      })

      if (!result.accepted) {
        const code = result.error_code ?? 'UNKNOWN'
        const message = result.error_message ?? t('master.station.errors.commandRejected')
        addLog('error', t('master.station.logs.commandFailed', { commandName, code, message }))
        return {
          accepted: false,
          errorCode: code,
          errorMessage: message,
        }
      }

      addLog(
        'info',
        t('master.station.logs.commandSent', {
          commandName,
          traceId: result.trace_id ?? 'n/a',
        }),
      )
      commandCount.value++
      return {
        accepted: true,
        traceId: result.trace_id ?? null,
      }
    } catch (error) {
      addLog('error', t('master.station.logs.commandFailedError', { commandName, error }))
      return {
        accepted: false,
        errorCode: 'INTERNAL_ERROR',
        errorMessage: String(error),
      }
    }
  }

  const sendIec104Command = async (
    command: Iec104CommandPayload,
    commandName: string,
    preferredConnectionId?: string | null,
    slaveId?: number | null,
  ) => {
    const result = await dispatchIec104Command(command, commandName, preferredConnectionId, slaveId)
    return result.accepted
  }

  const sendCounterInterrogation = async (connectionId?: string, slaveId?: number | null) => {
    return sendCounterCommand(1, 'read', connectionId, slaveId)
  }

  const sendCounterCommand = async (
    request: number,
    freeze: CounterInterrogationQualifier['freeze'],
    connectionId?: string,
    slaveId?: number | null,
  ) => {
    return sendIec104Command(
      {
        type: 'counter-interrogation',
        qcc: { request, freeze },
      },
      t('master.station.commands.counterInterrogation'),
      connectionId,
      slaveId,
    )
  }

  const sendCounterFreeze = async (connectionId?: string, slaveId?: number | null) => {
    return sendCounterCommand(1, 'freeze', connectionId, slaveId)
  }

  const sendClockSynchronization = async (connectionId?: string, slaveId?: number | null) => {
    return sendIec104Command(
      {
        type: 'clock-synchronization',
      },
      t('master.station.commands.clockSync'),
      connectionId,
      slaveId,
    )
  }

  const sendResetProcessCommand = async (
    connectionId?: string,
    slaveId?: number | null,
    qrp: ResetProcessMode = 'general-reset',
  ) => {
    return sendIec104Command(
      {
        type: 'reset-process-command',
        ioa: 0,
        qrp,
      },
      t('master.station.commands.resetProcess', { qrp }),
      connectionId,
      slaveId,
    )
  }

  const sendTestCommand = async (connectionId?: string, slaveId?: number | null) => {
    return sendIec104Command(
      {
        type: 'test-command',
        ioa: 0,
      },
      t('master.station.commands.testCommand'),
      connectionId,
      slaveId,
    )
  }

  const connectLinkProfile = async (
    linkProfileId: number,
    autoStartDataTransfer?: boolean | null,
  ) => {
    if (!currentStationId.value) return null
    lastConnectionError.value = { code: null, message: null }
    try {
      const connectionId = await apiConnectLinkProfile(
        currentStationId.value,
        linkProfileId,
        autoStartDataTransfer ?? null,
      )
      activeConnectionId.value = connectionId
      addLog(
        'info',
        t('master.station.logs.connectLinkProfileSuccess', { profileId: linkProfileId }),
      )
      await loadConnections()
      return connectionId
    } catch (error) {
      const normalized = normalizeApiError(error) ?? { code: null, message: String(error) }
      lastConnectionError.value = normalized
      addLog(
        'error',
        t('master.station.logs.connectLinkProfileFailed', {
          profileId: linkProfileId,
          code: normalized.code ? t('master.station.codeSuffix', { code: normalized.code }) : '',
          message: normalized.message,
        }),
      )
      return null
    }
  }

  const sendTestFrame = async (connectionId?: string) => {
    const targetConnectionId = connectionId ?? activeConnectionId.value
    if (!currentStationId.value || !targetConnectionId) {
      addLog('warning', t('master.station.logs.missingStationOrConnectionForTestFrame'))
      return false
    }

    const conn = connections.value.find((item) => item.id === targetConnectionId)
    if (!conn || !isTransportConnected(conn)) {
      addLog('warning', t('master.station.logs.tcpNotConnectedForTestFrame'))
      return false
    }

    try {
      await apiSendTestFrame(currentStationId.value, targetConnectionId)
      addLog('info', t('master.station.logs.testFrameSent', { connectionId: targetConnectionId }))
      commandCount.value++
      return true
    } catch (error) {
      addLog('error', t('master.station.logs.testFrameFailed', { error }))
      return false
    }
  }

  const clampQos = (ql: number | undefined): SetpointQos['ql'] => {
    const numeric = Number(ql)
    if (!Number.isFinite(numeric)) return 0
    return Math.min(127, Math.max(0, Math.trunc(numeric)))
  }

  const parseI16 = (value: unknown): number | null => {
    const numeric = Number(value)
    if (!Number.isInteger(numeric) || numeric < -32768 || numeric > 32767) return null
    return numeric
  }

  const normalizeSetpointQos = (se: SelectExecuteMode, ql?: number): SetpointQos => ({
    se,
    ql: clampQos(ql),
  })

  const clampQocQu = (
    qu: number | undefined,
    qualifierMode: 'standard' | 'raw_test' = 'standard',
  ): number => {
    const numeric = Number(qu)
    if (!Number.isFinite(numeric)) return 0
    return Math.min(qualifierMode === 'raw_test' ? 31 : 3, Math.max(0, Math.trunc(numeric)))
  }

  const controlActionLabel = (se: SelectExecuteMode): string => {
    if (se === 'select') return t('master.station.controlActions.select')
    if (se === 'cancel') return t('master.station.controlActions.cancel')
    return t('master.station.controlActions.execute')
  }

  const sendSingleControlCommand = async (
    address: number,
    value: unknown,
    options?: { connectionId?: string | null; se?: SelectExecuteMode; qu?: number },
  ) => {
    const qu = clampQocQu(options?.qu)
    return sendIec104Command(
      {
        type: 'single-command',
        ioa: address,
        value: Boolean(value),
        se: options?.se ?? 'execute',
        qu,
      },
      t('master.station.commands.singleControl', { address, qu }),
      options?.connectionId,
    )
  }

  const sendControlCommandByType = async (
    commandType: ControlCommandType,
    address: number,
    value: unknown,
    options?: {
      connectionId?: string | null
      slaveId?: number | null
      se?: SelectExecuteMode
      qu?: number
      setpointType?: 'normalized' | 'scaled' | 'short-float'
      ql?: number
      qualifierMode?: 'standard' | 'raw_test'
      typeId?: number
    },
  ) => {
    const se = options?.se ?? 'execute'
    const connectionId = options?.connectionId
    const slaveId = options?.slaveId
    const qualifierMode = options?.qualifierMode ?? 'standard'
    const qu = clampQocQu(options?.qu, qualifierMode)
    const typeId = options?.typeId

    switch (commandType) {
      case 'single-command':
        return dispatchIec104Command(
          {
            type: typeId === 58 ? 'single-command-timed' : 'single-command',
            ioa: address,
            value: Boolean(value),
            se,
            qu,
          },
          t('master.station.commands.singleControlAction', {
            action: controlActionLabel(se),
            address,
            qu,
          }),
          connectionId,
          slaveId,
          qualifierMode,
        )
      case 'double-command': {
        const raw = Number(value)
        const mapped: 1 | 2 = raw === 2 ? 2 : 1
        return dispatchIec104Command(
          {
            type: typeId === 59 ? 'double-command-timed' : 'double-command',
            ioa: address,
            value: mapped,
            se,
            qu,
          },
          t('master.station.commands.doubleControlAction', {
            action: controlActionLabel(se),
            address,
            qu,
          }),
          connectionId,
          slaveId,
          qualifierMode,
        )
      }
      case 'regulating-step': {
        const raw = Number(value)
        const step: 1 | 2 = raw === 2 ? 2 : 1
        return dispatchIec104Command(
          {
            type: typeId === 60 ? 'regulating-step-command-timed' : 'regulating-step-command',
            ioa: address,
            step,
            se,
            qu,
          },
          t('master.station.commands.regulatingStepAction', {
            action: controlActionLabel(se),
            address,
            qu,
          }),
          connectionId,
          slaveId,
          qualifierMode,
        )
      }
      case 'set-point': {
        const setpointType = options?.setpointType ?? 'short-float'
        const qos = normalizeSetpointQos(se, options?.ql)

        if (setpointType === 'normalized') {
          const normalized = Number(value)
          return dispatchIec104Command(
            {
              type: typeId === 61 ? 'set-point-normalized-timed' : 'set-point-normalized',
              ioa: address,
              value: normalized,
              qos,
            },
            t('master.station.commands.normalizedSetpointAction', {
              action: controlActionLabel(se),
              address,
              ql: qos.ql,
            }),
            connectionId,
            slaveId,
          )
        }

        if (setpointType === 'scaled') {
          const scaled = parseI16(value)
          if (scaled == null) {
            return {
              accepted: false,
              errorCode: 'INVALID_VALUE',
              errorMessage: t('master.station.logs.invalidScaledSetpoint'),
            }
          }
          return dispatchIec104Command(
            {
              type: typeId === 62 ? 'set-point-scaled-timed' : 'set-point-scaled',
              ioa: address,
              value: scaled,
              qos,
            },
            t('master.station.commands.scaledSetpointAction', {
              action: controlActionLabel(se),
              address,
              ql: qos.ql,
            }),
            connectionId,
            slaveId,
          )
        }

        const numeric = Number(value)
        const setpoint = Number.isFinite(numeric) ? numeric : 0
        return dispatchIec104Command(
          {
            type: typeId === 63 ? 'set-point-short-float-timed' : 'set-point-short-float',
            ioa: address,
            value: setpoint,
            qos,
          },
          t('master.station.commands.shortFloatSetpointAction', {
            action: controlActionLabel(se),
            address,
            ql: qos.ql,
          }),
          connectionId,
          slaveId,
        )
      }
      case 'bit-string-command': {
        const bitString = Number(value)
        if (!Number.isInteger(bitString) || bitString < 0 || bitString > 0xffff_ffff) {
          return {
            accepted: false,
            errorCode: 'INVALID_VALUE',
            errorMessage: t('master.station.logs.invalidBitString'),
          }
        }
        return dispatchIec104Command(
          {
            type: typeId === 64 ? 'bit-string-command-timed' : 'bit-string-command',
            ioa: address,
            value: bitString,
          },
          t('master.station.commands.bitString', {
            address,
            value: `0x${bitString.toString(16).toUpperCase().padStart(8, '0')}`,
          }),
          connectionId,
          slaveId,
        )
      }
      case 'read-command':
        return dispatchIec104Command(
          { type: 'read-command', ioa: address },
          t('master.station.commands.readCommand', { address }),
          connectionId,
          slaveId,
        )
      default:
        addLog('warning', t('master.station.logs.unsupportedCommand', { commandType }))
        return {
          accepted: false,
          errorCode: 'UNSUPPORTED_COMMAND',
          errorMessage: t('master.station.logs.unsupportedCommand', { commandType }),
        }
    }
  }

  const startDataTransfer = async (connectionId?: string | null) => {
    const targetConnectionId = connectionId ?? activeConnectionId.value
    if (!currentStationId.value || !targetConnectionId) return false
    try {
      await apiStartMasterDataTransfer(currentStationId.value, targetConnectionId)
      addLog(
        'info',
        t('master.station.logs.startDataTransferSent', { connectionId: targetConnectionId }),
      )
      await loadConnections()
      return true
    } catch (error) {
      addLog('error', t('master.station.logs.startDataTransferFailed', { error }))
      return false
    }
  }

  const stopDataTransfer = async (connectionId?: string | null) => {
    const targetConnectionId = connectionId ?? activeConnectionId.value
    if (!currentStationId.value || !targetConnectionId) return false
    try {
      await apiStopMasterDataTransfer(currentStationId.value, targetConnectionId)
      addLog(
        'info',
        t('master.station.logs.stopDataTransferSent', { connectionId: targetConnectionId }),
      )
      await loadConnections()
      return true
    } catch (error) {
      addLog('error', t('master.station.logs.stopDataTransferFailed', { error }))
      return false
    }
  }

  const startFileTransfer = async (
    request: MasterFileTransferRequest,
    displayName = t('master.runtime.fileTransferActions.fallback'),
  ): Promise<MasterFileTransferSession | null> => {
    if (!currentStationId.value) {
      addLog('warning', t('master.station.logs.initFirst'))
      return null
    }
    lastFileTransferError.value = { code: null, message: null }
    try {
      const session = await apiStartMasterFileTransfer(currentStationId.value, request)
      addLog(
        'info',
        t('master.station.logs.fileTransferStarted', {
          displayName,
          sessionId: session.session_id,
        }),
      )
      return session
    } catch (error) {
      const normalized = normalizeApiError(error) ?? { code: null, message: String(error) }
      lastFileTransferError.value = normalized
      addLog(
        'error',
        t('master.station.logs.fileTransferStartFailed', {
          displayName,
          code: normalized.code ? t('master.station.codeSuffix', { code: normalized.code }) : '',
          message: normalized.message,
        }),
      )
      return null
    }
  }

  const getFileTransferSession = async (
    connectionId: string,
  ): Promise<MasterFileTransferSession | null> => {
    if (!currentStationId.value || !connectionId) return null
    try {
      return await apiGetMasterFileTransferSession(currentStationId.value, connectionId)
    } catch (error) {
      addLog('error', t('master.station.logs.readFileTransferSessionFailed', { error }))
      return null
    }
  }

  const cancelFileTransfer = async (
    connectionId: string,
  ): Promise<MasterFileTransferSession | null> => {
    if (!currentStationId.value || !connectionId) return null
    try {
      const session = await apiCancelMasterFileTransfer(currentStationId.value, connectionId)
      addLog(
        'warning',
        t('master.station.logs.fileTransferCancelled', { sessionId: session.session_id }),
      )
      return session
    } catch (error) {
      addLog('error', t('master.station.logs.cancelFileTransferFailed', { error }))
      return null
    }
  }

  return {
    isConnected,
    showConnectDialog,
    connectionCount,
    dataPointCount,
    commandCount,
    connections,
    pointData,
    messages,
    clearMessageState,
    logs,
    lastConnectionError,
    lastFileTransferError,
    connectForm,
    currentStationId,
    activeConnectionId,
    initializeMasterStation,
    loadData,
    loadConnections,
    loadDataPoints,
    loadMessages,
    connectToSlave,
    disconnectFromSlave,
    setActiveConnection,
    sendGeneralInterrogation,
    sendCounterInterrogation,
    sendCounterFreeze,
    sendCounterCommand,
    sendClockSynchronization,
    sendResetProcessCommand,
    sendTestCommand,
    connectLinkProfile,
    sendTestFrame,
    startDataTransfer,
    stopDataTransfer,
    sendControlCommandByType,
    sendSingleControlCommand,
    startFileTransfer,
    getFileTransferSession,
    cancelFileTransfer,
    addLog,
  }
}
