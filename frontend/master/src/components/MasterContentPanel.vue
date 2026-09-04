<template>
  <div class="master-content-panel">
    <div class="manager-content">
      <div v-if="activeTabs.length === 0" class="empty-state">
        <el-empty :image-size="120">
          <template #description>
            <div class="empty-guide">
              <p class="empty-title">{{ t('master.contentPanel.empty.title') }}</p>
              <p class="empty-hint">{{ t('master.contentPanel.empty.hint') }}</p>
            </div>
          </template>
        </el-empty>
      </div>

      <div v-else class="tabs-container">
        <div class="tabs-header">
          <div class="tabs-wrapper app-capsule-tabs-shell">
            <el-tabs v-model="activeTabName" class="tabs-only app-capsule-tabs">
              <el-tab-pane v-for="tab in activeTabs" :key="tab.id" :name="tab.id">
                <template #label>
                  <el-dropdown
                    trigger="contextmenu"
                    @command="handleTabContextMenu($event, tab.id)"
                    popper-class="app-menu-dropdown"
                  >
                    <div class="custom-tab-label app-capsule-tab-label">
                      <el-icon class="tab-icon app-capsule-tab-icon"
                        ><component :is="getTabIcon(tab.dataType)"
                      /></el-icon>
                      <span class="tab-title app-capsule-tab-title">{{
                        getTabDisplayLabel(tab)
                      }}</span>
                      <el-tooltip
                        :content="getTabTooltip(tab)"
                        placement="bottom"
                        :enterable="false"
                      >
                        <span class="tab-badge app-capsule-tab-badge">{{ getTabBadge(tab) }}</span>
                      </el-tooltip>
                      <button
                        type="button"
                        class="tab-close-btn app-capsule-tab-close"
                        @click.stop="removeTab(tab.id)"
                      >
                        <el-icon><Close /></el-icon>
                      </button>
                    </div>
                    <template #dropdown>
                      <el-dropdown-menu>
                        <el-dropdown-item command="close">{{
                          t('master.contentPanel.tabs.closeCurrent')
                        }}</el-dropdown-item>
                        <el-dropdown-item command="closeOthers">{{
                          t('master.contentPanel.tabs.closeOthers')
                        }}</el-dropdown-item>
                        <el-dropdown-item command="closeAll" divided class="menu-item-danger">{{
                          t('master.contentPanel.tabs.closeAll')
                        }}</el-dropdown-item>
                      </el-dropdown-menu>
                    </template>
                  </el-dropdown>
                </template>
              </el-tab-pane>
            </el-tabs>
          </div>
        </div>

        <MasterContentToolbar
          :current-tab="currentTab"
          :current-tab-is-control="currentTabIsControl"
          :current-control-tab-uses-built-in="currentControlTabUsesBuiltIn"
          :show-control-subtype-filter="showControlSubtypeFilter"
          :has-active-filters="hasActiveFilters"
          :active-filter-summary="activeFilterSummary"
          :quick-filters="quickFilters"
          :recent-window-options="recentWindowOptions"
          :remote-control-filter-slider-style="remoteControlFilterSliderStyle"
          :column-visibility="columnVisibility"
          @refresh="handleRefreshData"
          @clear-filters="clearQuickFilters"
          @density-change="handleDensityChange"
        />

        <MasterPointTable
          v-if="currentTab"
          :current-tab="currentTab"
          :current-tab-empty-state="currentTabEmptyState"
          :virtual-table-rows="virtualTableRows"
          :table-size="tableSize"
          :set-table-ref="setDataTableRef"
          :get-table-row-key="getTableRowKey"
          :resolve-table-row-class-name="resolveTableRowClassName"
          :resolve-table-row-style="resolveTableRowStyle"
          :handle-sort-change="handleSortChange"
          :handle-row-context-menu="handleRowContextMenu"
          :is-virtual-spacer-row="isVirtualSpacerRow"
          :format-ioa-address="formatIoaAddress"
          :show-control-type-column="showControlTypeColumn"
          :column-visibility="columnVisibility"
          :get-control-type-label="getControlTypeLabel"
          :current-tab-is-control="currentTabIsControl"
          :current-tab-has-quality="currentTabHasQuality"
          :has-value-descriptor-detail="hasValueDescriptorDetail"
          :get-value-descriptor-tooltip="getValueDescriptorTooltip"
          :get-semantic-value-class="getSemanticValueClass"
          :format-value="formatValue"
          :quality-tag-type="qualityTagType"
          :control-status-tooltip="controlStatusTooltip"
          :control-status-tag-type="controlStatusTagType"
          :control-status-text="controlStatusText"
          :format-point-source="formatPointSource"
          :format-timestamp="formatTimestamp"
          :open-edit-dialog="openEditDialog"
          :open-history-dialog="openHistoryDialog"
          :handle-control-data-point="handleControlDataPoint"
        />
      </div>
    </div>

    <el-dialog
      v-model="showEditDialog"
      width="560px"
      :before-close="handleEditDialogBeforeClose"
      :close-on-click-modal="false"
      :close-on-press-escape="pointEditCloseEnabled"
      :show-close="pointEditCloseEnabled"
      :lock-scroll="false"
      class="point-edit-dialog app-dialog-shell"
    >
      <template #header>
        <div class="app-dialog-header">
          <div>
            <div class="app-dialog-title">{{ t('master.contentPanel.edit.title') }}</div>
            <div class="app-dialog-subtitle-badges" v-if="editDialogSubtitleBadges.length > 0">
              <span
                v-for="(badge, index) in editDialogSubtitleBadges"
                :key="`master-point-edit-badge-${index}`"
                class="app-dialog-subtitle-badge"
              >
                {{ badge }}
              </span>
            </div>
          </div>
        </div>
      </template>

      <div
        class="app-dialog-body app-dialog-scroll-shadow conn-flat-body"
        @keyup.enter="handleEditDialogEnter"
      >
        <el-form
          v-if="editingPointMeta"
          :model="editingPointMeta"
          class="app-dialog-form point-edit-form"
        >
          <section class="conn-section">
            <div class="conn-section-title">{{ t('master.contentPanel.edit.basic') }}</div>
            <div class="app-dialog-field-grid--2">
              <label class="app-dialog-field-label">{{
                t('master.contentPanel.edit.address')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control">
                <el-input
                  :model-value="formatIoaAddress(editingPointMeta.address)"
                  disabled
                  class="app-dialog-input point-edit-readonly font-mono"
                />
              </el-form-item>
              <label class="app-dialog-field-label">{{ t('master.contentPanel.edit.name') }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control">
                <el-input
                  v-model="editingPointMeta.name"
                  maxlength="120"
                  class="app-dialog-input"
                />
              </el-form-item>
              <label class="app-dialog-field-label app-dialog-field--full">{{
                t('master.contentPanel.edit.description')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control app-dialog-field--full">
                <el-input
                  v-model="editingPointMeta.description"
                  maxlength="240"
                  class="app-dialog-input"
                />
              </el-form-item>
            </div>
          </section>
        </el-form>
      </div>

      <template #footer>
        <div class="app-dialog-footer">
          <div class="app-dialog-actions">
            <el-button
              class="app-btn-secondary"
              :disabled="!pointEditCloseEnabled"
              @click="closeEditDialog"
              >{{ t('common.cancel') }}</el-button
            >
            <el-button type="primary" :loading="savingPointMeta" @click="savePointMeta">{{
              t('common.save')
            }}</el-button>
          </div>
        </div>
      </template>
    </el-dialog>

    <div
      v-show="contextMenu.visible"
      class="app-context-menu"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      @click="closeContextMenu"
    >
      <div class="app-context-menu-item" @click="handleContextAction('edit')">
        <el-icon><Edit /></el-icon> {{ t('master.contentPanel.actions.editPoint') }}
      </div>
      <div class="app-context-menu-divider"></div>
      <div
        class="app-context-menu-item"
        @click="handleContextAction(currentTabIsControl ? 'command' : 'history')"
      >
        <el-icon><component :is="currentTabIsControl ? Operation : Clock" /></el-icon>
        {{
          currentTabIsControl
            ? t('master.contentPanel.actions.sendCommand')
            : t('master.contentPanel.actions.viewHistory')
        }}
      </div>
      <div
        v-if="!currentTabIsControl"
        class="app-context-menu-item"
        @click="handleContextAction('read')"
      >
        <el-icon><Operation /></el-icon> {{ t('master.contentPanel.actions.readCurrentValue') }}
      </div>
      <div class="app-context-menu-divider"></div>
      <div class="app-context-menu-item" @click="handleContextAction('copy-addr')">
        <el-icon><CopyDocument /></el-icon> {{ t('master.contentPanel.actions.copyAddress') }}
      </div>
      <div class="app-context-menu-item" @click="handleContextAction('copy-json')">
        <el-icon><CopyDocument /></el-icon> {{ t('master.contentPanel.actions.copyRowJson') }}
      </div>
    </div>

    <MasterHistoryDialog
      :visible="historyDialog.visible"
      :dialog="historyDialog"
      :latest-entry="historyDialogLatestEntry"
      :entries="historyDialogEntries"
      :ca-text="historyDialogCaText"
      :format-ioa-address="formatIoaAddress"
      @update:visible="historyDialog.visible = $event"
      @toggle-entry="toggleHistoryEntry"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, toRef, watch } from 'vue'
import { Clock, Close, CopyDocument, Edit, Operation } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

import type { ControlCommandType, SlaveConnection } from '@/types/master'
import type {
  MasterContentTabRow as TabRow,
  MasterPointMetaUpdatePayload,
} from '@/types/masterContentPanel'
import type { MasterPointData } from '@/composables/useMasterPointData'
import type { PointDef } from '@shared/api/types'
import {
  formatControlConfirmationTextLocalized,
  formatInformationValueSuffix,
  formatInformationValueTooltipLocalized,
  formatIec104TypeCompactLabel,
  formatKeyValueTooltip,
  hasPointQuality,
  hasInformationValueDetail,
  formatSemanticPointValueLocalized,
  iec104TypeNameToId,
  semanticPointValueClass,
} from '@shared/api/iec104'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'
import { t } from '@shared/i18n'
import { translateApiError } from '@shared/i18n/errors'

import MasterContentToolbar from './MasterContentToolbar.vue'
import MasterHistoryDialog from './MasterHistoryDialog.vue'
import MasterPointTable from './MasterPointTable.vue'
import { useMasterContentInteractions } from '@/composables/useMasterContentInteractions'

const props = withDefaults(
  defineProps<{
    selectedSlave?: SlaveConnection | null
    pointData: MasterPointData
    messages?: unknown[]
    profilePointDefsMap?: Record<string, PointDef[]>
    ioaDisplayFormat?: 'dec' | 'hex'
    savePointMeta: (payload: MasterPointMetaUpdatePayload) => Promise<void>
  }>(),
  {
    selectedSlave: null,
    messages: () => [],
    profilePointDefsMap: () => ({}),
    ioaDisplayFormat: 'dec',
  },
)

const emit = defineEmits<{
  sendCommand: [command: ControlCommandType, params?: any]
  connect: [slaveId: string]
  disconnect: [slaveId: string]
  refreshData: []
  activeTabChange: [tab: { profileId: string; dataType: string } | null]
}>()

function formatTimestamp(value: string) {
  const ts = Date.parse(String(value))
  if (!Number.isFinite(ts)) return '—'
  const d = new Date(ts)
  const pad = (n: number, len = 2) => String(n).padStart(len, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`
}

function formatOptionalCp56(value: string | null | undefined) {
  if (!value) return '—'
  return formatTimestamp(value)
}

function formatPointSource(source: string | null | undefined) {
  if (source === 'built_in') return t('master.contentPanel.pointSources.builtIn')
  if (source === 'manual') return t('master.contentPanel.pointSources.manual')
  if (source === 'imported') return t('master.contentPanel.pointSources.imported')
  return '—'
}

function clockTime(value: string) {
  const ts = Date.parse(String(value))
  if (!Number.isFinite(ts)) return '—'
  const d = new Date(ts)
  const pad = (n: number, len = 2) => String(n).padStart(len, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`
}

function formatCot(raw: unknown) {
  const cause = Math.trunc(Number(raw))
  if (!Number.isFinite(cause) || cause < 0) return '—'
  const cot = cause & 0x3f
  const negative = (cause & 0x40) !== 0
  const label =
    (
      {
        1: t('master.contentPanel.cot.labels.periodic'),
        3: t('master.contentPanel.cot.labels.spontaneous'),
        6: t('master.contentPanel.cot.labels.activation'),
        7: t('master.contentPanel.cot.labels.activationConfirm'),
        8: t('master.contentPanel.cot.labels.deactivation'),
        9: t('master.contentPanel.cot.labels.deactivationConfirm'),
        10: t('master.contentPanel.cot.labels.activationTermination'),
        20: t('master.contentPanel.cot.labels.interrogation'),
        44: t('master.contentPanel.cot.labels.unknownType'),
        45: t('master.contentPanel.cot.labels.unknownCot'),
        46: t('master.contentPanel.cot.labels.unknownCommonAddress'),
        47: t('master.contentPanel.cot.labels.unknownObjectAddress'),
      } as Record<number, string>
    )[cot] ?? t('master.contentPanel.cot.labels.cot')
  return t(negative ? 'master.contentPanel.cot.negative' : 'master.contentPanel.cot.normal', {
    label,
    cot,
  })
}

const {
  activeTabs,
  activeTabName,
  tableSize,
  dataTableRef,
  quickFilters,
  columnVisibility,
  recentWindowOptions,
  currentTab,
  currentTabIsControl,
  currentTabEmptyState,
  currentControlTabUsesBuiltIn,
  showControlSubtypeFilter,
  showControlTypeColumn,
  hasActiveFilters,
  activeFilterSummary,
  virtualTableRows,
  isVirtualSpacerRow,
  resolveTableRowClassName,
  resolveTableRowStyle,
  getTableRowKey,
  remoteControlFilterSliderStyle,
  historyDialog,
  historyDialogCaText,
  historyDialogEntries,
  historyDialogLatestEntry,
  formatIoaAddress,
  clearQuickFilters,
  handleDensityChange,
  handleRefreshData,
  handleSortChange,
  handleControlDataPoint,
  handleReadDataPoint,
  openHistoryDialog,
  toggleHistoryEntry,
  addTab,
  removeTab,
  closeOtherTabs,
  closeAllTabs,
  getTabIcon,
  getTabDisplayLabel,
  getTabBadge,
  getTabTooltip,
  getControlTypeLabel,
  focusPoint,
  beginPointTableReplacement,
  completePointTableReplacement,
} = useMasterContentInteractions({
  selectedSlave: toRef(props, 'selectedSlave'),
  pointData: props.pointData,
  profilePointDefsMap: toRef(props, 'profilePointDefsMap'),
  ioaDisplayFormat: toRef(props, 'ioaDisplayFormat'),
  emitRefreshData: () => emit('refreshData'),
  emitSendCommand: (command, params) => emit('sendCommand', command, params),
  formatCot,
  formatOptionalCp56,
  formatTimestamp,
})
const setDataTableRef = (instance: unknown) => {
  dataTableRef.value = instance
}

const currentTabHasQuality = computed(() =>
  hasPointQuality(currentTab.value ? iec104TypeNameToId(currentTab.value.dataType) : null),
)

watch(
  currentTab,
  (tab) => {
    emit(
      'activeTabChange',
      tab ? { profileId: String(tab.profileId), dataType: tab.dataType } : null,
    )
  },
  { immediate: true },
)

const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  row: null as TabRow | null,
})
const showEditDialog = ref(false)
const savingPointMeta = ref(false)
const editingPointMeta = ref<MasterPointMetaUpdatePayload | null>(null)
const editDialogSnapshot = ref('')
const editDialogSubtitleBadges = computed(() => {
  const badges: string[] = []
  if (props.selectedSlave?.linkName) badges.push(props.selectedSlave.linkName)
  if (props.selectedSlave?.name) badges.push(props.selectedSlave.name)
  const typeId = currentTab.value ? iec104TypeNameToId(currentTab.value.dataType) : null
  if (typeId != null) badges.push(formatIec104TypeCompactLabel(typeId))
  return badges
})

const buildEditDialogSnapshot = () =>
  JSON.stringify({
    name: editingPointMeta.value?.name ?? '',
    description: editingPointMeta.value?.description ?? '',
    addresses: editingPointMeta.value?.addresses ?? [],
  })

const hasUnsavedEditDialogChanges = () => {
  if (!editingPointMeta.value || !editDialogSnapshot.value) return false
  return buildEditDialogSnapshot() !== editDialogSnapshot.value
}

const isContextMenuTarget = (target: EventTarget | null) =>
  target instanceof HTMLElement && Boolean(target.closest('.app-context-menu'))

const handleGlobalContextMenuClose = (event: MouseEvent) => {
  if (isContextMenuTarget(event.target)) return
  closeContextMenu()
}

const handleGlobalClickClose = (event: MouseEvent) => {
  if (isContextMenuTarget(event.target)) return
  closeContextMenu()
}

const closeContextMenu = () => {
  contextMenu.visible = false
  contextMenu.row = null
}

const openEditDialog = (row: TabRow) => {
  editingPointMeta.value = {
    address: row.address,
    addresses: [row.address],
    dataType: row.dataType,
    typeId: row.typeId,
    name: row.name ?? '',
    description: row.description ?? '',
  }
  editDialogSnapshot.value = buildEditDialogSnapshot()
  showEditDialog.value = true
  closeContextMenu()
}

const performCloseEditDialog = () => {
  showEditDialog.value = false
  savingPointMeta.value = false
  editingPointMeta.value = null
  editDialogSnapshot.value = ''
}

const {
  closeEnabled: pointEditCloseEnabled,
  handleBeforeClose: handleEditDialogBeforeClose,
  requestClose: closeEditDialog,
} = useDialogCloseGuard({
  isDirty: hasUnsavedEditDialogChanges,
  isBlocked: () => savingPointMeta.value,
  onClose: performCloseEditDialog,
})

const handleEditDialogEnter = () => {
  if (!showEditDialog.value || savingPointMeta.value) return
  void savePointMeta()
}

const savePointMeta = async () => {
  if (!editingPointMeta.value || savingPointMeta.value) return
  const normalizedName = editingPointMeta.value.name.trim()
  if (!normalizedName) {
    ElMessage.warning(t('master.contentPanel.edit.nameRequired'))
    return
  }

  savingPointMeta.value = true
  try {
    await props.savePointMeta({
      ...editingPointMeta.value,
      name: normalizedName,
      description: editingPointMeta.value.description.trim(),
    })
    performCloseEditDialog()
    ElMessage.success(t('master.contentPanel.edit.saved'))
  } catch (error) {
    ElMessage.error(
      t('master.contentPanel.edit.saveFailed', { error: translateApiError(error, String(error)) }),
    )
    savingPointMeta.value = false
  }
}

const handleRowContextMenu = (row: TabRow, column: unknown, event: MouseEvent) => {
  void column
  if (isVirtualSpacerRow(row)) return
  event.preventDefault()
  event.stopPropagation()
  contextMenu.row = row
  contextMenu.x = event.clientX
  contextMenu.y = event.clientY
  contextMenu.visible = true
}

const handleContextAction = async (
  action: 'edit' | 'history' | 'command' | 'read' | 'copy-addr' | 'copy-json',
) => {
  const row = contextMenu.row
  if (!row) return

  switch (action) {
    case 'edit':
      openEditDialog(row)
      return
    case 'history':
      openHistoryDialog(row)
      break
    case 'command':
      handleControlDataPoint(row)
      break
    case 'read':
      handleReadDataPoint(row)
      break
    case 'copy-addr':
      try {
        const displayAddress = formatIoaAddress(row.address)
        await navigator.clipboard.writeText(displayAddress)
        ElMessage.success(
          t('master.contentPanel.context.copiedAddress', { address: displayAddress }),
        )
      } catch {
        ElMessage.error(t('master.contentPanel.context.copyFailed'))
      }
      break
    case 'copy-json':
      try {
        await navigator.clipboard.writeText(JSON.stringify(row, null, 2))
        ElMessage.success(t('master.contentPanel.context.copiedRow'))
      } catch {
        ElMessage.error(t('master.contentPanel.context.copyFailed'))
      }
      break
  }

  closeContextMenu()
}

const handleTabContextMenu = (command: string, tabId: string) => {
  switch (command) {
    case 'close':
      removeTab(tabId)
      break
    case 'closeOthers':
      closeOtherTabs(tabId)
      break
    case 'closeAll':
      closeAllTabs()
      break
  }
}

onMounted(() => {
  document.addEventListener('mousedown', handleGlobalClickClose, true)
  document.addEventListener('contextmenu', handleGlobalContextMenuClose, true)
})

onUnmounted(() => {
  document.removeEventListener('mousedown', handleGlobalClickClose, true)
  document.removeEventListener('contextmenu', handleGlobalContextMenuClose, true)
})

function controlStatusText(row: TabRow): string {
  return (
    (
      {
        Idle: t('master.contentPanel.controlStatus.idle'),
        Selected: t('master.contentPanel.controlStatus.selected'),
        Executing: t('master.contentPanel.controlStatus.executing'),
        Ok: t('master.contentPanel.controlStatus.ok'),
        Ng: t('master.contentPanel.controlStatus.ng'),
        Timeout: t('master.contentPanel.controlStatus.timeout'),
      } as Record<string, string>
    )[String(row.controlStatusSnapshot?.state ?? 'Idle')] ??
    t('master.contentPanel.controlStatus.idle')
  )
}

function controlStatusTagType(row: TabRow): 'info' | 'warning' | 'success' | 'danger' {
  const state = String(row.controlStatusSnapshot?.state ?? 'Idle')
  if (state === 'Selected' || state === 'Executing') return 'warning'
  if (state === 'Ok') return 'success'
  if (state === 'Ng' || state === 'Timeout') return 'danger'
  return 'info'
}

function controlStatusTooltip(row: TabRow): string {
  const snapshot = row.controlStatusSnapshot
  if (!snapshot)
    return formatKeyValueTooltip([
      {
        label: t('master.contentPanel.controlStatus.stateDescription'),
        value: t('master.contentPanel.controlStatus.noRecentTrace'),
      },
    ])
  const rows: Array<{ label: string; value: string }> = []
  if (snapshot.actcon_at)
    rows.push({
      label: 'ActCon',
      value: `${formatControlConfirmationTextLocalized(snapshot.actcon_negative)} ${clockTime(snapshot.actcon_at)}`,
    })
  if (snapshot.actterm_at) rows.push({ label: 'ActTerm', value: clockTime(snapshot.actterm_at) })
  if (snapshot.latest_cause != null)
    rows.push({
      label: t('master.contentPanel.controlStatus.latestCot'),
      value: formatCot(snapshot.latest_cause),
    })
  if (snapshot.state === 'Timeout')
    rows.push({
      label: t('master.contentPanel.controlStatus.timeoutType'),
      value:
        snapshot.timeout_type === 'select'
          ? t('master.contentPanel.controlStatus.selectTimeout')
          : t('master.contentPanel.controlStatus.executeTimeout'),
    })
  if (snapshot.command_cp56)
    rows.push({
      label: t('master.contentPanel.columns.commandCp56'),
      value: snapshot.command_cp56,
    })
  return formatKeyValueTooltip(
    rows.length > 0
      ? rows
      : [
          {
            label: t('master.contentPanel.controlStatus.stateDescription'),
            value: t('master.contentPanel.controlStatus.idleNoRecentAck'),
          },
        ],
  )
}

function qualityTagType(level: TabRow['qualityLevel']) {
  return level === 'good' ? 'success' : level === 'invalid' ? 'danger' : 'warning'
}

function formatValue(row: TabRow) {
  return `${formatSemanticPointValueLocalized(row.typeId || row.dataType, row.value)}${formatInformationValueSuffix(row.qualityDetail)}`
}

function getValueDescriptorTooltip(row: TabRow) {
  return formatInformationValueTooltipLocalized(row.qualityDetail)
}

function hasValueDescriptorDetail(row: TabRow) {
  return hasInformationValueDetail(row.qualityDetail)
}

function getSemanticValueClass(row: TabRow) {
  return semanticPointValueClass(row.typeId || row.dataType, row.value)
}

defineExpose({ addTab, focusPoint, beginPointTableReplacement, completePointTableReplacement })
</script>

<style scoped>
.master-content-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: #f3f4f6;
  padding: 4px;
}

:deep(.custom-data-table .el-table__body tr.is-located-row > td.el-table__cell) {
  background-color: var(--el-color-primary-light-9) !important;
}
.manager-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 4px;
}
.tabs-container {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.tabs-header {
  flex-shrink: 0;
}
.empty-state {
  flex: 1;
  min-height: 0;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
:deep(.state-badge.el-tag) {
  min-width: 56px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding-inline: 10px;
}
.point-edit-form :deep(.el-form-item) {
  margin-bottom: 0;
}
.master-content-panel :deep(.el-dropdown),
.master-content-panel :deep(.el-dropdown *) {
  color: inherit !important;
}
.point-edit-readonly :deep(.el-input__inner) {
  color: #6b7280;
}
</style>
