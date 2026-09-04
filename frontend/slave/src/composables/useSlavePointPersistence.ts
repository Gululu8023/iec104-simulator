import { ElMessage } from 'element-plus'
import type { Ref } from 'vue'

import { t } from '@shared/i18n'
import type { SlaveResponse, UpdateDataPointRequest } from '@shared/api/types'
import { encodeUiQualityToRaw, parseSemanticPointValue } from '@shared/api/iec104'
import {
  bulkUpdateSlaveDataPoints,
  getStationSlavePointDefs,
  replaceStationSlavePointDefs,
  updateSlaveDataPoint,
} from '@/api/slave'
import { buildEditedPointDefs, normalizeOptionalControlIoa } from '@/utils/pointDefinitions'
import type { DeviceNode } from '@/types/slave'

type UseSlavePointPersistenceOptions = {
  currentStationId: Ref<string>
  selectedSlaveId: Ref<number | null>
  selectedSlave: Ref<DeviceNode | null>
  slavesByStation: Ref<Record<string, SlaveResponse[]>>
  serverConfig: { commonAddress: number }
  syncFromBackend: () => Promise<void>
  rebuildDeviceTree: () => void
}

export function useSlavePointPersistence(options: UseSlavePointPersistenceOptions) {
  const normalizeOptionalCommonAddress = (value: unknown): number | undefined => {
    const numeric = Number(value)
    if (!Number.isFinite(numeric)) return undefined
    const normalized = Math.trunc(numeric)
    return normalized >= 1 && normalized <= 65535 ? normalized : undefined
  }

  const resolveWriteCommonAddress = (value: unknown): number | undefined =>
    normalizeOptionalCommonAddress(value) ??
    normalizeOptionalCommonAddress(options.selectedSlave.value?.commonAddress) ??
    normalizeOptionalCommonAddress(options.serverConfig.commonAddress)

  const resolveSimulationCommonAddress = (
    points: Array<{ commonAddress?: number }>,
  ): number | undefined => {
    for (const point of points) {
      const commonAddress = normalizeOptionalCommonAddress(point?.commonAddress)
      if (commonAddress != null) return commonAddress
    }
    return resolveWriteCommonAddress(undefined)
  }

  const resolveSlaveId = (dataPoint: any): number | null => {
    const stationId = options.currentStationId.value
    if (!stationId) return null
    const stationSlaves = options.slavesByStation.value[stationId] ?? []
    const commonAddress = normalizeOptionalCommonAddress(dataPoint?.commonAddress)
    if (commonAddress != null) {
      const matched = stationSlaves.find((slave) => Number(slave.common_address) === commonAddress)
      if (matched) return matched.id
    }
    return options.selectedSlaveId.value != null &&
      stationSlaves.some((slave) => slave.id === options.selectedSlaveId.value)
      ? options.selectedSlaveId.value
      : null
  }

  const persistControlIoaMappings = async (requests: UpdateDataPointRequest[]) => {
    const mappingRequests = requests.filter((request) =>
      Object.prototype.hasOwnProperty.call(request, 'control_ioa'),
    )
    const updatesBySlave = new Map<number, Map<number, number | null>>()
    for (const request of mappingRequests) {
      const slaveId = resolveSlaveId({
        commonAddress: resolveWriteCommonAddress(request.common_address),
      })
      if (slaveId == null) continue
      const updates = updatesBySlave.get(slaveId) ?? new Map<number, number | null>()
      updates.set(
        Math.max(0, Math.trunc(Number(request.address) || 0)),
        normalizeOptionalControlIoa(request.control_ioa),
      )
      updatesBySlave.set(slaveId, updates)
    }

    for (const [slaveId, controlByAddress] of updatesBySlave) {
      const defs = await getStationSlavePointDefs(slaveId)
      let changed = false
      const nextDefs = defs.map((def) => {
        const address = Math.max(0, Math.trunc(Number(def.address) || 0))
        if (!controlByAddress.has(address)) return def
        const controlIoa = controlByAddress.get(address) ?? null
        if (normalizeOptionalControlIoa(def.control_ioa) === controlIoa) return def
        changed = true
        return { ...def, control_ioa: controlIoa }
      })
      if (changed) await replaceStationSlavePointDefs(slaveId, nextDefs)
    }
  }

  const persistEditedDefinition = async (dataPoint: any, address: number) => {
    const slaveId = resolveSlaveId(dataPoint)
    if (slaveId == null) throw new Error(t('slave.app.messages.missingSlaveIdUpdate'))
    const defs = await getStationSlavePointDefs(slaveId)
    await replaceStationSlavePointDefs(slaveId, buildEditedPointDefs(defs, dataPoint, address))
  }

  const handleDataPointEdit = async (dataPoint: any) => {
    const stationId = options.currentStationId.value
    if (!stationId) return
    try {
      const address = Math.max(0, Math.trunc(Number(dataPoint.address) || 0))
      await updateSlaveDataPoint(stationId, {
        address,
        value: parseSemanticPointValue(dataPoint.dataType, dataPoint.value, 0),
        quality: encodeUiQualityToRaw(dataPoint.qualityRaw ?? dataPoint.quality),
        common_address: resolveWriteCommonAddress(dataPoint.commonAddress),
      })
      await persistEditedDefinition(dataPoint, address)
      await options.syncFromBackend()
    } catch (error) {
      ElMessage.error(t('slave.app.messages.updateDataPointFailed', { error: String(error) }))
    }
  }

  const handleImportDataPoints = async (
    requests: UpdateDataPointRequest[],
    onSaved?: () => void,
  ) => {
    const stationId = options.currentStationId.value
    if (!stationId) {
      ElMessage.warning(t('slave.app.messages.initStationFirst'))
      return
    }
    if (!requests?.length) {
      ElMessage.warning(t('slave.app.messages.emptyImportList'))
      return
    }
    try {
      await persistControlIoaMappings(requests)
      const fallbackCommonAddress = resolveWriteCommonAddress(undefined)
      const normalizedRequests = requests.map((request) => ({
        address: request.address,
        value: request.value,
        quality: request.quality,
        common_address: resolveWriteCommonAddress(request.common_address ?? fallbackCommonAddress),
      }))
      const updated = await bulkUpdateSlaveDataPoints(stationId, normalizedRequests)
      await options.syncFromBackend()
      options.rebuildDeviceTree()
      onSaved?.()
      ElMessage.success(t('slave.app.messages.importedPoints', { count: updated }))
    } catch (error) {
      ElMessage.error(t('slave.app.messages.importFailed', { error: String(error) }))
    }
  }

  return {
    handleDataPointEdit,
    handleImportDataPoints,
    normalizeOptionalCommonAddress,
    resolveSimulationCommonAddress,
    resolveWriteCommonAddress,
  }
}
