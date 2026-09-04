import { computed, ref, type Ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'

import type {
  BackendDataPoint,
  BackendMasterMessage,
  CounterFreezeMode,
  InterrogationQualifier,
} from '@shared/api/types'
import {
  formatIec104TypeLabel,
  formatPointQualityLabelLocalized,
  formatSemanticPointValueLocalized,
} from '@shared/api/iec104'
import { resolveEffectiveQualityDetail } from '@shared/api/protocolQuality'
import { currentLocale, t } from '@shared/i18n'
import type { ControlCommandType, GlobalCommandType, SlaveConnection } from '../types/master'
import type { MasterSyncTask } from './useMasterSync'
import type { MasterPointData } from './useMasterPointData'

type CommandDialogTarget = {
  connectionId: string
  slaveId: number
  commonAddress?: number
}

interface UseMasterCommandDialogsOptions {
  messages: Ref<BackendMasterMessage[]>
  pointData: MasterPointData
  selectedSlave: Ref<SlaveConnection | null>
  slaveProfiles: Ref<SlaveConnection[]>
  loadingCommands: { gi: boolean }
  syncFromBackend: (resources?: MasterSyncTask[]) => Promise<unknown>
  sendGeneralInterrogation: (...args: any[]) => Promise<boolean>
  sendCounterCommand: (...args: any[]) => Promise<boolean>
  sendControlCommandByType: (...args: any[]) => Promise<{
    accepted: boolean
    traceId?: string | null
    errorMessage?: string | null
  }>
  registerManualRuntimeHintToast: (connectionId: string, hint: string) => void
  unregisterManualRuntimeHintToast: (connectionId: string, hint: string) => void
  handleRuntimeGlobalCommand: (command: GlobalCommandType) => void | Promise<void>
}

export function useMasterCommandDialogs(options: UseMasterCommandDialogsOptions) {
  const {
    messages,
    pointData,
    selectedSlave,
    slaveProfiles,
    loadingCommands,
    syncFromBackend,
    sendGeneralInterrogation,
    sendCounterCommand,
    sendControlCommandByType,
    registerManualRuntimeHintToast,
    unregisterManualRuntimeHintToast,
    handleRuntimeGlobalCommand,
  } = options

  function resolveCommandDialogSubtitleBadges(target: CommandDialogTarget | null): string[] {
    if (!target) return []
    const slave = slaveProfiles.value.find(
      (item) => item.connectionId === target.connectionId && item.slaveId === target.slaveId,
    )
    return [
      slave?.linkName || t('master.appDialogs.control.badges.unknownConnection'),
      slave?.name || t('master.appDialogs.control.badges.noTargetSlave'),
    ]
  }

  type ControlActionStage = 'select' | 'execute' | 'cancel'
  type CommandWaitStage = ControlActionStage | 'read'

  function findCommandStateByTrace(traceId: string): string | null {
    for (let idx = messages.value.length - 1; idx >= 0; idx--) {
      const msg = messages.value[idx]
      if (
        msg.command_trace_id === traceId &&
        msg.command_state &&
        msg.command_state !== 'dispatch'
      ) {
        return msg.command_state
      }
    }
    return null
  }

  function isTerminalCommandStateForStage(state: string, stage: CommandWaitStage): boolean {
    if (stage === 'read') {
      return (
        state === 'request-response' ||
        state === 'unknown-information-object' ||
        state === 'activation-negative' ||
        state === 'timeout'
      )
    }
    if (stage === 'select') {
      return (
        state === 'selection-confirmation' ||
        state === 'activation-negative' ||
        state === 'timeout' ||
        state === 'activation-termination'
      )
    }
    if (stage === 'cancel') {
      return (
        state === 'deactivation-confirmation' ||
        state === 'activation-negative' ||
        state === 'timeout'
      )
    }
    return (
      state === 'activation-termination' ||
      state === 'completion-on-confirmation' ||
      state === 'activation-negative' ||
      state === 'timeout'
    )
  }

  async function waitCommandStateByTrace(
    traceId: string,
    stage: CommandWaitStage,
    timeoutMs = 6000,
  ): Promise<string | null> {
    const deadline = Date.now() + timeoutMs
    while (Date.now() <= deadline) {
      const state = findCommandStateByTrace(traceId)
      if (state && isTerminalCommandStateForStage(state, stage)) return state
      await syncFromBackend(['messages'])
      const afterSyncState = findCommandStateByTrace(traceId)
      if (afterSyncState && isTerminalCommandStateForStage(afterSyncState, stage))
        return afterSyncState
      await new Promise((resolve) => setTimeout(resolve, 150))
    }
    return null
  }

  function isCommandStateSuccessForStage(state: string, stage: ControlActionStage): boolean {
    if (stage === 'cancel') return state === 'deactivation-confirmation'
    if (stage === 'select') return state === 'selection-confirmation'
    return state === 'activation-termination' || state === 'completion-on-confirmation'
  }

  function mapFailureMessageByState(
    command: ControlCommandType,
    stage: ControlActionStage,
    state: string | null,
  ): string {
    const stageLabel =
      stage === 'select'
        ? t('master.appDialogs.control.messages.stageSelect')
        : stage === 'cancel'
          ? t('master.appDialogs.control.messages.stageCancel')
          : t('master.appDialogs.control.messages.stageExecute')
    const params = { command, stage: stageLabel, state: state ?? '' }
    if (state === 'timeout') return t('master.appDialogs.control.messages.commandTimeout', params)
    if (state === 'activation-negative')
      return t('master.appDialogs.control.messages.commandRejected', params)
    if (state === 'activation-termination')
      return t('master.appDialogs.control.messages.commandTerminated', params)
    if (state) return t('master.appDialogs.control.messages.commandFailedWithState', params)
    return t('master.appDialogs.control.messages.commandNoReply', params)
  }

  const showCounterCommandDialog = ref(false)
  const counterCommandRequest = ref(1)
  const counterCommandAction = ref<CounterFreezeMode>('read')
  const counterCommandBusy = ref(false)
  const counterCommandTarget = ref<CommandDialogTarget | null>(null)
  const counterCommandSubtitleBadges = computed(() =>
    resolveCommandDialogSubtitleBadges(counterCommandTarget.value),
  )

  const showGeneralInterrogationDialog = ref(false)
  const generalInterrogationGroup = ref(0)
  const generalInterrogationTarget = ref<CommandDialogTarget | null>(null)
  const generalInterrogationSubtitleBadges = computed(() => {
    const badges = resolveCommandDialogSubtitleBadges(generalInterrogationTarget.value)
    const commonAddress = generalInterrogationTarget.value?.commonAddress
    if (commonAddress != null) badges.push(`CA: ${commonAddress}`)
    return badges
  })

  function handleGlobalCommand(command: GlobalCommandType) {
    if (command === 'general-interrogation') {
      const target = selectedSlave.value
      if (!target?.connectionId) return
      generalInterrogationGroup.value = 0
      generalInterrogationTarget.value = {
        connectionId: target.connectionId,
        slaveId: target.slaveId,
        commonAddress: target.commonAddress,
      }
      showGeneralInterrogationDialog.value = true
      return
    }
    if (command === 'counter-interrogation' || command === 'counter-freeze') {
      const target = selectedSlave.value
      if (!target?.connectionId) return
      counterCommandRequest.value = 1
      counterCommandAction.value = command === 'counter-freeze' ? 'freeze' : 'read'
      counterCommandTarget.value = {
        connectionId: target.connectionId,
        slaveId: target.slaveId,
      }
      showCounterCommandDialog.value = true
      return
    }
    if (command === 'open-read-command') {
      openReadCommandDialog()
      return
    }
    void handleRuntimeGlobalCommand(command)
  }

  async function submitGeneralInterrogation() {
    const target = generalInterrogationTarget.value
    if (!target) return
    const group = Math.trunc(generalInterrogationGroup.value)
    const qoi: InterrogationQualifier =
      group >= 1 && group <= 16 ? { kind: 'group', group } : { kind: 'station' }
    loadingCommands.gi = true
    registerManualRuntimeHintToast(target.connectionId, 'command-confirmed')
    try {
      const ok = await sendGeneralInterrogation(qoi, target.connectionId, target.slaveId)
      if (ok) {
        showGeneralInterrogationDialog.value = false
        ElMessage.success(t('master.appDialogs.generalInterrogation.sent'))
      } else {
        unregisterManualRuntimeHintToast(target.connectionId, 'command-confirmed')
        ElMessage.error(t('master.appDialogs.generalInterrogation.failed'))
      }
    } catch (error) {
      unregisterManualRuntimeHintToast(target.connectionId, 'command-confirmed')
      ElMessage.error(String(error))
    } finally {
      loadingCommands.gi = false
    }
  }

  async function submitCounterCommand() {
    const target = counterCommandTarget.value
    if (!target) return
    if (
      counterCommandAction.value === 'freeze-and-reset' ||
      counterCommandAction.value === 'reset'
    ) {
      try {
        await ElMessageBox.confirm(
          t('master.appDialogs.counter.destructiveConfirm'),
          t('master.appDialogs.counter.destructiveTitle'),
          { type: 'warning' },
        )
      } catch {
        return
      }
    }
    counterCommandBusy.value = true
    registerManualRuntimeHintToast(target.connectionId, 'command-confirmed')
    try {
      const ok = await sendCounterCommand(
        counterCommandRequest.value,
        counterCommandAction.value,
        target.connectionId,
        target.slaveId,
      )
      if (ok) {
        showCounterCommandDialog.value = false
        ElMessage.success(t('master.appDialogs.counter.sent'))
      } else {
        unregisterManualRuntimeHintToast(target.connectionId, 'command-confirmed')
        ElMessage.error(t('master.appDialogs.counter.failed'))
      }
    } catch (error) {
      unregisterManualRuntimeHintToast(target.connectionId, 'command-confirmed')
      if (error !== 'cancel' && error !== 'close') ElMessage.error(String(error))
    } finally {
      counterCommandBusy.value = false
    }
  }

  const showBitStringDialog = ref(false)
  const bitStringIoa = ref(0)
  const bitStringInput = ref('0x00000000')
  const bitStringDecimal = ref<number | null>(0)
  const bitStringBusy = ref(false)
  const bitStringParams = ref<any>(null)
  const bitStringSubtitleBadges = computed(() =>
    resolveCommandDialogSubtitleBadges(
      bitStringParams.value
        ? {
            connectionId: String(bitStringParams.value.targetSlaveId ?? ''),
            slaveId: Number(bitStringParams.value.slaveId ?? 0),
          }
        : null,
    ),
  )

  function parseBitStringInput(): number | null {
    const value = bitStringInput.value.trim()
    if (!/^(?:0[xX][0-9a-fA-F]{1,8}|[0-9]{1,10})$/.test(value)) return null
    const parsed = Number.parseInt(value, value.toLowerCase().startsWith('0x') ? 16 : 10)
    return Number.isSafeInteger(parsed) && parsed >= 0 && parsed <= 0xffff_ffff ? parsed : null
  }

  function normalizeBitStringInput() {
    const parsed = parseBitStringInput()
    bitStringDecimal.value = parsed
    if (parsed != null)
      bitStringInput.value = `0x${parsed.toString(16).toUpperCase().padStart(8, '0')}`
  }

  async function submitBitStringCommand() {
    const value = parseBitStringInput()
    if (value == null) {
      ElMessage.warning(t('master.appDialogs.bitString.invalid'))
      return
    }
    bitStringBusy.value = true
    const connectionId = bitStringParams.value?.targetSlaveId
    registerManualRuntimeHintToast(connectionId, 'command-confirmed')
    try {
      const result = await sendControlCommandByType(
        'bit-string-command',
        bitStringIoa.value,
        value,
        {
          connectionId: bitStringParams.value?.targetSlaveId,
          slaveId: bitStringParams.value?.slaveId,
          typeId: bitStringParams.value?.typeId,
        },
      )
      if (result.accepted) {
        showBitStringDialog.value = false
        ElMessage.success(t('master.appDialogs.bitString.sent'))
      } else {
        unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
        ElMessage.error(result.errorMessage ?? t('master.appDialogs.bitString.failed'))
      }
    } catch (error) {
      unregisterManualRuntimeHintToast(connectionId, 'command-confirmed')
      ElMessage.error(String(error))
    } finally {
      bitStringBusy.value = false
    }
  }

  const showReadCommandDialog = ref(false)
  const readCommandIoa = ref(0)
  const readCommandBusy = ref(false)
  const readCommandParams = ref<any>(null)
  const readCommandTraceId = ref<string | null>(null)
  const readCommandResult = ref<{
    status: 'idle' | 'pending' | 'success' | 'error'
    message: string
    point: BackendDataPoint | null
  }>({
    status: 'idle',
    message: '',
    point: null,
  })
  const readCommandSubtitleBadges = computed(() =>
    resolveCommandDialogSubtitleBadges(
      readCommandParams.value
        ? {
            connectionId: String(readCommandParams.value.targetSlaveId ?? ''),
            slaveId: Number(readCommandParams.value.slaveId ?? 0),
          }
        : null,
    ),
  )
  const readCommandSubmitLabel = computed(() =>
    readCommandResult.value.status === 'idle'
      ? t('master.appDialogs.readCommand.send')
      : t('master.appDialogs.readCommand.retry'),
  )
  const readCommandResponseType = computed(() =>
    readCommandResult.value.point
      ? formatIec104TypeLabel(readCommandResult.value.point.type_id)
      : '—',
  )
  const readCommandResponseValue = computed(() =>
    readCommandResult.value.point
      ? formatSemanticPointValueLocalized(
          readCommandResult.value.point.type_id,
          readCommandResult.value.point.value,
        )
      : '—',
  )
  const readCommandResponseQuality = computed(() =>
    readCommandResult.value.point
      ? formatPointQualityLabelLocalized(
          readCommandResult.value.point.type_id,
          readCommandResult.value.point.quality_common,
          readCommandResult.value.point.quality,
          resolveEffectiveQualityDetail(
            readCommandResult.value.point.type_id,
            readCommandResult.value.point.quality_detail,
            readCommandResult.value.point.quality,
            readCommandResult.value.point.value,
          ),
        )
      : '—',
  )
  const readCommandResponseTimestamp = computed(() => {
    const timestamp = readCommandResult.value.point?.timestamp
    if (!timestamp) return '—'
    const parsed = new Date(timestamp)
    return Number.isNaN(parsed.getTime()) ? timestamp : parsed.toLocaleString(currentLocale.value)
  })

  function clearReadCommandResult() {
    if (readCommandBusy.value) return
    readCommandTraceId.value = null
    readCommandResult.value = { status: 'idle', message: '', point: null }
  }

  function findReadResponsePoint(): BackendDataPoint | null {
    const connectionId = String(readCommandParams.value?.targetSlaveId ?? '').trim()
    const slaveId = Number(readCommandParams.value?.slaveId ?? 0)
    const commonAddress = Number(readCommandParams.value?.commonAddress ?? 0)
    const address = Math.trunc(Number(readCommandIoa.value))
    return pointData.findPoint(
      (point) =>
        point.address === address &&
        (!connectionId || point.connection_id === connectionId) &&
        (!(slaveId > 0) || Number(point.slave_id ?? 0) === slaveId) &&
        (!(commonAddress > 0) || Number(point.common_address ?? 0) === commonAddress),
    )
  }

  async function loadReadResponsePoint(): Promise<BackendDataPoint | null> {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      if (attempt > 0) await new Promise((resolve) => setTimeout(resolve, 100))
      await syncFromBackend(['points'])
      const point = findReadResponsePoint()
      if (point) return point
    }
    return null
  }

  function openReadCommandDialog(params: any = {}) {
    const target = selectedSlave.value
    readCommandIoa.value = Number(params.address ?? 0)
    readCommandParams.value = {
      targetSlaveId: params.targetSlaveId ?? target?.connectionId,
      slaveId: Number(params.slaveId ?? target?.slaveId ?? 0),
      commonAddress: Number(params.commonAddress ?? target?.commonAddress ?? 0),
    }
    clearReadCommandResult()
    showReadCommandDialog.value = true
  }

  async function submitReadCommand() {
    clearReadCommandResult()
    readCommandBusy.value = true
    readCommandResult.value = {
      status: 'pending',
      message: t('master.appDialogs.readCommand.waiting'),
      point: null,
    }
    try {
      const result = await sendControlCommandByType('read-command', readCommandIoa.value, 0, {
        connectionId: readCommandParams.value?.targetSlaveId,
        slaveId: readCommandParams.value?.slaveId,
      })
      if (!result.accepted) {
        readCommandResult.value = {
          status: 'error',
          message: result.errorMessage ?? t('master.appDialogs.readCommand.failed'),
          point: null,
        }
        return
      }
      if (!result.traceId) {
        readCommandResult.value = {
          status: 'error',
          message: t('master.appDialogs.readCommand.trackingUnavailable'),
          point: null,
        }
        return
      }
      readCommandTraceId.value = result.traceId
      const state = await waitCommandStateByTrace(result.traceId, 'read')
      if (state === 'request-response') {
        const point = await loadReadResponsePoint()
        readCommandResult.value = point
          ? {
              status: 'success',
              message: t('master.appDialogs.readCommand.succeeded'),
              point,
            }
          : {
              status: 'error',
              message: t('master.appDialogs.readCommand.valueUnavailable'),
              point: null,
            }
        return
      }
      readCommandResult.value = {
        status: 'error',
        message:
          state === 'unknown-information-object'
            ? t('master.appDialogs.readCommand.unknownIoa')
            : state === 'activation-negative'
              ? t('master.appDialogs.readCommand.rejected')
              : t('master.appDialogs.readCommand.timeout'),
        point: null,
      }
    } catch (error) {
      readCommandResult.value = { status: 'error', message: String(error), point: null }
    } finally {
      readCommandBusy.value = false
    }
  }

  return {
    showCounterCommandDialog,
    counterCommandRequest,
    counterCommandAction,
    counterCommandBusy,
    counterCommandSubtitleBadges,
    showGeneralInterrogationDialog,
    generalInterrogationGroup,
    generalInterrogationSubtitleBadges,
    handleGlobalCommand,
    submitGeneralInterrogation,
    submitCounterCommand,
    showBitStringDialog,
    bitStringIoa,
    bitStringInput,
    bitStringDecimal,
    bitStringBusy,
    bitStringParams,
    bitStringSubtitleBadges,
    normalizeBitStringInput,
    submitBitStringCommand,
    showReadCommandDialog,
    readCommandIoa,
    readCommandBusy,
    readCommandParams,
    readCommandTraceId,
    readCommandResult,
    readCommandSubtitleBadges,
    readCommandSubmitLabel,
    readCommandResponseType,
    readCommandResponseValue,
    readCommandResponseQuality,
    readCommandResponseTimestamp,
    clearReadCommandResult,
    openReadCommandDialog,
    submitReadCommand,
    waitCommandStateByTrace,
    isCommandStateSuccessForStage,
    mapFailureMessageByState,
  }
}
