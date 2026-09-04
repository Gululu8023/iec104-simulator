import { watch, type ComputedRef, type Ref } from 'vue'
import { ElMessage } from 'element-plus'

import type {
  SlaveContentDataPoint as DataPoint,
  SlaveContentMappingDialogContext as MappingDialogContext,
  SlaveContentTabData as TabData,
} from '@/types/slaveContentPanel'
import { t } from '@shared/i18n'
import type { BackendDataPoint, UpdateDataPointRequest } from '@shared/api/types'
import {
  getIec104Capability,
  iec104TypeNameToId,
  resolveIec104TypeId,
  resolveIec104TypeName,
} from '@shared/api/iec104'
import type { SlavePointData } from './useSlavePointData'

interface UseSlaveAddressMappingOptions {
  activeTabs: Ref<TabData[]>
  activeTabName: Ref<string>
  pointData: SlavePointData
  currentTab: ComputedRef<TabData | undefined>
  selectedRows: Ref<DataPoint[]>
  showMappingDialog: Ref<boolean>
  mappingDialogMode: Ref<'single' | 'batch'>
  mappingGlobalMode: Ref<boolean>
  mappingSourceDataType: Ref<string>
  mappingSourceTypeOptions: Ref<string[]>
  mappingBatchPoints: Ref<DataPoint[]>
  existingTargetPoints: Ref<DataPoint[]>
  mappingDialogContext: Ref<MappingDialogContext | null>
  normalizeCommonAddress: (value: unknown) => number | null
  normalizeControlIoa: (value: unknown) => number | null
  mapBackendPointToUi: (point: BackendDataPoint, dataType: string) => DataPoint
  isControlMappingType: (dataType: string) => boolean
  clearSelectionForTab: (tabId: string) => void
  emitImportDataPoints: (updates: UpdateDataPointRequest[], onSaved?: () => void) => void
}

export function useSlaveAddressMapping(options: UseSlaveAddressMappingOptions) {
  const {
    activeTabs,
    activeTabName,
    pointData,
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
  } = options

  // 命令地址映射 - 提取已存在的对应目标类型测点
  const resolveMappingScopeCommonAddress = (): number | null => {
    const contextCommonAddress = normalizeCommonAddress(mappingDialogContext.value?.commonAddress)
    if (contextCommonAddress != null) return contextCommonAddress
    return normalizeCommonAddress(currentTab.value?.slaveCommonAddress)
  }

  const loadExistingTargetPoints = (sourceDataType: string) => {
    const scopeCommonAddress = resolveMappingScopeCommonAddress()
    const sourceCapability = getIec104Capability(iec104TypeNameToId(sourceDataType))
    if (sourceCapability?.point_role !== 'control') return []
    return pointData
      .filterPoints((point) => {
        const capability = getIec104Capability(resolveIec104TypeId(point))
        return (
          capability?.point_role === 'monitor' &&
          capability.family === sourceCapability.family &&
          (scopeCommonAddress == null ||
            normalizeCommonAddress(point.common_address) === scopeCommonAddress)
        )
      })
      .map((point) => mapBackendPointToUi(point, resolveIec104TypeName(point)))
      .sort((left, right) => {
        const commonAddressDifference =
          (normalizeCommonAddress(left.commonAddress) ?? 0) -
          (normalizeCommonAddress(right.commonAddress) ?? 0)
        return commonAddressDifference || left.address - right.address
      })
  }

  const withMappingPreviewMeta = (points: DataPoint[], targetPoints: DataPoint[]): DataPoint[] => {
    const targetByOwner = new Map<number, number>()
    const targetByScopedOwner = new Map<string, number>()

    for (const target of targetPoints) {
      const ownerIoa = normalizeControlIoa(target.controlIoa)
      if (ownerIoa == null) continue

      const currentTarget = targetByOwner.get(ownerIoa)
      if (currentTarget == null || target.address < currentTarget) {
        targetByOwner.set(ownerIoa, target.address)
      }

      const commonAddress = normalizeCommonAddress(target.commonAddress)
      if (commonAddress == null) continue
      const scopedKey = `${commonAddress}:${ownerIoa}`
      const currentScopedTarget = targetByScopedOwner.get(scopedKey)
      if (currentScopedTarget == null || target.address < currentScopedTarget) {
        targetByScopedOwner.set(scopedKey, target.address)
      }
    }

    return points.map((point) => {
      const sourceIoa = Math.max(1, Math.trunc(Number(point.address) || 0))
      const commonAddress = normalizeCommonAddress(point.commonAddress)
      const mappedTargetIoa =
        commonAddress == null
          ? targetByOwner.get(sourceIoa)
          : targetByScopedOwner.get(`${commonAddress}:${sourceIoa}`)

      return {
        ...point,
        mappedTargetIoa: mappedTargetIoa ?? normalizeControlIoa(point.mappedTargetIoa),
      }
    })
  }

  const prepareMappingPoints = (sourceType: string, points: DataPoint[]) => {
    const targetPoints = loadExistingTargetPoints(sourceType)
    existingTargetPoints.value = targetPoints
    mappingBatchPoints.value = withMappingPreviewMeta(points, targetPoints)
  }

  const refreshExistingTargetPointsBySourceType = (sourceType: string) => {
    existingTargetPoints.value = loadExistingTargetPoints(sourceType)
  }

  // 刷新目标类型点表，以便映射弹窗匹配判断
  watch(
    () => showMappingDialog.value,
    (val) => {
      if (val) return
      mappingDialogContext.value = null
      mappingSourceTypeOptions.value = []
    },
  )

  const openSingleMapping = (row: DataPoint) => {
    if (!currentTab.value) return
    if (!isControlMappingType(currentTab.value.dataType)) {
      ElMessage.warning(t('slave.contentPanel.messages.mappingControlOnly'))
      return
    }
    mappingDialogContext.value = null
    mappingSourceTypeOptions.value = []
    mappingDialogMode.value = 'single'
    mappingGlobalMode.value = false
    mappingSourceDataType.value = currentTab.value.dataType
    prepareMappingPoints(mappingSourceDataType.value, [{ ...row }])
    showMappingDialog.value = true
  }

  const openBatchMapping = () => {
    if (!currentTab.value) return
    if (!isControlMappingType(currentTab.value.dataType)) {
      ElMessage.warning(t('slave.contentPanel.messages.mappingControlOnly'))
      return
    }
    mappingDialogContext.value = null
    mappingSourceTypeOptions.value = []
    const pointsToMap = selectedRows.value.length > 0 ? selectedRows.value : currentTab.value.data

    if (pointsToMap.length === 0) {
      ElMessage.warning(t('slave.contentPanel.messages.noMappablePoints'))
      return
    }

    mappingDialogMode.value = 'batch'
    mappingGlobalMode.value = false
    mappingSourceDataType.value = currentTab.value.dataType
    prepareMappingPoints(
      mappingSourceDataType.value,
      pointsToMap.map((point) => ({ ...point })),
    )
    showMappingDialog.value = true
  }

  const openGlobalMapping = (context?: MappingDialogContext) => {
    const contextCommonAddress = normalizeCommonAddress(context?.commonAddress)
    mappingDialogContext.value = {
      connectionName: String(context?.connectionName || '').trim(),
      slaveName: String(context?.slaveName || '').trim(),
      commonAddress: contextCommonAddress,
    }
    mappingDialogMode.value = 'batch'
    mappingGlobalMode.value = true

    const scopedPoints = pointData.filterPoints((point) => {
      if (contextCommonAddress == null) return true
      return normalizeCommonAddress(point.common_address) === contextCommonAddress
    })
    const availableControlTypes = [
      ...new Set(
        scopedPoints
          .filter(
            (point) => getIec104Capability(resolveIec104TypeId(point))?.point_role === 'control',
          )
          .map((point) => resolveIec104TypeName(point)),
      ),
    ].sort(
      (left, right) =>
        (iec104TypeNameToId(left) ?? Number.MAX_SAFE_INTEGER) -
          (iec104TypeNameToId(right) ?? Number.MAX_SAFE_INTEGER) || left.localeCompare(right),
    )
    if (availableControlTypes.length === 0) {
      ElMessage.warning(t('slave.contentPanel.messages.noControlPointsForMapping'))
      mappingGlobalMode.value = false
      mappingDialogContext.value = null
      mappingSourceTypeOptions.value = []
      return
    }

    mappingSourceTypeOptions.value = availableControlTypes

    const preferredType = String(currentTab.value?.dataType || '').trim()
    if (preferredType && availableControlTypes.includes(preferredType)) {
      mappingSourceDataType.value = preferredType
    } else {
      mappingSourceDataType.value = availableControlTypes[0]
    }

    updateGlobalMappingPoints()
    showMappingDialog.value = true
  }

  const updateGlobalMappingSource = (newType: string) => {
    if (
      mappingGlobalMode.value &&
      mappingSourceTypeOptions.value.length > 0 &&
      !mappingSourceTypeOptions.value.includes(newType)
    ) {
      return
    }
    mappingSourceDataType.value = newType
    if (mappingGlobalMode.value) {
      updateGlobalMappingPoints()
      return
    }
    refreshExistingTargetPointsBySourceType(newType)
  }

  const updateGlobalMappingPoints = () => {
    if (!mappingGlobalMode.value) return
    if (!isControlMappingType(mappingSourceDataType.value)) {
      mappingBatchPoints.value = []
      existingTargetPoints.value = []
      return
    }
    const scopeCommonAddress = resolveMappingScopeCommonAddress()
    const points = pointData
      .filterPoints((point) => {
        return (
          resolveIec104TypeName(point) === mappingSourceDataType.value &&
          (scopeCommonAddress == null ||
            normalizeCommonAddress(point.common_address) === scopeCommonAddress)
        )
      })
      .map((p) => mapBackendPointToUi(p, mappingSourceDataType.value))

    prepareMappingPoints(mappingSourceDataType.value, points)
  }

  const handleApplyBatchMapping = async (data: {
    updates: Array<{ address: number; commonAddress?: number; controlIoa: number | null }>
  }) => {
    const updatesByPoint = new Map<string, UpdateDataPointRequest>()
    const sourceType = mappingSourceDataType.value
    const sourceCapability = getIec104Capability(iec104TypeNameToId(sourceType))
    if (sourceCapability?.point_role !== 'control') {
      ElMessage.warning(t('slave.contentPanel.messages.mappingUnsupported'))
      return
    }

    const setPointUpdate = (request: UpdateDataPointRequest) => {
      const commonAddress = normalizeCommonAddress(request.common_address) ?? -1
      updatesByPoint.set(`${commonAddress}:${request.address}`, request)
    }

    const applyOptimisticTargetMapping = (
      address: number,
      commonAddress: number | null,
      controlIoa: number | null,
      targetDataType: string,
    ) => {
      activeTabs.value.forEach((tab) => {
        if (tab.dataType !== targetDataType) return
        if (
          commonAddress != null &&
          normalizeCommonAddress(tab.slaveCommonAddress) !== commonAddress
        )
          return
        const uiPoint = tab.data.find((point) => point.address === address)
        if (uiPoint) uiPoint.controlIoa = controlIoa
      })
    }

    const applyOptimisticSourceMapping = (
      address: number,
      commonAddress: number | null,
      targetIoa: number | null,
    ) => {
      activeTabs.value.forEach((tab) => {
        if (tab.dataType !== sourceType) return
        if (
          commonAddress != null &&
          normalizeCommonAddress(tab.slaveCommonAddress) !== commonAddress
        )
          return
        const uiPoint = tab.data.find((point) => point.address === address)
        if (uiPoint) uiPoint.mappedTargetIoa = targetIoa
      })
    }

    const familyMonitorPoints = pointData.filterPoints((point) => {
      const capability = getIec104Capability(resolveIec104TypeId(point))
      return capability?.point_role === 'monitor' && capability.family === sourceCapability.family
    })

    data.updates.forEach((update) => {
      const sourceControlIoa = Math.max(1, Math.trunc(Number(update.address) || 0))
      const updateCommonAddress = normalizeCommonAddress(update.commonAddress)
      const candidateTargetPoints = familyMonitorPoints.filter((point) => {
        if (updateCommonAddress == null) return true
        return normalizeCommonAddress(point.common_address) === updateCommonAddress
      })

      // 一对一清理：同一控制地址的旧绑定先清空
      candidateTargetPoints
        .filter((point) => normalizeControlIoa(point.control_ioa) === sourceControlIoa)
        .forEach((point) => {
          if (update.controlIoa != null && point.address === update.controlIoa) return
          const targetCommonAddress = normalizeCommonAddress(point.common_address) ?? undefined
          setPointUpdate({
            address: point.address,
            value: point.value,
            quality: point.quality ?? 0,
            common_address: targetCommonAddress,
            control_ioa: null,
          })
          applyOptimisticTargetMapping(
            point.address,
            normalizeCommonAddress(point.common_address),
            null,
            resolveIec104TypeName(point),
          )
        })

      if (update.controlIoa != null) {
        const targetPoint = candidateTargetPoints.find(
          (point) => point.address === update.controlIoa,
        )
        if (targetPoint) {
          const targetCommonAddress =
            normalizeCommonAddress(targetPoint.common_address) ?? undefined
          setPointUpdate({
            address: targetPoint.address,
            value: targetPoint.value,
            quality: targetPoint.quality ?? 0,
            common_address: targetCommonAddress,
            control_ioa: sourceControlIoa,
          })
          applyOptimisticTargetMapping(
            targetPoint.address,
            normalizeCommonAddress(targetPoint.common_address),
            sourceControlIoa,
            resolveIec104TypeName(targetPoint),
          )
        }
      }

      applyOptimisticSourceMapping(sourceControlIoa, updateCommonAddress, update.controlIoa)
    })
    const pointUpdates = Array.from(updatesByPoint.values())
    const selectedTabId = activeTabName.value
    const clearSelectionAfterSave = !mappingGlobalMode.value && selectedRows.value.length > 0

    if (pointUpdates.length > 0) {
      options.emitImportDataPoints(
        pointUpdates,
        clearSelectionAfterSave ? () => clearSelectionForTab(selectedTabId) : undefined,
      )
    }

    showMappingDialog.value = false
  }

  return {
    handleApplyBatchMapping,
    openBatchMapping,
    openGlobalMapping,
    openSingleMapping,
    updateGlobalMappingSource,
  }
}
