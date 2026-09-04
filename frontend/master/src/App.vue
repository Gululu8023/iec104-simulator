<template>
  <el-config-provider :locale="elementPlusLocale">
    <div class="master-app tree-shell">
      <!-- 菜单栏 -->
      <MasterToolbar
        :selectedSlave="selectedSlave"
        @menu-action="handleMenuAction"
        @connection-action="handleConnectionAction"
      />

      <!-- 状态栏 + 命令区 -->
      <MasterDashboard
        :selectedSlave="selectedSlave"
        :loadingCommands="loadingCommands"
        @global-command="handleGlobalCommand"
        @connection-action="handleConnectionAction"
      />

      <!-- 主体内容区 -->
      <main class="app-content tree-shell__content">
        <!-- 上半部分：三栏布局 -->
        <div
          class="main-panels tree-shell__panels"
          :style="{ height: isMessagePanelHidden ? '100%' : mainPanelsHeight + 'px' }"
        >
          <!-- 左侧从站列表 -->
          <aside class="left-panel tree-shell__sidebar" :style="{ width: leftPanelWidth + 'px' }">
            <MasterSlaveList
              ref="slaveListRef"
              class="tree-shell__tree"
              :profiles="slaveProfiles"
              :links="linkProfiles"
              :point-data="pointData"
              :profile-point-type-summary="profilePointTypeSummary"
              @slave-select="handleSlaveSelect"
              @datatype-select="handleDatatypeSelect"
              @profile-operation="handleProfileOperation"
              @link-operation="handleLinkOperation"
              @create-connection="openCreateLinkDialog"
              @create-profile="openCreateLinkDialog"
            />
            <div
              class="resize-handle-right tree-shell__side-resize"
              @mousedown="startResizeLeft"
            ></div>
          </aside>

          <!-- 中间主内容区 -->
          <section class="center-panel tree-shell__main">
            <MasterContentPanel
              ref="contentPanelRef"
              :selectedSlave="selectedSlave"
              :point-data="pointData"
              :messages="messages"
              :profile-point-defs-map="profilePointDefsMap"
              :ioa-display-format="ioaDisplayFormat"
              :save-point-meta="handleSaveMasterPointMeta"
              @send-command="handleSendCommand"
              @connect="handleSlaveConnect"
              @disconnect="handleSlaveDisconnect"
              @refresh-data="handleRefreshData"
              @active-tab-change="handleActiveContentTabChange"
            />
          </section>
        </div>

        <!-- 垂直拖拽手柄 -->
        <div
          class="horizontal-resize-handle tree-shell__bottom-resize"
          @mousedown="startResizeVertical"
        ></div>

        <BottomMonitorWorkspace
          ref="messagePanelRef"
          :stationList="stationListForMessage"
          :stationId="currentStationId"
          stationKind="master"
          :soe-events="soeEvents"
          :detail-parse-view-mode="messageDetailViewMode"
          @panel-hidden="handleMessagePanelHidden"
          @soe-visibility-change="handleSoeVisibilityChange"
          @message-select="handleMessageSelect"
          @soe-select="handleSoeSelect"
          @clear-soe="handleClearMasterSoeEvents"
        />
      </main>

      <MasterSlaveProfileDialog
        v-model:visible="showProfileDialog"
        :mode="profileDialogMode"
        :badges="profileDialogSubtitleBadges"
        :form="profileForm"
        :before-close="handleProfileDialogBeforeClose"
        @badge-click="handleProfileDialogBadgeClick"
        @enter="handleProfileDialogEnter"
        @close="closeProfileDialog"
        @save="submitProfileDialog"
      />

      <MasterLinkDialog
        v-model:visible="showLinkDialog"
        v-model:custom-enabled="linkParamsCustomEnabled"
        :mode="linkDialogMode"
        :badges="linkDialogSubtitleBadges"
        :form="linkForm"
        :collapsed-description="linkParamsCollapsedDesc"
        :warn-w-k="linkParamsWarnWK"
        :warn-t2-t1="linkParamsWarnT2T1"
        :before-close="handleLinkDialogBeforeClose"
        @badge-click="handleLinkDialogBadgeClick"
        @custom-change="handleLinkParamsCustomEnabledChange"
        @close="closeLinkDialog"
        @save="submitLinkDialog"
      />
      <MasterInterrogationDialogs
        v-model:general-visible="showGeneralInterrogationDialog"
        v-model:general-group="generalInterrogationGroup"
        v-model:counter-visible="showCounterCommandDialog"
        v-model:counter-request="counterCommandRequest"
        v-model:counter-action="counterCommandAction"
        :general-badges="generalInterrogationSubtitleBadges"
        :general-loading="loadingCommands.gi"
        :counter-badges="counterCommandSubtitleBadges"
        :counter-loading="counterCommandBusy"
        @submit-general="submitGeneralInterrogation"
        @submit-counter="submitCounterCommand"
      />

      <MasterBitStringDialog
        v-model:visible="showBitStringDialog"
        v-model:ioa="bitStringIoa"
        v-model:input="bitStringInput"
        :badges="bitStringSubtitleBadges"
        :decimal="bitStringDecimal"
        :busy="bitStringBusy"
        @normalize="normalizeBitStringInput"
        @submit="submitBitStringCommand"
      />

      <MasterReadCommandDialog
        v-model:visible="showReadCommandDialog"
        v-model:ioa="readCommandIoa"
        :badges="readCommandSubtitleBadges"
        :busy="readCommandBusy"
        :status="readCommandResult.status"
        :message="readCommandResult.message"
        :point-name="readCommandResult.point?.name ?? ''"
        :response-type="readCommandResponseType"
        :response-value="readCommandResponseValue"
        :response-quality="readCommandResponseQuality"
        :response-timestamp="readCommandResponseTimestamp"
        :submit-label="readCommandSubmitLabel"
        @ioa-change="clearReadCommandResult"
        @submit="submitReadCommand"
      />

      <MasterControlCommandDialog
        v-model:visible="showControlConfirmDialog"
        v-model:asdu-type="currentAsduType"
        v-model:address="controlConfirmAddress"
        v-model:qualifier="controlQualifier"
        v-model:qualifier-mode="controlQualifierMode"
        v-model:dispatch-mode="controlDispatchMode"
        :before-close="handleControlConfirmBeforeClose"
        :control-flow-active="controlFlowActive"
        :is-setpoint-type="isSetpointType"
        :control-confirm-subtitle-badges="controlConfirmSubtitleBadges"
        :asdu-type-options="asduTypeOptions"
        :control-confirm-state="controlConfirmState"
        :is-discrete-command="isDiscreteCommand"
        :is-double-command="isDoubleCommand"
        :is-step-command="isStepCommand"
        :setpoint-step="setpointStep"
        :setpoint-precision="setpointPrecision"
        :setpoint-min="setpointMin"
        :setpoint-max="setpointMax"
        :qualifier-field-label="qualifierFieldLabel"
        :qualifier-options="qualifierOptions"
        :control-step-class="controlStepClass"
        :control-step-circle-label="controlStepCircleLabel"
        :is-control-step-line-active="isControlStepLineActive"
        :control-execute-result="controlExecuteResult"
        :control-cancel-button-text="controlCancelButtonText"
        :control-confirm-button-text="controlConfirmButtonText"
        @cancel="cancelControlCommand"
        @submit="confirmControlExecute"
      />
      <MasterFileTransferDialog
        v-model:visible="showFileTransferDialog"
        :connection-id="selectedSlave?.connectionId ?? null"
        :connection-name="selectedSlave?.linkName ?? null"
        :slave-id="selectedSlave ? Number(selectedSlave.slaveId) : null"
        :slave-name="selectedSlave?.name ?? null"
        :session="fileTransferSession"
        :single-section-max-file-size="fileTransferLimits?.single_section_max_file_size ?? null"
        :busy="fileTransferActionBusy"
        @start="handleStartFileTransfer"
        @cancel="handleCancelFileTransfer"
      />

      <MasterGlobalSettingsDialog
        v-model:visible="showGlobalSettingsDialog"
        :format="ioaDisplayFormat"
        :reset-process-mode="resetProcessMode"
        :message-detail-view-mode="messageDetailViewMode"
        @save="handleSaveGlobalSettings"
      />

      <PointTableImportDialog
        v-model:visible="showPointTableImportDialog"
        :title="t('master.appDialogs.pointTableImportTitle')"
        :targets="masterPointTableImportTargets"
        @applied="handleMasterPointTableImportApplied"
      />

      <HelpDocsDialog v-model:visible="showHelpDocsDialog" :doc-key="activeHelpDocKey" />
      <AboutDialog v-model:visible="showAboutDialogVisible" title-key="about.masterTitle" />
    </div>
  </el-config-provider>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { currentLocale, t } from '@shared/i18n'
import { hasReachedPointSyncCursor } from '@shared/api/pointSync'
import { elementPlusLocale } from '@shared/i18n/elementPlus'

// 导入组件
import MasterToolbar from './components/MasterToolbar.vue'
import MasterSlaveList from './components/MasterSlaveList.vue'
import MasterContentPanel from './components/MasterContentPanel.vue'
import MasterDashboard from './components/MasterDashboard.vue'
import MasterFileTransferDialog from './components/MasterFileTransferDialog.vue'
import MasterGlobalSettingsDialog from './components/MasterGlobalSettingsDialog.vue'
import MasterInterrogationDialogs from './components/MasterInterrogationDialogs.vue'
import MasterLinkDialog from './components/MasterLinkDialog.vue'
import MasterBitStringDialog from './components/MasterBitStringDialog.vue'
import MasterControlCommandDialog from './components/MasterControlCommandDialog.vue'
import MasterReadCommandDialog from './components/MasterReadCommandDialog.vue'
import MasterSlaveProfileDialog from './components/MasterSlaveProfileDialog.vue'
import BottomMonitorWorkspace from '../../shared/components/BottomMonitorWorkspace.vue'
import HelpDocsDialog from '@shared/components/HelpDocsDialog.vue'
import AboutDialog from '@shared/components/AboutDialog.vue'
import PointTableImportDialog from '@shared/components/PointTableImportDialog.vue'
import type { HelpDocKey } from '@shared/content/helpDocs'

// 导入类型
import type { ControlCommandType, GlobalCommandType, SlaveConnection } from './types/master'
import type { MasterPointMetaUpdatePayload } from './types/masterContentPanel'
import type {
  BackendMasterMessage,
  BackendSoeEvent,
  MessageDetailParseViewMode,
  MasterFileTransferSession,
  MasterFileTransferLimits,
  PointTableImportApplyResult,
  ResetProcessMode,
  SlaveIoaDisplayFormat,
} from '@shared/api/types'
import {
  buildPointTableImportTargetKey,
  type PointTableImportTargetOption,
} from '@shared/api/pointTableImport'
import { normalizeIecObjectDisplayName } from '@shared/api/iec104'
import { getIecDataTypeNodeLabel } from '@shared/ui/dataTypeIcon'

import { useMasterDialogs } from './composables/useMasterDialogs'
import { useMasterControlCommand } from './composables/useMasterControlCommand'
import { useMasterCommandDialogs } from './composables/useMasterCommandDialogs'
import { useResizableWorkspaceLayout } from '@shared/composables/useResizableWorkspaceLayout'
import { useMasterMessages } from './composables/useMasterMessages'
import { useMasterProfiles } from './composables/useMasterProfiles'
import { useMasterRuntimeActions } from './composables/useMasterRuntimeActions'
import { useMasterShellActions } from './composables/useMasterShellActions'
import { useMasterSelection } from './composables/useMasterSelection'
import { useMasterSync } from './composables/useMasterSync'
import { useMasterStation } from './composables/useMasterStation'
import {
  clearMasterSoeEvents,
  getMasterFileTransferLimits,
  getLinkSlavePointDefs,
  getMasterSoeEvents,
  setMasterMessageTracking,
  updateLinkSlave,
} from './api/master'

function syncAppTitle(): void {
  const title = t('app.masterTitle')
  document.title = title
  void getCurrentWindow()
    .setTitle(title)
    .catch((error) => {
      console.warn('[i18n] failed to update master window title', error)
    })
}

watch(currentLocale, syncAppTitle, { immediate: true })

const activeTab = ref('dashboard')
const showHelpDocsDialog = ref(false)
const showAboutDialogVisible = ref(false)
const activeHelpDocKey = ref<HelpDocKey>('user-guide')
const loadingCommands = reactive({
  gi: false,
  ci: false,
  cf: false,
  cs: false,
  rp: false,
})

let runtimeHandleSendCommand: (command: ControlCommandType, params?: any) => void = () => {}

const {
  connections,
  pointData,
  messages,
  logs,
  lastConnectionError,
  lastFileTransferError,
  connectForm,
  currentStationId,
  clearMessageState,
  initializeMasterStation,
  loadConnections,
  loadDataPoints,
  loadMessages,
  connectToSlave,
  connectLinkProfile,
  disconnectFromSlave,
  setActiveConnection,
  sendGeneralInterrogation,
  sendCounterInterrogation,
  sendCounterFreeze,
  sendCounterCommand,
  sendClockSynchronization,
  sendResetProcessCommand,
  sendTestCommand,
  sendTestFrame,
  startDataTransfer,
  stopDataTransfer,
  sendControlCommandByType,
  startFileTransfer,
  getFileTransferSession,
  cancelFileTransfer,
  addLog,
} = useMasterStation()

const selectedSlave = ref<SlaveConnection | null>(null)
const soeEvents = ref<BackendSoeEvent[]>([])
const lastSoeId = ref(0)
const showFileTransferDialog = ref(false)
const fileTransferSession = ref<MasterFileTransferSession | null>(null)
const fileTransferLimits = ref<MasterFileTransferLimits | null>(null)
const fileTransferActionBusy = ref(false)

watch(
  [showFileTransferDialog, () => selectedSlave.value?.connectionId, currentStationId],
  async ([visible, connectionId, stationId]) => {
    fileTransferLimits.value = null
    if (!visible || !connectionId || !stationId) return
    try {
      const limits = await getMasterFileTransferLimits(stationId, connectionId)
      if (
        showFileTransferDialog.value &&
        selectedSlave.value?.connectionId === connectionId &&
        currentStationId.value === stationId
      ) {
        fileTransferLimits.value = limits
      }
    } catch (error) {
      console.warn('[file-transfer] failed to load transfer limits', error)
      fileTransferLimits.value = null
    }
  },
)
const showGlobalSettingsDialog = ref(false)
const showPointTableImportDialog = ref(false)
const ioaDisplayFormat = ref<SlaveIoaDisplayFormat>('dec')
const resetProcessMode = ref<ResetProcessMode>('general-reset')
const messageDetailViewMode = ref<MessageDetailParseViewMode>('table')
const {
  linkProfiles,
  profilePointTypeSummary,
  profilePointDefsMap,
  slaveProfiles,
  stationListForMessage,
  showProfileDialog,
  profileDialogMode,
  profileDialogSubtitleBadges,
  profileForm,
  showLinkDialog,
  linkDialogMode,
  linkDialogSubtitleBadges,
  linkForm,
  linkParamsCustomEnabled,
  linkParamsCollapsedDesc,
  linkParamsWarnWK,
  linkParamsWarnT2T1,
  refreshProfiles,
  handleLinkParamsCustomEnabledChange,
  openCreateProfileDialog,
  openEditProfileDialog,
  closeProfileDialog,
  handleProfileDialogBeforeClose,
  handleProfileDialogEnter,
  submitProfileDialog,
  openCreateLinkDialog,
  openEditLinkDialog,
  closeLinkDialog,
  handleLinkDialogBeforeClose,
  submitLinkDialog,
  deleteSelectedProfile,
  deleteLinkWithCascade,
  resolveConnectTargetForProfile,
  resolveConnectionIdBySlaveId,
  resolveRuntimeLinkStateByConnectionId,
  listRuntimeConnectionIdsForLink,
} = useMasterProfiles({
  currentStationId,
  connectForm,
  connections,
  disconnectFromSlave,
  onProfilesChanged: () => syncSelectedSlaveRuntimeState(),
  onProfileFocused: (profileId) => reselectSlaveByProfileId(profileId),
  getSelectedProfileId: () => selectedSlave.value?.profileId ?? null,
  clearSelectedSlave: () => clearSelectedSlave(),
})

const masterPointTableImportTargets = computed<PointTableImportTargetOption[]>(() => {
  const slave = selectedSlave.value
  if (!slave) return []
  const target = {
    kind: 'master-link-slave' as const,
    slave_id: Number(slave.profileId),
  }
  return [
    {
      key: buildPointTableImportTargetKey(target),
      label: `${slave.name} · COA ${slave.commonAddress}`,
      subtitleBadges: [slave.linkName, slave.name],
      description: t('master.appDialogs.pointTableImportDescription', {
        linkName: slave.linkName,
        host: slave.host,
        port: slave.port,
      }),
      target,
    },
  ]
})

const {
  selectedLinkProfileId,
  hasSelectedLinkProfile,
  selectSlave,
  clearSelectedSlave,
  syncSelectedSlaveRuntimeState,
  reselectSlaveByProfileId,
  handleSlaveSelect,
} = useMasterSelection({
  selectedSlave,
  slaveProfiles,
  setActiveConnection,
  addLog,
})

const openPointTableImportDialog = () => {
  if (!selectedSlave.value) {
    ElMessage.warning(t('master.runtime.warnings.selectSlaveFirst'))
    return
  }
  showPointTableImportDialog.value = true
}

const openHelpDocument = (key: HelpDocKey) => {
  activeHelpDocKey.value = key
  showHelpDocsDialog.value = true
}

const handleSaveMasterPointMeta = async (payload: MasterPointMetaUpdatePayload) => {
  const targetSlave = selectedSlave.value
  if (!targetSlave) {
    throw new Error(t('master.runtime.warnings.selectSlaveFirst'))
  }

  const pointDefs = await getLinkSlavePointDefs(targetSlave.profileId)
  const targetIndex = pointDefs.findIndex(
    (def) =>
      def.address === payload.address &&
      (def.data_type === payload.dataType || def.type_id === payload.typeId),
  )

  if (targetIndex === -1) {
    throw new Error(t('master.appDialogs.pointDefNotFound', { address: payload.address }))
  }

  const targetAddresses = new Set(payload.addresses?.length ? payload.addresses : [payload.address])

  const nextPointDefs = pointDefs.map((def) =>
    targetAddresses.has(def.address)
      ? {
          ...def,
          name: targetAddresses.size === 1 ? payload.name : def.name,
          description: targetAddresses.size === 1 ? payload.description || null : def.description,
        }
      : def,
  )

  await updateLinkSlave(targetSlave.profileId, {
    slave: {
      name: targetSlave.name,
      common_address: targetSlave.commonAddress,
    },
    point_defs: nextPointDefs,
  })

  await refreshProfiles()
  reselectSlaveByProfileId(targetSlave.profileId)
  await syncFromBackend(['points'])
}

const {
  loadMasterGlobalSettings,
  openGlobalSettingsDialog,
  handleSaveGlobalSettings,
  showAboutDialog,
} = useMasterDialogs({
  showFileTransferDialog,
  fileTransferSession,
  showGlobalSettingsDialog,
  ioaDisplayFormat,
  resetProcessMode,
  messageDetailViewMode,
  showAboutDialog: showAboutDialogVisible,
  addLog,
})

const contentPanelRef = ref()
const slaveListRef = ref()
const messagePanelRef = ref()
const isSoePanelVisible = ref(false)

const { leftPanelWidth, mainPanelsHeight, startResizeLeft, startResizeVertical } =
  useResizableWorkspaceLayout()

const {
  isMessagePanelHidden,
  handleMessagePanelHidden: syncMessagePanelHiddenState,
  toggleMessagePanel,
  handleMessageSelect,
} = useMasterMessages(messagePanelRef, addLog)

let runtimeRefreshFileTransferSession: (options?: {
  includeTerminal?: boolean
}) => Promise<void> = async () => {}
const refreshFileTransferSession = (options: { includeTerminal?: boolean } = {}) =>
  runtimeRefreshFileTransferSession(options)

const {
  handleMessagePanelVisibilityChange,
  handleSoePanelVisibilityChange,
  syncFromBackend,
  startRuntimeSync,
  stopRuntimeSync,
  registerManualRuntimeHintToast,
  unregisterManualRuntimeHintToast,
} = useMasterSync({
  currentStationId,
  isMessagePanelHidden,
  isSoePanelVisible,
  showFileTransferDialog,
  fileTransferSession,
  loadConnections,
  loadDataPoints,
  getPointCursor: () => pointData.cursor(),
  loadMessages,
  setMasterMessageTracking,
  loadMasterSoeIncrementally,
  appendBackendMessages,
  appendMasterSoeEvents,
  syncSelectedSlaveRuntimeState,
  refreshFileTransferSession,
  resetMessageState: () => {
    clearMessageState()
    messagePanelRef.value?.clearMessages?.()
  },
  addLog,
})

const {
  handleConnectionAction,
  handleGlobalCommand: handleRuntimeGlobalCommand,
  handleLinkOperation,
  handleProfileOperation,
  handleRefreshData,
  handleSlaveConnect,
  handleSlaveDisconnect,
  handleStartFileTransfer,
  handleCancelFileTransfer,
  refreshFileTransferSession: runtimeRefreshFileTransferSessionImpl,
} = useMasterRuntimeActions({
  activeTab,
  loadingCommands,
  selectedSlave,
  selectedLinkProfileId,
  hasSelectedLinkProfile,
  slaveProfiles,
  linkProfiles,
  connections,
  showFileTransferDialog,
  fileTransferSession,
  fileTransferActionBusy,
  resetProcessMode,
  lastConnectionError,
  lastFileTransferError,
  selectSlave,
  openCreateLinkDialog,
  openEditProfileDialog,
  openCreateProfileDialog,
  openEditLinkDialog,
  deleteSelectedProfile,
  deleteLinkWithCascade,
  resolveConnectTargetForProfile,
  resolveConnectionIdBySlaveId,
  resolveRuntimeLinkStateByConnectionId,
  listRuntimeConnectionIdsForLink,
  openPointTableImportDialog,
  connectToSlave,
  connectLinkProfile,
  disconnectFromSlave,
  startDataTransfer,
  stopDataTransfer,
  sendGeneralInterrogation,
  sendCounterInterrogation,
  sendCounterFreeze,
  sendClockSynchronization,
  sendResetProcessCommand,
  sendTestCommand,
  sendTestFrame,
  startFileTransfer,
  getFileTransferSession,
  cancelFileTransfer,
  refreshProfiles,
  syncFromBackend: () => syncFromBackend(),
  registerManualRuntimeHintToast,
  unregisterManualRuntimeHintToast,
  toggleMessagePanel,
  toggleSoePanel: () => messagePanelRef.value?.toggleSoePanel?.(),
  handleSendCommand: (command, params) => runtimeHandleSendCommand(command, params),
  addLog,
})
runtimeRefreshFileTransferSession = runtimeRefreshFileTransferSessionImpl

const handleMessagePanelHidden = async (hidden: boolean) => {
  syncMessagePanelHiddenState(hidden)
  await handleMessagePanelVisibilityChange(hidden)
}

const handleSoeVisibilityChange = async (visible: boolean) => {
  isSoePanelVisible.value = visible
  await handleSoePanelVisibilityChange(visible)
}

async function handleMasterPointTableImportApplied(result: PointTableImportApplyResult) {
  const targetProfileId = result.target.kind === 'master-link-slave' ? result.target.slave_id : null
  if (targetProfileId != null) {
    contentPanelRef.value?.beginPointTableReplacement?.(targetProfileId)
  }
  try {
    await refreshProfiles()
    await syncFromBackend(['points'])
    if (!hasReachedPointSyncCursor(pointData.cursor(), result.applied_point_cursor)) {
      await syncFromBackend(['points'])
    }

    switch (result.target.kind) {
      case 'master-link-slave':
        reselectSlaveByProfileId(result.target.slave_id)
        break
      default:
        break
    }
  } catch (error) {
    ElMessage.warning(t('master.appDialogs.pointTableImportRefreshFailed', { error }))
  } finally {
    if (targetProfileId != null) {
      contentPanelRef.value?.completePointTableReplacement?.(targetProfileId)
    }
  }
}

const {
  bootstrap,
  cleanup,
  handleLinkDialogBadgeClick,
  handleMenuAction: handleShellMenuAction,
  handleProfileDialogBadgeClick,
} = useMasterShellActions({
  logs,
  leftPanelWidth,
  mainPanelsHeight,
  loadMasterGlobalSettings,
  initializeMasterStation,
  hideMessagePanel: () => messagePanelRef.value?.hidePanel?.(),
  refreshProfiles,
  syncFromBackend: () => syncFromBackend(),
  startRuntimeSync,
  stopRuntimeSync,
  openGlobalSettingsDialog,
  openMessageParserDialog: () => messagePanelRef.value?.openMessageParserDialog?.(),
  openHelpDocument,
  showAboutDialog,
  addLog,
})

const handleMenuAction = (action: string) => {
  handleShellMenuAction(action)
}

function appendBackendMessages(newMessages: BackendMasterMessage[]) {
  if (newMessages.length === 0) return
  for (const msg of newMessages) {
    const isCommandStateMetaRecord = Boolean(msg.command_state && !msg.hex_data)
    if (!isCommandStateMetaRecord && messagePanelRef.value) {
      const runtimeConnectionId = String(msg.connection_id ?? '').trim()
      const commonAddress =
        typeof msg.common_address === 'number' && Number.isFinite(msg.common_address)
          ? msg.common_address
          : null
      const connectionStation = stationListForMessage.value.find(
        (station) =>
          station.isConnectionLevel &&
          String(station.runtimeConnectionId || '').trim() === runtimeConnectionId,
      )
      const slaveStation =
        runtimeConnectionId && commonAddress != null
          ? stationListForMessage.value.find(
              (station) =>
                !station.isConnectionLevel &&
                String(station.runtimeConnectionId || '').trim() === runtimeConnectionId &&
                Number(station.commonAddress ?? 0) === commonAddress,
            )
          : null
      messagePanelRef.value.addMessage({
        id: String(msg.id),
        timestamp: new Date(msg.timestamp).getTime(),
        type: msg.direction,
        content: msg.content,
        hexData: msg.hex_data ?? '',
        parsed: {
          typeId: msg.type_id ?? undefined,
          cot: msg.cause ?? undefined,
          commonAddress: msg.common_address ?? undefined,
          traceId: msg.command_trace_id ?? undefined,
          commandState: msg.command_state ?? undefined,
        },
        runtimeConnectionId,
        uiConnectionKey: connectionStation?.uiConnectionKey || connectionStation?.id || '',
        slaveUiKey: slaveStation?.slaveUiKey || slaveStation?.id || '',
      })
    }

    if (msg.command_trace_id && msg.command_state && msg.command_state !== 'dispatch') {
      addLog(
        'info',
        t('master.appDialogs.control.logs.commandTrace', {
          traceId: msg.command_trace_id,
          state: msg.command_state,
        }),
      )
      if (msg.command_state === 'unknown-information-object') {
        const handledByReadDialog =
          showReadCommandDialog.value && readCommandTraceId.value === msg.command_trace_id
        if (!handledByReadDialog) {
          ElMessage.error(t('master.appDialogs.readCommand.unknownIoa'))
        }
      }
    }
  }
}

function appendMasterSoeEvents(newEvents: BackendSoeEvent[]) {
  if (newEvents.length === 0) return
  const knownIds = new Set(soeEvents.value.map((event) => event.id))
  const uniqueEvents = newEvents.filter((event) => {
    if (knownIds.has(event.id)) return false
    knownIds.add(event.id)
    return true
  })
  if (uniqueEvents.length === 0) return
  soeEvents.value = [...soeEvents.value, ...uniqueEvents].slice(-10000)
  lastSoeId.value = uniqueEvents[uniqueEvents.length - 1].id
}

async function loadMasterSoeIncrementally(isCurrent: () => boolean = () => true) {
  const stationId = currentStationId.value
  if (!stationId) return [] as BackendSoeEvent[]
  const merged: BackendSoeEvent[] = []
  let cursor = lastSoeId.value > 0 ? lastSoeId.value : null
  try {
    for (let batchIndex = 0; batchIndex < 25; batchIndex += 1) {
      const batch = await getMasterSoeEvents(stationId, cursor, 200)
      if (!isCurrent() || currentStationId.value !== stationId) return []
      if (batch.length === 0) break
      merged.push(...batch)
      const lastId = batch[batch.length - 1]?.id
      if (typeof lastId !== 'number' || !Number.isFinite(lastId)) break
      cursor = lastId
      if (batch.length < 200) break
    }
  } catch (error) {
    addLog('error', t('master.appDialogs.soeLoadFailed', { error }))
    return [] as BackendSoeEvent[]
  }
  return merged
}

async function handleClearMasterSoeEvents() {
  if (!currentStationId.value) return
  await clearMasterSoeEvents(currentStationId.value)
  soeEvents.value = []
  lastSoeId.value = 0
}

function resolveMasterSoeDataType(typeId: number): string | null {
  if (typeId === 30) return 'M_SP_NA_1'
  if (typeId === 31) return 'M_DP_NA_1'
  if (typeId === 32) return 'M_ST_TB_1'
  if (typeId === 34) return 'M_ME_TD_1'
  if (typeId === 35) return 'M_ME_TE_1'
  if (typeId === 36) return 'M_ME_TF_1'
  if (typeId === 37) return 'M_IT_TB_1'
  return null
}

function resolveSlaveForSoeEvent(event: BackendSoeEvent): SlaveConnection | null {
  const slaveId = Number(event.slave_id ?? 0)
  const commonAddress = Number(event.common_address ?? 0)
  const connectionId = String(event.connection_id ?? '').trim()

  return (
    slaveProfiles.value.find((slave) => {
      if (
        slaveId > 0 &&
        (Number(slave.slaveId) === slaveId || Number(slave.profileId) === slaveId)
      ) {
        return true
      }
      if (
        connectionId &&
        slave.connectionId === connectionId &&
        Number(slave.commonAddress) === commonAddress
      ) {
        return true
      }
      return false
    }) ?? null
  )
}

async function handleSoeSelect(event: BackendSoeEvent) {
  const targetDataType = resolveMasterSoeDataType(event.type_id)
  const targetSlave = resolveSlaveForSoeEvent(event)

  if (targetSlave) {
    handleSlaveSelect(targetSlave)
    await nextTick()
  }

  if (targetDataType && contentPanelRef.value) {
    const focusSlave = targetSlave ?? selectedSlave.value
    if (focusSlave) {
      contentPanelRef.value.focusPoint?.({
        profileId: focusSlave.profileId,
        parentSlaveId: focusSlave.profileId,
        dataType: targetDataType,
        label: getIecDataTypeNodeLabel(targetDataType) || targetDataType,
        linkProfileId: focusSlave.linkProfileId,
        connectionId: focusSlave.connectionId,
        slaveCommonAddress: focusSlave.commonAddress,
        slaveName: focusSlave.name,
        address: event.ioa,
      })
    }
  }

  messagePanelRef.value?.clearFocusFilter?.()
}

onMounted(async () => {
  await bootstrap()
})

onUnmounted(() => {
  cleanup()
})

function handleDatatypeSelect(datatype: any) {
  if (contentPanelRef.value) {
    contentPanelRef.value.addTab({
      ...datatype,
      label: normalizeIecObjectDisplayName(String(datatype?.label ?? '')),
    })
  }
}

function handleActiveContentTabChange(tab: { profileId: string; dataType: string } | null) {
  if (tab && String(selectedSlave.value?.profileId ?? '') !== String(tab.profileId)) {
    const targetSlave = slaveProfiles.value.find(
      (slave) => String(slave.profileId) === String(tab.profileId),
    )
    if (targetSlave) selectSlave(targetSlave)
  }
  slaveListRef.value?.syncCurrentDatatypeNode?.(tab)
}

const {
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
} = useMasterCommandDialogs({
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
})
const {
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
} = useMasterControlCommand({
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
})
runtimeHandleSendCommand = handleSendCommand
</script>

<style scoped>
/* 响应式设计 */

@media (max-width: 1200px) {
  .left-panel {
    width: 260px;
  }
}

@media (max-width: 1000px) {
  .app-content {
    flex-direction: column;
  }

  .left-panel,
  .center-panel {
    width: 100%;
    height: 300px;
  }
}
</style>
