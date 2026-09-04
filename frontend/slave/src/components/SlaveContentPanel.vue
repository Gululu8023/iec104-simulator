<template>
  <div class="slave-content-panel">
    <div class="manager-content">
      <!-- 无标签页时的提示 -->
      <div v-if="activeTabs.length === 0" class="empty-state">
        <el-empty :image-size="120">
          <template #description>
            <div class="empty-guide">
              <p class="empty-title">{{ t('slave.contentPanel.empty.title') }}</p>
              <p class="empty-hint">{{ t('slave.contentPanel.empty.hint') }}</p>
            </div>
          </template>
        </el-empty>
      </div>

      <!-- 标签页 -->
      <div v-else class="tabs-container">
        <DataTabs
          v-model="activeTabName"
          :tabs="activeTabs"
          :get-tab-icon="getTabIcon"
          :get-tab-display-label="getTabDisplayLabel"
          :get-tab-tooltip="getTabTooltip"
          :get-tab-badge="getTabBadge"
          @tab-click="handleTabClick"
          @tab-command="handleTabContextMenu($event.command, $event.tabId)"
          @close="removeTab"
        />

        <PointToolbar
          :current-tab="currentTab"
          :has-active-filters="hasActiveFilters"
          :active-filter-summary="activeFilterSummary"
          :quick-filters="quickFilters"
          :recent-window-options="recentWindowOptions"
          :selected-rows="selectedRows"
          :current-tab-is-control="currentTabIsControl"
          :has-active-simulation="hasActiveSimulation"
          :is-current-tab-control-mapping-type="isCurrentTabControlMappingType"
          :is-current-tab-monitor-type="isCurrentTabMonitorType"
          :is-current-tab-integrated-total="isCurrentTabIntegratedTotal"
          :is-current-tab-mapping-family="isCurrentTabMappingFamily"
          :table-size="tableSize"
          :column-visibility="columnVisibility"
          :clear-quick-filters="clearQuickFilters"
          :refresh-data="refreshData"
          :clear-current-tab-selection="clearCurrentTabSelection"
          :reset-data="resetData"
          :toggle-simulation-from-toolbar="toggleSimulationFromToolbar"
          :open-batch-mapping="openBatchMapping"
          :open-batch-gi-group-dialog="openBatchGiGroupDialog"
          :handle-density-change="handleDensityChange"
        />

        <PointTable
          v-if="currentTab"
          :current-tab="currentTab"
          :current-tab-empty-state="currentTabEmptyState"
          :table-ref="setDataTableRef"
          :virtual-table-rows="virtualTableRows"
          :table-size="tableSize"
          :column-visibility="columnVisibility"
          :current-tab-is-control="currentTabIsControl"
          :current-tab-has-quality="currentTabHasQuality"
          :is-current-tab-monitor-type="isCurrentTabMonitorType"
          :is-current-tab-integrated-total="isCurrentTabIntegratedTotal"
          :is-current-tab-mapping-family="isCurrentTabMappingFamily"
          :is-current-tab-control-mapping-type="isCurrentTabControlMappingType"
          :get-table-row-key="getTableRowKey"
          :resolve-table-row-class-name="resolveTableRowClassName"
          :resolve-table-row-style="resolveTableRowStyle"
          :handle-sort-change="handleSortChange"
          :handle-selection-change="handleSelectionChange"
          :handle-selection-cell-click="handleSelectionCellClick"
          :handle-row-context-menu="handleRowContextMenu"
          :is-selectable-table-row="isSelectableTableRow"
          :format-ioa-table-cell="formatIoaTableCell"
          :is-virtual-spacer-row="isVirtualSpacerRow"
          :is-point-simulating="isPointSimulating"
          :get-simulation-tooltip="getSimulationTooltip"
          :get-simulation-icon="getSimulationIcon"
          :has-value-descriptor-detail="hasValueDescriptorDetail"
          :get-value-descriptor-tooltip="getValueDescriptorTooltip"
          :get-semantic-value-class="getSemanticValueClass"
          :format-value="formatValue"
          :get-quality-tooltip="getQualityTooltip"
          :get-quality-type="getQualityType"
          :get-quality-text="getQualityText"
          :get-extra-info-tooltip="getExtraInfoTooltip"
          :get-control-status-tag-type="getControlStatusTagType"
          :get-control-status-text="getControlStatusText"
          :format-gi-group-value="formatGiGroupValue"
          :format-counter-group-value="formatCounterGroupValue"
          :get-cot-column-text="getCotColumnText"
          :get-cp56-column-text="getCp56ColumnText"
          :get-mapping-target-display-text="getMappingTargetDisplayText"
          :get-mapping-target-type-cell-text="getMappingTargetTypeCellText"
          :get-mapping-target-address-cell-text="getMappingTargetAddressCellText"
          :format-point-source="formatPointSource"
          :format-timestamp="formatTimestamp"
          :edit-data-point="editDataPoint"
          :open-simulation="openSimulation"
          :open-single-mapping="openSingleMapping"
        />
      </div>

      <PointEditDialog
        v-model:visible="showEditDialog"
        v-model:gi-group="editingGiGroupValue"
        v-model:counter-group="editingCounterGroupValue"
        :before-close="handleEditDialogBeforeClose"
        :current-tab="currentTab"
        :editing-data-point="editingDataPoint"
        :format-type-compact="formatTypeCompact"
        :is-editing-control-point="isEditingControlPoint"
        :is-editing-single-point="isEditingSinglePoint"
        :is-editing-double-point="isEditingDoublePoint"
        :is-editing-integrated-total="isEditingIntegratedTotal"
        :editing-value-profile="editingValueProfile"
        :protocol-quality-kind="protocolQualityKind"
        :protocol-quality-group-label="protocolQualityGroupLabel"
        :editing-quality-summary-tag-type="editingQualitySummaryTagType"
        :editing-quality-summary-text="editingQualitySummaryText"
        :protocol-quality-field-defs="protocolQualityFieldDefs"
        :is-protocol-flag-active="isProtocolFlagActive"
        :toggle-protocol-flag="toggleProtocolFlag"
        @enter="handleEditDialogEnter"
        @close="closeEditDialog"
        @save="saveDataPoint"
      />

      <!-- 数据模拟对话框 -->
      <DataSimulationDialog
        v-model="showSimulationDialog"
        :selectedPoints="simulationDialogPoints"
        :subtitleBadges="simulationDialogBadges"
        :simulation-active="hasActiveSimulation"
        :global-simulation-interval-ms="simulationIntervalMs"
        @start="handleSimulationStart"
        @stop="handleSimulationStop"
      />

      <GroupAssignmentDialog
        v-model:visible="showGiGroupDialog"
        v-model:gi-group="batchGiGroupValue"
        v-model:counter-group="batchCounterGroupValue"
        :title="giGroupDialogTitle"
        :badges="giGroupDialogBadges"
        :rows="giGroupTableRows"
        :point-count="giGroupDialogPoints.length"
        :integrated-total="isCurrentTabIntegratedTotal"
        :table-ref="setGiGroupTableRef"
        :row-key="getGiGroupRowKey"
        :row-class-name="resolveGiGroupRowClassName"
        :row-style="resolveGiGroupRowStyle"
        :is-spacer="isGiGroupSpacerRow"
        :format-ioa="formatIoaAddress"
        :format-gi-group="formatGiGroupValue"
        :format-counter-group="formatCounterGroupValue"
        @submit="submitBatchGiGroup"
      />

      <!-- 命令地址映射对话框 -->
      <AddressMappingDialog
        v-model:visible="showMappingDialog"
        :mode="mappingDialogMode"
        :global-mode="mappingGlobalMode"
        :ioa-display-format="ioaDisplayFormat"
        :source-data-type="mappingSourceDataType"
        :source-type-options="mappingSourceTypeOptions"
        :subtitleBadges="mappingDialogBadges"
        @update:source-data-type="updateGlobalMappingSource"
        :batch-points="mappingBatchPoints"
        :existing-target-points="existingTargetPoints"
        @apply-batch="handleApplyBatchMapping"
      />

      <!-- 右键菜单 -->
      <div
        v-show="contextMenu.visible"
        class="app-context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click="closeContextMenu"
      >
        <div class="app-context-menu-item" @click="handleContextAction('simulate')">
          <el-icon><VideoPlay /></el-icon> {{ t('slave.contentPanel.context.simulate') }}
        </div>
        <div class="app-context-menu-item" @click="handleContextAction('edit')">
          <el-icon><Edit /></el-icon> {{ t('slave.contentPanel.context.edit') }}
        </div>
        <div
          v-if="isCurrentTabControlMappingType"
          class="app-context-menu-item"
          @click="handleContextAction('map')"
        >
          <el-icon><Link /></el-icon> {{ t('slave.contentPanel.context.commandMapping') }}
        </div>
        <div class="app-context-menu-divider"></div>
        <div class="app-context-menu-item" @click="handleContextAction('copy-addr')">
          <el-icon><CopyDocument /></el-icon> {{ t('slave.contentPanel.context.copyAddress') }}
        </div>
        <div class="app-context-menu-item" @click="handleContextAction('filter-message')">
          <el-icon><Search /></el-icon> {{ t('slave.contentPanel.context.filterMessages') }}
        </div>
        <div class="app-context-menu-item" @click="handleContextAction('copy-json')">
          <el-icon><DocumentCopy /></el-icon> {{ t('slave.contentPanel.context.copyJson') }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, reactive, nextTick, onMounted, onUnmounted, toRef } from 'vue'
import { VideoPlay, Search, Edit, DocumentCopy, CopyDocument, Link } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import DataSimulationDialog from './DataSimulationDialog.vue'
import DataTabs from './DataTabs.vue'
import AddressMappingDialog from './AddressMappingDialog.vue'
import GroupAssignmentDialog from './GroupAssignmentDialog.vue'
import PointEditDialog from './PointEditDialog.vue'
import PointTable from './PointTable.vue'
import PointToolbar from './PointToolbar.vue'
import { currentLocale, t } from '@shared/i18n'
import { useSlaveContentView } from '@/composables/useSlaveContentView'
import {
  useSlaveContentActions,
  type ActivePointSimulationSummary,
  type DataPointSimulationPayload,
} from '@/composables/useSlaveContentActions'
import { useSlaveControlStatus } from '@/composables/useSlaveControlStatus'
import { useSlaveAddressMapping } from '@/composables/useSlaveAddressMapping'
import { useSlaveContentTabs } from '@/composables/useSlaveContentTabs'
import type { SlavePointData } from '@/composables/useSlavePointData'
import type {
  SlaveContentDataPoint as DataPoint,
  SlaveContentActiveTabIdentity,
  SlaveContentMappingDialogContext as MappingDialogContext,
  SlaveContentRecentWindowOption,
  SlaveContentTabData as TabData,
} from '@/types/slaveContentPanel'
import type {
  BackendDataPoint,
  BackendDataPointQualityCommon,
  BackendDataPointQualityDetail,
  SlaveIoaDisplayFormat,
  UpdateDataPointRequest,
} from '@shared/api/types'
import { getIecDataTypeTabLabel, getIecDataTypeTreeIcon } from '@shared/ui/dataTypeIcon'
import {
  buildQualityCommonFromRaw,
  buildDefaultIec104PointName,
  formatSemanticPointValueLocalized,
  formatPointQualityLabelLocalized,
  formatIec104TypeCompactLabel,
  formatIec104TypeLabel,
  formatInformationValueSuffix,
  formatInformationValueTooltipLocalized,
  formatPointQualityTooltipLocalized,
  getIec104Capability,
  hasPointQuality,
  hasInformationValueDetail,
  iec104ProtocolDefaultValue,
  iec104TypeNameToId,
  isCommandType,
  qualityLevelFromCommon,
  resolveIec104TypeId,
  resolveIec104TypeName,
  semanticPointValueClass,
} from '@shared/api/iec104'
import { resolveEffectiveQualityDetail } from '@shared/api/protocolQuality'
import {
  useTableVirtualScroll,
  type VirtualSpacerRow,
} from '@shared/composables/useTableVirtualScroll'
import { resolveAdjacentTabId } from '@shared/ui/tabClose'
import {
  getPointTableSortValue,
  sortPointTableRows,
  sortPointTableRowsInFrames,
  type PointTableSortOrder,
  type PointTableSortProp,
} from '@shared/ui/pointTableSort'

const props = withDefaults(
  defineProps<{
    pointData: SlavePointData
    currentStationId?: string
    ioaDisplayFormat?: SlaveIoaDisplayFormat
    activePointSimulation?: ActivePointSimulationSummary | null
    simulationIntervalMs?: number
  }>(),
  {
    currentStationId: '',
    ioaDisplayFormat: 'dec',
    activePointSimulation: null,
    simulationIntervalMs: 1000,
  },
)

// 发射事件
const emit = defineEmits<{
  dataPointEdit: [dataPoint: DataPoint]
  dataPointSimulate: [payload: DataPointSimulationPayload]
  inspectMessage: [address: number] // 关联定位报文
  importDataPoints: [requests: UpdateDataPointRequest[], onSaved?: () => void]
  activeTabChange: [tab: SlaveContentActiveTabIdentity | null]
  batchGiGroup: [
    payload: {
      tab: SlaveContentActiveTabIdentity
      addresses: number[]
      giGroup: number | null
      counterGroup?: number | null
      onSaved?: () => void
    },
  ]
}>()

// 响应式数据
const activeTabs = ref<TabData[]>([])
const activeTabName = ref<string>('')
const suspendedSlaveIds = new Set<number>()
const selectedRows = ref<DataPoint[]>([])
const selectionByTab = ref<Record<string, number[]>>({})
const focusedAddressByTab = ref<Record<string, number>>({})
const rowsByAddressByTab = new WeakMap<TabData, Map<number, DataPoint>>()
const rowIndexesByTab = new WeakMap<TabData, Map<number, number>>()
const dataTableRef = ref<any>(null)
let syncedSortTable: any = null
let syncedSortKey = ''
const setDataTableRef = (instance: unknown) => {
  if (dataTableRef.value !== instance) {
    syncedSortTable = null
    syncedSortKey = ''
  }
  dataTableRef.value = instance
}
const giGroupTableRef = ref<any>(null)
const setGiGroupTableRef = (value: unknown) => {
  giGroupTableRef.value = value
}
let restoringTableSort = false

// 地址映射状态
const showMappingDialog = ref(false)
const mappingDialogMode = ref<'single' | 'batch'>('single')
const mappingGlobalMode = ref(false)
const mappingSourceDataType = ref('')
const mappingSourceTypeOptions = ref<string[]>([])
const mappingBatchPoints = ref<DataPoint[]>([])
const existingTargetPoints = ref<DataPoint[]>([])
const mappingDialogContext = ref<MappingDialogContext | null>(null)
const showGiGroupDialog = ref(false)
const batchGiGroupValue = ref(-1)
const batchCounterGroupValue = ref(-1)

// 表格密度
const tableSize = ref<'small' | 'default' | 'large'>('default')
const handleDensityChange = (size: 'small' | 'default' | 'large') => {
  tableSize.value = size
}

const quickFilters = reactive({
  onlyMapped: false,
  onlyAbnormalQuality: false,
  onlyRecentChange: false,
  recentWindowSec: 10,
})

const columnVisibility = reactive({
  showType: false,
  showDescription: false,
  showMappingTarget: true,
  showGiGroup: true,
  showCounterGroup: true,
  showCot: false,
  showCp56: false,
  showPointSource: false,
})

const recentWindowOptions: SlaveContentRecentWindowOption[] = [
  { label: '10s', value: 10 },
  { label: '30s', value: 30 },
  { label: '1m', value: 60 },
]

// 右键菜单状态
const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  row: null as DataPoint | null,
})

const isContextMenuTarget = (target: EventTarget | null) => {
  return target instanceof HTMLElement && Boolean(target.closest('.app-context-menu'))
}

const handleGlobalContextMenuClose = (event: MouseEvent) => {
  if (isContextMenuTarget(event.target)) return
  closeContextMenu()
}

const handleGlobalClickClose = (event: MouseEvent) => {
  if (isContextMenuTarget(event.target)) return
  closeContextMenu()
}

// 处理右键菜单
const handleRowContextMenu = (row: DataPoint, column: any, event: MouseEvent) => {
  void column
  if (isVirtualSpacerRow(row)) return
  event.preventDefault()
  event.stopPropagation()
  contextMenu.row = row
  contextMenu.x = event.clientX
  contextMenu.y = event.clientY
  contextMenu.visible = true
}

// 关闭右键菜单
const closeContextMenu = () => {
  contextMenu.visible = false
  contextMenu.row = null
}

onMounted(() => {
  document.addEventListener('mousedown', handleGlobalClickClose, true)
  document.addEventListener('contextmenu', handleGlobalContextMenuClose, true)
})

onUnmounted(() => {
  document.removeEventListener('mousedown', handleGlobalClickClose, true)
  document.removeEventListener('contextmenu', handleGlobalContextMenuClose, true)
})

// 处理菜单动作
const handleContextAction = async (action: string) => {
  if (!contextMenu.row) return

  const row = contextMenu.row

  switch (action) {
    case 'simulate':
      openSimulation(row)
      break
    case 'edit':
      handleEdit(row)
      break
    case 'copy-addr':
      try {
        const displayAddress = formatIoaAddress(row.address)
        await navigator.clipboard.writeText(displayAddress)
        ElMessage.success(
          t('slave.contentPanel.messages.copiedAddress', { address: displayAddress }),
        )
      } catch {
        ElMessage.error(t('slave.contentPanel.messages.copyFailed'))
      }
      break
    case 'filter-message':
      emit('inspectMessage', row.address)
      ElMessage.success(
        t('slave.contentPanel.messages.messageLocated', { address: formatIoaAddress(row.address) }),
      )
      break
    case 'copy-json':
      try {
        await navigator.clipboard.writeText(JSON.stringify(row, null, 2))
        ElMessage.success(t('slave.contentPanel.messages.copiedRow'))
      } catch {
        ElMessage.error(t('slave.contentPanel.messages.copyFailed'))
      }
      break
    case 'map':
      openSingleMapping(row)
      break
  }
}

// 处理表格多选
const handleSelectionChange = (selection: DataPoint[]) => {
  const tabId = currentTab.value?.id
  const realSelection = selection.filter((item) => !isVirtualSpacerRow(item))
  const selectedAddresses = new Set(
    realSelection.map((item) => normalizeTableAddress(item.address)),
  )
  if (!tabId) {
    selectedRows.value = realSelection.map((item) => ({ ...item }))
    return
  }

  let addresses: number[] = []
  if (isVirtualScrollEnabled.value) {
    const merged = new Set(
      (selectionByTab.value[tabId] ?? []).map((item) => normalizeTableAddress(item)),
    )
    for (const row of visibleVirtualRows.value) {
      merged.delete(normalizeTableAddress(row.address))
    }
    for (const address of selectedAddresses) {
      merged.add(address)
    }
    addresses = Array.from(merged)
  } else {
    addresses = Array.from(selectedAddresses)
  }

  selectionByTab.value = {
    ...selectionByTab.value,
    [tabId]: addresses,
  }
  selectedRows.value = buildSelectedRowsByAddressSet(new Set(addresses))
}

const clearSelectionForTab = (tabId: string) => {
  const nextSelection = { ...selectionByTab.value }
  delete nextSelection[tabId]
  selectionByTab.value = nextSelection
  if (currentTab.value?.id !== tabId) return
  selectedRows.value = []
  nextTick(() => dataTableRef.value?.clearSelection?.())
}

const clearCurrentTabSelection = () => {
  const tabId = currentTab.value?.id
  if (tabId) clearSelectionForTab(tabId)
}

const handleSelectionCellClick = (
  row: DataPoint,
  column: { type?: string },
  _cell: HTMLTableCellElement,
  event: Event,
) => {
  if (column.type !== 'selection' || isVirtualSpacerRow(row)) return
  const target = event.target
  if (target instanceof Element && target.closest('.el-checkbox')) return
  dataTableRef.value?.toggleRowSelection?.(row)
}

const {
  currentTab,
  simulationDialogBadges,
  mappingDialogBadges,
  hasActiveFilters,
  activeFilterSummary,
  clearQuickFilters,
  currentTabFilteredData,
  currentTabEmptyState,
  currentTabIsControl,
  isCurrentTabControlMappingType,
  getMappingTargetTypeCellText,
  getMappingTargetAddressCellText,
  getMappingTargetDisplayText,
} = useSlaveContentView({
  activeTabs,
  activeTabName,
  mappingSourceDataType,
  mappingDialogContext,
  pointData: props.pointData,
  quickFilters,
  recentWindowOptions,
  formatIoaAddress,
  normalizeControlIoa,
  normalizeCommonAddress,
  getControlStatusText: (point) => getControlStatusText(point),
})

const isCurrentTabMonitorType = computed(
  () =>
    getIec104Capability(iec104TypeNameToId(currentTab.value?.dataType || ''))?.point_role ===
    'monitor',
)
const currentTabHasQuality = computed(() =>
  hasPointQuality(iec104TypeNameToId(currentTab.value?.dataType || '')),
)
const isCurrentTabIntegratedTotal = computed(
  () =>
    getIec104Capability(iec104TypeNameToId(currentTab.value?.dataType || ''))?.family ===
    'integrated_total',
)
const mappingFamilies = new Set([
  'single_point',
  'double_point',
  'step_position',
  'bitstring',
  'normalized',
  'scaled',
  'short_float',
])
const isCurrentTabMappingFamily = computed(() => {
  const family = getIec104Capability(iec104TypeNameToId(currentTab.value?.dataType || ''))?.family
  return family != null && mappingFamilies.has(family)
})

const giGroupDialogTitle = computed(() =>
  t(
    isCurrentTabIntegratedTotal.value
      ? 'slave.contentPanel.giGroup.combinedTitle'
      : 'slave.contentPanel.giGroup.title',
  ),
)

const giGroupDialogBadges = computed(() => {
  const tab = currentTab.value
  if (!tab) return []
  return [
    tab.connectionName,
    tab.slaveName,
    tab.slaveCommonAddress == null ? '' : `CA: ${tab.slaveCommonAddress}`,
  ].filter((value): value is string => Boolean(value))
})

const giGroupDialogPoints = computed(() =>
  [...selectedRows.value].sort((left, right) => left.address - right.address),
)
const giGroupRowHeight = computed(() => 40)
const giGroupVirtualScroll = useTableVirtualScroll<DataPoint>({
  tableRef: giGroupTableRef,
  sourceRows: giGroupDialogPoints,
  rowHeight: giGroupRowHeight,
  threshold: 50,
  overscan: 6,
})
const giGroupTableRows = computed(() => giGroupVirtualScroll.renderRows.value)
const isGiGroupSpacerRow = (row: unknown): row is VirtualSpacerRow =>
  giGroupVirtualScroll.isVirtualSpacerRow(row)
const getGiGroupRowKey = (row: DataPoint | VirtualSpacerRow) =>
  isGiGroupSpacerRow(row) ? row.__virtualKey : String(row.address)
const resolveGiGroupRowClassName = ({ row }: { row: DataPoint | VirtualSpacerRow }) =>
  giGroupVirtualScroll.resolveRowClassName(row)
const resolveGiGroupRowStyle = ({ row }: { row: DataPoint | VirtualSpacerRow }) =>
  giGroupVirtualScroll.resolveRowStyle(row)

const formatGiGroupValue = (group: number | null | undefined) =>
  group == null
    ? t('slave.contentPanel.giGroup.stationOnly')
    : t('slave.contentPanel.giGroup.group', { group })

const formatCounterGroupValue = (group: number | null | undefined) =>
  group == null
    ? t('slave.contentPanel.counterGroup.allOnly')
    : t('slave.contentPanel.counterGroup.group', { group })

const formatPointSource = (source: DataPoint['pointSource']) => {
  if (source === 'built_in') return t('slave.contentPanel.pointSources.builtIn')
  if (source === 'manual') return t('slave.contentPanel.pointSources.manual')
  if (source === 'imported') return t('slave.contentPanel.pointSources.imported')
  return '—'
}

const openBatchGiGroupDialog = () => {
  if (!isCurrentTabMonitorType.value || selectedRows.value.length === 0) return
  const groups = new Set(selectedRows.value.map((point) => point.giGroup ?? null))
  batchGiGroupValue.value = groups.size === 1 ? ([...groups][0] ?? 0) : -1
  if (isCurrentTabIntegratedTotal.value) {
    const counterGroups = new Set(selectedRows.value.map((point) => point.counterGroup ?? null))
    batchCounterGroupValue.value = counterGroups.size === 1 ? ([...counterGroups][0] ?? 0) : -1
  } else {
    batchCounterGroupValue.value = -1
  }
  showGiGroupDialog.value = true
}

const submitBatchGiGroup = () => {
  const tab = currentTab.value
  if (
    !tab ||
    selectedRows.value.length === 0 ||
    batchGiGroupValue.value < 0 ||
    (isCurrentTabIntegratedTotal.value && batchCounterGroupValue.value < 0)
  )
    return
  emit('batchGiGroup', {
    tab: {
      id: tab.id,
      parentStationId: tab.parentStationId,
      parentSlaveId: tab.parentSlaveId,
      slaveId: tab.slaveId,
      dataType: tab.dataType,
    },
    addresses: selectedRows.value.map((point) => Math.trunc(point.address)),
    giGroup: batchGiGroupValue.value === 0 ? null : batchGiGroupValue.value,
    counterGroup: isCurrentTabIntegratedTotal.value
      ? batchCounterGroupValue.value === 0
        ? null
        : batchCounterGroupValue.value
      : undefined,
    onSaved: () => clearSelectionForTab(tab.id),
  })
  showGiGroupDialog.value = false
}

const activeSimulationPointKeys = computed(() => {
  const session = currentTabSimulation.value
  if (!session) return new Set<string>()
  return new Set(
    session.points.map(
      (point) =>
        `${normalizeCommonAddress(point.commonAddress ?? currentTab.value?.slaveCommonAddress) ?? -1}:${Math.max(0, Math.trunc(Number(point.address) || 0))}`,
    ),
  )
})

const hasActiveSimulation = computed(() => activeSimulationPointKeys.value.size > 0)

const {
  clearSimulationState,
  closeEditDialog,
  editDataPoint,
  editingDataPoint,
  editingValueProfile,
  editingQualitySummaryTagType,
  editingQualitySummaryText,
  handleEditDialogBeforeClose,
  getSimulationIcon,
  getSimulationTooltip,
  handleEdit,
  handleEditDialogEnter,
  handleSimulationStart,
  handleSimulationStop,
  isEditingControlPoint,
  isEditingDoublePoint,
  isEditingIntegratedTotal,
  isEditingSinglePoint,
  isPointSimulating,
  isProtocolFlagActive,
  openSimulation,
  protocolQualityFieldDefs,
  protocolQualityGroupLabel,
  protocolQualityKind,
  saveDataPoint,
  showEditDialog,
  showSimulationDialog,
  simulationDialogPoints,
  toggleProtocolFlag,
  toggleSimulationFromToolbar,
  currentTabSimulation,
} = useSlaveContentActions({
  currentTab,
  currentStationId: toRef(props, 'currentStationId'),
  activePointSimulation: toRef(props, 'activePointSimulation'),
  pointEditor: {
    currentTab,
    selectedRows,
    hasActiveSimulation,
    activeSimulationPointKeys,
    normalizeCommonAddress,
    emitDataPointEdit: (dataPoint) => emit('dataPointEdit', dataPoint),
    emitDataPointSimulate: (payload) => emit('dataPointSimulate', payload),
  },
})

const editingGiGroupValue = computed({
  get: () => editingDataPoint.value?.giGroup ?? 0,
  set: (value: number) => {
    if (editingDataPoint.value) editingDataPoint.value.giGroup = value === 0 ? null : value
  },
})

const editingCounterGroupValue = computed({
  get: () => editingDataPoint.value?.counterGroup ?? 0,
  set: (value: number) => {
    if (editingDataPoint.value) editingDataPoint.value.counterGroup = value === 0 ? null : value
  },
})

const virtualRowHeightPx = computed(() => {
  if (tableSize.value === 'small') return 36
  if (tableSize.value === 'large') return 48
  return 44
})

const tableVirtualScroll = useTableVirtualScroll<DataPoint>({
  tableRef: dataTableRef,
  sourceRows: currentTabFilteredData,
  rowHeight: virtualRowHeightPx,
  threshold: 60,
  overscan: 12,
})

const virtualTableRows = computed(() => tableVirtualScroll.renderRows.value as DataPoint[])
const visibleVirtualRows = computed(() => tableVirtualScroll.visibleRows.value)
const isVirtualScrollEnabled = computed(() => tableVirtualScroll.isVirtualEnabled.value)

const normalizeTableAddress = (value: unknown): number =>
  Math.max(0, Math.trunc(Number(value) || 0))

const buildSelectedRowsByAddressSet = (addresses: Set<number>): DataPoint[] => {
  const tab = currentTab.value
  if (!tab) return []
  const { rowsByAddress } = getTabRowLookup(tab)
  const rows: DataPoint[] = []
  for (const address of addresses) {
    const row = rowsByAddress.get(address)
    if (row) rows.push({ ...row })
  }
  return rows
}

const resolveTableRowClassName = ({ row }: { row: DataPoint | VirtualSpacerRow }): string => {
  const baseClass = tableVirtualScroll.resolveRowClassName(row)
  if (isVirtualSpacerRow(row)) return baseClass
  const focusedAddress = focusedAddressByTab.value[activeTabName.value]
  return focusedAddress === normalizeTableAddress(row.address)
    ? `${baseClass} is-located-row`.trim()
    : baseClass
}

const resolveTableRowStyle = ({
  row,
}: {
  row: DataPoint | VirtualSpacerRow
}): Record<string, string> => {
  return tableVirtualScroll.resolveRowStyle(row)
}

const isSelectableTableRow = (row: DataPoint | VirtualSpacerRow): boolean =>
  tableVirtualScroll.isSelectableRow(row)

const isVirtualSpacerRow = (row: unknown): row is VirtualSpacerRow =>
  tableVirtualScroll.isVirtualSpacerRow(row)

const getTableRowKey = (row: DataPoint | VirtualSpacerRow) => {
  if (isVirtualSpacerRow(row)) return row.__virtualKey
  const tabId = activeTabName.value || 'tab'
  return `${tabId}:${normalizeTableAddress(row.address)}`
}

const restoreSelectionForCurrentTab = () => {
  const tab = currentTab.value
  if (!tab) {
    selectedRows.value = []
    nextTick(() => dataTableRef.value?.clearSelection?.())
    return
  }

  const rawAddresses = selectionByTab.value[tab.id] ?? []
  const addresses = Array.from(new Set(rawAddresses.map((addr) => normalizeTableAddress(addr))))
  const selectedSet = new Set(addresses)

  selectedRows.value = buildSelectedRowsByAddressSet(selectedSet)

  nextTick(() => {
    const table = dataTableRef.value
    if (!table) return

    table.clearSelection?.()
    if (selectedSet.size === 0) return

    const rowsToRestore = isVirtualScrollEnabled.value
      ? visibleVirtualRows.value
      : currentTabFilteredData.value
    for (const row of rowsToRestore) {
      if (selectedSet.has(normalizeTableAddress(row.address))) {
        table.toggleRowSelection?.(row, true)
      }
    }
  })
}

function normalizeControlIoa(value: unknown): number | null {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  if (normalized < 1 || normalized > 0xffffff) return null
  return normalized
}

const IOA_HEX_WIDTH = 6

function formatIoaAddress(value: unknown): string {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return '--'
  const normalized = Math.max(0, Math.trunc(numeric))
  if (props.ioaDisplayFormat === 'hex') {
    return `0x${normalized.toString(16).toUpperCase().padStart(IOA_HEX_WIDTH, '0')}`
  }
  return String(normalized)
}

const formatIoaTableCell = (_row: DataPoint, _column: unknown, cellValue: unknown): string => {
  return formatIoaAddress(cellValue)
}

function normalizeCommonAddress(value: unknown): number | null {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  if (normalized < 1 || normalized > 65535) return null
  return normalized
}

const {
  getControlStatusTagType,
  getControlStatusText,
  getCotColumnText,
  getCp56ColumnText,
  getExtraInfoTooltip,
} = useSlaveControlStatus<DataPoint>({
  isControlPointType,
})

const normalizeSlaveId = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  if (normalized < 1) return null
  return normalized
}

const getIndexedPointsForTab = (tab: TabData) => {
  if (
    !props.pointData.stationId.value ||
    props.pointData.stationId.value !== String(tab.parentStationId || '').trim()
  ) {
    return null
  }
  const typeId = iec104TypeNameToId(tab.dataType) ?? 0
  return props.pointData.getPointsForTab({
    typeId,
    commonAddress: normalizeCommonAddress(tab.slaveCommonAddress),
    slaveId: normalizeSlaveId(tab.slaveId ?? tab.parentSlaveId),
  })
}

const isControlMappingType = (dataType: string): boolean => {
  return getIec104Capability(iec104TypeNameToId(dataType))?.point_role === 'control'
}

function isControlPointType(dataType: string): boolean {
  const typeId = iec104TypeNameToId(String(dataType || '').trim())
  return isCommandType(typeId)
}

// 获取Tab图标
const getTabIcon = (dataType: string) => {
  return getIecDataTypeTreeIcon(dataType)
}

const getTabDisplayLabel = (tab: TabData) => {
  void currentLocale.value
  return getIecDataTypeTabLabel(tab.dataType) || tab.label
}

const formatTypeCompact = (dataType: string) =>
  formatIec104TypeCompactLabel(iec104TypeNameToId(dataType))

// 获取Tab徽标需要显示的字符 (优先 CA，无 CA 则提取从站名关键字符)
const getTabBadge = (tab: any) => {
  if (tab.slaveCommonAddress !== undefined) {
    return `CA:${tab.slaveCommonAddress}`
  }
  if (tab.slaveName) {
    const match = tab.slaveName.match(/[A-Z0-9]+$/) // 提取末尾的字母或数字
    if (match) return match[0]
    return tab.slaveName.slice(0, 1) // 兜底取首字
  }
  return 'S' + tab.parentSlaveId
}

// 获取Tab完整提示
const getTabTooltip = (tab: any) => {
  const connectionName =
    String(tab.connectionName || '').trim() || t('slave.contentPanel.tabs.unknownConnection')
  const slaveName = String(tab.slaveName || '').trim() || t('slave.contentPanel.tabs.unknownSlave')
  return `${t('slave.contentPanel.tabs.tooltip', { connection: connectionName, slave: slaveName })} · ${formatIec104TypeLabel(iec104TypeNameToId(tab.dataType))}`
}

// 添加标签页
const addTab = (datatype: any) => {
  const stationId = String(datatype.parentStationId ?? '').trim()
  const slaveId = Number(datatype.slaveId ?? datatype.parentSlaveId ?? 0) || undefined
  const tabId = `${stationId || 'station'}:${slaveId || datatype.parentSlaveId}:${datatype.dataType}`
  const existingTab = activeTabs.value.find((tab) => tab.id === tabId)

  if (existingTab) {
    // 如果标签页已存在，更新可能会变化的属性并切换
    existingTab.connectionName = datatype.connectionName
    existingTab.slaveName =
      datatype.slaveName ||
      t('slave.profileDialogs.defaultNames.slave', { index: datatype.parentSlaveId })
    existingTab.slaveId = slaveId
    existingTab.slaveCommonAddress = datatype.slaveCommonAddress
    existingTab.label = datatype.label
    activeTabName.value = tabId
    return
  }

  // 获取从站名称
  const slaveName =
    datatype.slaveName ||
    t('slave.profileDialogs.defaultNames.slave', { index: datatype.parentSlaveId })

  // 创建新标签页，标签显示：从站名 - 数据类型
  const newTab: TabData = {
    id: tabId,
    label: datatype.label,
    dataType: datatype.dataType,
    parentStationId: stationId || undefined,
    parentSlaveId: datatype.parentSlaveId,
    slaveId,
    slaveCommonAddress: datatype.slaveCommonAddress,
    slaveName: slaveName,
    connectionName: datatype.connectionName,
    searchText: '',
    sortProp: 'address',
    sortOrder: 'ascending',
    data: [],
    loading: true,
    dataRevision: 0,
  }

  activeTabs.value.push(newTab)
  activeTabName.value = tabId
  scheduleTabLoad(newTab)
}

const mapQuality = (
  qualityCommon: BackendDataPointQualityCommon | null | undefined,
  quality: number,
  qualityDetail: BackendDataPointQualityDetail,
): DataPoint['quality'] => {
  return qualityLevelFromCommon(qualityCommon, quality, qualityDetail)
}

const buildLocalQualityDetailNone = (): BackendDataPointQualityDetail => ({ kind: 'none' })

const mapBackendPointToUi = (
  point: BackendDataPoint,
  dataType: string,
  resolvedTypeId?: number,
): DataPoint => {
  const typeId = resolvedTypeId ?? resolveIec104TypeId(point)
  const effectiveQualityDetail = resolveEffectiveQualityDetail(
    typeId,
    point.quality_detail,
    point.quality ?? 0,
    point.value,
  )
  return {
    address: point.address,
    commonAddress: point.common_address ?? undefined,
    slaveId: point.slave_id ?? null,
    name: point.name || buildDefaultIec104PointName(dataType, point.address),
    description: String(point.description ?? '').trim(),
    value: point.value,
    quality: mapQuality(point.quality_common, point.quality, effectiveQualityDetail),
    qualityRaw: point.quality,
    qualityCommon: point.quality_common ?? null,
    qualityDetail: effectiveQualityDetail,
    timestamp: new Date(point.timestamp).getTime(),
    dataType,
    typeId,
    controlIoa: point.control_ioa ?? null,
    giGroup: point.gi_group ?? null,
    counterGroup: point.counter_group ?? null,
    pointSource: point.point_source ?? null,
    controlStatusSnapshot: point.control_status_snapshot ?? null,
  }
}

const TAB_PROJECTION_FRAME_BUDGET_MS = 4

const waitForProjectionTurn = (active: boolean) =>
  new Promise<void>((resolve) => {
    if (!active && typeof window.requestIdleCallback === 'function') {
      window.requestIdleCallback(() => resolve(), { timeout: 50 })
      return
    }
    requestAnimationFrame(() => resolve())
  })

const rebuildTabRowLookup = (tab: TabData) => {
  const rowsByAddress = new Map<number, DataPoint>()
  const rowIndexes = new Map<number, number>()
  tab.data.forEach((row, index) => {
    rowsByAddress.set(row.address, row)
    rowIndexes.set(row.address, index)
  })
  rowsByAddressByTab.set(tab, rowsByAddress)
  rowIndexesByTab.set(tab, rowIndexes)
  return { rowsByAddress, rowIndexes }
}

const getTabRowLookup = (tab: TabData) => {
  const rowsByAddress = rowsByAddressByTab.get(tab)
  const rowIndexes = rowIndexesByTab.get(tab)
  return rowsByAddress && rowIndexes ? { rowsByAddress, rowIndexes } : rebuildTabRowLookup(tab)
}

const loadDataPointsForTab = async (tab: TabData, revision: number) => {
  const indexedPoints = getIndexedPointsForTab(tab)
  if (indexedPoints == null) return false

  const existingByAddress = new Map<number, DataPoint>()
  const previousSortValues = new Map<number, string | number | null>()
  for (const row of tab.data) {
    existingByAddress.set(row.address, row)
    previousSortValues.set(row.address, getPointTableSortValue(row, tab.sortProp))
  }

  const matched: DataPoint[] = []
  let frameStartedAt = performance.now()
  for (const indexedPoint of indexedPoints) {
    const point = indexedPoint.point
    matched.push({
      ...mapBackendPointToUi(point, indexedPoint.dataType, indexedPoint.typeId),
      slaveId: point.slave_id ?? tab.slaveId ?? null,
      commonAddress: point.common_address ?? tab.slaveCommonAddress,
    })
    if (performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS) {
      await waitForProjectionTurn(tab.id === activeTabName.value)
      if (!activeTabs.value.includes(tab) || tab.dataRevision !== revision) return false
      frameStartedAt = performance.now()
    }
  }
  for (let index = 0; index < matched.length; index += 1) {
    const nextRow = matched[index]
    const existing = existingByAddress.get(nextRow.address)
    if (!existing) continue
    Object.assign(existing, nextRow)
    matched[index] = existing
  }
  const matchedByAddress = new Map(matched.map((row) => [row.address, row]))
  const pointSetChanged =
    tab.data.length !== matched.length || tab.data.some((row) => !matchedByAddress.has(row.address))
  const rows = pointSetChanged
    ? matched
    : tab.data.map((row) => matchedByAddress.get(row.address) as DataPoint)
  const sortValueChanged =
    !pointSetChanged &&
    rows.some(
      (row) => previousSortValues.get(row.address) !== getPointTableSortValue(row, tab.sortProp),
    )
  const isDefaultAddressOrder = tab.sortProp === 'address' && tab.sortOrder === 'ascending'
  const rowsAreAddressSorted = rows.every(
    (row, index) => index === 0 || rows[index - 1].address <= row.address,
  )
  let nextData = rows
  if ((pointSetChanged || sortValueChanged) && !(isDefaultAddressOrder && rowsAreAddressSorted)) {
    const sorted = await sortPointTableRowsInFrames(
      rows,
      tab.sortProp,
      tab.sortOrder,
      () => activeTabs.value.includes(tab) && tab.dataRevision === revision,
      TAB_PROJECTION_FRAME_BUDGET_MS,
    )
    if (!sorted) return false
    nextData = sorted
  }
  tab.data = nextData
  rebuildTabRowLookup(tab)
  return true
}

const { scheduleTabLoad, cancelTabLoad, cancelAllTabLoads } = useSlaveContentTabs({
  activeTabs,
  activeTabName,
  loadTab: async (tab, revision) => {
    if (tab.id !== activeTabName.value) await waitForProjectionTurn(false)
    return loadDataPointsForTab(tab, revision)
  },
})

// 格式化值
const formatValue = (row: DataPoint) => {
  return `${formatSemanticPointValueLocalized(row.typeId || row.dataType, row.value)}${formatInformationValueSuffix(row.qualityDetail)}`
}

const getValueDescriptorTooltip = (row: DataPoint) => {
  return formatInformationValueTooltipLocalized(row.qualityDetail)
}

const hasValueDescriptorDetail = (row: DataPoint) => {
  return hasInformationValueDetail(row.qualityDetail)
}

// 获取质量类型
const getQualityType = (quality: string) => {
  switch (quality) {
    case 'good':
      return 'success'
    case 'invalid':
      return 'danger'
    case 'questionable':
      return 'warning'
    default:
      return 'info'
  }
}

// 获取质量文本（显示具体标志位）
const getQualityText = (row: DataPoint) => {
  const qualityDetail = resolveEffectiveQualityDetail(
    row.typeId,
    row.qualityDetail,
    row.qualityRaw ?? 0,
    row.value,
  )
  return formatPointQualityLabelLocalized(
    row.typeId,
    row.qualityCommon,
    row.qualityRaw ?? 0,
    qualityDetail,
  )
}

// 获取质量术语提示（展开所有标志位描述）
const getQualityTooltip = (row: DataPoint) => {
  const qualityDetail = resolveEffectiveQualityDetail(
    row.typeId,
    row.qualityDetail,
    row.qualityRaw ?? 0,
    row.value,
  )
  return formatPointQualityTooltipLocalized(
    row.typeId,
    row.qualityCommon,
    qualityDetail,
    row.qualityRaw ?? 0,
  )
}

// 格式化时间戳 - YYYY-MM-DD HH:mm:ss.SSS
const formatTimestamp = (timestamp: number) => {
  const d = new Date(timestamp)
  const pad = (n: number, len = 2) => String(n).padStart(len, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`
}

// 重置数据 (危险操作 - 二次确认)
const resetData = async () => {
  if (!currentTab.value) return

  try {
    await ElMessageBox.confirm(
      t('slave.contentPanel.confirm.resetMessage'),
      t('slave.contentPanel.confirm.resetTitle'),
      {
        confirmButtonText: t('slave.contentPanel.confirm.reset'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
        autofocus: false,
      },
    )
    // 用户确认后执行：同步写回后端，避免下一次刷新又回到旧值。
    const resetValues = new Map<number, number>()
    const updates: UpdateDataPointRequest[] = currentTab.value.data.map((point) => {
      const capability = getIec104Capability(point.typeId)
      if (!capability || capability.point_role !== 'monitor' || !capability.simulation) {
        throw new Error(t('slave.pointEditor.messages.unsupportedCapability'))
      }
      const resetValue = iec104ProtocolDefaultValue(point.typeId)
      resetValues.set(point.address, resetValue)
      return {
        address: Math.max(0, Math.trunc(Number(point.address) || 0)),
        value: resetValue,
        quality: 0,
        common_address: point.commonAddress ?? currentTab.value?.slaveCommonAddress,
      }
    })

    currentTab.value.data.forEach((point) => {
      point.value = resetValues.get(point.address) ?? 0
      point.timestamp = Date.now()
      point.quality = 'good'
      point.qualityRaw = 0
      point.qualityCommon = buildQualityCommonFromRaw(0)
      point.qualityDetail = buildLocalQualityDetailNone()
    })

    emit('importDataPoints', updates)
  } catch (error) {
    if (error instanceof Error) ElMessage.error(error.message)
  }
}

const {
  handleApplyBatchMapping,
  openBatchMapping,
  openGlobalMapping,
  openSingleMapping,
  updateGlobalMappingSource,
} = useSlaveAddressMapping({
  activeTabs,
  activeTabName,
  pointData: props.pointData,
  currentTab,
  selectedRows,
  showMappingDialog,
  mappingDialogMode,
  mappingGlobalMode,
  mappingSourceDataType,
  mappingSourceTypeOptions,
  mappingBatchPoints,
  existingTargetPoints,
  mappingDialogContext,
  normalizeCommonAddress,
  normalizeControlIoa,
  mapBackendPointToUi,
  isControlMappingType,
  clearSelectionForTab,
  emitImportDataPoints: (updates, onSaved) => emit('importDataPoints', updates, onSaved),
})

// 获取语义化值颜色样式
const getSemanticValueClass = (row: DataPoint) => {
  return semanticPointValueClass(row.typeId || row.dataType, row.value)
}

const isSortableProp = (value: unknown): value is PointTableSortProp =>
  ['address', 'name', 'quality', 'timestamp'].includes(String(value))

const handleSortChange = (payload: { prop?: string; order?: PointTableSortOrder | null }) => {
  if (restoringTableSort) return
  const tab = currentTab.value
  if (!tab) return
  if (isSortableProp(payload.prop) && payload.order) {
    tab.sortProp = payload.prop
    tab.sortOrder = payload.order
  } else {
    tab.sortProp = 'address'
    tab.sortOrder = 'ascending'
  }
  tab.data = sortPointTableRows(tab.data, tab.sortProp, tab.sortOrder)
  rebuildTabRowLookup(tab)
  if (payload.order) {
    syncedSortTable = dataTableRef.value
    syncedSortKey = `${tab.id}:${tab.sortProp}:${tab.sortOrder}`
  } else {
    syncedSortTable = null
    syncedSortKey = ''
    syncTableSortIndicator()
  }
}

const syncTableSortIndicator = () => {
  nextTick(() => {
    const tab = currentTab.value
    const table = dataTableRef.value
    if (!tab || !table?.sort) return
    const sortKey = `${tab.id}:${tab.sortProp}:${tab.sortOrder}`
    if (syncedSortTable === table && syncedSortKey === sortKey) return
    syncedSortTable = table
    syncedSortKey = sortKey
    restoringTableSort = true
    table.sort(tab.sortProp, tab.sortOrder)
    nextTick(() => {
      restoringTableSort = false
    })
  })
}

// 标签页事件处理
const removeTab = (targetName: string) => {
  cancelTabLoad(targetName)
  const nextActiveTabId = resolveAdjacentTabId(activeTabs.value, targetName, activeTabName.value)
  const { [targetName]: _removed, ...restSelection } = selectionByTab.value
  selectionByTab.value = restSelection
  const { [targetName]: _removedFocus, ...restFocusedAddresses } = focusedAddressByTab.value
  focusedAddressByTab.value = restFocusedAddresses

  const index = activeTabs.value.findIndex((tab) => tab.id === targetName)
  if (index !== -1) {
    activeTabs.value.splice(index, 1)

    if (activeTabName.value === targetName) {
      activeTabName.value = nextActiveTabId
    }
  }

  nextTick(() => {
    restoreSelectionForCurrentTab()
  })
}

const handleTabClick = (tab: any) => {
  activeTabName.value = tab.paneName
}

const closeAllTabs = () => {
  cancelAllTabLoads()
  activeTabs.value = []
  activeTabName.value = ''
  selectedRows.value = []
  selectionByTab.value = {}
  focusedAddressByTab.value = {}
  nextTick(() => dataTableRef.value?.clearSelection?.())
}

// Tab 右键菜单处理
const handleTabContextMenu = (command: string, tabId: string) => {
  switch (command) {
    case 'close':
      removeTab(tabId)
      break
    case 'closeOthers':
      for (const tab of activeTabs.value) {
        if (tab.id !== tabId) cancelTabLoad(tab.id)
      }
      activeTabs.value = activeTabs.value.filter((tab) => tab.id === tabId)
      activeTabName.value = tabId
      selectionByTab.value = selectionByTab.value[tabId]
        ? { [tabId]: [...selectionByTab.value[tabId]] }
        : {}
      focusedAddressByTab.value =
        focusedAddressByTab.value[tabId] == null
          ? {}
          : { [tabId]: focusedAddressByTab.value[tabId] }
      restoreSelectionForCurrentTab()
      break
    case 'closeAll':
      closeAllTabs()
      break
  }
}

// 刷新数据
const refreshData = () => {
  const tab = currentTab.value
  if (tab) scheduleTabLoad(tab)
}

const pointMatchesTab = (tab: TabData, point: BackendDataPoint): boolean => {
  if (resolveIec104TypeId(point) !== (iec104TypeNameToId(tab.dataType) ?? 0)) return false
  const tabCommonAddress = normalizeCommonAddress(tab.slaveCommonAddress)
  const tabSlaveId = normalizeSlaveId(tab.slaveId ?? tab.parentSlaveId)
  const commonAddress = normalizeCommonAddress(point.common_address)
  const slaveId = normalizeSlaveId(point.slave_id)
  if (tabCommonAddress != null) {
    return commonAddress === tabCommonAddress || (commonAddress == null && slaveId === tabSlaveId)
  }
  return tabSlaveId == null || slaveId == null || slaveId === tabSlaveId
}

const mapRuntimePointForTab = (tab: TabData, point: BackendDataPoint): DataPoint => {
  const typeId = resolveIec104TypeId(point)
  return {
    ...mapBackendPointToUi(point, resolveIec104TypeName(point), typeId),
    slaveId: point.slave_id ?? tab.slaveId ?? null,
    commonAddress: point.common_address ?? tab.slaveCommonAddress,
  }
}

const patchTabFromRuntimeDelta = async (
  tab: TabData,
  delta: NonNullable<SlavePointData['lastDelta']['value']>,
) => {
  const revision = ++tab.dataRevision
  const changes = new Map<number, DataPoint | null>()
  let frameStartedAt = performance.now()
  const yieldWhenNeeded = async () => {
    await waitForProjectionTurn(tab.id === activeTabName.value)
    frameStartedAt = performance.now()
    return activeTabs.value.includes(tab) && tab.dataRevision === revision
  }

  for (const removal of delta.removed) {
    if (removal.previous && pointMatchesTab(tab, removal.previous)) {
      changes.set(removal.previous.address, null)
    }
    if (
      performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
      !(await yieldWhenNeeded())
    ) {
      return
    }
  }
  for (const upsert of delta.upserts) {
    const previousMatches = upsert.previous != null && pointMatchesTab(tab, upsert.previous)
    const currentMatches = pointMatchesTab(tab, upsert.point)
    if (previousMatches && !currentMatches) changes.set(upsert.point.address, null)
    if (currentMatches) changes.set(upsert.point.address, mapRuntimePointForTab(tab, upsert.point))
    if (
      performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
      !(await yieldWhenNeeded())
    ) {
      return
    }
  }
  if (changes.size === 0 || !activeTabs.value.includes(tab) || tab.dataRevision !== revision) return

  const { rowsByAddress, rowIndexes } = getTabRowLookup(tab)
  let membershipChanged = false
  let sortValueChanged = false
  for (const [address, nextRow] of changes) {
    const currentRow = rowsByAddress.get(address)
    if (nextRow == null) {
      if (currentRow) {
        rowsByAddress.delete(address)
        membershipChanged = true
      }
      continue
    }
    if (!currentRow) {
      rowsByAddress.set(address, nextRow)
      membershipChanged = true
      continue
    }
    if (
      getPointTableSortValue(currentRow, tab.sortProp) !==
      getPointTableSortValue(nextRow, tab.sortProp)
    ) {
      sortValueChanged = true
    }
    rowsByAddress.set(address, nextRow)
    const rowIndex = rowIndexes.get(address)
    if (rowIndex != null) tab.data[rowIndex] = nextRow
  }

  if (membershipChanged || sortValueChanged) {
    tab.data = sortPointTableRows([...rowsByAddress.values()], tab.sortProp, tab.sortOrder)
    rebuildTabRowLookup(tab)
  }
}

let runtimePatchQueue = Promise.resolve()
let runtimePatchEpoch = 0
const refreshFromBackend = () => {
  const stationId = props.pointData.stationId.value
  const delta = props.pointData.lastDelta.value
  if (!delta || delta.reset) {
    runtimePatchEpoch += 1
    for (const tab of activeTabs.value) {
      if (
        String(tab.parentStationId || '').trim() === stationId &&
        !suspendedSlaveIds.has(Number(tab.slaveId ?? tab.parentSlaveId))
      ) {
        scheduleTabLoad(tab)
      }
    }
    return
  }

  const patchEpoch = runtimePatchEpoch
  runtimePatchQueue = runtimePatchQueue.then(async () => {
    if (patchEpoch !== runtimePatchEpoch || props.pointData.stationId.value !== stationId) return
    const tabs = [...activeTabs.value].sort((left, right) => {
      if (left.id === activeTabName.value) return -1
      if (right.id === activeTabName.value) return 1
      return 0
    })
    for (const tab of tabs) {
      if (patchEpoch !== runtimePatchEpoch) return
      if (String(tab.parentStationId || '').trim() !== stationId) continue
      if (suspendedSlaveIds.has(Number(tab.slaveId ?? tab.parentSlaveId))) continue
      if (tab.id !== activeTabName.value) await waitForProjectionTurn(false)
      await patchTabFromRuntimeDelta(tab, delta)
    }
    const tab = currentTab.value
    if (tab) {
      const { rowsByAddress } = getTabRowLookup(tab)
      const addresses = (selectionByTab.value[tab.id] ?? []).filter((address) =>
        rowsByAddress.has(normalizeTableAddress(address)),
      )
      selectionByTab.value = { ...selectionByTab.value, [tab.id]: addresses }
      selectedRows.value = buildSelectedRowsByAddressSet(new Set(addresses))
    }
  })
}

const beginPointTableReplacement = (slaveId: number) => {
  suspendedSlaveIds.add(Number(slaveId))
  for (const tab of activeTabs.value) {
    if (Number(tab.slaveId ?? tab.parentSlaveId) !== Number(slaveId)) continue
    cancelTabLoad(tab.id)
    tab.loading = true
  }
}

const completePointTableReplacement = (slaveId: number) => {
  suspendedSlaveIds.delete(Number(slaveId))
  for (const tab of activeTabs.value) {
    if (Number(tab.slaveId ?? tab.parentSlaveId) === Number(slaveId)) scheduleTabLoad(tab)
  }
}

const focusPoint = (payload: {
  parentStationId?: string | null
  parentSlaveId?: string | number | null
  slaveId?: string | number | null
  slaveCommonAddress?: number | null
  connectionName?: string
  slaveName?: string
  dataType: string
  label?: string
  address: number
}) => {
  const targetAddress = Math.max(0, Math.trunc(Number(payload.address) || 0))
  const parentSlaveId = Number(payload.parentSlaveId ?? payload.slaveId ?? 0) || undefined
  const dataType = String(payload.dataType ?? '')
    .trim()
    .toUpperCase()
  if (!payload.parentStationId || !parentSlaveId || !dataType) return

  addTab({
    ...payload,
    parentStationId: payload.parentStationId,
    parentSlaveId,
    slaveId: parentSlaveId,
    dataType,
    label: payload.label || payload.slaveName || dataType,
  })

  const tabId = `${payload.parentStationId}:${parentSlaveId}:${dataType}`
  const tab = activeTabs.value.find((item) => item.id === tabId)
  if (!tab) return
  tab.searchText = String(targetAddress)
  focusedAddressByTab.value = {
    ...focusedAddressByTab.value,
    [tabId]: targetAddress,
  }
  activeTabName.value = tabId
}

watch(props.pointData.revision, refreshFromBackend)

watch(
  () => activeTabName.value,
  () => {
    syncTableSortIndicator()
    restoreSelectionForCurrentTab()
    const tab = currentTab.value
    emit(
      'activeTabChange',
      tab
        ? {
            id: tab.id,
            parentStationId: tab.parentStationId,
            parentSlaveId: tab.parentSlaveId,
            slaveId: tab.slaveId,
            dataType: tab.dataType,
          }
        : null,
    )
  },
)

// 暴露方法给父组件
defineExpose({
  addTab,
  refreshFromBackend,
  openGlobalMapping,
  focusPoint,
  clearSimulationState,
  beginPointTableReplacement,
  completePointTableReplacement,
})
</script>

<style scoped>
.slave-content-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: #f3f4f6; /* 背景浅灰 */
  padding: 4px; /* 增加呼吸感 */
}

.manager-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: 0;
  background: white; /* 纯白卡片 */
  border-radius: 4px; /* 4. 圆角 4px (Strict) */
  box-shadow:
    0 1px 3px 0 rgb(0 0 0 / 0.1),
    0 1px 2px -1px rgb(0 0 0 / 0.1); /* 淡阴影 */
  border: 1px solid #e5e7eb;
}

.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: white;
  width: 100%;
}

.tabs-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tabs-header {
  flex-shrink: 0;
}

/* 危险操作按钮悬停效果 */
.btn-danger-hover {
  transition: all 0.3s;
}

.btn-danger-hover:hover {
  color: #f56c6c !important;
  background-color: #fef0f0 !important;
  border-color: #fde2e2 !important;
}

:deep(.app-dialog-table .virtual-spacer-row > td.el-table__cell) {
  padding: 0 !important;
  border-bottom: 0 !important;
  background: transparent !important;
}

:deep(.app-dialog-table .virtual-spacer-row .cell) {
  padding: 0 !important;
  min-height: 0 !important;
  height: 100% !important;
  visibility: hidden;
}

:deep(.el-dropdown),
:deep(.el-dropdown *) {
  color: inherit !important;
}

:deep(.el-tabs__nav[role='tablist']) {
  border: none !important;
}

:deep(.el-tabs__nav-prev),
:deep(.el-tabs__nav-next) {
  height: 28px !important;
  line-height: 28px !important;
}

.action-buttons {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

/* Hover 时显示操作按钮 */
:deep(.el-table__row:hover) .action-buttons {
  opacity: 1;
}

.action-btn {
  color: #6b7280 !important;
  border-color: #e5e7eb !important;
  background: transparent !important;
}

.action-btn:hover {
  color: #3b82f6 !important;
  border-color: #3b82f6 !important;
  background: #eff6ff !important;
}

.el-button {
  margin: 0;
}

:deep(.app-dialog-table .el-table__row) {
  cursor: pointer;
}

:deep(.app-dialog-table .el-table__body td.el-table__cell) {
  padding: 6px 0;
}

.point-edit-dialog :deep(.el-form-item),
.point-def-generator-dialog :deep(.el-form-item) {
  margin-bottom: 18px;
}

.point-edit-dialog :deep(.el-form-item:last-child),
.point-def-generator-dialog :deep(.el-form-item:last-child) {
  margin-bottom: 0;
}
</style>
