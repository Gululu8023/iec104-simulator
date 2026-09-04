import { computed, ref, type Ref } from 'vue'

import { ElMessage } from 'element-plus'

import { t } from '@shared/i18n'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'
import {
  formatPointQualityLabelLocalized,
  getIec104Capability,
  parseSemanticPointValue,
  qualityLevelFromCommon,
  resolveIec104TypeId,
  type CommonQualityFlagKey,
} from '@shared/api/iec104'
import {
  buildEditableQualityRaw,
  buildLocalQualityCommon,
  buildLocalQualityDetailFromRaw,
  extractEditableCommonQualityFlags,
  extractEditableDescriptorQualityFlags,
  getProtocolQualityFieldsLocalized,
  getProtocolQualityGroupLabelLocalized,
  isBcrQualityType,
  normalizeBcrSequence,
  normalizeCommonQualityFlags,
  normalizeDescriptorQualityFlags,
  resolveProtocolQualityKind,
  type ProtocolFlagDef,
  type QualityDescriptorFlagKey,
} from '@shared/api/protocolQuality'

import type {
  SlaveContentDataPoint as DataPoint,
  SlaveContentTabData as TabData,
} from '@/types/slaveContentPanel'
import {
  formatWaveformSimulationSummary,
  getWaveformShortCode,
  type WaveformSimulationConfig,
} from '@/utils/waveformSimulation'

type EditableDataPoint = DataPoint & {
  commonQualityFlags: CommonQualityFlagKey[]
  descriptorQualityFlags: QualityDescriptorFlagKey[]
  bcrSequence: number
}

export interface DataPointSimulationPayload {
  mode:
    | 'single'
    | 'fixed'
    | 'linear'
    | 'waveform'
    | 'stop'
    | 'batch-random'
    | 'batch-increment'
    | 'batch-toggle'
  config: Record<string, unknown>
  points: DataPoint[]
}

export interface ActivePointSimulationSummary {
  stationId: string
  config: WaveformSimulationConfig
  points: Array<{
    address: number
    commonAddress?: number
  }>
}

interface UseSlavePointEditorOptions {
  currentTab: Ref<TabData | null | undefined>
  selectedRows: Ref<DataPoint[]>
  hasActiveSimulation: Ref<boolean>
  activeSimulationPointKeys: Ref<Set<string>>
  currentTabSimulation: Ref<ActivePointSimulationSummary | null>
  normalizeCommonAddress: (value: unknown) => number | null
  emitDataPointEdit: (dataPoint: DataPoint) => void
  emitDataPointSimulate: (payload: DataPointSimulationPayload) => void
}

export function useSlavePointEditor(options: UseSlavePointEditorOptions) {
  const showEditDialog = ref(false)
  const editingDataPoint = ref<EditableDataPoint | null>(null)
  const editDialogSnapshot = ref('')
  const showSimulationDialog = ref(false)
  const simulationDialogPoints = ref<DataPoint[]>([])

  const editingCapability = computed(() => {
    const point = editingDataPoint.value
    if (!point) return null
    const typeId = resolveIec104TypeId({ type_id: point.typeId, data_type: point.dataType })
    return getIec104Capability(typeId)
  })
  const editingValueProfile = computed(() => editingCapability.value?.simulation ?? null)
  const isEditingSinglePoint = computed(() => editingCapability.value?.value_model === 'boolean')
  const isEditingDoublePoint = computed(
    () => editingCapability.value?.value_model === 'double_point',
  )
  const isEditingControlPoint = computed(() => editingCapability.value?.point_role === 'control')
  const isEditingIntegratedTotal = computed(
    () => editingCapability.value?.family === 'integrated_total',
  )

  const buildEditDialogSnapshot = () => {
    if (!editingDataPoint.value) return ''
    const shouldTrackValue = editingCapability.value?.point_role === 'monitor'
    return JSON.stringify({
      address: Math.max(0, Math.trunc(Number(editingDataPoint.value.address) || 0)),
      name: String(editingDataPoint.value.name || '').trim(),
      description: String(editingDataPoint.value.description || '').trim(),
      giGroup: editingDataPoint.value.giGroup ?? null,
      counterGroup: editingDataPoint.value.counterGroup ?? null,
      qualityRaw: buildEditableQualityRaw(editingDataPoint.value),
      value: shouldTrackValue
        ? parseSemanticPointValue(
            editingDataPoint.value.dataType,
            editingDataPoint.value.value,
            Number(editingDataPoint.value.value) || 0,
          )
        : null,
    })
  }

  const hasEditDialogUnsavedChanges = computed(() => {
    if (!showEditDialog.value) return false
    if (!editingDataPoint.value || !editDialogSnapshot.value) return false
    return buildEditDialogSnapshot() !== editDialogSnapshot.value
  })

  const protocolQualityKind = computed(() => {
    const typeId = editingDataPoint.value?.typeId ?? 0
    return resolveProtocolQualityKind(typeId)
  })

  const protocolQualityGroupLabel = computed(() =>
    getProtocolQualityGroupLabelLocalized(protocolQualityKind.value),
  )

  const protocolQualityFieldDefs = computed(() =>
    getProtocolQualityFieldsLocalized(protocolQualityKind.value),
  )

  const editingQualitySummaryText = computed(() => {
    if (!editingDataPoint.value) return 'Good'
    const raw = buildEditableQualityRaw(editingDataPoint.value)
    const common = buildLocalQualityCommon(editingDataPoint.value.typeId, raw)
    const detail = buildLocalQualityDetailFromRaw(
      editingDataPoint.value.typeId,
      raw,
      editingDataPoint.value.value,
    )
    return formatPointQualityLabelLocalized(editingDataPoint.value.typeId, common, raw, detail)
  })

  const editingQualitySummaryTagType = computed(() => {
    if (!editingDataPoint.value) return 'success'
    const raw = buildEditableQualityRaw(editingDataPoint.value)
    const common = buildLocalQualityCommon(editingDataPoint.value.typeId, raw)
    const detail = buildLocalQualityDetailFromRaw(
      editingDataPoint.value.typeId,
      raw,
      editingDataPoint.value.value,
    )
    const level = qualityLevelFromCommon(common, raw, detail)
    if (level === 'good') return 'success'
    if (level === 'invalid') return 'danger'
    return 'warning'
  })

  const isProtocolFlagActive = (flag: ProtocolFlagDef): boolean => {
    if (!editingDataPoint.value) return false
    if (flag.group === 'common') {
      return editingDataPoint.value.commonQualityFlags.includes(flag.key as CommonQualityFlagKey)
    }
    return editingDataPoint.value.descriptorQualityFlags.includes(
      flag.key as QualityDescriptorFlagKey,
    )
  }

  const toggleProtocolFlag = (flag: ProtocolFlagDef, active: string | number | boolean) => {
    if (!editingDataPoint.value) return
    const nextActive = Boolean(active)
    if (flag.group === 'common') {
      const key = flag.key as CommonQualityFlagKey
      const current = new Set(editingDataPoint.value.commonQualityFlags)
      nextActive ? current.add(key) : current.delete(key)
      editingDataPoint.value.commonQualityFlags = normalizeCommonQualityFlags(
        editingDataPoint.value.typeId,
        [...current],
      )
      return
    }

    const key = flag.key as QualityDescriptorFlagKey
    const current = new Set(editingDataPoint.value.descriptorQualityFlags)
    nextActive ? current.add(key) : current.delete(key)
    editingDataPoint.value.descriptorQualityFlags = normalizeDescriptorQualityFlags(
      editingDataPoint.value.typeId,
      [...current],
    )
  }

  const performCloseEditDialog = () => {
    showEditDialog.value = false
    editingDataPoint.value = null
    editDialogSnapshot.value = ''
  }

  const { handleBeforeClose: handleEditDialogBeforeClose, requestClose: closeEditDialog } =
    useDialogCloseGuard({
      isDirty: () => hasEditDialogUnsavedChanges.value,
      onClose: performCloseEditDialog,
    })

  const handleEditDialogEnter = (event: KeyboardEvent) => {
    const target = event.target as HTMLElement | null
    if (!target || target.tagName === 'TEXTAREA') return
    void saveDataPoint()
  }

  const editDataPoint = (dataPoint: DataPoint) => {
    editingDataPoint.value = {
      ...dataPoint,
      commonQualityFlags: extractEditableCommonQualityFlags(
        dataPoint.typeId,
        dataPoint.qualityRaw ?? 0,
      ),
      descriptorQualityFlags: extractEditableDescriptorQualityFlags(
        dataPoint.typeId,
        dataPoint.qualityRaw ?? 0,
      ),
      bcrSequence: isBcrQualityType(dataPoint.typeId)
        ? normalizeBcrSequence((dataPoint.qualityRaw ?? 0) & 0x1f)
        : 0,
      value: parseSemanticPointValue(dataPoint.dataType, dataPoint.value, 0),
    }
    editDialogSnapshot.value = buildEditDialogSnapshot()
    showEditDialog.value = true
  }

  const saveDataPoint = () => {
    if (editingDataPoint.value && options.currentTab.value) {
      const capability = editingCapability.value
      if (!capability || !capability.point_configurable) {
        ElMessage.error(t('slave.pointEditor.messages.unsupportedCapability'))
        return
      }
      if (capability.point_role === 'monitor') {
        const profile = capability.simulation
        const value = Number(editingDataPoint.value.value)
        const integerRequired =
          profile?.input_kind === 'integer' || profile?.input_kind === 'select'
        if (
          !profile ||
          !Number.isFinite(value) ||
          value < profile.min ||
          value > profile.max ||
          (integerRequired && !Number.isInteger(value))
        ) {
          ElMessage.error(t('slave.pointEditor.messages.invalidProtocolValue'))
          return
        }
      }
      const nextQualityRaw = buildEditableQualityRaw(editingDataPoint.value)
      const nextQualityCommon = buildLocalQualityCommon(
        editingDataPoint.value.typeId,
        nextQualityRaw,
      )
      const nextQualityDetail = buildLocalQualityDetailFromRaw(
        editingDataPoint.value.typeId,
        nextQualityRaw,
        editingDataPoint.value.value,
      )
      const {
        commonQualityFlags: _commonQualityFlags,
        descriptorQualityFlags: _descriptorQualityFlags,
        bcrSequence: _bcrSequence,
        ...basePoint
      } = editingDataPoint.value
      const normalized: DataPoint = {
        ...basePoint,
        quality: qualityLevelFromCommon(nextQualityCommon, nextQualityRaw, nextQualityDetail),
        qualityRaw: nextQualityRaw,
        qualityCommon: nextQualityCommon,
        qualityDetail: nextQualityDetail,
        value: parseSemanticPointValue(
          editingDataPoint.value.dataType,
          editingDataPoint.value.value,
          Number(editingDataPoint.value.value) || 0,
        ),
      }
      const index = options.currentTab.value.data.findIndex(
        (point) => point.address === normalized.address,
      )
      if (index !== -1) {
        options.currentTab.value.data[index] = { ...normalized }
        options.emitDataPointEdit(normalized)
        ElMessage.success(t('slave.pointEditor.messages.pointUpdated'))
      }
    }
    performCloseEditDialog()
  }

  const simulateData = () => {
    const tab = options.currentTab.value
    if (!tab) return
    if (options.selectedRows.value.length === 0) {
      if (tab.data.length === 0) {
        ElMessage.warning(t('slave.pointEditor.messages.noDataPointsToSimulate'))
        return
      }
      simulationDialogPoints.value = tab.data.map((item) => ({ ...item }))
    } else {
      simulationDialogPoints.value = options.selectedRows.value.map((item) => ({ ...item }))
    }
    showSimulationDialog.value = true
  }

  const toggleSimulationFromToolbar = () => {
    if (options.hasActiveSimulation.value) {
      handleSimulationStop()
      return
    }
    simulateData()
  }

  const clearSimulationState = () => {
    showSimulationDialog.value = false
  }

  const handleSimulationStart = (mode: string, config: WaveformSimulationConfig, points: any[]) => {
    options.emitDataPointSimulate({
      mode: mode as DataPointSimulationPayload['mode'],
      config: { ...(config as unknown as Record<string, unknown>) },
      points: points.map((point) => ({ ...(point as DataPoint) })),
    })
    showSimulationDialog.value = false
  }

  const handleSimulationStop = () => {
    showSimulationDialog.value = false
    options.emitDataPointSimulate({
      mode: 'stop',
      config: {},
      points: [],
    })
  }

  const buildSimulationPointKey = (address: number, commonAddress?: number | null): string =>
    `${options.normalizeCommonAddress(commonAddress) ?? -1}:${Math.max(0, Math.trunc(Number(address) || 0))}`

  const isPointSimulating = (row: DataPoint) =>
    options.activeSimulationPointKeys.value.has(
      buildSimulationPointKey(
        row.address,
        row.commonAddress ?? options.currentTab.value?.slaveCommonAddress,
      ),
    )

  const getSimulationIcon = (row: DataPoint) => {
    if (!isPointSimulating(row)) return ''
    return getWaveformShortCode(options.currentTabSimulation.value?.config.waveformType)
  }

  const getSimulationTooltip = (row: DataPoint) => {
    if (!isPointSimulating(row)) return ''
    if (!options.currentTabSimulation.value) return t('slave.waveform.status.simulating')
    return formatWaveformSimulationSummary(options.currentTabSimulation.value.config)
  }

  const openSimulation = (row: DataPoint) => {
    simulationDialogPoints.value = [{ ...row }]
    showSimulationDialog.value = true
  }

  const handleEdit = (row: DataPoint) => {
    editDataPoint(row)
  }

  return {
    clearSimulationState,
    closeEditDialog,
    editDataPoint,
    editingDataPoint,
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
    editingValueProfile,
    isPointSimulating,
    isProtocolFlagActive,
    openSimulation,
    protocolQualityFieldDefs,
    protocolQualityGroupLabel,
    protocolQualityKind,
    saveDataPoint,
    showEditDialog,
    showSimulationDialog,
    simulateData,
    simulationDialogPoints,
    toggleProtocolFlag,
    toggleSimulationFromToolbar,
  }
}
