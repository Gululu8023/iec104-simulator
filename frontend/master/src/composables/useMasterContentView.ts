import { computed, type Ref } from 'vue'
import type { SlaveConnection } from '@/types/master'
import {
  isMasterControlGroupMarker,
  resolveMasterControlCommandType,
  resolveMasterControlGroupByDataType,
  resolveMasterControlGroupByMarker,
} from '@shared/api/masterControl'
import { t } from '@shared/i18n'
import type {
  MasterContentEmptyState,
  MasterContentQuickFilters,
  MasterContentRecentWindowOption,
  MasterContentTabRow,
  MasterContentTabState,
} from '@/types/masterContentPanel'

type UseMasterContentViewOptions = {
  activeTabs: Ref<MasterContentTabState[]>
  activeTabName: Ref<string>
  selectedSlave: Ref<SlaveConnection | null | undefined>
  profileHasImportedPointDefs: (profileId: string) => boolean
  profileHasBuiltInPointDefs: (profileId: string, tabDataType: string) => boolean
  quickFilters: MasterContentQuickFilters
  recentWindowOptions: ReadonlyArray<MasterContentRecentWindowOption>
  formatIoaAddress: (value: unknown) => string
}

export function resolveMasterTabConnectionId(
  tab: MasterContentTabState | null,
  selectedSlave: SlaveConnection | null | undefined,
): string | null {
  if (!tab) return null
  if (tab.connectionId) return tab.connectionId
  return selectedSlave && Number(selectedSlave.profileId) === Number(tab.profileId)
    ? (selectedSlave.connectionId ?? null)
    : null
}

export function useMasterContentView(options: UseMasterContentViewOptions) {
  const currentTab = computed(
    () => options.activeTabs.value.find((tab) => tab.id === options.activeTabName.value) ?? null,
  )
  const currentTabControlGroup = computed(() => {
    const tab = currentTab.value
    if (!tab) return null
    return (
      resolveMasterControlGroupByMarker(tab.dataType) ??
      resolveMasterControlGroupByDataType(tab.dataType)
    )
  })
  const currentTabIsControl = computed(() =>
    Boolean(currentTabControlGroup.value || currentTab.value?.dataType.startsWith('C_')),
  )
  const currentProfileHasImportedPointDefs = computed(() => {
    const tab = currentTab.value
    return tab ? options.profileHasImportedPointDefs(String(tab.profileId)) : false
  })
  const showControlSubtypeFilter = computed(
    () =>
      currentTabControlGroup.value?.id === 'remote-control' &&
      isMasterControlGroupMarker(currentTab.value?.dataType),
  )
  const showControlTypeColumn = computed(
    () => currentTabControlGroup.value?.id === 'remote-control',
  )
  const hasActiveFilters = computed(
    () =>
      options.quickFilters.onlyMapped ||
      options.quickFilters.onlyAbnormalQuality ||
      options.quickFilters.onlyRecentChange,
  )
  const activeFilterSummary = computed(() =>
    [
      options.quickFilters.onlyMapped && t('master.contentView.filters.onlyMapped'),
      options.quickFilters.onlyAbnormalQuality && t('master.contentView.filters.abnormalQuality'),
      options.quickFilters.onlyRecentChange &&
        t('master.contentView.filters.recentChange', {
          window:
            options.recentWindowOptions.find(
              (item) => item.value === options.quickFilters.recentWindowSec,
            )?.label ?? '10s',
        }),
    ]
      .filter(Boolean)
      .join('、'),
  )
  const recentChangeWindowMs = computed(() =>
    Math.trunc((Number(options.quickFilters.recentWindowSec) || 10) * 1000),
  )
  const currentControlTabUsesBuiltIn = computed(() => {
    const tab = currentTab.value
    if (!tab) return false
    if (!currentTabIsControl.value || currentProfileHasImportedPointDefs.value) return false
    if (options.profileHasBuiltInPointDefs(String(tab.profileId), tab.dataType)) return true
    return tab.data.some((row) => row.pointSource === 'built_in')
  })
  const currentTabFilteredData = computed(() => {
    const tab = currentTab.value
    if (!tab) return [] as MasterContentTabRow[]

    const search = tab.searchText.trim().toLowerCase()
    const needsControlSubtypeFilter =
      showControlSubtypeFilter.value && tab.controlSubtypeFilter !== 'all'
    if (!search && !hasActiveFilters.value && !needsControlSubtypeFilter) return tab.data

    const now = Date.now()
    return tab.data.filter((row) => {
      if (
        search &&
        ![
          String(row.address),
          options.formatIoaAddress(row.address),
          row.name,
          row.dataType,
          row.description,
        ].some((value) => String(value).toLowerCase().includes(search))
      ) {
        return false
      }
      if (options.quickFilters.onlyMapped && row.controlIoa == null) return false
      if (options.quickFilters.onlyAbnormalQuality && row.qualityLevel === 'good') return false
      if (needsControlSubtypeFilter) {
        if (resolveMasterControlCommandType(row.dataType) !== tab.controlSubtypeFilter) return false
      }
      if (options.quickFilters.onlyRecentChange) {
        const ts = Date.parse(
          String(row.controlStatusSnapshot?.updated_at || (row.isRuntime ? row.timestamp : '')),
        )
        if (!Number.isFinite(ts) || now - ts > recentChangeWindowMs.value) return false
      }
      return true
    })
  })
  const currentTabEmptyState = computed<MasterContentEmptyState | null>(() => {
    const tab = currentTab.value
    if (!tab) return null
    if (currentTabFilteredData.value.length > 0) return null
    if (tab.data.length > 0 && (tab.searchText.trim() || hasActiveFilters.value)) {
      return {
        title: t('master.contentView.empty.noMatchesTitle'),
        hint: t('master.contentView.empty.noMatchesHint'),
      }
    }
    if (
      !resolveMasterTabConnectionId(tab, options.selectedSlave.value) &&
      !currentTabIsControl.value
    ) {
      return {
        title: t('master.contentView.empty.linkNotStartedTitle'),
        hint: t('master.contentView.empty.linkNotStartedHint'),
      }
    }
    if (currentTabIsControl.value) {
      return currentProfileHasImportedPointDefs.value
        ? {
            title: t('master.contentView.empty.noControlTitle'),
            hint: t('master.contentView.empty.noImportedControlHint'),
          }
        : {
            title: t('master.contentView.empty.noControlTitle'),
            hint: t('master.contentView.empty.importControlHint'),
          }
    }
    return {
      title: t('master.contentView.empty.noRuntimeTitle'),
      hint: t('master.contentView.empty.noRuntimeHint'),
    }
  })

  return {
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
  }
}
