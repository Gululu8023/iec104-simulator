import { computed, ref, type Ref } from 'vue'
import { ElMessage } from 'element-plus'

import type { SelectExecuteMode } from '@shared/api/types'
import { formatIec104TypeLabel, iec104TypeNameToId } from '@shared/api/iec104'
import { t } from '@shared/i18n'
import { isBusinessLinkReadyState } from '@shared/ui/connectionStatus'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'
import type { ControlCommandType, SlaveConnection } from '../types/master'

type ControlActionStage = 'select' | 'execute' | 'cancel'
type CommandDispatchResult = {
  accepted: boolean
  traceId?: string | null
  errorMessage?: string | null
}

interface UseMasterControlCommandOptions {
  selectedSlave: Ref<SlaveConnection | null>
  resolveRuntimeLinkStateByConnectionId: (connectionId: string | null | undefined) => any
  sendControlCommandByType: (
    command: ControlCommandType,
    address: number,
    value: any,
    options: Record<string, unknown>,
  ) => Promise<CommandDispatchResult>
  waitCommandStateByTrace: (traceId: string, stage: ControlActionStage) => Promise<string | null>
  isCommandStateSuccessForStage: (state: string, stage: ControlActionStage) => boolean
  mapFailureMessageByState: (
    command: ControlCommandType,
    stage: ControlActionStage,
    state: string | null,
  ) => string
  addLog: (level: string, message: string) => void
  bitStringParams: Ref<any>
  bitStringIoa: Ref<number>
  bitStringInput: Ref<string>
  normalizeBitStringInput: () => void
  showBitStringDialog: Ref<boolean>
  openReadCommandDialog: (params?: any) => void
}

export function useMasterControlCommand(options: UseMasterControlCommandOptions) {
  const {
    selectedSlave,
    resolveRuntimeLinkStateByConnectionId,
    sendControlCommandByType,
    waitCommandStateByTrace,
    isCommandStateSuccessForStage,
    mapFailureMessageByState,
    addLog,
    bitStringParams,
    bitStringIoa,
    bitStringInput,
    normalizeBitStringInput,
    showBitStringDialog,
    openReadCommandDialog,
  } = options

  const controlConfirmSubtitleBadges = computed(() => {
    const badges: string[] = []
    badges.push(
      selectedSlave.value?.linkName || t('master.appDialogs.control.badges.unknownConnection'),
    )
    badges.push(selectedSlave.value?.name || t('master.appDialogs.control.badges.noTargetSlave'))
    return badges
  })
  const controlConfirmDialogSnapshot = ref('')
  const controlCommandBusy = ref(false)

  // 遥控二步确认状态
  const controlConfirmState = ref<{
    step: 'idle' | 'select' | 'confirm' | 'execute' | 'finish'
    command: ControlCommandType | null
    params: any
  }>({
    step: 'idle',
    command: null,
    params: null,
  })
  const controlDispatchMode = ref<'auto-sbo' | 'manual' | 'direct'>('auto-sbo')
  const controlQualifierMode = ref<'standard' | 'raw_test'>('standard')
  const controlExecuteResult = ref<{
    status: 'success' | 'error' | null
    message: string
    stage: ControlActionStage | null
  }>({
    status: null,
    message: '',
    stage: null,
  })

  // 遥控确认对话框
  const showControlConfirmDialog = ref(false)
  const controlConfirmAddress = computed<number>({
    get() {
      const parsed = Number(controlConfirmState.value.params?.address)
      if (!Number.isFinite(parsed)) return 1
      return Math.max(0, Math.trunc(parsed))
    },
    set(nextValue) {
      const parsed = Number(nextValue)
      const normalized = Number.isFinite(parsed) ? Math.max(0, Math.trunc(parsed)) : 1
      if (!controlConfirmState.value.params) {
        controlConfirmState.value.params = {}
      }
      controlConfirmState.value.params.address = normalized
    },
  })

  // ── 遥控确认 Dialog: computed helpers ──
  function typeLabel(typeName: string): string {
    return formatIec104TypeLabel(iec104TypeNameToId(typeName))
  }

  const asduTypeOptions = computed(() => {
    const cmd = controlConfirmState.value.command
    if (cmd === 'single-command' || cmd === 'double-command' || cmd === 'regulating-step') {
      return [
        { value: '45', label: typeLabel('C_SC_NA_1') },
        { value: '46', label: typeLabel('C_DC_NA_1') },
        { value: '47', label: typeLabel('C_RC_NA_1') },
        { value: '58', label: typeLabel('C_SC_TA_1') },
        { value: '59', label: typeLabel('C_DC_TA_1') },
        { value: '60', label: typeLabel('C_RC_TA_1') },
      ]
    } else if (String(cmd).startsWith('set-point')) {
      return [
        { value: '48', label: typeLabel('C_SE_NA_1') },
        { value: '49', label: typeLabel('C_SE_NB_1') },
        { value: '50', label: typeLabel('C_SE_NC_1') },
        { value: '61', label: typeLabel('C_SE_TA_1') },
        { value: '62', label: typeLabel('C_SE_TB_1') },
        { value: '63', label: typeLabel('C_SE_TC_1') },
      ]
    }
    return []
  })

  const currentAsduType = computed({
    get() {
      const cmd = controlConfirmState.value.command
      const configured = Number(controlConfirmState.value.params?.typeId)
      if (configured > 0) return String(configured)
      if (cmd === 'single-command') return '45'
      if (cmd === 'double-command') return '46'
      if (cmd === 'regulating-step') return '47'
      if (cmd === 'set-point') return '50'
      return ''
    },
    set(val: string) {
      if (!controlConfirmState.value.params) {
        controlConfirmState.value.params = {}
      }
      const typeId = Number(val)
      controlConfirmState.value.params.typeId = typeId
      if ([48, 49, 50, 61, 62, 63].includes(typeId)) {
        controlConfirmState.value.command = 'set-point'
        controlConfirmState.value.params.setpointType = [48, 61].includes(typeId)
          ? 'normalized'
          : [49, 62].includes(typeId)
            ? 'scaled'
            : 'short-float'
        controlConfirmState.value.params.ql = normalizeSetpointQualifier(
          controlConfirmState.value.params.ql,
        )
        delete controlConfirmState.value.params.qu
      } else {
        controlConfirmState.value.command = [45, 58].includes(typeId)
          ? 'single-command'
          : [46, 59].includes(typeId)
            ? 'double-command'
            : 'regulating-step'
        controlConfirmState.value.params.qu = normalizeDiscreteQualifier(
          controlConfirmState.value.params.qu,
        )
        delete controlConfirmState.value.params.ql
        // Default values when switching discrete command type
        if ([45, 58].includes(typeId)) {
          controlConfirmState.value.params.value = 0 // default OFF
        } else if ([46, 59].includes(typeId)) {
          controlConfirmState.value.params.value = 1 // default OFF
        } else if ([47, 60].includes(typeId)) {
          controlConfirmState.value.params.value = 1 // default LOWER
        }
      }
    },
  })

  const isDiscreteCommand = computed(() => {
    const cmd = controlConfirmState.value.command
    return cmd === 'single-command' || cmd === 'double-command'
  })
  const isDoubleCommand = computed(() => controlConfirmState.value.command === 'double-command')
  const isStepCommand = computed(() => controlConfirmState.value.command === 'regulating-step')

  // 设定值子类型
  const currentSetpointType = computed({
    get: () => controlConfirmState.value.params?.setpointType || 'short-float',
    set: (val: string) => {
      if (controlConfirmState.value.params) {
        controlConfirmState.value.params.setpointType = val
      }
    },
  })

  // 设定值范围参数（P13: 范围校验）
  const setpointMin = computed(() => {
    const sub = currentSetpointType.value
    if (sub === 'normalized') return -1
    if (sub === 'scaled') return -32768
    return -3.4e38
  })
  const setpointMax = computed(() => {
    const sub = currentSetpointType.value
    if (sub === 'normalized') return 32767 / 32768
    if (sub === 'scaled') return 32767
    return 3.4e38
  })
  const setpointStep = computed(() => {
    const sub = currentSetpointType.value
    if (sub === 'normalized') return 0.001
    if (sub === 'scaled') return 1
    return 0.01
  })
  const setpointPrecision = computed(() => {
    const sub = currentSetpointType.value
    if (sub === 'normalized') return 4
    if (sub === 'scaled') return 0
    return 6
  })

  const normalizeDiscreteQualifier = (value: unknown): number => {
    const parsed = Number(value)
    if (!Number.isFinite(parsed)) return 0
    return Math.min(
      controlQualifierMode.value === 'raw_test' ? 31 : 3,
      Math.max(0, Math.trunc(parsed)),
    )
  }

  const normalizeSetpointQualifier = (value: unknown): number => {
    const parsed = Number(value)
    if (!Number.isFinite(parsed)) return 0
    return Math.min(127, Math.max(0, Math.trunc(parsed)))
  }

  const controlQualifier = computed({
    get: () => {
      if (isSetpointType.value) {
        return normalizeSetpointQualifier(controlConfirmState.value.params?.ql)
      }
      return normalizeDiscreteQualifier(controlConfirmState.value.params?.qu)
    },
    set: (val: number) => {
      if (controlConfirmState.value.params) {
        if (isSetpointType.value) {
          controlConfirmState.value.params.ql = normalizeSetpointQualifier(val)
        } else {
          controlConfirmState.value.params.qu = normalizeDiscreteQualifier(val)
        }
      }
    },
  })

  const isSetpointType = computed(() => controlConfirmState.value.command === 'set-point')

  const qualifierFieldLabel = computed(() => (isSetpointType.value ? 'QOS.QL' : 'QOC.QU'))

  const qualifierOptions = computed(() => {
    // 控制命令: QOC.QU 0~3
    return [
      { value: 0, label: t('master.appDialogs.control.qualifierOptions.default') },
      { value: 1, label: t('master.appDialogs.control.qualifierOptions.shortPulse') },
      { value: 2, label: t('master.appDialogs.control.qualifierOptions.longPulse') },
      { value: 3, label: t('master.appDialogs.control.qualifierOptions.persistent') },
    ]
  })

  /** 当前 SBO 步骤：auto 模式跟踪 controlConfirmState.step；manual 模式也跟踪 step */
  const manualSboStep = computed(() => controlConfirmState.value.step)

  type ControlProgressStep = 'select' | 'confirm' | 'execute' | 'finish'
  type ControlProgressStatus = 'pending' | 'active' | 'done' | 'error'
  const controlProgressStepOrder: ControlProgressStep[] = ['select', 'confirm', 'execute', 'finish']

  const controlFailedProgressStep = computed<ControlProgressStep | null>(() => {
    if (controlExecuteResult.value.status !== 'error') return null
    if (
      controlExecuteResult.value.stage === 'execute' ||
      controlExecuteResult.value.stage === 'cancel'
    ) {
      return 'execute'
    }
    if (controlExecuteResult.value.stage === 'select') return 'select'
    return null
  })

  function getControlStepStatus(step: ControlProgressStep): ControlProgressStatus {
    const failedStep = controlFailedProgressStep.value
    if (failedStep) {
      if (failedStep === step) return 'error'
      return controlProgressStepOrder.indexOf(step) < controlProgressStepOrder.indexOf(failedStep)
        ? 'done'
        : 'pending'
    }

    if (step === 'select') {
      if (manualSboStep.value === 'select') return 'active'
      if (manualSboStep.value === 'execute' || manualSboStep.value === 'finish') return 'done'
      return 'pending'
    }

    if (step === 'confirm') {
      if (manualSboStep.value === 'execute' || manualSboStep.value === 'finish') return 'done'
      return 'pending'
    }

    if (step === 'execute') {
      if (manualSboStep.value === 'execute') return 'active'
      if (manualSboStep.value === 'finish' && controlExecuteResult.value.status === 'success')
        return 'done'
      return 'pending'
    }

    if (manualSboStep.value === 'finish' && controlExecuteResult.value.status === 'success')
      return 'done'
    return 'pending'
  }

  function controlStepClass(step: ControlProgressStep) {
    const status = getControlStepStatus(step)
    return {
      active: status === 'active',
      done: status === 'done',
      error: status === 'error',
    }
  }

  function controlStepCircleLabel(step: ControlProgressStep, fallback: string): string {
    const status = getControlStepStatus(step)
    if (status === 'done') return '✓'
    if (status === 'error') return '×'
    return fallback
  }

  function isControlStepLineActive(step: Exclude<ControlProgressStep, 'finish'>): boolean {
    return getControlStepStatus(step) === 'done'
  }

  const controlConfirmButtonText = computed(() => {
    if (manualSboStep.value === 'finish') return t('master.appDialogs.control.buttons.finish')
    if (controlDispatchMode.value === 'auto-sbo') return t('master.appDialogs.control.buttons.send')
    if (controlDispatchMode.value === 'direct')
      return t('master.appDialogs.control.buttons.execute')
    return manualSboStep.value === 'execute'
      ? t('master.appDialogs.control.buttons.execute')
      : t('master.appDialogs.control.buttons.select')
  })
  const controlCancelButtonText = computed(() => {
    if (controlDispatchMode.value === 'manual' && manualSboStep.value === 'execute')
      return t('master.appDialogs.control.buttons.revoke')
    return t('master.appDialogs.control.buttons.cancel')
  })

  function clearControlExecuteResult() {
    controlExecuteResult.value = { status: null, message: '', stage: null }
  }

  function buildControlConfirmDialogSnapshot() {
    return JSON.stringify({
      command: controlConfirmState.value.command,
      step: controlConfirmState.value.step,
      params: controlConfirmState.value.params ?? null,
      dispatchMode: controlDispatchMode.value,
      qualifierMode: controlQualifierMode.value,
      qualifier: controlQualifier.value,
    })
  }

  function hasUnsavedControlConfirmChanges() {
    if (!showControlConfirmDialog.value) return false
    if (!controlConfirmDialogSnapshot.value) return false
    if (controlExecuteResult.value.status !== null) return false
    if (controlConfirmState.value.step === 'finish' || controlConfirmState.value.step === 'idle') {
      return false
    }
    return buildControlConfirmDialogSnapshot() !== controlConfirmDialogSnapshot.value
  }

  const controlFlowActive = computed(
    () =>
      controlCommandBusy.value ||
      (controlConfirmState.value.step === 'execute' && controlExecuteResult.value.status === null),
  )

  const {
    handleBeforeClose: handleControlConfirmBeforeClose,
    requestClose: requestCloseControlConfirm,
  } = useDialogCloseGuard({
    isDirty: hasUnsavedControlConfirmChanges,
    isBlocked: () => controlFlowActive.value,
    onClose: resetControlState,
  })

  function finishControlExecution(
    status: 'success' | 'error',
    message: string,
    stage: ControlActionStage | null = null,
  ) {
    controlExecuteResult.value = { status, message, stage }
    if (status === 'success') {
      controlConfirmState.value.step = 'finish'
    }
  }

  function isBusinessLinkReady(connectionId: string | null | undefined): boolean {
    return isBusinessLinkReadyState(resolveRuntimeLinkStateByConnectionId(connectionId).link)
  }

  function handleSendCommand(command: ControlCommandType, params?: any) {
    const targetConnectionId = params?.targetSlaveId ?? selectedSlave.value?.connectionId
    const targetSlaveId = Number(params?.slaveId ?? selectedSlave.value?.slaveId ?? 0)
    if (!targetConnectionId) {
      ElMessage.warning(t('master.appDialogs.control.messages.notConnected'))
      return
    }
    if (!isBusinessLinkReady(targetConnectionId)) {
      ElMessage.warning(t('master.appDialogs.control.messages.linkNotStarted'))
      return
    }

    if (command === 'bit-string-command') {
      bitStringParams.value = {
        ...params,
        targetSlaveId: targetConnectionId,
        slaveId: targetSlaveId,
      }
      bitStringIoa.value = Number(params?.address ?? 0)
      bitStringInput.value = `0x${Math.max(0, Number(params?.value ?? 0))
        .toString(16)
        .toUpperCase()
        .padStart(8, '0')}`
      normalizeBitStringInput()
      showBitStringDialog.value = true
      return
    }
    if (command === 'read-command') {
      openReadCommandDialog({
        ...params,
        targetSlaveId: targetConnectionId,
        slaveId: targetSlaveId,
      })
      return
    }

    controlDispatchMode.value = 'auto-sbo'
    controlQualifierMode.value = 'standard'
    const normalizedParams = { ...params }
    if (command === 'set-point') {
      normalizedParams.setpointType = normalizedParams.setpointType ?? 'short-float'
      normalizedParams.ql = normalizeSetpointQualifier(normalizedParams.ql)
      delete normalizedParams.qu
    } else {
      normalizedParams.qu = normalizeDiscreteQualifier(normalizedParams.qu)
      delete normalizedParams.ql
    }

    // 始终重置状态并打开发送弹窗，防止连续点击无法打开的问题
    controlConfirmState.value = {
      step: 'select',
      command,
      params: {
        ...normalizedParams,
        targetSlaveId: targetConnectionId,
        slaveId: targetSlaveId,
      },
    }
    clearControlExecuteResult()
    controlConfirmDialogSnapshot.value = buildControlConfirmDialogSnapshot()
    showControlConfirmDialog.value = true
    addLog(
      'info',
      t('master.appDialogs.control.logs.confirmWaiting', {
        command,
        qualifier:
          command === 'set-point'
            ? `QOS.QL=${normalizedParams.ql}`
            : `QOC.QU=${normalizedParams.qu}`,
      }),
    )
  }

  // 确认执行遥控命令（SBO: Select -> Execute）
  async function runControlExecute() {
    if (!controlConfirmState.value.command) return
    let currentStep = controlConfirmState.value.step
    if (currentStep === 'idle') return
    if (currentStep === 'finish') {
      resetControlState()
      return
    }

    clearControlExecuteResult()

    const { command, params } = controlConfirmState.value
    const parsedAddress = Number(params?.address)
    const address = Number.isFinite(parsedAddress) ? Math.max(0, Math.trunc(parsedAddress)) : 1
    const value = params?.value ?? 1
    const targetSlaveId = params?.targetSlaveId ?? selectedSlave.value?.connectionId
    const slaveId = Number(params?.slaveId ?? selectedSlave.value?.slaveId ?? 0)
    if (!targetSlaveId) {
      ElMessage.warning(t('master.appDialogs.control.messages.notConnected'))
      resetControlState()
      return
    }
    if (!isBusinessLinkReady(targetSlaveId)) {
      ElMessage.warning(t('master.appDialogs.control.messages.linkNotStarted'))
      resetControlState()
      return
    }
    const buildOptions = (se: SelectExecuteMode) => ({
      connectionId: targetSlaveId,
      slaveId,
      se,
      qu: isSetpointType.value ? undefined : controlQualifier.value,
      setpointType: params?.setpointType,
      ql: isSetpointType.value ? controlQualifier.value : undefined,
      qualifierMode: controlQualifierMode.value,
      typeId: params?.typeId,
    })

    if (controlDispatchMode.value === 'direct' && currentStep === 'select') {
      currentStep = 'execute'
      controlConfirmState.value.step = 'execute'
    }

    // ── 自动模式：Select + Execute 一次完成 ──
    if (controlDispatchMode.value === 'auto-sbo') {
      controlConfirmState.value.step = 'execute'
      const selectDispatch = await sendControlCommandByType(
        command,
        address,
        value,
        buildOptions('select'),
      )
      if (!selectDispatch.accepted) {
        addLog('error', t('master.appDialogs.control.logs.selectFailed', { command, address }))
        const message =
          selectDispatch.errorMessage ??
          t('master.appDialogs.control.messages.commandSelectFailed', { command })
        finishControlExecution('error', message, 'select')
        return
      }
      if (selectDispatch.traceId) {
        const selectState = await waitCommandStateByTrace(selectDispatch.traceId, 'select')
        if (!selectState || !isCommandStateSuccessForStage(selectState, 'select')) {
          const message = mapFailureMessageByState(command, 'select', selectState)
          addLog(
            'error',
            t('master.appDialogs.control.logs.selectFailedState', {
              command,
              address,
              state: selectState ?? 'no-reply',
            }),
          )
          finishControlExecution('error', message, 'select')
          return
        }
      }

      await new Promise((resolve) => setTimeout(resolve, 120))

      const executeDispatch = await sendControlCommandByType(
        command,
        address,
        value,
        buildOptions('execute'),
      )
      if (!executeDispatch.accepted) {
        addLog('error', t('master.appDialogs.control.logs.executeFailed', { command, address }))
        const message =
          executeDispatch.errorMessage ??
          t('master.appDialogs.control.messages.commandSendFailed', { command })
        finishControlExecution('error', message, 'execute')
        return
      }
      if (executeDispatch.traceId) {
        const executeState = await waitCommandStateByTrace(executeDispatch.traceId, 'execute')
        if (!executeState || !isCommandStateSuccessForStage(executeState, 'execute')) {
          const message = mapFailureMessageByState(command, 'execute', executeState)
          addLog(
            'error',
            t('master.appDialogs.control.logs.executeFailedState', {
              command,
              address,
              state: executeState ?? 'no-reply',
            }),
          )
          finishControlExecution('error', message, 'execute')
          return
        }
      }

      {
        addLog('success', t('master.appDialogs.control.logs.executeSuccess', { command, address }))
        const message = t('master.appDialogs.control.messages.commandSuccess', { command })
        finishControlExecution('success', message)
      }
      return
    }

    // ── 手动模式：步骤驱动 ──
    if (currentStep === 'select') {
      // 第一步：发送 Select
      const selectDispatch = await sendControlCommandByType(
        command,
        address,
        value,
        buildOptions('select'),
      )
      if (selectDispatch.accepted && selectDispatch.traceId) {
        const selectState = await waitCommandStateByTrace(selectDispatch.traceId, 'select')
        if (!selectState || !isCommandStateSuccessForStage(selectState, 'select')) {
          addLog(
            'error',
            t('master.appDialogs.control.logs.selectFailedState', {
              command,
              address,
              state: selectState ?? 'no-reply',
            }),
          )
          const message = mapFailureMessageByState(command, 'select', selectState)
          finishControlExecution('error', message, 'select')
          return
        }
      }
      if (selectDispatch.accepted) {
        addLog('success', t('master.appDialogs.control.logs.selectSuccess', { command, address }))
        // 推进到 Execute 步骤，保持 dialog 打开
        controlConfirmState.value.step = 'execute'
      } else {
        addLog('error', t('master.appDialogs.control.logs.selectFailed', { command, address }))
        const message =
          selectDispatch.errorMessage ??
          t('master.appDialogs.control.messages.commandSelectFailed', { command })
        finishControlExecution('error', message, 'select')
      }
      return
    }

    if (currentStep === 'execute') {
      // 第二步：发送 Execute
      const executeDispatch = await sendControlCommandByType(
        command,
        address,
        value,
        buildOptions('execute'),
      )
      if (executeDispatch.accepted && executeDispatch.traceId) {
        const executeState = await waitCommandStateByTrace(executeDispatch.traceId, 'execute')
        if (!executeState || !isCommandStateSuccessForStage(executeState, 'execute')) {
          addLog(
            'error',
            t('master.appDialogs.control.logs.executeFailedState', {
              command,
              address,
              state: executeState ?? 'no-reply',
            }),
          )
          const message = mapFailureMessageByState(command, 'execute', executeState)
          finishControlExecution('error', message, 'execute')
          return
        }
      }
      if (executeDispatch.accepted) {
        addLog('success', t('master.appDialogs.control.logs.executeSuccess', { command, address }))
        const message = t('master.appDialogs.control.messages.commandSuccess', { command })
        finishControlExecution('success', message)
      } else {
        addLog('error', t('master.appDialogs.control.logs.executeFailed', { command, address }))
        const message =
          executeDispatch.errorMessage ??
          t('master.appDialogs.control.messages.commandSendFailed', { command })
        finishControlExecution('error', message, 'execute')
      }
    }
  }

  async function confirmControlExecute() {
    controlCommandBusy.value = true
    try {
      await runControlExecute()
    } finally {
      controlCommandBusy.value = false
    }
  }

  // 取消/撤销遥控命令（manual + execute 阶段触发撤销下发）
  async function cancelControlCommand() {
    if (!controlFlowActive.value) {
      await requestCloseControlConfirm()
      return
    }
    const { command, step, params } = controlConfirmState.value
    if (!command) {
      resetControlState()
      return
    }
    if (step === 'finish' || step === 'idle') {
      resetControlState()
      return
    }

    const shouldSendCancel = controlDispatchMode.value === 'manual' && step === 'execute'
    if (!shouldSendCancel) {
      addLog('warning', t('master.appDialogs.control.logs.cancel', { command }))
      resetControlState()
      return
    }

    clearControlExecuteResult()

    const parsedAddress = Number(params?.address)
    const address = Number.isFinite(parsedAddress) ? Math.max(0, Math.trunc(parsedAddress)) : 1
    const value = params?.value ?? 1
    const targetConnectionId = params?.targetSlaveId ?? selectedSlave.value?.connectionId
    const slaveId = Number(params?.slaveId ?? selectedSlave.value?.slaveId ?? 0)
    if (!targetConnectionId) {
      ElMessage.warning(t('master.appDialogs.control.messages.notConnected'))
      resetControlState()
      return
    }
    if (!isBusinessLinkReady(targetConnectionId)) {
      ElMessage.warning(t('master.appDialogs.control.messages.linkNotStarted'))
      resetControlState()
      return
    }

    const cancelDispatch = await sendControlCommandByType(command, address, value, {
      connectionId: targetConnectionId,
      slaveId,
      se: 'cancel',
      setpointType: params?.setpointType,
      ql: isSetpointType.value ? controlQualifier.value : undefined,
      qu: isSetpointType.value ? undefined : controlQualifier.value,
      qualifierMode: controlQualifierMode.value,
      typeId: params?.typeId,
    })

    if (cancelDispatch.accepted && cancelDispatch.traceId) {
      const cancelState = await waitCommandStateByTrace(cancelDispatch.traceId, 'cancel')
      if (!cancelState || !isCommandStateSuccessForStage(cancelState, 'cancel')) {
        addLog(
          'error',
          t('master.appDialogs.control.logs.cancelFailedState', {
            command,
            address,
            state: cancelState ?? 'no-reply',
          }),
        )
        const message = mapFailureMessageByState(command, 'cancel', cancelState)
        finishControlExecution('error', message, 'cancel')
        return
      }
    }

    if (cancelDispatch.accepted) {
      addLog('success', t('master.appDialogs.control.logs.cancelSuccess', { command, address }))
      const message = t('master.appDialogs.control.messages.commandCancelled', { command })
      finishControlExecution('success', message)
      return
    }

    addLog('error', t('master.appDialogs.control.logs.cancelFailed', { command, address }))
    const message =
      cancelDispatch.errorMessage ??
      t('master.appDialogs.control.messages.commandCancelFailed', { command })
    finishControlExecution('error', message, 'cancel')
  }

  // 重置遥控状态
  function resetControlState() {
    controlConfirmState.value = {
      step: 'idle',
      command: null,
      params: null,
    }
    controlConfirmDialogSnapshot.value = ''
    clearControlExecuteResult()
    controlQualifierMode.value = 'standard'
    showControlConfirmDialog.value = false
  }

  return {
    showControlConfirmDialog,
    currentAsduType,
    controlConfirmAddress,
    controlQualifier,
    controlQualifierMode,
    controlDispatchMode,
    controlFlowActive,
    isSetpointType,
    controlConfirmSubtitleBadges,
    asduTypeOptions,
    controlConfirmState,
    isDiscreteCommand,
    isDoubleCommand,
    isStepCommand,
    setpointStep,
    setpointPrecision,
    setpointMin,
    setpointMax,
    qualifierFieldLabel,
    qualifierOptions,
    controlStepClass,
    controlStepCircleLabel,
    isControlStepLineActive,
    controlExecuteResult,
    controlCancelButtonText,
    controlConfirmButtonText,
    handleControlConfirmBeforeClose,
    cancelControlCommand,
    confirmControlExecute,
    handleSendCommand,
  }
}
