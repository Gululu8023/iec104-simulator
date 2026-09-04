<template>
  <el-config-provider :locale="elementPlusLocale">
    <div class="slave-app tree-shell">
      <!-- 菜单栏 -->
      <SlaveToolbar
        :selectedSlave="selectedSlave"
        :station-time-offset-ms="stationTimeOffsetMs"
        @slave-operation="handleSlaveOperation"
        @menu-action="handleMenuAction"
      />

      <!-- 状态栏 + 命令区 -->
      <SlaveDashboard
        :selectedSlave="selectedSlave"
        :stats="slaveStats"
        :global-simulation-enabled="simulationProfile.enabled"
        :sync-revision="statusBarSyncRevision"
        @slave-command="handleSlaveCommand"
      />

      <!-- 主体内容区 -->
      <main class="app-content tree-shell__content">
        <!-- 上半部分：三栏布局 -->
        <div
          class="main-panels tree-shell__panels"
          :style="{ height: isMessagePanelHidden ? '100%' : mainPanelsHeight + 'px' }"
        >
          <!-- 左侧设备树 -->
          <aside class="left-panel tree-shell__sidebar" :style="{ width: leftPanelWidth + 'px' }">
            <div class="tree-container tree-shell__tree">
              <SlaveDeviceTree
                ref="deviceTreeRef"
                :stations="stations"
                :station-id="currentStationId"
                :selected-slave-status="selectedSlave?.status ?? null"
                :station-runtime-status-map="stationRuntimeStatusMap"
                :station-name="stationName"
                :host="serverConfig.host"
                :port="serverConfig.port"
                :default-common-address="serverConfig.commonAddress"
                :link-params="linkParams"
                :redundancy-mode="redundancyMode"
                :is-running="isRunning"
                :slaves-by-station="slavesByStation"
                :slave-point-type-summary="slavePointTypeSummary"
                :station-point-type-summary="stationPointTypeSummary"
                @node-click="handleNodeClick"
                @type-id-click="handleTypeIdClick"
                @slave-operation="handleSlaveOperation"
                @refresh-requested="handleDeviceRefreshRequested"
                @station-profile-submit="handleStationProfileSubmit"
              />
            </div>
            <div
              class="resize-handle-right tree-shell__side-resize"
              @mousedown="startResizeLeft"
            ></div>
          </aside>

          <!-- 中间数据点管理 -->
          <section class="center-panel tree-shell__main">
            <SlaveContentPanel
              ref="contentPanelRef"
              :point-data="slavePointData"
              :current-station-id="currentStationId"
              :ioa-display-format="ioaDisplayFormat"
              :active-point-simulation="activePointSimulation"
              :simulation-interval-ms="simulationProfile.interval_ms"
              @data-point-edit="handleDataPointEdit"
              @data-point-simulate="handleDataPointSimulate"
              @inspect-message="handleInspectMessage"
              @import-data-points="handleImportDataPoints"
              @active-tab-change="handleActiveTabChange"
              @batch-gi-group="handleBatchGiGroup"
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
          :stationList="currentStationList"
          :stationId="currentStationId"
          stationKind="slave"
          :soe-events="currentSoeEvents"
          :detail-parse-view-mode="messageDetailViewMode"
          @message-select="handleMessageSelect"
          @panel-hidden="handleMessagePanelHidden"
          @soe-visibility-change="handleSoeVisibilityChange"
          @soe-select="handleSoeSelect"
          @clear-soe="handleClearSlaveSoeEvents"
        />
      </main>

      <SlaveGlobalSettingsDialog
        v-model:visible="showGlobalSettingsDialog"
        :policy="commandMismatchPolicy"
        :format="ioaDisplayFormat"
        :message-detail-view-mode="messageDetailViewMode"
        :simulation-interval-ms="simulationProfile.interval_ms"
        :simulation-auto-broadcast="simulationProfile.auto_broadcast"
        :saving="commandMismatchPolicySaving"
        @save="handleSaveGlobalSettings"
      />

      <PointTableImportDialog
        v-model:visible="showPointTableImportDialog"
        :title="t('slave.app.importTitle')"
        :targets="slavePointTableImportTargets"
        :initial-target-key="pointTableImportPreferredTargetKey"
        :normalize-payload="normalizeSlavePointTableImportPayload"
        @applied="handleSlavePointTableImportApplied"
      />

      <HelpDocsDialog v-model:visible="showHelpDocsDialog" :doc-key="activeHelpDocKey" />
      <AboutDialog v-model:visible="showAboutDialogVisible" title-key="about.slaveTitle" />
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
import { confirmDangerousAction, confirmExitApp } from '@shared/ui/dialogConfirm'

import SlaveDeviceTree from './components/SlaveDeviceTree.vue'
import SlaveContentPanel from './components/SlaveContentPanel.vue'
import BottomMonitorWorkspace from '@shared/components/BottomMonitorWorkspace.vue'
import HelpDocsDialog from '@shared/components/HelpDocsDialog.vue'
import AboutDialog from '@shared/components/AboutDialog.vue'
import PointTableImportDialog from '@shared/components/PointTableImportDialog.vue'
import { resolveHelpDocKeyFromAction, type HelpDocKey } from '@shared/content/helpDocs'
import SlaveToolbar from './components/SlaveToolbar.vue'
import SlaveDashboard from './components/SlaveDashboard.vue'
import SlaveGlobalSettingsDialog from './components/SlaveGlobalSettingsDialog.vue'

import type { DeviceNode } from './types/slave'
import type { SlaveContentActiveTabIdentity } from './types/slaveContentPanel'
import { type PointTypeSummary } from './types/pointTypeSummary'
import type {
  BackendConnectionInfo,
  BackendSoeEvent,
  BackendSlaveMessage,
  MessageDetailParseViewMode,
  PointTableImportApplyResult,
  SlaveCommandMismatchPolicy,
  SlaveIoaDisplayFormat,
  SlaveResponse,
} from '@shared/api/types'
import {
  buildPointTableImportTargetKey,
  type PointTableImportTargetOption,
} from '@shared/api/pointTableImport'
import { stopAllStations } from '@shared/api/common'
import { getIec104Capability, getInstalledIec104Capabilities } from '@shared/api/iec104'
import { useResizableWorkspaceLayout } from '@shared/composables/useResizableWorkspaceLayout'
import { useSlaveDialogs } from './composables/useSlaveDialogs'
import { useSlaveSelection } from './composables/useSlaveSelection'
import { useSlaveSimulation } from './composables/useSlaveSimulation'
import { useSlaveSync } from './composables/useSlaveSync'
import { useSlaveStation } from './composables/useSlaveStation'
import {
  clearSlaveSoeEvents,
  getSlaveConnections,
  getSlaveStationTimeOffset,
  syncSlaveDataPoints,
  getSlaveMessages,
  setSlaveMessageTracking,
  getSlaveSoeEvents,
  getStationSlavePointDefs,
  replaceStationSlavePointDefs,
  forceUploadSlaveData,
  deleteSlave,
  updateSlaveSimulationProfile,
} from './api/slave'
import { deriveUnifiedConnStatus } from '@shared/ui/connectionStatus'
import { downloadPointTableTemplate } from '@shared/ui/pointTableTemplate'
import { normalizeSlavePointTableImportPayload } from './utils/pointDefinitions'
import { useSlaveStationProfileSubmission } from './composables/useSlaveStationProfileSubmission'
import { useSlavePointPersistence } from './composables/useSlavePointPersistence'
import { useSlavePointSimulation } from './composables/useSlavePointSimulation'
import { useSlaveStationCatalog } from './composables/useSlaveStationCatalog'
import { useSlavePointData } from './composables/useSlavePointData'

function syncAppTitle(): void {
  const title = t('app.slaveTitle')
  document.title = title
  void getCurrentWindow()
    .setTitle(title)
    .catch((error) => {
      console.warn('[i18n] failed to update slave window title', error)
    })
}

watch(currentLocale, syncAppTitle, { immediate: true })

const deviceTreeRef = ref()
const contentPanelRef = ref()
const messagePanelRef = ref()
const showGlobalSettingsDialog = ref(false)
const showPointTableImportDialog = ref(false)
const showHelpDocsDialog = ref(false)
const showAboutDialogVisible = ref(false)
const activeHelpDocKey = ref<HelpDocKey>('user-guide')
const pointTableImportPreferredTargetKey = ref<string | null>(null)
const commandMismatchPolicy = ref<SlaveCommandMismatchPolicy>('strict')
const ioaDisplayFormat = ref<SlaveIoaDisplayFormat>('dec')
const messageDetailViewMode = ref<MessageDetailParseViewMode>('table')
const commandMismatchPolicySaving = ref(false)

const selectedSlave = ref<DeviceNode | null>(null)
const slavePointData = useSlavePointData()
const backendMessages = ref<BackendSlaveMessage[]>([])
const stationTimeOffsetMs = ref(0)
const soeEventsByStation = ref<Record<string, BackendSoeEvent[]>>({})
const lastSoeIdByStation = ref<Record<string, number>>({})
const stationRuntimeStatusMap = ref<Record<string, DeviceNode['status']>>({})
const slavesByStation = ref<Record<string, SlaveResponse[]>>({})
const runtimeConnectionsByStation = ref<Record<string, BackendConnectionInfo[]>>({})
const slavePointTypeSummary = ref<Record<number, PointTypeSummary>>({})
const slaveAsduAliasMap = ref<Record<number, Record<string, string>>>({})
const selectedSlaveId = ref<number | null>(null)

const stationPointTypeSummary = ref<Record<string, PointTypeSummary>>({})

const slaveStats = reactive({
  singlePointCount: 0,
  measuredCount: 0,
  pendingQueue: 0,
  txUnconfirmed: null as number | null,
  rxPending: null as number | null,
  connectingCount: 0,
  errorCount: 0,
})
const statusBarSyncRevision = ref(0)

const {
  isRunning,
  currentStationId,
  stationName,
  serverConfig,
  linkParams,
  simulationProfile,
  redundancyMode,
  stations,
  initializeSlaveStation,
  refreshStations,
  selectStation,
  createStation,
  toggleServer,
  startServer,
  stopServer,
  deleteCurrentStation,
} = useSlaveStation()

const { currentStationList, refreshStationPointTypeSummary } = useSlaveStationCatalog({
  stations,
  currentStationId,
  selectedSlaveId,
  slavesByStation,
  runtimeConnectionsByStation,
  stationPointTypeSummary,
  slavePointTypeSummary,
  slaveAsduAliasMap,
  soeEventsByStation,
  lastSoeIdByStation,
})

const {
  clearSelection,
  selectNode,
  resolveStationIdFromNode,
  resolveSlaveIdFromNode,
  findLatestListenerNode,
  findLatestSlaveNode,
  resolveLatestSlaveNode,
  syncUiForCurrentStation,
} = useSlaveSelection({
  currentStationId,
  deviceTreeRef,
  selectedSlave,
  selectedSlaveId,
  slavesByStation,
})

const currentSoeEvents = computed(() => soeEventsByStation.value[currentStationId.value] ?? [])
const isSoePanelVisible = ref(false)

const { leftPanelWidth, mainPanelsHeight, startResizeLeft, startResizeVertical } =
  useResizableWorkspaceLayout()

const {
  isMessagePanelHidden,
  handleMessagePanelHidden: syncMessagePanelHiddenState,
  toggleMessagePanel,
  handleAvalancheTest,
} = useSlaveSimulation(messagePanelRef, currentStationId, selectedSlave, slaveStats)

const {
  loadSlaveGlobalSettings,
  openGlobalSettingsDialog,
  handleSaveGlobalSettings,
  showAboutDialog,
} = useSlaveDialogs({
  currentStationId,
  showGlobalSettingsDialog,
  commandMismatchPolicy,
  ioaDisplayFormat,
  messageDetailViewMode,
  commandMismatchPolicySaving,
  showAboutDialog: showAboutDialogVisible,
  simulationProfile,
})

const lastMessageId = ref(0)
let lastLocalOnlyHintAt = 0
const slavePointTableImportTargets = computed<PointTableImportTargetOption[]>(() => {
  const stationId = String(currentStationId.value || '').trim()
  if (!stationId) return []

  const options: PointTableImportTargetOption[] = []

  for (const slave of slavesByStation.value[stationId] ?? []) {
    const target = {
      kind: 'slave-station-slave' as const,
      slave_id: slave.id,
    }
    options.push({
      key: buildPointTableImportTargetKey(target),
      label: `${slave.name} · COA ${slave.common_address}`,
      subtitleBadges: [stationName.value || t('slave.app.currentConnection'), slave.name],
      description: t('slave.app.importTargetDescription', {
        name: slave.name,
        commonAddress: slave.common_address,
      }),
      target,
    })
  }

  return options
})

const resolvePointTableImportDefaultTargetKey = (
  preferredSlaveId?: number | null,
): string | null => {
  if (preferredSlaveId != null) {
    const preferredKey = buildPointTableImportTargetKey({
      kind: 'slave-station-slave',
      slave_id: preferredSlaveId,
    })
    if (slavePointTableImportTargets.value.some((option) => option.key === preferredKey)) {
      return preferredKey
    }
  }

  if (selectedSlaveId.value != null) {
    const selectedKey = buildPointTableImportTargetKey({
      kind: 'slave-station-slave',
      slave_id: selectedSlaveId.value,
    })
    if (slavePointTableImportTargets.value.some((option) => option.key === selectedKey)) {
      return selectedKey
    }
  }

  return slavePointTableImportTargets.value[0]?.key ?? null
}

const openSlavePointTableImportDialog = (preferredSlaveId?: number | null) => {
  if (!currentStationId.value) {
    ElMessage.warning(t('slave.app.messages.selectConnectionOrSlaveFirst'))
    return
  }
  if (slavePointTableImportTargets.value.length === 0) {
    ElMessage.warning(t('slave.app.messages.createSlaveBeforeImport'))
    return
  }

  pointTableImportPreferredTargetKey.value = resolvePointTableImportDefaultTargetKey(
    preferredSlaveId ?? null,
  )
  showPointTableImportDialog.value = true
}

const openEdit = () => {
  void openListenerConfigDialog(selectedSlave.value)
}

const openTools = () => {
  deviceTreeRef.value?.refreshDevices()
  void syncFromBackend()
}

const openHelpDocument = (key: HelpDocKey) => {
  activeHelpDocKey.value = key
  showHelpDocsDialog.value = true
}

const handleNodeClick = (node: DeviceNode) => {
  if (node.type === 'listener' || node.type === 'slave') {
    const { stationId } = selectNode(node)
    if (stationId && stationId !== currentStationId.value) {
      void handleStationChange(stationId)
    }
  }
}

const handleTypeIdClick = (node: DeviceNode) => {
  if (node.type !== 'type-id') return

  const targetStationId = resolveStationIdFromNode(node)
  const targetSlaveId = resolveSlaveIdFromNode(node)
  if (targetSlaveId != null) selectedSlaveId.value = targetSlaveId

  const latestSlave =
    targetSlaveId != null ? findLatestSlaveNode(targetSlaveId, targetStationId) : null
  const latestListener = targetStationId ? findLatestListenerNode(targetStationId) : null
  if (latestSlave) selectedSlave.value = latestSlave

  const resolvedSlaveName =
    latestSlave?.name || latestSlave?.label || node.parentSlaveName || t('slave.app.unknownSlave')
  contentPanelRef.value?.addTab({
    ...node,
    parentStationId: targetStationId || node.parentStationId,
    parentSlaveId: String(targetSlaveId ?? node.parentSlaveId ?? ''),
    slaveId: targetSlaveId ?? undefined,
    parentSlaveName: resolvedSlaveName,
    slaveName: resolvedSlaveName,
    slaveCommonAddress: latestSlave?.commonAddress ?? node.slaveCommonAddress,
    connectionName: latestListener ? latestListener.name || t('slave.app.unknownConnection') : '',
  })
}

let activeTabSyncRevision = 0
const activeContentTab = ref<SlaveContentActiveTabIdentity | null>(null)

const isActiveContentTab = (tab: SlaveContentActiveTabIdentity): boolean => {
  const active = activeContentTab.value
  return Boolean(
    active &&
    active.id === tab.id &&
    active.parentStationId === tab.parentStationId &&
    active.parentSlaveId === tab.parentSlaveId &&
    active.slaveId === tab.slaveId &&
    active.dataType === tab.dataType,
  )
}

const handleActiveTabChange = (tab: SlaveContentActiveTabIdentity | null) => {
  activeContentTab.value = tab ? { ...tab } : null
  const revision = ++activeTabSyncRevision

  void (async () => {
    if (!tab) {
      deviceTreeRef.value?.syncCurrentTypeNode?.(null)
      return
    }

    const stationId = String(tab.parentStationId || '').trim()
    const slaveId = Number(tab.slaveId ?? tab.parentSlaveId)
    const dataType = String(tab.dataType || '').trim()
    if (!stationId || !Number.isFinite(slaveId) || slaveId <= 0 || !dataType) {
      deviceTreeRef.value?.syncCurrentTypeNode?.(null)
      return
    }

    if (stationId !== currentStationId.value) {
      await handleStationChange(stationId)
    }
    if (revision !== activeTabSyncRevision) return

    const latestSlave = findLatestSlaveNode(slaveId, stationId)
    if (latestSlave) {
      selectedSlaveId.value = slaveId
      selectedSlave.value = latestSlave
    }

    await nextTick()
    if (revision !== activeTabSyncRevision) return
    deviceTreeRef.value?.syncCurrentTypeNode?.({ stationId, slaveId, dataType })
  })()
}

const handleBatchGiGroup = async (payload: {
  tab: SlaveContentActiveTabIdentity
  addresses: number[]
  giGroup: number | null
  counterGroup?: number | null
  onSaved?: () => void
}) => {
  if (!isActiveContentTab(payload.tab)) {
    ElMessage.error(t('slave.contentPanel.giGroup.scopeChanged'))
    return
  }
  const stationId = String(payload.tab.parentStationId || currentStationId.value).trim()
  if (!stationId || stationId !== currentStationId.value) {
    ElMessage.error(t('slave.contentPanel.giGroup.scopeChanged'))
    return
  }
  const addresses = new Set(
    payload.addresses
      .map((address) => Math.trunc(Number(address)))
      .filter((address) => address >= 1),
  )
  if (addresses.size === 0) return
  const group = payload.giGroup == null ? null : Math.trunc(payload.giGroup)
  if (group != null && (group < 1 || group > 16)) {
    ElMessage.error(t('slave.contentPanel.giGroup.invalid'))
    return
  }
  const updatesCounterGroup = payload.counterGroup !== undefined
  const counterGroup = payload.counterGroup == null ? null : Math.trunc(payload.counterGroup)
  if (updatesCounterGroup && counterGroup != null && (counterGroup < 1 || counterGroup > 4)) {
    ElMessage.error(t('slave.contentPanel.counterGroup.invalid'))
    return
  }

  try {
    const slaveId = Number(payload.tab.slaveId ?? payload.tab.parentSlaveId)
    if (!Number.isInteger(slaveId) || slaveId <= 0) {
      throw new Error(t('slave.app.messages.missingSlaveIdUpdate'))
    }
    const defs = await getStationSlavePointDefs(slaveId)
    const targetType = String(payload.tab.dataType || '')
      .trim()
      .toUpperCase()
    const matched = defs.filter(
      (def) =>
        addresses.has(def.address) && String(def.data_type).trim().toUpperCase() === targetType,
    )
    if (matched.length !== addresses.size) {
      ElMessage.error(t('slave.contentPanel.giGroup.pointsChanged'))
      return
    }
    if (!isActiveContentTab(payload.tab)) {
      ElMessage.error(t('slave.contentPanel.giGroup.scopeChanged'))
      return
    }
    const nextDefs = defs.map((def) =>
      addresses.has(def.address) && String(def.data_type).trim().toUpperCase() === targetType
        ? {
            ...def,
            gi_group: group,
            ...(updatesCounterGroup ? { counter_group: counterGroup } : {}),
          }
        : def,
    )
    await replaceStationSlavePointDefs(slaveId, nextDefs)
    await syncFromBackend()
    payload.onSaved?.()
    ElMessage.success(t('slave.contentPanel.giGroup.saved', { count: addresses.size }))
  } catch (error) {
    ElMessage.error(t('slave.contentPanel.giGroup.saveFailed', { error: String(error) }))
  }
}

const resolveGlobalMappingContext = (): {
  connectionName: string
  slaveName: string
  commonAddress: number
} | null => {
  const latestSlave = resolveLatestSlaveNode(selectedSlave.value)
  if (!latestSlave || latestSlave.type !== 'slave') return null

  const stationId = resolveStationIdFromNode(latestSlave)
  if (!stationId) return null

  const latestListener = findLatestListenerNode(stationId)
  const connectionName = String(
    latestListener?.name || latestListener?.label || stationName.value || '',
  ).trim()
  const slaveName = String(latestSlave.name || latestSlave.label || '').trim()
  const commonAddress = normalizeOptionalCommonAddress(
    latestSlave.commonAddress ??
      latestSlave.common_address ??
      selectedSlave.value?.commonAddress ??
      selectedSlave.value?.common_address,
  )
  if (!connectionName || !slaveName || commonAddress == null) return null

  return { connectionName, slaveName, commonAddress }
}

const ensureTargetStationSelected = async (slave: DeviceNode | null | undefined) => {
  const targetStationId = resolveStationIdFromNode(slave)
  if (!targetStationId || targetStationId === currentStationId.value) return
  await handleStationChange(targetStationId)
}

const openListenerConfigDialog = async (slave: DeviceNode | null | undefined) => {
  const targetNode = slave ?? selectedSlave.value
  const targetStationId = resolveStationIdFromNode(targetNode) || currentStationId.value
  const targetListener = targetStationId ? findLatestListenerNode(targetStationId) : null
  if (targetListener) {
    if (
      await ensureStationDisconnectedForConfig(
        targetListener,
        t('slave.app.actions.editConnectionConfig'),
      )
    ) {
      return
    }
    if (targetStationId && targetStationId !== currentStationId.value) {
      await handleStationChange(targetStationId)
    }
    deviceTreeRef.value?.handleEditConnection(targetListener)
    return
  }
  deviceTreeRef.value?.handleCreateListener?.()
}

const deleteStationWithConfirm = async (targetListener: DeviceNode | null) => {
  if (!targetListener) {
    ElMessage.warning(t('slave.app.messages.selectSlaveConnectionFirst'))
    return
  }
  if (
    await ensureStationDisconnectedForConfig(targetListener, t('slave.app.actions.deleteStation'))
  )
    return

  const confirmed = await confirmDangerousAction(
    t('slave.app.confirm.deleteConnectionMessage', {
      name: targetListener.label ?? currentStationId.value,
    }),
    t('slave.app.confirm.deleteStationTitle'),
    t('slave.app.confirm.delete'),
  )
  if (!confirmed) return

  if (await deleteCurrentStation()) {
    slavePointData.clear()
    backendMessages.value = []
    lastMessageId.value = 0
    messagePanelRef.value?.clearMessages()
    selectedSlaveId.value = null
    syncUiForCurrentStation()
    await refreshStationPointTypeSummary()
    await syncFromBackend()
    deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.()
    ElMessage.success(t('slave.app.messages.stationDeleted'))
  }
}

const handleSlaveOperation = (operation: string, slave: DeviceNode | null = null) => {
  switch (operation) {
    case 'create-station':
      deviceTreeRef.value?.handleCreateListener?.()
      break
    case 'create-slave':
      {
        const targetStationId = resolveStationIdFromNode(slave) || currentStationId.value
        const targetListener = targetStationId ? findLatestListenerNode(targetStationId) : null
        if (!targetListener) {
          ElMessage.warning(t('slave.app.messages.createAndSelectConnectionBeforeSlave'))
          break
        }
        deviceTreeRef.value?.handleCreateSlave(targetListener)
      }
      break
    case 'start-listening':
    case 'open-connection':
      void (async () => {
        await ensureTargetStationSelected(slave)
        const ok = await startServer()
        if (ok) {
          syncUiForCurrentStation()
          await syncFromBackend()
        }
      })()
      break
    case 'stop-listening':
    case 'close-connection':
      void (async () => {
        await ensureTargetStationSelected(slave)
        const ok = await stopServer()
        if (ok) {
          syncUiForCurrentStation()
          await syncFromBackend()
        }
      })()
      break
    case 'restart-listening':
      void (async () => {
        await ensureTargetStationSelected(slave)
        const stopOk = await stopServer()
        if (!stopOk) return
        const startOk = await startServer()
        if (!startOk) return
        syncUiForCurrentStation()
        await syncFromBackend()
        ElMessage.success(t('slave.app.messages.listeningRestarted'))
      })()
      break
    case 'edit-connection':
    case 'edit-station':
      void (async () => {
        await ensureTargetStationSelected(slave)
        const targetStationId = resolveStationIdFromNode(slave) || currentStationId.value
        const targetListener = targetStationId ? findLatestListenerNode(targetStationId) : null
        if (
          await ensureStationDisconnectedForConfig(
            targetListener,
            t('slave.app.actions.editConnectionConfig'),
          )
        ) {
          return
        }
        deviceTreeRef.value?.handleEditConnection(targetListener)
      })()
      break
    case 'edit-slave':
      void (async () => {
        await ensureTargetStationSelected(slave)
        const slaveId =
          resolveSlaveIdFromNode(slave) ??
          (selectedSlaveId.value != null ? selectedSlaveId.value : null)
        const targetStationId = resolveStationIdFromNode(slave) || currentStationId.value
        const targetSlave = slaveId != null ? findLatestSlaveNode(slaveId, targetStationId) : null
        if (!targetSlave) {
          ElMessage.warning(t('slave.app.messages.selectOneSlaveFirst'))
          return
        }
        if (
          await ensureStationDisconnectedForConfig(
            targetSlave,
            t('slave.app.actions.editSlaveConfig'),
          )
        ) {
          return
        }
        deviceTreeRef.value?.handleEditSlave(targetSlave)
      })()
      break
    case 'delete-slave':
      void (async () => {
        await ensureTargetStationSelected(slave)
        if (slave?.type === 'listener') {
          await deleteStationWithConfirm(slave)
          return
        }

        const slaveId =
          resolveSlaveIdFromNode(slave) ??
          (selectedSlaveId.value != null ? selectedSlaveId.value : null)
        const targetStationId = resolveStationIdFromNode(slave) || currentStationId.value
        if (!slaveId || !targetStationId) {
          ElMessage.warning(t('slave.app.messages.selectOneSlaveFirst'))
          return
        }
        const targetSlave = findLatestSlaveNode(slaveId, targetStationId)
        if (!targetSlave) {
          ElMessage.warning(t('slave.app.messages.slaveMissingOrDeleted'))
          return
        }
        if (
          await ensureStationDisconnectedForConfig(
            targetSlave,
            t('slave.app.actions.deleteStation'),
          )
        )
          return

        const confirmed = await confirmDangerousAction(
          t('slave.app.confirm.deleteSlaveMessage', {
            name: targetSlave.label || slaveId,
          }),
          t('slave.app.confirm.deleteStationTitle'),
          t('slave.app.confirm.delete'),
        )
        if (!confirmed) return

        try {
          await deleteSlave(slaveId)
          if (selectedSlaveId.value === slaveId) {
            selectedSlaveId.value = null
          }
          await refreshStationPointTypeSummary()
          await syncFromBackend()
          deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.()
          ElMessage.success(t('slave.app.messages.stationDeleted'))
        } catch (error) {
          ElMessage.error(t('slave.app.messages.deleteStationFailed', { error: String(error) }))
        }
      })()
      break
    case 'delete-station':
    case 'delete-connection':
      void (async () => {
        await ensureTargetStationSelected(slave)
        const targetStationId = resolveStationIdFromNode(slave) || currentStationId.value
        const targetListener = targetStationId ? findLatestListenerNode(targetStationId) : null
        await deleteStationWithConfirm(targetListener)
      })()
      break
    case 'communication-settings':
    case 'connection-settings':
      void (async () => {
        await openListenerConfigDialog(slave)
      })()
      break
    case 'toggle-message-panel':
      toggleMessagePanel()
      break
    case 'refresh-devices':
      deviceTreeRef.value?.refreshDevices()
      break
    case 'avalanche-test':
      if (!ensureLiveCommandReady(t('slave.app.actions.avalancheTest'))) return
      void (async () => {
        await notifyLocalOnlySimulationIfNeeded(t('slave.app.actions.avalancheTest'))
        await handleAvalancheTest(resolveWriteCommonAddress(slave?.commonAddress))
        await syncFromBackend()
      })()
      break
    case 'toggle':
      void (async () => {
        await toggleServer()
        syncUiForCurrentStation()
        await syncFromBackend()
      })()
      break
  }
}

const handleMenuAction = (action: string) => {
  const helpDocKey = resolveHelpDocKeyFromAction(action)
  if (helpDocKey) {
    openHelpDocument(helpDocKey)
    return
  }

  switch (action) {
    case 'open-config':
      break
    case 'save-config':
      break
    case 'exit':
      void (async () => {
        const confirmed = await confirmExitApp()
        if (!confirmed) return

        try {
          await stopAllStations()
        } catch (error) {
          ElMessage.warning(t('slave.app.messages.stopBeforeExitFailed', { error: String(error) }))
        }
        try {
          await getCurrentWindow().close()
        } catch (error) {
          ElMessage.error(t('slave.app.messages.exitFailed', { error: String(error) }))
        }
      })()
      break
    case 'clear-logs':
      messagePanelRef.value?.clearMessages()
      break
    case 'import-point-defs':
      openSlavePointTableImportDialog(selectedSlaveId.value)
      break
    case 'toggle-soe-panel':
      messagePanelRef.value?.toggleSoePanel?.()
      break
    case 'reset-layout':
      leftPanelWidth.value = 280
      mainPanelsHeight.value = 550
      break
    case 'about':
      void showAboutDialog()
      break
    case 'edit':
      openEdit()
      break
    case 'tools':
      openTools()
      break
    case 'global-settings':
    case 'settings':
      void openGlobalSettingsDialog()
      break
    case 'message-parser':
      messagePanelRef.value?.openMessageParserDialog?.()
      break
    case 'download-point-table-template':
      void downloadPointTableTemplate()
      break
    default:
      console.warn('Unknown menu action:', action)
  }
}

const toggleGlobalSimulation = async () => {
  if (!currentStationId.value) {
    ElMessage.warning(t('slave.app.messages.selectStationFirst'))
    return
  }

  const nextEnabled = !simulationProfile.enabled
  if (nextEnabled) {
    if (!isRunning.value) {
      ElMessage.warning(t('slave.app.messages.startListeningBeforeSimulation'))
      return
    }
    await notifyLocalOnlySimulationIfNeeded(t('slave.app.actions.globalSimulation'))
  }

  const payload = {
    enabled: nextEnabled,
    interval_ms: Math.max(1000, Math.round(Number(simulationProfile.interval_ms) || 1000)),
    delta_ratio: Math.min(1, Math.max(0.001, Number(simulationProfile.delta_ratio) || 0.01)),
    min_value: Number(simulationProfile.min_value ?? -1000),
    max_value: Number(simulationProfile.max_value ?? 1000),
    seed: Number(simulationProfile.seed) || Date.now(),
    auto_broadcast: simulationProfile.auto_broadcast !== false,
  }

  try {
    await updateSlaveSimulationProfile(currentStationId.value, payload)
    simulationProfile.enabled = nextEnabled
    await syncFromBackend()
    ElMessage.success(
      nextEnabled
        ? t('slave.app.messages.globalSimulationStarted')
        : t('slave.app.messages.globalSimulationStopped'),
    )
  } catch (error) {
    ElMessage.error(
      t('slave.app.messages.globalSimulationFailed', {
        action: nextEnabled
          ? t('slave.app.messages.globalSimulationStartAction')
          : t('slave.app.messages.globalSimulationStopAction'),
        error: String(error),
      }),
    )
  }
}

const handleSlaveCommand = (command: string) => {
  switch (command) {
    case 'toggle-listener':
      void (async () => {
        await toggleServer()
        syncUiForCurrentStation()
        await syncFromBackend()
      })()
      break
    case 'avalanche-test':
      if (!ensureLiveCommandReady(t('slave.app.actions.avalancheTest'))) return
      void (async () => {
        await notifyLocalOnlySimulationIfNeeded(t('slave.app.actions.avalancheTest'))
        await handleAvalancheTest(resolveWriteCommonAddress(undefined))
        await syncFromBackend()
      })()
      break
    case 'force-send':
      if (!ensureLiveCommandReady(t('slave.app.actions.forceUpload'))) return
      if (currentStationId.value) {
        void (async () => {
          try {
            if (!(await hasDataTransferConnection())) {
              ElMessage.warning(t('slave.app.messages.forceUploadRequiresConnection'))
              return
            }
            const commonAddress = resolveWriteCommonAddress(undefined)
            if (commonAddress == null) return
            const uploaded = await forceUploadSlaveData(currentStationId.value, commonAddress)
            await syncFromBackend()
            if (uploaded === 0) {
              ElMessage.warning(t('slave.app.messages.forceUploadNoPoints'))
              return
            }
            ElMessage.success(t('slave.app.messages.backendUploadTriggered'))
          } catch (error) {
            ElMessage.error(t('slave.app.messages.forceUploadFailed', { error: String(error) }))
          }
        })()
      }
      break
    case 'toggle-simulation':
      void toggleGlobalSimulation()
      break
    case 'global-mapping':
      {
        const mappingContext = resolveGlobalMappingContext()
        if (!mappingContext) {
          ElMessage.warning(t('slave.dashboard.mapping.selectSlaveFirst'))
          return
        }
        contentPanelRef.value?.openGlobalMapping(mappingContext)
      }
      break
    default:
      console.warn('Unknown command:', command)
  }
}

const normalizeOptionalSlaveId = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  return normalized > 0 ? normalized : null
}

let syncPointRuntime = async () => {}
const {
  handleDataPointEdit,
  handleImportDataPoints,
  normalizeOptionalCommonAddress,
  resolveSimulationCommonAddress,
  resolveWriteCommonAddress,
} = useSlavePointPersistence({
  currentStationId,
  selectedSlaveId,
  selectedSlave,
  slavesByStation,
  serverConfig,
  syncFromBackend: () => syncPointRuntime(),
  rebuildDeviceTree: () => deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.(),
})

const resolveSlaveMessageUiKeys = (runtimeConnectionId: string, commonAddress: number | null) => {
  const trimmedConnectionId = String(runtimeConnectionId || '').trim()
  const stationsForPanel = currentStationList.value
  const connectionStation = stationsForPanel.find(
    (station) =>
      station.isConnectionLevel &&
      String(station.runtimeConnectionId || '').trim() === trimmedConnectionId,
  )
  const slaveStation =
    trimmedConnectionId && commonAddress != null
      ? stationsForPanel.find(
          (station) =>
            !station.isConnectionLevel &&
            String(station.runtimeConnectionId || '').trim() === trimmedConnectionId &&
            Number(station.commonAddress ?? 0) === commonAddress,
        )
      : null

  return {
    uiConnectionKey: connectionStation?.uiConnectionKey || connectionStation?.id || '',
    slaveUiKey: slaveStation?.slaveUiKey || slaveStation?.id || '',
  }
}

const handleSlavePointTableImportApplied = async (result: PointTableImportApplyResult) => {
  const targetSlaveId = result.target.kind === 'slave-station-slave' ? result.target.slave_id : null
  if (targetSlaveId != null) {
    contentPanelRef.value?.beginPointTableReplacement?.(targetSlaveId)
  }
  try {
    if (result.target.kind === 'slave-station-slave') {
      selectedSlaveId.value = result.target.slave_id
    }

    await refreshStationPointTypeSummary()
    await syncFromBackend(['points'])
    if (!hasReachedPointSyncCursor(slavePointData.cursor(), result.applied_point_cursor)) {
      await syncFromBackend(['points'])
    }
    deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.()
  } catch (error) {
    ElMessage.warning(t('slave.app.messages.importRefreshFailed', { error: String(error) }))
  } finally {
    if (targetSlaveId != null) {
      contentPanelRef.value?.completePointTableReplacement?.(targetSlaveId)
    }
  }
}

const { activePointSimulation, handleDataPointSimulate, stopPointWaveformSimulation } =
  useSlavePointSimulation({
    currentStationId,
    contentPanelRef,
    normalizeOptionalSlaveId,
    resolveWriteCommonAddress,
    resolveSimulationCommonAddress,
    notifyLocalOnlySimulationIfNeeded: (actionLabel) =>
      notifyLocalOnlySimulationIfNeeded(actionLabel),
    syncFromBackend: () => syncPointRuntime(),
  })

const handleInspectMessage = (address: number) => {
  messagePanelRef.value?.applyFocusFilter?.({
    commonAddress: resolveWriteCommonAddress(undefined),
    ioa: address,
  })
}

const resolveSlaveSoeDataType = (typeId: number): string | null => {
  const source = getIec104Capability(typeId)
  if (!source || source.point_role !== 'monitor') return null
  return (
    getInstalledIec104Capabilities().find(
      (candidate) =>
        candidate.point_role === 'monitor' &&
        candidate.family === source.family &&
        candidate.timestamp_model === 'none',
    )?.name ?? null
  )
}

const findSlaveNodeByCommonAddress = (
  stationId: string,
  commonAddress: number,
): DeviceNode | null => {
  const treeNodes = (deviceTreeRef.value?.deviceTreeData ?? []) as DeviceNode[]
  for (const listener of treeNodes) {
    if (listener.type !== 'listener' || listener.id !== stationId) continue
    for (const child of listener.children ?? []) {
      if (child.type !== 'slave') continue
      const childCommonAddress = Number(child.commonAddress ?? child.common_address ?? 0)
      if (childCommonAddress === commonAddress) {
        return child
      }
    }
  }
  return null
}

const handleClearSlaveSoeEvents = async () => {
  if (!currentStationId.value) return
  await clearSlaveSoeEvents(currentStationId.value)
  resetStationSoeState(currentStationId.value)
}

const handleSoeSelect = async (event: BackendSoeEvent) => {
  const stationId = currentStationId.value
  if (!stationId) return

  const targetDataType = resolveSlaveSoeDataType(event.type_id)
  const targetSlaveId = Number(event.slave_id ?? 0)
  const targetSlave =
    (targetSlaveId > 0 ? findLatestSlaveNode(targetSlaveId, stationId) : null) ??
    findSlaveNodeByCommonAddress(stationId, Number(event.common_address))

  if (targetSlave) {
    selectedSlave.value = targetSlave
    selectedSlaveId.value =
      Number(targetSlave.slaveId ?? targetSlave.id ?? 0) || selectedSlaveId.value
    await nextTick()
  }

  if (targetDataType && contentPanelRef.value) {
    const activeSlave = targetSlave ?? resolveLatestSlaveNode(selectedSlave.value)
    const activeSlaveId = resolveSlaveIdFromNode(activeSlave)
    if (activeSlaveId != null) {
      contentPanelRef.value.focusPoint?.({
        parentStationId: stationId,
        parentSlaveId: activeSlaveId,
        slaveId: activeSlaveId,
        slaveCommonAddress: Number(event.common_address),
        connectionName: findLatestListenerNode(stationId)?.name ?? stationName.value,
        slaveName:
          activeSlave?.name ??
          activeSlave?.label ??
          t('slave.app.slaveWithId', { id: activeSlaveId }),
        dataType: targetDataType,
        label: targetDataType,
        address: event.ioa,
      })
    }
  }

  messagePanelRef.value?.clearFocusFilter?.()
}

const handleMessageSelect = (_message: any) => {}

const handleDeviceRefreshRequested = () => {
  void (async () => {
    const rows = await refreshStations()
    if (!currentStationId.value && rows.length > 0) {
      await selectStation(rows[0].station_id)
      await syncMessageTrackingState(rows[0].station_id)
      syncUiForCurrentStation()
    } else if (
      currentStationId.value &&
      !rows.some((station) => station.station_id === currentStationId.value)
    ) {
      const fallbackStation = rows[0]
      if (fallbackStation) {
        await selectStation(fallbackStation.station_id)
        await syncMessageTrackingState(fallbackStation.station_id)
        syncUiForCurrentStation()
      } else {
        selectedSlave.value = null
      }
    }

    await refreshStationPointTypeSummary()
    const slaveCandidates = slavesByStation.value[currentStationId.value] ?? []
    if (
      selectedSlaveId.value != null &&
      !slaveCandidates.some((item) => item.id === selectedSlaveId.value)
    ) {
      selectedSlaveId.value = null
    }
    syncUiForCurrentStation()
    await syncFromBackend()
    deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.()
  })()
}

const {
  handleMessagePanelVisibilityChange,
  handleSoePanelVisibilityChange,
  syncMessageTrackingState,
  syncFromBackend,
  startRuntimeSync,
  stopRuntimeSync,
  resetStationSoeState,
} = useSlaveSync({
  currentStationId,
  isMessagePanelHidden,
  isSoePanelVisible,
  stations,
  selectedSlaveId,
  setStationConnections: (stationId, connections) => {
    runtimeConnectionsByStation.value = {
      ...runtimeConnectionsByStation.value,
      [stationId]: connections,
    }
  },
  stationRuntimeStatusMap,
  slaveStats,
  statusBarSyncRevision,
  pointData: slavePointData,
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
  resolveMessageUiKeys: resolveSlaveMessageUiKeys,
  syncUiForCurrentStation,
})
syncPointRuntime = () => syncFromBackend()

const handleMessagePanelHidden = async (hidden: boolean) => {
  syncMessagePanelHiddenState(hidden)
  await handleMessagePanelVisibilityChange(hidden)
}

const handleSoeVisibilityChange = async (visible: boolean) => {
  isSoePanelVisible.value = visible
  await handleSoePanelVisibilityChange(visible)
}

const handleStationChange = async (stationId: string) => {
  stationTimeOffsetMs.value = 0
  if (!stationId) {
    slavePointData.clear()
    backendMessages.value = []
    lastMessageId.value = 0
    messagePanelRef.value?.clearMessages()
    clearSelection()
    stationRuntimeStatusMap.value = {}
    return
  }

  // 先清空旧站数据，再切换 stationId，防止 watch 用旧 dataPoints 合并到新站
  slavePointData.clear(stationId)
  backendMessages.value = []
  lastMessageId.value = 0
  messagePanelRef.value?.clearMessages()
  await selectStation(stationId)
  await syncMessageTrackingState(stationId)
  await refreshStationPointTypeSummary()
  clearSelection()
  syncUiForCurrentStation()
  await syncFromBackend()
  deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.()
}

const isConnectionBusy = (connection: BackendConnectionInfo) => {
  const status = deriveUnifiedConnStatus(connection.transport_state, connection.data_transfer_state)
  return (
    status.severity === 'status-processing' ||
    status.severity === 'status-unready' ||
    status.severity === 'status-normal'
  )
}

const isDataTransferConnection = (connection: BackendConnectionInfo) => {
  const status = deriveUnifiedConnStatus(connection.transport_state, connection.data_transfer_state)
  return status.severity === 'status-normal'
}

const hasDataTransferConnection = async (): Promise<boolean> => {
  if (!currentStationId.value) return false
  const connections = await getSlaveConnections(currentStationId.value)
  return connections.some(isDataTransferConnection)
}

const notifyLocalOnlySimulationIfNeeded = async (actionLabel: string): Promise<boolean> => {
  if (!currentStationId.value) return false

  let hasBroadcastLink = false
  try {
    hasBroadcastLink = await hasDataTransferConnection()
  } catch {
    hasBroadcastLink = false
  }

  if (hasBroadcastLink) return true

  const now = Date.now()
  if (now - lastLocalOnlyHintAt > 1200) {
    lastLocalOnlyHintAt = now
    ElMessage.warning(t('slave.app.messages.localOnlySimulation', { action: actionLabel }))
  }
  return false
}

const ensureStationDisconnectedForConfig = async (
  station: DeviceNode | null,
  actionLabel: string,
): Promise<boolean> => {
  const stationId = resolveStationIdFromNode(station) || currentStationId.value
  if (!stationId) return false

  try {
    const connections = await getSlaveConnections(stationId)
    if (connections.some(isConnectionBusy)) {
      ElMessage.warning(t('slave.app.messages.stationBusy', { action: actionLabel }))
      return true
    }
  } catch (error) {
    ElMessage.warning(t('slave.app.messages.checkConnectionFailed', { error: String(error) }))
    return false
  }

  return false
}

const ensureLiveCommandReady = (actionLabel: string): boolean => {
  if (!currentStationId.value) {
    ElMessage.warning(t('slave.app.messages.selectStationBeforeAction', { action: actionLabel }))
    return false
  }
  if (!selectedSlave.value) {
    ElMessage.warning(t('slave.app.messages.selectStationBeforeAction', { action: actionLabel }))
    return false
  }
  if (!isRunning.value) {
    ElMessage.warning(t('slave.app.messages.startListeningBeforeAction', { action: actionLabel }))
    return false
  }
  return true
}

const { handleStationProfileSubmit } = useSlaveStationProfileSubmission({
  selectedSlave,
  selectedSlaveId,
  currentStationId,
  isRunning,
  stationName,
  serverConfig,
  linkParams,
  redundancyMode,
  resolveLatestSlaveNode,
  resolveStationIdFromNode,
  findLatestListenerNode,
  handleStationChange,
  ensureStationDisconnectedForConfig,
  createStation,
  stopServer,
  refreshStations,
  refreshStationPointTypeSummary,
  syncFromBackend: () => syncFromBackend(),
  syncUiForCurrentStation,
  rebuildDeviceTree: () => deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.(),
})

onMounted(async () => {
  await initializeSlaveStation()
  messagePanelRef.value?.hidePanel?.()
  await loadSlaveGlobalSettings({ silent: true })
  syncUiForCurrentStation()
  await refreshStationPointTypeSummary()
  await syncFromBackend()
  deviceTreeRef.value?.rebuildDeviceTreeFromBackend?.()
  await startRuntimeSync()
})

onUnmounted(() => {
  void stopPointWaveformSimulation({ clearUi: false, reason: 'unmount' })
  stopRuntimeSync()
})
</script>

<style scoped>
.slave-app > *:not(.bottom-panel) {
  margin: 0;
}

.unified-toolbar-area {
  padding: 8px;
  background: #f5f7fa;
}

.status-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: #64748b;
  padding: 6px 12px;
  background: #f1f5f9;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: #ef4444;
  transition: background-color 0.3s;
}

.status-indicator.active .status-dot {
  background-color: #10b981;
}

.station-switcher {
  padding: 8px;
  border-bottom: 1px solid #e5e7eb;
}

/* 响应式设计 */

@media (max-width: 1200px) {
  .left-panel {
    width: 230px;
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
