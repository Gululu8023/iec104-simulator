import { computed, nextTick, onUnmounted, reactive, ref, type Ref, watch } from 'vue'

import { ElMessage } from 'element-plus'

import {
  formatIec104TypeCompactLabel,
  formatIec104TypeLabel,
  iec104TypeNameToId,
  normalizeIecObjectDisplayName,
  qualityLevelFromCommon,
  resolveIec104TypeId,
} from '@shared/api/iec104'
import {
  MASTER_REMOTE_CONTROL_FILTERS,
  isMasterControlDataType,
  isMasterControlGroupMarker,
  resolveMasterControlCommandType,
  resolveMasterControlGroupByMarker,
  resolveMasterControlGroupLabelByMarker,
  resolveMasterSetpointTypeByDataType,
  resolveMasterTabDataTypes,
  type MasterRemoteControlFilterId,
} from '@shared/api/masterControl'
import { resolveEffectiveQualityDetail } from '@shared/api/protocolQuality'
import {
  useTableVirtualScroll,
  type VirtualSpacerRow,
} from '@shared/composables/useTableVirtualScroll'
import {
  getIecDataTypeLabel,
  getIecDataTypeTabLabel,
  getIecDataTypeTreeIcon,
} from '@shared/ui/dataTypeIcon'
import { resolveAdjacentTabId } from '@shared/ui/tabClose'
import {
  getPointTableSortValue,
  sortPointTableRows,
  sortPointTableRowsInFrames,
  type PointTableSortOrder,
  type PointTableSortProp,
} from '@shared/ui/pointTableSort'
import { currentLocale, t } from '@shared/i18n'

import type { ControlCommandType, SlaveConnection } from '@/types/master'
import type {
  MasterContentRecentWindowOption,
  MasterContentTabRow as TabRow,
  MasterContentTabState as TabState,
} from '@/types/masterContentPanel'
import type { BackendDataPoint, PointDef } from '@shared/api/types'

import { resolveMasterTabConnectionId, useMasterContentView } from './useMasterContentView'
import type {
  IndexedMasterPointDef as IndexedPointDef,
  IndexedMasterRuntimePoint as IndexedRuntimePoint,
  MasterPointData,
} from './useMasterPointData'
import { useMasterPointHistory } from './useMasterPointHistory'

type FocusPointPayload = {
  profileId?: string | number | null
  parentSlaveId?: string | number | null
  slaveId?: string | number | null
  dataType: string
  label?: string
  linkProfileId?: number | null
  connectionId?: string | null
  slaveCommonAddress?: number | null
  slaveName?: string
  address: number
}

type UseMasterContentInteractionsOptions = {
  selectedSlave: Ref<SlaveConnection | null | undefined>
  pointData: MasterPointData
  profilePointDefsMap: Ref<Record<string, PointDef[]>>
  ioaDisplayFormat: Ref<'dec' | 'hex'>
  emitRefreshData: () => void
  emitSendCommand: (command: ControlCommandType, params?: any) => void
  formatCot: (raw: unknown) => string
  formatOptionalCp56: (value: string | null | undefined) => string
  formatTimestamp: (value: string) => string
}

const TAB_PROJECTION_FRAME_BUDGET_MS = 4

export function useMasterContentInteractions(options: UseMasterContentInteractionsOptions) {
  const activeTabs = ref<TabState[]>([])
  const activeTabName = ref('')
  const tableSize = ref<'small' | 'default' | 'large'>('default')
  const dataTableRef = ref<any>(null)
  let restoringTableSort = false
  let lastSortedTable: any = null
  let lastTableSortState = ''
  const pendingTabLoads = new Set<string>()
  const suspendedProfileIds = new Set<string>()
  let tabLoadFrame: number | null = null
  let tabLoadCommitFrame: number | null = null
  const quickFilters = reactive({
    onlyMapped: false,
    onlyAbnormalQuality: false,
    onlyRecentChange: false,
    recentWindowSec: 10,
  })
  const columnVisibility = reactive({
    showType: false,
    showDescription: false,
    showCause: true,
    showReportCount: true,
    showOaCa: false,
    showCp56: false,
    showPointSource: false,
  })
  const recentWindowOptions: MasterContentRecentWindowOption[] = [
    { label: '10s', value: 10 },
    { label: '30s', value: 30 },
    { label: '1m', value: 60 },
  ]

  const formatIoaAddress = (value: unknown): string => {
    const numeric = Number(value)
    if (!Number.isFinite(numeric)) return '--'
    const normalized = Math.max(0, Math.trunc(numeric))
    if (options.ioaDisplayFormat.value === 'hex') {
      return `0x${normalized.toString(16).toUpperCase().padStart(6, '0')}`
    }
    return String(normalized)
  }

  const {
    currentTab,
    currentTabControlGroup,
    currentTabFilteredData,
    currentTabEmptyState,
    currentTabIsControl,
    showControlSubtypeFilter,
    showControlTypeColumn,
    hasActiveFilters,
    activeFilterSummary,
    currentControlTabUsesBuiltIn,
  } = useMasterContentView({
    activeTabs,
    activeTabName,
    selectedSlave: options.selectedSlave,
    profileHasImportedPointDefs: (profileId) => {
      void options.profilePointDefsMap.value
      return options.pointData.profileHasImportedPointDefs(profileId)
    },
    profileHasBuiltInPointDefs: (profileId, tabDataType) => {
      void options.profilePointDefsMap.value
      return options.pointData.profileHasBuiltInPointDefs(profileId, resolveTabTypeIds(tabDataType))
    },
    quickFilters,
    recentWindowOptions,
    formatIoaAddress,
  })

  const virtualRowHeightPx = computed(() =>
    tableSize.value === 'small' ? 36 : tableSize.value === 'large' ? 48 : 44,
  )
  const tableVirtualScroll = useTableVirtualScroll<TabRow>({
    tableRef: dataTableRef,
    sourceRows: currentTabFilteredData,
    rowHeight: virtualRowHeightPx,
    threshold: 60,
    overscan: 12,
  })
  const virtualTableRows = computed(() => tableVirtualScroll.renderRows.value as TabRow[])
  const isVirtualSpacerRow = (row: unknown): row is VirtualSpacerRow =>
    tableVirtualScroll.isVirtualSpacerRow(row)
  const focusedAddressByTab = ref<Record<string, number>>({})
  const rowsByAddressByTab = new WeakMap<TabState, Map<number, TabRow>>()
  const rowIndexesByTab = new WeakMap<TabState, Map<number, number>>()
  const resolveTableRowClassName = ({ row }: { row: TabRow | VirtualSpacerRow }) => {
    const baseClass = tableVirtualScroll.resolveRowClassName(row, 'data-row')
    if (isVirtualSpacerRow(row)) return baseClass
    const focusedAddress = focusedAddressByTab.value[activeTabName.value]
    return focusedAddress === normalizeTableAddress(row.address)
      ? `${baseClass} is-located-row`
      : baseClass
  }
  const resolveTableRowStyle = ({ row }: { row: TabRow | VirtualSpacerRow }) =>
    tableVirtualScroll.resolveRowStyle(row)
  const normalizeTableAddress = (value: unknown): number =>
    Math.max(0, Math.trunc(Number(value) || 0))
  const getTableRowKey = (row: TabRow | VirtualSpacerRow) =>
    isVirtualSpacerRow(row)
      ? row.__virtualKey
      : `${activeTabName.value}:${normalizeTableAddress(row.address)}`

  const remoteControlFilterIndexMap = new Map<MasterRemoteControlFilterId, number>(
    MASTER_REMOTE_CONTROL_FILTERS.map((option, index) => [option.value, index]),
  )
  const remoteControlFilterSliderStyle = computed(() => ({
    '--app-segment-count': String(MASTER_REMOTE_CONTROL_FILTERS.length),
    '--app-segment-index': String(
      remoteControlFilterIndexMap.get(currentTab.value?.controlSubtypeFilter ?? 'all') ?? 0,
    ),
    '--app-segment-min-width': '60px',
  }))

  const {
    buildPointHistoryKey,
    historyDialog,
    historyDialogCaText,
    historyDialogEntries,
    historyDialogLatestEntry,
    openHistoryDialog,
    recordMonitorPointHistory,
    toggleHistoryEntry,
  } = useMasterPointHistory({
    currentTab,
    selectedSlave: options.selectedSlave,
    formatCot: options.formatCot,
    formatOptionalCp56: options.formatOptionalCp56,
    formatTimestamp: options.formatTimestamp,
    isControlDataType: isMasterControlDataType,
    qualityLevelFromCommon,
  })

  function resolveTabTypeIds(tabDataType: string): number[] {
    return resolveMasterTabDataTypes(tabDataType)
      .map((dataType) => iec104TypeNameToId(dataType))
      .filter((typeId): typeId is number => typeId != null)
  }

  function getRuntimePointsForTab(tab: TabState): IndexedRuntimePoint[] {
    return options.pointData.getRuntimePoints(buildRuntimePointQuery(tab))
  }

  function buildRuntimePointQuery(tab: TabState) {
    return {
      typeIds: resolveTabTypeIds(tab.dataType),
      slaveId: tab.profileId,
      linkProfileId: tab.linkProfileId,
      connectionId: tab.connectionId,
    }
  }

  function getRuntimePointForTabAddress(
    tab: TabState,
    address: number,
  ): IndexedRuntimePoint | null {
    return options.pointData.getRuntimePointAt(buildRuntimePointQuery(tab), address)
  }

  function getProfilePointDefsForTab(tab: TabState): IndexedPointDef[] {
    return options.pointData.getProfilePointDefs(tab.profileId, resolveTabTypeIds(tab.dataType))
  }

  function getProfilePointDefForTabAddress(
    tab: TabState,
    typeIds: ReadonlySet<number>,
    address: number,
  ): IndexedPointDef | null {
    return options.pointData.getProfilePointDefAt(tab.profileId, typeIds, address)
  }

  function isControlTab(tab: TabState): boolean {
    return tab.dataType.startsWith('C_') || isMasterControlGroupMarker(tab.dataType)
  }

  const waitForProjectionTurn = (active: boolean) =>
    new Promise<void>((resolve) => {
      if (!active && typeof window.requestIdleCallback === 'function') {
        window.requestIdleCallback(() => resolve(), { timeout: 50 })
        return
      }
      requestAnimationFrame(() => resolve())
    })

  async function flushScheduledTabLoads() {
    const activeId = activeTabName.value
    const tabIds = [...pendingTabLoads].sort((left, right) => {
      if (left === activeId) return -1
      if (right === activeId) return 1
      return 0
    })
    pendingTabLoads.clear()

    for (const tabId of tabIds) {
      const tab = activeTabs.value.find((candidate) => candidate.id === tabId)
      if (!tab) continue
      const revision = tab.dataRevision
      if (tab.id !== activeTabName.value) await waitForProjectionTurn(false)
      const nextRows = await buildTabData(tab, revision)
      if (!nextRows || !activeTabs.value.includes(tab) || tab.dataRevision !== revision) continue
      patchTabRows(tab, nextRows)
      tab.loading = false
    }

    tabLoadCommitFrame = null
    if (pendingTabLoads.size > 0) scheduleTabLoadPump()
  }

  function scheduleTabLoadPump() {
    if (tabLoadFrame != null || tabLoadCommitFrame != null) return
    tabLoadFrame = requestAnimationFrame(() => {
      tabLoadFrame = null
      tabLoadCommitFrame = requestAnimationFrame(() => void flushScheduledTabLoads())
    })
  }

  function scheduleTabLoad(tab: TabState) {
    if (suspendedProfileIds.has(String(tab.profileId))) return
    if (pendingTabLoads.has(tab.id)) return
    tab.dataRevision += 1
    pendingTabLoads.add(tab.id)
    scheduleTabLoadPump()
  }

  function cancelTabLoad(tab: TabState) {
    tab.dataRevision += 1
    pendingTabLoads.delete(tab.id)
    if (pendingTabLoads.size > 0) return
    if (tabLoadFrame != null) cancelAnimationFrame(tabLoadFrame)
    if (tabLoadCommitFrame != null) cancelAnimationFrame(tabLoadCommitFrame)
    tabLoadFrame = null
    tabLoadCommitFrame = null
  }

  function cancelAllTabLoads() {
    for (const tab of activeTabs.value) cancelTabLoad(tab)
    if (tabLoadFrame != null) cancelAnimationFrame(tabLoadFrame)
    if (tabLoadCommitFrame != null) cancelAnimationFrame(tabLoadCommitFrame)
    tabLoadFrame = null
    tabLoadCommitFrame = null
  }

  watch(options.pointData.revision, () => {
    recordMonitorPointHistory(options.pointData.lastUpserts.value)
    refreshTabsDataForRuntimeChanges()
  })

  watch(
    options.profilePointDefsMap,
    (pointDefsByProfile, previousPointDefsByProfile) => {
      options.pointData.updateProfilePointDefs(pointDefsByProfile)
      const changedProfileIds = new Set(
        [
          ...Object.keys(pointDefsByProfile),
          ...Object.keys(previousPointDefsByProfile ?? {}),
        ].filter(
          (profileId) => pointDefsByProfile[profileId] !== previousPointDefsByProfile?.[profileId],
        ),
      )
      for (const tab of activeTabs.value) {
        if (isControlTab(tab) && changedProfileIds.has(String(tab.profileId))) scheduleTabLoad(tab)
      }
    },
    { deep: false, immediate: true },
  )

  watch(
    options.selectedSlave,
    (slave) => {
      if (!slave) return
      for (const tab of activeTabs.value) {
        if (Number(tab.profileId) !== Number(slave.profileId)) continue
        const connectionId = slave.connectionId ?? null
        if (
          tab.connectionId === connectionId &&
          tab.linkProfileId === slave.linkProfileId &&
          tab.slaveCommonAddress === slave.commonAddress
        ) {
          continue
        }
        tab.connectionId = connectionId
        tab.linkProfileId = slave.linkProfileId
        tab.slaveCommonAddress = slave.commonAddress
        scheduleTabLoad(tab)
      }
    },
    { deep: false },
  )

  watch(activeTabName, () => {
    syncTableSortIndicator()
  })

  watch(dataTableRef, () => {
    syncTableSortIndicator()
  })

  onUnmounted(cancelAllTabLoads)

  function addTab(datatype: any) {
    const dataType = String(datatype.dataType ?? '')
    const tabId = `${datatype.parentSlaveId}-${dataType}`
    const label =
      normalizeIecObjectDisplayName(String(datatype.label ?? '').replace(/\(\d+\)\s*$/g, '')) ||
      resolveMasterControlGroupLabelByMarker(dataType) ||
      getIecDataTypeLabel(dataType)
    const existing = activeTabs.value.find((tab) => tab.id === tabId)
    if (existing) {
      existing.label = label
      existing.connectionId = datatype.connectionId ?? existing.connectionId
      existing.linkProfileId = Number(datatype.linkProfileId ?? existing.linkProfileId ?? 0) || null
      activeTabName.value = tabId
      return
    }

    const tab: TabState = {
      id: tabId,
      label,
      dataType,
      profileId: String(datatype.profileId ?? datatype.parentSlaveId ?? ''),
      linkProfileId: Number(datatype.linkProfileId ?? 0) || null,
      connectionId: datatype.connectionId ?? options.selectedSlave.value?.connectionId ?? null,
      slaveCommonAddress: datatype.slaveCommonAddress,
      slaveName: datatype.slaveName || t('soePanel.source.slave', { id: datatype.parentSlaveId }),
      searchText: '',
      controlSubtypeFilter: 'all',
      sortProp: 'address',
      sortOrder: 'ascending',
      data: [],
      loading: true,
      dataRevision: 0,
    }
    activeTabs.value.push(tab)
    activeTabName.value = tab.id
    scheduleTabLoad(tab)
  }

  function removeTab(tabId: string) {
    const nextActiveTabId = resolveAdjacentTabId(activeTabs.value, tabId, activeTabName.value)
    const removedTab = activeTabs.value.find((tab) => tab.id === tabId)
    if (removedTab) cancelTabLoad(removedTab)
    activeTabs.value = activeTabs.value.filter((tab) => tab.id !== tabId)
    const nextFocusedAddresses = { ...focusedAddressByTab.value }
    delete nextFocusedAddresses[tabId]
    focusedAddressByTab.value = nextFocusedAddresses
    if (activeTabName.value === tabId) activeTabName.value = nextActiveTabId
  }

  function closeOtherTabs(tabId: string) {
    const preserved = activeTabs.value.find((tab) => tab.id === tabId)
    for (const tab of activeTabs.value) {
      if (tab !== preserved) cancelTabLoad(tab)
    }
    activeTabs.value = preserved ? [preserved] : []
    activeTabName.value = preserved?.id ?? ''
    focusedAddressByTab.value =
      focusedAddressByTab.value[tabId] == null ? {} : { [tabId]: focusedAddressByTab.value[tabId] }
  }

  function closeAllTabs() {
    cancelAllTabLoads()
    activeTabs.value = []
    activeTabName.value = ''
    focusedAddressByTab.value = {}
  }

  function refreshAllTabsData() {
    for (const tab of activeTabs.value) scheduleTabLoad(tab)
  }

  function runtimePointMatchesTab(
    point: BackendDataPoint,
    tab: TabState,
    typeIds: ReadonlySet<number>,
  ): boolean {
    if (!typeIds.has(resolveIec104TypeId(point))) return false
    const scopeMatches =
      (Number(tab.profileId) > 0 && Number(point.slave_id ?? 0) === Number(tab.profileId)) ||
      (Number(tab.linkProfileId ?? 0) > 0 &&
        Number(point.link_profile_id ?? 0) === Number(tab.linkProfileId)) ||
      (Boolean(tab.connectionId) && String(point.connection_id ?? '') === String(tab.connectionId))
    return scopeMatches && shouldUseRuntimePoint(point, tab)
  }

  async function patchTabFromRuntimeDelta(
    tab: TabState,
    delta: NonNullable<MasterPointData['lastDelta']['value']>,
  ) {
    const revision = ++tab.dataRevision
    const typeIds = new Set(resolveTabTypeIds(tab.dataType))
    const affectedAddresses = new Set<number>()
    let frameStartedAt = performance.now()
    const yieldWhenNeeded = async () => {
      await waitForProjectionTurn(tab.id === activeTabName.value)
      frameStartedAt = performance.now()
      return activeTabs.value.includes(tab) && tab.dataRevision === revision
    }

    for (const removal of delta.removed) {
      if (removal.previous && runtimePointMatchesTab(removal.previous, tab, typeIds)) {
        affectedAddresses.add(removal.previous.address)
      }
      if (
        performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
        !(await yieldWhenNeeded())
      ) {
        return
      }
    }
    for (const upsert of delta.upserts) {
      if (
        (upsert.previous && runtimePointMatchesTab(upsert.previous, tab, typeIds)) ||
        runtimePointMatchesTab(upsert.point, tab, typeIds)
      ) {
        affectedAddresses.add(upsert.point.address)
      }
      if (
        performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
        !(await yieldWhenNeeded())
      ) {
        return
      }
    }
    if (
      affectedAddresses.size === 0 ||
      !activeTabs.value.includes(tab) ||
      tab.dataRevision !== revision
    ) {
      return
    }

    const changes = new Map<number, TabRow | null>()
    for (const address of affectedAddresses) {
      const runtimePoint = getRuntimePointForTabAddress(tab, address)
      if (runtimePoint && shouldUseRuntimePoint(runtimePoint.point, tab)) {
        changes.set(address, backendPointToRow(runtimePoint, tab))
      } else {
        const pointDef = isControlTab(tab)
          ? getProfilePointDefForTabAddress(tab, typeIds, address)
          : null
        changes.set(address, pointDef ? pointDefToRow(pointDef) : null)
      }
      if (
        performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
        !(await yieldWhenNeeded())
      ) {
        return
      }
    }

    const { rowsByAddress, rowIndexes } = getTabRowLookup(tab)
    let membershipChanged = false
    let sortValueChanged = false
    let rowChanged = false
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
      if (buildRowSignature(currentRow) === buildRowSignature(nextRow)) continue
      if (
        getPointTableSortValue(currentRow, tab.sortProp) !==
        getPointTableSortValue(nextRow, tab.sortProp)
      ) {
        sortValueChanged = true
      }
      rowsByAddress.set(address, nextRow)
      const rowIndex = rowIndexes.get(address)
      if (rowIndex != null) tab.data[rowIndex] = nextRow
      rowChanged = true
    }

    if (membershipChanged || sortValueChanged) {
      tab.data = sortTabRows(tab, [...rowsByAddress.values()])
      rebuildTabRowLookup(tab)
    } else if (rowChanged) {
      rowsByAddressByTab.set(tab, rowsByAddress)
    }
  }

  let runtimePatchQueue = Promise.resolve()
  let runtimePatchEpoch = 0
  function refreshTabsDataForRuntimeChanges() {
    const delta = options.pointData.lastDelta.value
    if (!delta || delta.reset) {
      runtimePatchEpoch += 1
      for (const tab of activeTabs.value) scheduleTabLoad(tab)
      return
    }
    const patchEpoch = runtimePatchEpoch
    runtimePatchQueue = runtimePatchQueue.then(async () => {
      if (patchEpoch !== runtimePatchEpoch) return
      const tabs = [...activeTabs.value].sort((left, right) => {
        if (left.id === activeTabName.value) return -1
        if (right.id === activeTabName.value) return 1
        return 0
      })
      for (const tab of tabs) {
        if (patchEpoch !== runtimePatchEpoch) return
        if (suspendedProfileIds.has(String(tab.profileId))) continue
        if (tab.id !== activeTabName.value) await waitForProjectionTurn(false)
        await patchTabFromRuntimeDelta(tab, delta)
      }
    })
  }

  function beginPointTableReplacement(profileId: string | number) {
    const key = String(profileId)
    suspendedProfileIds.add(key)
    for (const tab of activeTabs.value) {
      if (String(tab.profileId) !== key) continue
      cancelTabLoad(tab)
      tab.loading = true
    }
  }

  function completePointTableReplacement(profileId: string | number) {
    const key = String(profileId)
    suspendedProfileIds.delete(key)
    for (const tab of activeTabs.value) {
      if (String(tab.profileId) === key) scheduleTabLoad(tab)
    }
  }

  function rebuildTabRowLookup(tab: TabState) {
    const rowsByAddress = new Map<number, TabRow>()
    const rowIndexes = new Map<number, number>()
    tab.data.forEach((row, index) => {
      rowsByAddress.set(row.address, row)
      rowIndexes.set(row.address, index)
    })
    rowsByAddressByTab.set(tab, rowsByAddress)
    rowIndexesByTab.set(tab, rowIndexes)
    return { rowsByAddress, rowIndexes }
  }

  function getTabRowLookup(tab: TabState) {
    const rowsByAddress = rowsByAddressByTab.get(tab)
    const rowIndexes = rowIndexesByTab.get(tab)
    return rowsByAddress && rowIndexes ? { rowsByAddress, rowIndexes } : rebuildTabRowLookup(tab)
  }

  function patchTabRows(tab: TabState, nextRows: TabRow[]): boolean {
    const nextByAddress = new Map(nextRows.map((row) => [row.address, row]))
    if (
      tab.data.length !== nextRows.length ||
      tab.data.some((row) => !nextByAddress.has(row.address))
    ) {
      tab.data = sortTabRows(tab, nextRows)
      rebuildTabRowLookup(tab)
      return true
    }

    let changed = false
    let sortValueChanged = false
    for (let index = 0; index < tab.data.length; index += 1) {
      const currentRow = tab.data[index]
      const nextRow = nextByAddress.get(currentRow.address)
      if (!nextRow || buildRowSignature(currentRow) === buildRowSignature(nextRow)) continue
      if (
        getPointTableSortValue(currentRow, tab.sortProp) !==
        getPointTableSortValue(nextRow, tab.sortProp)
      ) {
        sortValueChanged = true
      }
      tab.data[index] = nextRow
      changed = true
    }
    if (sortValueChanged) tab.data = sortTabRows(tab, tab.data)
    if (changed) rebuildTabRowLookup(tab)
    return changed
  }

  function sortTabRows(tab: TabState, rows: readonly TabRow[]): TabRow[] {
    if (
      tab.sortProp === 'address' &&
      tab.sortOrder === 'ascending' &&
      rows.every((row, index) => index === 0 || rows[index - 1].address <= row.address)
    ) {
      return Array.isArray(rows) ? rows : [...rows]
    }
    return sortPointTableRows(rows, tab.sortProp, tab.sortOrder)
  }

  function isSortableProp(value: unknown): value is PointTableSortProp {
    return ['address', 'name', 'reportCount', 'timestamp'].includes(String(value))
  }

  function handleSortChange(payload: { prop?: string; order?: PointTableSortOrder | null }) {
    if (restoringTableSort) return
    const tab = currentTab.value
    if (!tab) return
    if (isSortableProp(payload.prop) && payload.order) {
      tab.sortProp = payload.prop
      tab.sortOrder = payload.order
      lastSortedTable = dataTableRef.value
      lastTableSortState = `${tab.sortProp}:${tab.sortOrder}`
    } else {
      tab.sortProp = 'address'
      tab.sortOrder = 'ascending'
      lastTableSortState = ''
    }
    tab.data = sortTabRows(tab, tab.data)
    rebuildTabRowLookup(tab)
    if (!payload.order) syncTableSortIndicator()
  }

  function syncTableSortIndicator() {
    nextTick(() => {
      const tab = currentTab.value
      const table = dataTableRef.value
      if (!tab || !table?.sort) return
      const sortState = `${tab.sortProp}:${tab.sortOrder}`
      if (lastSortedTable === table && lastTableSortState === sortState) return
      lastSortedTable = table
      lastTableSortState = sortState
      restoringTableSort = true
      table.sort(tab.sortProp, tab.sortOrder)
      nextTick(() => {
        restoringTableSort = false
      })
    })
  }

  async function buildTabData(tab: TabState, revision: number): Promise<TabRow[] | null> {
    const map = new Map<number, TabRow>()
    const runtimeOrderByAddress = new Map<number, number>()
    let frameStartedAt = performance.now()
    const yieldWhenNeeded = async () => {
      await waitForProjectionTurn(tab.id === activeTabName.value)
      frameStartedAt = performance.now()
      return activeTabs.value.includes(tab) && tab.dataRevision === revision
    }
    for (const indexedPoint of getRuntimePointsForTab(tab)) {
      const point = indexedPoint.point
      if (!shouldUseRuntimePoint(point, tab)) continue
      const previousOrder = runtimeOrderByAddress.get(point.address)
      if (previousOrder != null && previousOrder > indexedPoint.order) continue
      runtimeOrderByAddress.set(point.address, indexedPoint.order)
      map.set(point.address, backendPointToRow(indexedPoint, tab))
      if (
        performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
        !(await yieldWhenNeeded())
      ) {
        return null
      }
    }
    if (isControlTab(tab)) {
      for (const indexedPointDef of getProfilePointDefsForTab(tab)) {
        if (map.has(indexedPointDef.pointDef.address)) continue
        map.set(indexedPointDef.pointDef.address, pointDefToRow(indexedPointDef))
        if (
          performance.now() - frameStartedAt >= TAB_PROJECTION_FRAME_BUDGET_MS &&
          !(await yieldWhenNeeded())
        ) {
          return null
        }
      }
    }
    return sortPointTableRowsInFrames(
      Array.from(map.values()),
      tab.sortProp,
      tab.sortOrder,
      () => activeTabs.value.includes(tab) && tab.dataRevision === revision,
      TAB_PROJECTION_FRAME_BUDGET_MS,
    )
  }

  function backendPointToRow(indexedPoint: IndexedRuntimePoint, tab: TabState): TabRow {
    const { point, typeId, dataType } = indexedPoint
    const isControl = isMasterControlDataType(dataType) || isMasterControlGroupMarker(dataType)
    const effectiveQualityDetail = resolveEffectiveQualityDetail(
      typeId,
      point.quality_detail,
      Number(point.quality ?? 0),
      point.value,
    )

    const rawCause = point.latest_cause == null ? null : Math.trunc(Number(point.latest_cause))
    const originatorAddress =
      rawCause != null && Number.isFinite(rawCause) ? (rawCause >> 8) & 0xff : null

    return {
      address: point.address,
      name: point.name,
      description: point.description ?? '',
      dataType,
      typeId,
      historyKey: isMasterControlDataType(dataType)
        ? null
        : buildPointHistoryKey(point.slave_id ?? tab.profileId, point.address, dataType),
      value: Number(point.value ?? 0),
      qualityCode: Number(point.quality ?? 0),
      qualityLevel: qualityLevelFromCommon(
        point.quality_common,
        point.quality,
        effectiveQualityDetail,
      ),
      qualityCommon: point.quality_common ?? null,
      qualityDetail: effectiveQualityDetail,
      timestamp: String(point.timestamp ?? ''),
      latestCauseText: options.formatCot(point.latest_cause),
      reportCount: point.report_count == null ? null : Number(point.report_count),
      oaCaText: `${originatorAddress ?? '—'} / ${point.common_address ?? '—'}`,
      cp56Text: isControl
        ? (point.control_status_snapshot?.command_cp56 ?? '—')
        : options.formatOptionalCp56(point.latest_event_timestamp),
      pointSource: point.point_source ?? null,
      controlStatusSnapshot: point.control_status_snapshot ?? null,
      controlIoa: point.control_ioa ?? null,
      isRuntime: true,
    }
  }

  function pointDefToRow(indexedPointDef: IndexedPointDef): TabRow {
    const { pointDef: def, typeId, dataType } = indexedPointDef
    const value = Number(def.default_value)
    return {
      address: def.address,
      name: def.name,
      description: def.description ?? '',
      dataType,
      typeId,
      historyKey: null,
      value: Number.isFinite(value) ? value : 0,
      qualityCode: 0,
      qualityLevel: 'good',
      qualityCommon: null,
      qualityDetail: null,
      timestamp: '',
      latestCauseText: '—',
      reportCount: null,
      oaCaText: '— / —',
      cp56Text: '—',
      pointSource: def.source ?? null,
      controlStatusSnapshot: null,
      controlIoa: def.control_ioa ?? null,
      isRuntime: false,
    }
  }

  function shouldUseRuntimePoint(point: BackendDataPoint, tab: TabState): boolean {
    return (
      tab.dataType.startsWith('C_') ||
      isMasterControlGroupMarker(tab.dataType) ||
      point.report_count != null ||
      point.latest_cause != null
    )
  }

  function clearQuickFilters() {
    quickFilters.onlyMapped = false
    quickFilters.onlyAbnormalQuality = false
    quickFilters.onlyRecentChange = false
    quickFilters.recentWindowSec = 10
  }

  function handleDensityChange(size: 'small' | 'default' | 'large') {
    tableSize.value = size
  }

  function handleRefreshData() {
    refreshAllTabsData()
    options.emitRefreshData()
  }

  function handleControlDataPoint(row: TabRow) {
    const command =
      resolveMasterControlCommandType(row.dataType) ??
      (currentTabControlGroup.value?.id === 'remote-adjust' ? 'set-point' : 'single-command')
    const extraParams: Record<string, unknown> = {
      name: row.name,
      typeId: row.typeId,
    }
    const setpointType = resolveMasterSetpointTypeByDataType(row.dataType)
    if (command === 'set-point' && setpointType) {
      extraParams.setpointType = setpointType
    }
    sendControl(command, row.address, row.value, extraParams)
  }

  function handleReadDataPoint(row: TabRow) {
    sendControl('read-command', row.address, 0, { name: row.name, typeId: row.typeId })
  }

  function sendControl(
    command: ControlCommandType,
    address: number,
    value: number,
    extraParams: Record<string, unknown> = {},
  ) {
    const tab = currentTab.value
    const connectionId = resolveMasterTabConnectionId(tab, options.selectedSlave.value)
    if (!tab || !connectionId) {
      ElMessage.warning(t('master.runtime.warnings.currentSlaveDisconnected'))
      return
    }
    options.emitSendCommand(command, {
      address,
      value,
      targetSlaveId: connectionId,
      slaveId: Number(tab.profileId),
      ...extraParams,
    })
  }

  function buildRowSignature(row: TabRow | undefined) {
    if (!row) return ''
    return [
      row.address,
      row.name,
      row.description,
      row.dataType,
      row.typeId,
      row.value,
      row.qualityCode,
      row.qualityLevel,
      row.timestamp,
      row.latestCauseText,
      row.reportCount ?? '',
      row.oaCaText,
      row.cp56Text,
      row.pointSource ?? '',
      row.controlIoa ?? '',
      row.isRuntime ? '1' : '0',
      row.controlStatusSnapshot?.state ?? '',
      row.controlStatusSnapshot?.updated_at ?? '',
      row.controlStatusSnapshot?.latest_cause ?? '',
      row.controlStatusSnapshot?.actcon_negative ?? '',
      row.controlStatusSnapshot?.actcon_at ?? '',
      row.controlStatusSnapshot?.actterm_at ?? '',
      row.controlStatusSnapshot?.timeout_type ?? '',
      row.controlStatusSnapshot?.select_execute ?? '',
      row.controlStatusSnapshot?.command_cp56 ?? '',
    ].join('|')
  }

  function getTabIcon(dataType: string) {
    return getIecDataTypeTreeIcon(
      resolveMasterControlGroupByMarker(dataType)?.iconDataType ?? dataType,
    )
  }

  function getTabBadge(tab: TabState) {
    return tab.slaveCommonAddress != null ? `CA:${tab.slaveCommonAddress}` : `S${tab.profileId}`
  }

  function getTabDisplayLabel(tab: TabState) {
    void currentLocale.value
    return (
      resolveMasterControlGroupLabelByMarker(tab.dataType) ||
      getIecDataTypeTabLabel(tab.dataType) ||
      tab.label
    )
  }

  function getTabTooltip(tab: TabState) {
    return t('master.contentPanel.tabs.tooltip', {
      slaveName: tab.slaveName,
      typeLabel:
        resolveMasterControlGroupLabelByMarker(tab.dataType) ||
        formatIec104TypeLabel(iec104TypeNameToId(tab.dataType)),
      profileId: tab.profileId,
    })
  }

  function getControlTypeLabel(row: TabRow) {
    return formatIec104TypeCompactLabel(row.typeId)
  }

  function focusPoint(payload: FocusPointPayload) {
    const targetAddress = Math.max(0, Math.trunc(Number(payload.address) || 0))
    const profileId = payload.profileId ?? payload.parentSlaveId ?? payload.slaveId
    const dataType = String(payload.dataType ?? '')
      .trim()
      .toUpperCase()
    if (!profileId || !dataType) return

    addTab({
      ...payload,
      parentSlaveId: profileId,
      profileId,
      dataType,
      label: payload.label || getIecDataTypeLabel(dataType),
    })

    const tabId = `${profileId}-${dataType}`
    const tab = activeTabs.value.find((item) => item.id === tabId)
    if (!tab) return
    tab.searchText = String(targetAddress)
    focusedAddressByTab.value = {
      ...focusedAddressByTab.value,
      [tabId]: targetAddress,
    }
    activeTabName.value = tabId
  }

  return {
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
  }
}
