import { computed, type Ref } from 'vue'
import {
  formatIec104TypeCompactLabel,
  formatIec104TypeLabel,
  getIec104Capability,
  iec104TypeNameToId,
} from '@shared/api/iec104'
import { currentLocale, t } from '@shared/i18n'
import type {
  SlaveContentDataPoint,
  SlaveContentEmptyState,
  SlaveContentMappingDialogContext,
  SlaveContentQuickFilters,
  SlaveContentRecentWindowOption,
  SlaveContentTabData,
} from '@/types/slaveContentPanel'
import type { SlavePointData } from './useSlavePointData'

type UseSlaveContentViewOptions = {
  activeTabs: Ref<SlaveContentTabData[]>
  activeTabName: Ref<string>
  mappingSourceDataType: Ref<string>
  mappingDialogContext: Ref<SlaveContentMappingDialogContext | null>
  pointData: SlavePointData
  quickFilters: SlaveContentQuickFilters
  recentWindowOptions: ReadonlyArray<SlaveContentRecentWindowOption>
  formatIoaAddress: (value: unknown) => string
  normalizeControlIoa: (value: unknown) => number | null
  normalizeCommonAddress: (value: unknown) => number | null
  getControlStatusText: (point: SlaveContentDataPoint) => string
}

export function useSlaveContentView(options: UseSlaveContentViewOptions) {
  const isControlMappingType = (dataType: string): boolean => {
    return (
      getIec104Capability(iec104TypeNameToId(String(dataType || '').trim()))?.point_role ===
      'control'
    )
  }

  const isControlPointType = (dataType: string): boolean => {
    return isControlMappingType(dataType)
  }

  const currentTab = computed(() =>
    options.activeTabs.value.find((tab) => tab.id === options.activeTabName.value),
  )
  const selectedDataTypeLabel = computed(() => {
    void currentLocale.value
    const tab = currentTab.value
    return tab ? formatIec104TypeLabel(iec104TypeNameToId(tab.dataType)) : ''
  })
  const mappingSourceTypeLabel = computed(() => {
    void currentLocale.value
    const sourceType = String(options.mappingSourceDataType.value || '').trim()
    if (!sourceType) return selectedDataTypeLabel.value
    return formatIec104TypeLabel(iec104TypeNameToId(sourceType))
  })
  const simulationDialogBadges = computed(() => {
    const tab = currentTab.value
    const badges: string[] = []
    if (tab?.connectionName) badges.push(tab.connectionName)
    if (tab?.slaveName) badges.push(tab.slaveName)
    if (selectedDataTypeLabel.value) badges.push(selectedDataTypeLabel.value)
    return badges
  })
  const mappingDialogBadges = computed(() => {
    const tab = currentTab.value
    const context = options.mappingDialogContext.value
    const badges: string[] = []
    const connectionName = String(tab?.connectionName || context?.connectionName || '').trim()
    const slaveName = String(tab?.slaveName || context?.slaveName || '').trim()
    if (connectionName) badges.push(connectionName)
    if (slaveName) badges.push(slaveName)
    if (mappingSourceTypeLabel.value) badges.push(mappingSourceTypeLabel.value)
    return badges
  })
  const hasActiveFilters = computed(
    () =>
      options.quickFilters.onlyMapped ||
      options.quickFilters.onlyAbnormalQuality ||
      options.quickFilters.onlyRecentChange,
  )
  const activeFilterSummary = computed(() => {
    const labels: string[] = []
    if (options.quickFilters.onlyMapped) labels.push(t('slave.contentView.filters.onlyMapped'))
    if (options.quickFilters.onlyAbnormalQuality) {
      labels.push(t('slave.contentView.filters.abnormalQuality'))
    }
    if (options.quickFilters.onlyRecentChange) {
      const recentLabel =
        options.recentWindowOptions.find(
          (item) => item.value === options.quickFilters.recentWindowSec,
        )?.label ?? '10s'
      labels.push(t('slave.contentView.filters.recentChange', { window: recentLabel }))
    }
    if (labels.length === 0) return ''
    return t('slave.contentView.filters.summary', {
      labels: labels.join(t('slave.contentView.filters.separator')),
    })
  })
  const clearQuickFilters = () => {
    options.quickFilters.onlyMapped = false
    options.quickFilters.onlyAbnormalQuality = false
    options.quickFilters.onlyRecentChange = false
    options.quickFilters.recentWindowSec = 10
  }
  const isCurrentTabControlMappingType = computed(() => {
    return isControlMappingType(String(currentTab.value?.dataType || '').trim())
  })
  const currentTabIsControl = computed(() =>
    isControlPointType(String(currentTab.value?.dataType || '').trim()),
  )
  const getReverseMappedSourceAddresses = (point: SlaveContentDataPoint): number[] => {
    void options.pointData.mappingRevision.value
    return options.pointData.getReverseSourceAddresses(
      options.normalizeCommonAddress(point.commonAddress),
      point.address,
    )
  }
  const getMappingTypeDisplayText = (point: SlaveContentDataPoint): string => {
    const formatTypes = (types: Iterable<string>) =>
      [...types].map((type) => formatIec104TypeCompactLabel(iec104TypeNameToId(type))).join(', ')
    if (isControlMappingType(point.dataType)) {
      void options.pointData.mappingRevision.value
      const commonAddress = options.normalizeCommonAddress(point.commonAddress)
      const mappedTypes = options.pointData.getMonitorTypes(commonAddress, point.address)
      if (mappedTypes?.length) return formatTypes(mappedTypes)
      const optimisticMapped = options.normalizeControlIoa(point.mappedTargetIoa)
      if (optimisticMapped == null) return ''
      const targetType = options.pointData.getPointType(commonAddress, optimisticMapped)
      return targetType ? formatTypes([targetType]) : ''
    }

    const forward = options.normalizeControlIoa(point.controlIoa)
    if (forward == null) return ''

    void options.pointData.mappingRevision.value
    const tags = options.pointData.getControlTypes(
      options.normalizeCommonAddress(point.commonAddress),
      forward,
    )
    return formatTypes(tags)
  }
  const getMappingDisplayText = (point: SlaveContentDataPoint): string => {
    if (isControlMappingType(point.dataType)) {
      const reverse = getReverseMappedSourceAddresses(point)
      if (reverse.length > 0) {
        return reverse.map((address) => options.formatIoaAddress(address)).join(',')
      }
      const optimisticMapped = options.normalizeControlIoa(point.mappedTargetIoa)
      if (optimisticMapped != null) {
        return options.formatIoaAddress(optimisticMapped)
      }
      return ''
    }

    const forward = options.normalizeControlIoa(point.controlIoa)
    if (forward != null) {
      return options.formatIoaAddress(forward)
    }
    return ''
  }
  const getMappingTargetTypeCellText = (point: SlaveContentDataPoint): string => {
    const targetType = getMappingTypeDisplayText(point)
    if (targetType) return targetType
    return getMappingDisplayText(point) ? `(${t('slave.contentView.unknown')})` : ''
  }
  const getMappingTargetAddressCellText = (point: SlaveContentDataPoint): string => {
    const targetAddress = getMappingDisplayText(point)
    if (targetAddress) return targetAddress
    return getMappingTypeDisplayText(point) ? '--' : ''
  }
  const getMappingTargetDisplayText = (point: SlaveContentDataPoint): string => {
    const targetType = getMappingTargetTypeCellText(point)
    const targetAddress = getMappingTargetAddressCellText(point)
    if (!targetType && !targetAddress) return ''
    return `${targetType}@${targetAddress}`
  }
  const recentChangeWindowMs = computed(() => {
    const seconds = Number(options.quickFilters.recentWindowSec)
    if (!Number.isFinite(seconds) || seconds <= 0) return 10_000
    return Math.trunc(seconds * 1000)
  })
  const currentTabFilteredData = computed(() => {
    const tab = currentTab.value
    if (!tab) return [] as SlaveContentDataPoint[]

    const search = tab.searchText.trim().toLowerCase()
    if (!search && !hasActiveFilters.value) return tab.data

    const now = Date.now()
    return tab.data.filter((point) => {
      if (options.quickFilters.onlyMapped && !getMappingTargetDisplayText(point)) {
        return false
      }
      if (options.quickFilters.onlyAbnormalQuality && point.quality === 'good') {
        return false
      }
      if (
        options.quickFilters.onlyRecentChange &&
        now - point.timestamp > recentChangeWindowMs.value
      ) {
        return false
      }
      if (!search) return true

      const forward = options.normalizeControlIoa(point.controlIoa)
      const reverse = isControlMappingType(point.dataType)
        ? getReverseMappedSourceAddresses(point)
        : []
      const formattedAddress = options.formatIoaAddress(point.address).toLowerCase()
      const formattedForward =
        forward != null ? options.formatIoaAddress(forward).toLowerCase() : ''
      const formattedReverse = reverse.map((address) =>
        options.formatIoaAddress(address).toLowerCase(),
      )
      const mappingTargetText = getMappingTargetDisplayText(point).toLowerCase()
      const statusText = options.getControlStatusText(point).toLowerCase()
      return (
        point.address.toString().includes(search) ||
        formattedAddress.includes(search) ||
        point.name.toLowerCase().includes(search) ||
        String(point.description || '')
          .toLowerCase()
          .includes(search) ||
        mappingTargetText.includes(search) ||
        statusText.includes(search) ||
        (forward != null &&
          (forward.toString().includes(search) || formattedForward.includes(search))) ||
        reverse.some(
          (address, idx) =>
            address.toString().includes(search) || formattedReverse[idx].includes(search),
        )
      )
    })
  })
  const currentTabEmptyState = computed<SlaveContentEmptyState | null>(() => {
    const tab = currentTab.value
    if (!tab) return null
    if (currentTabFilteredData.value.length > 0) return null
    if (tab.data.length > 0 && (tab.searchText.trim() || hasActiveFilters.value)) {
      return {
        title: t('slave.contentView.empty.noMatchesTitle'),
        hint: t('slave.contentView.empty.noMatchesHint'),
      }
    }
    if (isControlPointType(String(tab.dataType || '').trim())) {
      return {
        title: t('slave.contentView.empty.noControlTitle'),
        hint: t('slave.contentView.empty.noControlHint'),
      }
    }
    return {
      title: t('slave.contentView.empty.noDataTitle'),
      hint: t('slave.contentView.empty.noDataHint'),
    }
  })

  return {
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
    getReverseMappedSourceAddresses,
    getMappingTypeDisplayText,
    getMappingDisplayText,
    getMappingTargetTypeCellText,
    getMappingTargetAddressCellText,
    getMappingTargetDisplayText,
  }
}
