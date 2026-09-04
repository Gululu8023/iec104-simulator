import { ElMessage } from 'element-plus'
import type { Ref } from 'vue'

import type {
  BackendConnectionInfo,
  LinkProfileResponse,
  MasterFileTransferRequest,
  MasterFileTransferSession,
  InterrogationQualifier,
  ResetProcessMode,
} from '@shared/api/types'
import { deriveUnifiedConnStatus, isBusinessLinkReadyState } from '@shared/ui/connectionStatus'
import { t } from '@shared/i18n'
import type { ControlCommandType, GlobalCommandType, SlaveConnection } from '../types/master'

type RuntimeLinkState = {
  transport: string
  link: string
}

type LoadingCommandsState = {
  gi: boolean
  ci: boolean
  cf: boolean
  cs: boolean
  rp: boolean
}

type UseMasterRuntimeActionsOptions = {
  activeTab: Ref<string>
  loadingCommands: LoadingCommandsState
  selectedSlave: Ref<SlaveConnection | null>
  selectedLinkProfileId: Ref<number | null>
  hasSelectedLinkProfile: Ref<boolean>
  slaveProfiles: Ref<SlaveConnection[]>
  linkProfiles: Ref<LinkProfileResponse[]>
  connections: Ref<BackendConnectionInfo[]>
  showFileTransferDialog: Ref<boolean>
  fileTransferSession: Ref<MasterFileTransferSession | null>
  fileTransferActionBusy: Ref<boolean>
  resetProcessMode: Ref<ResetProcessMode>
  lastConnectionError: Ref<{ code: string | null; message: string | null }>
  lastFileTransferError: Ref<{ code: string | null; message: string | null }>
  selectSlave: (slave: SlaveConnection) => void
  openCreateLinkDialog: () => void
  openEditProfileDialog: (profileId: number) => void
  openCreateProfileDialog: (linkProfileId: number) => void
  openEditLinkDialog: (linkProfileId: number) => void
  deleteSelectedProfile: (profileId: number) => Promise<void>
  deleteLinkWithCascade: (linkProfileId: number) => Promise<void>
  resolveConnectTargetForProfile: (profileId: number) => { host: string; port: number } | null
  resolveConnectionIdBySlaveId: (slaveId: string) => string | null
  resolveRuntimeLinkStateByConnectionId: (
    connectionId: string | null | undefined,
  ) => RuntimeLinkState
  listRuntimeConnectionIdsForLink: (linkProfileId: number) => string[]
  openPointTableImportDialog: () => void
  connectToSlave: (request?: {
    host: string
    port: number
    profileId?: number | null
    autoStartDataTransfer?: boolean | null
  }) => Promise<string | null>
  connectLinkProfile: (
    linkProfileId: number,
    autoStartDataTransfer?: boolean | null,
  ) => Promise<string | null>
  disconnectFromSlave: (connectionId?: string | null) => Promise<boolean>
  startDataTransfer: (connectionId?: string | null) => Promise<boolean>
  stopDataTransfer: (connectionId?: string | null) => Promise<boolean>
  sendGeneralInterrogation: (
    qoi: InterrogationQualifier,
    connectionId?: string,
    slaveId?: number | null,
  ) => Promise<boolean>
  sendCounterInterrogation: (connectionId?: string, slaveId?: number | null) => Promise<boolean>
  sendCounterFreeze: (connectionId?: string, slaveId?: number | null) => Promise<boolean>
  sendClockSynchronization: (connectionId?: string, slaveId?: number | null) => Promise<boolean>
  sendResetProcessCommand: (
    connectionId?: string,
    slaveId?: number | null,
    qrp?: ResetProcessMode,
  ) => Promise<boolean>
  sendTestCommand: (connectionId?: string, slaveId?: number | null) => Promise<boolean>
  sendTestFrame: (connectionId?: string) => Promise<boolean>
  startFileTransfer: (
    request: MasterFileTransferRequest,
    displayName?: string,
  ) => Promise<MasterFileTransferSession | null>
  getFileTransferSession: (connectionId: string) => Promise<MasterFileTransferSession | null>
  cancelFileTransfer: (connectionId: string) => Promise<MasterFileTransferSession | null>
  refreshProfiles: () => Promise<void>
  syncFromBackend: () => Promise<void>
  registerManualRuntimeHintToast: (
    connectionId: string,
    expected: 'command-confirmed' | 'test-frame-confirmed',
  ) => void
  unregisterManualRuntimeHintToast: (
    connectionId: string,
    expected: 'command-confirmed' | 'test-frame-confirmed',
  ) => void
  toggleMessagePanel: () => void
  toggleSoePanel: () => void
  handleSendCommand: (command: ControlCommandType, params?: any) => void
  addLog: (level: string, message: string) => void
}

const isTerminalFileTransferState = (state?: string | null) =>
  state === 'success' || state === 'failed' || state === 'cancelled'

export function useMasterRuntimeActions(options: UseMasterRuntimeActionsOptions) {
  const resolveConnectionFailureText = (fallback: string): string => {
    if (options.lastConnectionError.value.code === 'DUPLICATE_CONNECTION') {
      return options.lastConnectionError.value.message || t('master.runtime.duplicateConnection')
    }
    return options.lastConnectionError.value.message || fallback
  }

  const parseRemoteAddress = (remote: string): { host: string; port: number } | null => {
    const lastColon = remote.lastIndexOf(':')
    if (lastColon <= 0) return null
    const host = remote.slice(0, lastColon)
    const port = Number(remote.slice(lastColon + 1))
    if (!host || !Number.isFinite(port) || port <= 0) return null
    return { host, port }
  }

  const toggleConnection = async () => {
    if (!options.selectedSlave.value) {
      ElMessage.warning(t('master.runtime.warnings.selectSlaveFirst'))
      return
    }
    const status = deriveUnifiedConnStatus(
      options.selectedSlave.value.transportState,
      options.selectedSlave.value.dataTransferState,
    )
    const isActive = status.severity !== 'status-idle'
    await handleConnectionAction(isActive ? 'disconnect' : 'connect')
  }

  async function handleLinkOperation(
    operation: 'connect-link' | 'disconnect-link' | 'create-station' | 'edit-link' | 'delete-link',
    linkProfileId: number,
  ) {
    const targetLink = options.linkProfiles.value.find((link) => link.id === linkProfileId) ?? null

    switch (operation) {
      case 'connect-link':
        if (!targetLink) {
          ElMessage.warning(t('master.runtime.warnings.linkMissing'))
          return
        }
        if (await options.connectLinkProfile(linkProfileId, null)) {
          await options.syncFromBackend()
          ElMessage.success(
            t('master.runtime.success.tcpConnected', {
              target: `${targetLink.host}:${targetLink.port}`,
            }),
          )
        } else {
          ElMessage.error(
            resolveConnectionFailureText(
              t('master.runtime.errors.connectFailed', {
                target: `${targetLink.host}:${targetLink.port}`,
              }),
            ),
          )
        }
        return
      case 'disconnect-link': {
        const runtimeConnectionIds = options.listRuntimeConnectionIdsForLink(linkProfileId)
        if (runtimeConnectionIds.length === 0) {
          ElMessage.info(t('master.runtime.info.linkNotEstablished'))
          return
        }

        for (const connectionId of runtimeConnectionIds) {
          await options.disconnectFromSlave(connectionId)
        }
        await options.syncFromBackend()
        ElMessage.success(t('master.runtime.success.disconnected'))
        return
      }
      case 'create-station':
        options.openCreateProfileDialog(linkProfileId)
        return
      case 'edit-link':
        options.openEditLinkDialog(linkProfileId)
        return
      case 'delete-link':
        await options.deleteLinkWithCascade(linkProfileId)
        await options.syncFromBackend()
        return
      default:
        options.addLog('warning', t('master.runtime.logs.unknownLinkOperation', { operation }))
    }
  }

  async function handleConnectionAction(action: string) {
    switch (action) {
      case 'add-connection':
        options.openCreateLinkDialog()
        break
      case 'add-slave':
        if (!options.hasSelectedLinkProfile.value || options.selectedLinkProfileId.value == null) {
          ElMessage.warning(t('master.runtime.warnings.selectSlaveForLink'))
          return
        }
        await handleLinkOperation('create-station', options.selectedLinkProfileId.value)
        break
      case 'connect':
        if (options.selectedSlave.value) {
          options.addLog(
            'info',
            t('master.runtime.logs.connectLink', { name: options.selectedSlave.value.linkName }),
          )
          await handleLinkOperation('connect-link', options.selectedSlave.value.linkProfileId)
        } else {
          ElMessage.warning(t('master.runtime.warnings.selectConnectionOrSlave'))
        }
        break
      case 'disconnect':
        if (options.selectedSlave.value) {
          options.addLog(
            'info',
            t('master.runtime.logs.disconnectLink', { name: options.selectedSlave.value.linkName }),
          )
          await handleLinkOperation('disconnect-link', options.selectedSlave.value.linkProfileId)
        } else {
          ElMessage.warning(t('master.runtime.warnings.selectConnectionOrSlave'))
        }
        break
      case 'reconnect-settings':
        await openReconnectSettings()
        break
      case 'edit':
        if (options.selectedSlave.value) {
          options.openEditProfileDialog(options.selectedSlave.value.profileId)
        } else {
          ElMessage.warning(t('master.runtime.warnings.selectSlaveFirst'))
        }
        break
      case 'import-point-defs':
        options.openPointTableImportDialog()
        break
      case 'delete':
        if (options.selectedSlave.value) {
          await options.deleteSelectedProfile(options.selectedSlave.value.profileId)
          await options.syncFromBackend()
        } else {
          ElMessage.warning(t('master.runtime.warnings.selectSlaveFirst'))
        }
        break
      case 'toggle-message-panel':
        options.toggleMessagePanel()
        break
      case 'toggle':
        await toggleConnection()
        break
      case 'edit-link':
        if (!options.hasSelectedLinkProfile.value || options.selectedLinkProfileId.value == null) {
          ElMessage.warning(t('master.runtime.warnings.selectSlaveForLink'))
          return
        }
        await handleLinkOperation('edit-link', options.selectedLinkProfileId.value)
        break
      case 'delete-link':
        if (!options.hasSelectedLinkProfile.value || options.selectedLinkProfileId.value == null) {
          ElMessage.warning(t('master.runtime.warnings.selectSlaveForLink'))
          return
        }
        await handleLinkOperation('delete-link', options.selectedLinkProfileId.value)
        break
      case 'reconnect':
        if (options.selectedSlave.value) {
          const linkProfileId = options.selectedSlave.value.linkProfileId
          options.addLog(
            'info',
            t('master.runtime.logs.reconnectLink', { name: options.selectedSlave.value.linkName }),
          )
          await handleLinkOperation('disconnect-link', linkProfileId)
          await new Promise((resolve) => setTimeout(resolve, 500))
          await handleLinkOperation('connect-link', linkProfileId)
        } else {
          ElMessage.warning(t('master.runtime.warnings.selectConnectionOrSlave'))
        }
        break
      case 'refresh-devices':
        await options.refreshProfiles()
        await options.syncFromBackend()
        break
      case 'toggle-soe-panel':
        options.toggleSoePanel()
        break
      case 'manage':
        options.activeTab.value = 'connections'
        options.addLog('info', t('master.runtime.logs.openConnectionManager'))
        break
      default:
        options.addLog('warning', t('master.runtime.logs.unknownConnectionAction', { action }))
    }
  }

  async function handleProfileOperation(
    operation: 'connect' | 'disconnect' | 'edit' | 'delete',
    slave: SlaveConnection,
  ) {
    options.selectSlave(slave)
    await handleConnectionAction(operation)
  }

  async function handleSlaveConnect(slaveId: string) {
    const matchedProfile = options.slaveProfiles.value.find((profile) => profile.id === slaveId)
    if (matchedProfile) {
      const connectTarget = options.resolveConnectTargetForProfile(matchedProfile.profileId)
      if (!connectTarget) {
        ElMessage.warning(t('master.runtime.warnings.targetMissing'))
        return
      }
      if (await options.connectLinkProfile(matchedProfile.linkProfileId, true)) {
        await options.syncFromBackend()
        ElMessage.success(
          t('master.runtime.success.tcpConnected', {
            target: `${connectTarget.host}:${connectTarget.port}`,
          }),
        )
      } else {
        ElMessage.error(
          resolveConnectionFailureText(
            t('master.runtime.errors.connectFailed', {
              target: `${connectTarget.host}:${connectTarget.port}`,
            }),
          ),
        )
      }
      return
    }

    const target = options.connections.value.find((conn) => conn.id === slaveId)
    if (!target) {
      ElMessage.warning(t('master.runtime.warnings.targetMissing'))
      return
    }
    const parsed = parseRemoteAddress(target.remote_addr)
    if (!parsed) {
      ElMessage.error(
        t('master.runtime.errors.invalidRemoteAddress', { address: target.remote_addr }),
      )
      return
    }

    if (typeof target.profile_id === 'number') {
      if (await options.connectLinkProfile(target.profile_id, true)) {
        await options.syncFromBackend()
        ElMessage.success(
          t('master.runtime.success.tcpConnected', { target: `${parsed.host}:${parsed.port}` }),
        )
      } else {
        ElMessage.error(
          resolveConnectionFailureText(
            t('master.runtime.errors.connectFailed', { target: `${parsed.host}:${parsed.port}` }),
          ),
        )
      }
      return
    }

    if (await options.connectToSlave({ ...parsed, profileId: target.profile_id ?? undefined })) {
      await options.syncFromBackend()
      ElMessage.success(
        t('master.runtime.success.tcpConnected', { target: `${parsed.host}:${parsed.port}` }),
      )
    } else {
      ElMessage.error(
        resolveConnectionFailureText(
          t('master.runtime.errors.connectFailed', { target: `${parsed.host}:${parsed.port}` }),
        ),
      )
    }
  }

  async function handleSlaveDisconnect(slaveId: string) {
    const targetConnectionId = options.resolveConnectionIdBySlaveId(slaveId)
    if (!targetConnectionId) {
      ElMessage.info(t('master.runtime.warnings.currentSlaveDisconnected'))
      return
    }

    if (await options.disconnectFromSlave(targetConnectionId)) {
      await options.syncFromBackend()
      ElMessage.success(t('master.runtime.success.disconnected'))
    }
  }

  const refreshFileTransferSession = async (runtimeOptions: { includeTerminal?: boolean } = {}) => {
    const includeTerminal = runtimeOptions.includeTerminal ?? options.showFileTransferDialog.value
    const connectionId = options.selectedSlave.value?.connectionId
    if (!connectionId) {
      options.fileTransferSession.value = null
      return
    }
    if (
      options.fileTransferSession.value?.connection_id &&
      options.fileTransferSession.value.connection_id !== connectionId
    ) {
      options.fileTransferSession.value = null
    }
    const session = await options.getFileTransferSession(connectionId)
    if (options.selectedSlave.value?.connectionId !== connectionId) {
      return
    }
    if (session && (includeTerminal || !isTerminalFileTransferState(session.state))) {
      options.fileTransferSession.value = session
      return
    }
    options.fileTransferSession.value = null
  }

  const resolveFileTransferActionLabel = (request: MasterFileTransferRequest): string => {
    switch (request.mode) {
      case 'directory':
        return t('master.runtime.fileTransferActions.directory')
      case 'download':
        return t('master.runtime.fileTransferActions.download')
      case 'query-log':
        return t('master.runtime.fileTransferActions.queryLog')
      case 'upload':
        return t('master.runtime.fileTransferActions.upload')
      default:
        return t('master.runtime.fileTransferActions.fallback')
    }
  }

  const resolveFileTransferStartErrorMessage = (request: MasterFileTransferRequest): string => {
    const code = options.lastFileTransferError.value.message
      ? options.lastFileTransferError.value.code
      : null
    const message = String(options.lastFileTransferError.value.message ?? '').trim()
    switch (code) {
      case 'NOT_CONNECTED':
        return t('master.runtime.errors.fileTransferNotConnected')
      case 'PROTOCOL_STATE':
        return t('master.runtime.errors.fileTransferProtocolState')
      case 'FILE_TRANSFER_BUSY':
        return t('master.runtime.errors.fileTransferBusy')
      case 'UPLOAD_FILE_MISSING':
        return message || t('master.runtime.errors.uploadFileMissing')
      case 'FILE_TOO_LARGE':
        return message || t('master.runtime.errors.fileTooLarge')
      case 'VALIDATION_ERROR':
        return (
          message ||
          t('master.runtime.errors.invalidFileTransferParams', {
            action: resolveFileTransferActionLabel(request),
          })
        )
      default:
        return (
          message ||
          t('master.runtime.errors.startFileTransferFailed', {
            action: resolveFileTransferActionLabel(request),
          })
        )
    }
  }

  const handleStartFileTransfer = async (request: MasterFileTransferRequest) => {
    if (options.fileTransferActionBusy.value) {
      ElMessage.info(t('master.runtime.info.fileTransferBusy'))
      return
    }
    options.fileTransferActionBusy.value = true
    try {
      const targetConnectionId = request.connection_id
      const session = await options.startFileTransfer(
        request,
        resolveFileTransferActionLabel(request),
      )
      if (!session) {
        ElMessage.error(resolveFileTransferStartErrorMessage(request))
        return
      }
      if (options.selectedSlave.value?.connectionId === targetConnectionId) {
        options.fileTransferSession.value = session
      }
      if (request.mode === 'directory') {
        ElMessage.success(t('master.runtime.success.directorySent'))
      } else if (request.mode === 'download') {
        ElMessage.success(t('master.runtime.success.downloadStarted'))
      } else if (request.mode === 'query-log') {
        ElMessage.success(t('master.runtime.success.queryLogSent'))
      } else {
        ElMessage.success(t('master.runtime.success.uploadStarted'))
      }
    } finally {
      options.fileTransferActionBusy.value = false
    }
  }

  const handleCancelFileTransfer = async () => {
    if (options.fileTransferActionBusy.value) {
      ElMessage.info(t('master.runtime.info.fileTransferBusy'))
      return
    }
    const connectionId = options.selectedSlave.value?.connectionId
    if (!connectionId) {
      ElMessage.warning(t('master.runtime.warnings.fileTransferNoConnection'))
      return
    }
    options.fileTransferActionBusy.value = true
    try {
      const session = await options.cancelFileTransfer(connectionId)
      if (!session) {
        ElMessage.error(t('master.runtime.errors.cancelFileTransferFailed'))
        return
      }
      if (options.selectedSlave.value?.connectionId === connectionId) {
        options.fileTransferSession.value = session
      }
      ElMessage.warning(t('master.runtime.fileTransferCancelled'))
    } finally {
      options.fileTransferActionBusy.value = false
    }
  }

  async function handleGlobalCommand(command: GlobalCommandType) {
    if (!options.selectedSlave.value) {
      ElMessage.warning(t('master.runtime.warnings.selectSlaveFirst'))
      return
    }

    const selected = options.selectedSlave.value
    const connectionId = selected.connectionId

    if (command === 'toggle-data-transfer') {
      if (!connectionId) {
        ElMessage.warning(t('master.runtime.warnings.slaveNotConnectedStartStop'))
        return
      }
      const runtimeState = options.resolveRuntimeLinkStateByConnectionId(connectionId)
      if (runtimeState.transport !== 'connected') {
        ElMessage.warning(t('master.runtime.warnings.tcpNotConnectedStartLink'))
        return
      }
      if (runtimeState.link === 'starting' || runtimeState.link === 'stopping') {
        ElMessage.info(t('master.runtime.warnings.linkSwitching'))
        return
      }

      if (runtimeState.link === 'started') {
        const ok = await options.stopDataTransfer(connectionId)
        if (ok) {
          await options.syncFromBackend()
          ElMessage.success(t('master.runtime.success.stopDataTransferSent'))
        } else {
          ElMessage.error(t('master.runtime.errors.stopDataTransferFailed'))
        }
        return
      }

      const ok = await options.startDataTransfer(connectionId)
      if (ok) {
        await options.syncFromBackend()
        ElMessage.success(t('master.runtime.success.startDataTransferSent'))
      } else {
        ElMessage.error(t('master.runtime.errors.startDataTransferFailed'))
      }
      return
    }

    if (!connectionId) {
      ElMessage.warning(t('master.runtime.warnings.slaveNotConnectedMenu'))
      return
    }

    if (command === 'test-frame') {
      const runtimeState = options.resolveRuntimeLinkStateByConnectionId(connectionId)
      if (runtimeState.transport !== 'connected') {
        ElMessage.warning(t('master.runtime.warnings.tcpNotConnectedTestFrame'))
        return
      }
      options.registerManualRuntimeHintToast(connectionId, 'test-frame-confirmed')
      if (await options.sendTestFrame(connectionId)) {
        ElMessage.success(t('master.runtime.success.testFrameSent'))
      } else {
        options.unregisterManualRuntimeHintToast(connectionId, 'test-frame-confirmed')
        ElMessage.error(t('master.runtime.errors.testFrameFailed'))
      }
      return
    }

    if (command === 'open-remote-control') {
      options.handleSendCommand('single-command', {
        address: 1,
        value: 0,
        targetSlaveId: connectionId,
        slaveId: selected.slaveId,
        name: selected.name,
      })
      return
    }

    if (command === 'open-remote-adjust') {
      options.handleSendCommand('set-point', {
        address: 1,
        value: 0,
        setpointType: 'short-float',
        targetSlaveId: connectionId,
        slaveId: selected.slaveId,
        name: selected.name,
      })
      return
    }

    if (command === 'open-file-transfer') {
      if (
        !isBusinessLinkReadyState(options.resolveRuntimeLinkStateByConnectionId(connectionId).link)
      ) {
        ElMessage.warning(t('master.runtime.warnings.linkNotStarted'))
        return
      }
      options.showFileTransferDialog.value = true
      void refreshFileTransferSession({ includeTerminal: false }).catch((error) => {
        console.error('refresh file transfer session failed:', error)
      })
      return
    }

    if (
      !isBusinessLinkReadyState(options.resolveRuntimeLinkStateByConnectionId(connectionId).link)
    ) {
      ElMessage.warning(t('master.runtime.warnings.linkNotStarted'))
      return
    }

    switch (command) {
      case 'general-interrogation':
        options.loadingCommands.gi = true
        options.registerManualRuntimeHintToast(connectionId, 'command-confirmed')
        try {
          const ok = await options.sendGeneralInterrogation(
            { kind: 'station' },
            connectionId,
            selected.slaveId,
          )
          if (ok) {
            ElMessage.success(t('master.runtime.success.generalInterrogationSent'))
          } else {
            options.unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
            ElMessage.error(t('master.runtime.errors.generalInterrogationFailed'))
          }
        } finally {
          options.loadingCommands.gi = false
        }
        break
      case 'counter-interrogation':
        options.loadingCommands.ci = true
        options.registerManualRuntimeHintToast(connectionId, 'command-confirmed')
        try {
          const ok = await options.sendCounterInterrogation(connectionId, selected.slaveId)
          if (ok) {
            ElMessage.success(t('master.runtime.success.counterInterrogationSent'))
          } else {
            options.unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
            ElMessage.error(t('master.runtime.errors.counterInterrogationFailed'))
          }
        } finally {
          options.loadingCommands.ci = false
        }
        break
      case 'counter-freeze':
        options.loadingCommands.cf = true
        options.registerManualRuntimeHintToast(connectionId, 'command-confirmed')
        try {
          const ok = await options.sendCounterFreeze(connectionId, selected.slaveId)
          if (ok) {
            ElMessage.success(t('master.runtime.success.counterFreezeSent'))
          } else {
            options.unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
            ElMessage.error(t('master.runtime.errors.counterFreezeFailed'))
          }
        } finally {
          options.loadingCommands.cf = false
        }
        break
      case 'clock-synchronization':
        options.loadingCommands.cs = true
        options.registerManualRuntimeHintToast(connectionId, 'command-confirmed')
        try {
          const ok = await options.sendClockSynchronization(connectionId, selected.slaveId)
          if (ok) {
            ElMessage.success(t('master.runtime.success.clockSyncSent'))
          } else {
            options.unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
            ElMessage.error(t('master.runtime.errors.clockSyncFailed'))
          }
        } finally {
          options.loadingCommands.cs = false
        }
        break
      case 'reset-process-command':
        options.loadingCommands.rp = true
        options.registerManualRuntimeHintToast(connectionId, 'command-confirmed')
        try {
          const ok = await options.sendResetProcessCommand(
            connectionId,
            selected.slaveId,
            options.resetProcessMode.value,
          )
          if (ok) {
            await options.syncFromBackend()
            ElMessage.success(t('master.runtime.success.resetProcessSent'))
          } else {
            options.unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
            ElMessage.error(t('master.runtime.errors.resetProcessFailed'))
          }
        } finally {
          options.loadingCommands.rp = false
        }
        break
      case 'test-command':
        options.registerManualRuntimeHintToast(connectionId, 'command-confirmed')
        if (await options.sendTestCommand(connectionId, selected.slaveId)) {
          ElMessage.success(t('master.runtime.success.testCommandSent'))
        } else {
          options.unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
          ElMessage.error(t('master.runtime.errors.testCommandFailed'))
        }
        break
      default:
        options.addLog('warning', t('master.runtime.logs.unknownGlobalCommand', { command }))
    }
  }

  const handleRefreshData = () => {
    void options.syncFromBackend()
    options.addLog('info', t('master.runtime.logs.refreshData'))
  }

  const openReconnectSettings = async () => {
    const linkProfileId =
      options.selectedSlave.value?.linkProfileId ?? options.selectedLinkProfileId.value
    if (linkProfileId == null) {
      ElMessage.warning(t('master.runtime.warnings.selectSlaveForLink'))
      return
    }
    options.openEditLinkDialog(linkProfileId)
  }

  return {
    refreshFileTransferSession,
    handleConnectionAction,
    handleGlobalCommand,
    handleLinkOperation,
    handleProfileOperation,
    handleRefreshData,
    handleSlaveConnect,
    handleSlaveDisconnect,
    handleStartFileTransfer,
    handleCancelFileTransfer,
  }
}
