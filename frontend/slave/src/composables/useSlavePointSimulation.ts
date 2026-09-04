import { computed, shallowRef, type Ref } from 'vue'
import { ElMessage } from 'element-plus'

import { t } from '@shared/i18n'
import {
  simulateSlaveDataChanges,
  startSlaveSelectedPointSimulation,
  stopSlaveSelectedPointSimulation,
  updateSlaveDataPoint,
} from '@/api/slave'
import {
  normalizeWaveformSimulationConfig,
  type WaveformSimulationConfig,
} from '@/utils/waveformSimulation'

interface ActivePointSimulationSummary {
  stationId: string
  config: WaveformSimulationConfig
  points: Array<{ address: number; commonAddress?: number }>
}

type SimulationPoint = {
  address: number
  value?: number
  commonAddress?: number
  slaveId?: number | null
  qualityRaw?: number
  typeId?: number | null
  dataType?: string | null
}

type UseSlavePointSimulationOptions = {
  currentStationId: Ref<string>
  contentPanelRef: Ref<any>
  normalizeOptionalSlaveId: (value: unknown) => number | null
  resolveWriteCommonAddress: (value: unknown) => number | undefined
  resolveSimulationCommonAddress: (points: SimulationPoint[]) => number | undefined
  notifyLocalOnlySimulationIfNeeded: (actionLabel: string) => Promise<boolean>
  syncFromBackend: () => Promise<void>
}

export function useSlavePointSimulation(options: UseSlavePointSimulationOptions) {
  const sessions = shallowRef<Record<string, ActivePointSimulationSummary>>({})
  const activePointSimulation = computed(() => {
    const stationId = String(options.currentStationId.value || '').trim()
    return stationId ? (sessions.value[stationId] ?? null) : null
  })

  const normalizePoints = (points: SimulationPoint[]) =>
    points
      .map((point) => ({
        address: Math.max(0, Math.trunc(Number(point.address) || 0)),
        commonAddress: options.resolveWriteCommonAddress(point.commonAddress),
        slaveId: options.normalizeOptionalSlaveId(point.slaveId),
        qualityRaw: Math.max(0, Math.trunc(Number(point.qualityRaw ?? 0) || 0)),
      }))
      .filter((point) => Number.isFinite(point.address))

  const stopPointWaveformSimulation = async (
    runtimeOptions: { clearUi?: boolean; reason?: 'manual' | 'unmount' } = {},
  ) => {
    const { clearUi = true, reason = 'manual' } = runtimeOptions
    const stationId = String(options.currentStationId.value || '').trim()
    if (!stationId) return true
    try {
      await stopSlaveSelectedPointSimulation(stationId)
    } catch (error) {
      if (reason !== 'unmount') {
        ElMessage.error(t('slave.app.messages.stopDataSimulationFailed', { error: String(error) }))
      }
      return false
    }
    const nextSessions = { ...sessions.value }
    delete nextSessions[stationId]
    sessions.value = nextSessions
    if (clearUi) options.contentPanelRef.value?.clearSimulationState?.()
    if (reason === 'manual') ElMessage.success(t('slave.app.messages.dataSimulationStopped'))
    return true
  }

  const updatePoints = async (
    stationId: string,
    points: SimulationPoint[],
    resolveValue: (point: SimulationPoint) => number,
  ) => {
    for (const point of points) {
      await updateSlaveDataPoint(stationId, {
        address: Number(point.address) || 0,
        value: resolveValue(point),
        quality: Math.max(0, Math.trunc(Number(point.qualityRaw ?? 0) || 0)),
        common_address: options.resolveWriteCommonAddress(point.commonAddress),
      })
    }
  }

  const handleDataPointSimulate = async (payload: any) => {
    const stationId = options.currentStationId.value
    if (!stationId) return
    const mode = String(payload?.mode || 'single')
    if (mode === 'stop') {
      await stopPointWaveformSimulation()
      return
    }

    try {
      const points: SimulationPoint[] = Array.isArray(payload?.points) ? payload.points : []
      const config = payload?.config ?? {}
      const targetCommonAddress = options.resolveSimulationCommonAddress(points)
      await options.notifyLocalOnlySimulationIfNeeded(t('slave.app.actions.dataSimulation'))

      if (mode === 'waveform' || mode === 'fixed' || mode === 'linear') {
        if (points.length === 0) {
          ElMessage.warning(t('slave.app.messages.selectDataPointFirst'))
          return
        }
        const normalizedConfig = normalizeWaveformSimulationConfig({
          ...config,
          waveformType: mode === 'waveform' ? config.waveformType : mode,
        })
        const normalizedPoints = normalizePoints(points)
        await startSlaveSelectedPointSimulation(stationId, {
          points: normalizedPoints.map((point) => ({
            address: point.address,
            common_address: point.commonAddress,
          })),
          config: normalizedConfig,
        })
        sessions.value = {
          ...sessions.value,
          [stationId]: {
            stationId,
            config: normalizedConfig,
            points: normalizedPoints.map((point) => ({
              address: point.address,
              commonAddress: point.commonAddress,
            })),
          },
        }
        await options.syncFromBackend()
        ElMessage.success(t('slave.app.messages.dataSimulationStarted'))
        return
      }

      if (mode === 'single') {
        await simulateSlaveDataChanges(stationId, targetCommonAddress)
      } else if (mode === 'batch-random') {
        const minValue = Number(config.minValue ?? 0)
        const maxValue = Number(config.maxValue ?? 100)
        const low = Math.min(minValue, maxValue)
        const high = Math.max(minValue, maxValue)
        await updatePoints(stationId, points, () => low + Math.random() * (high - low))
      } else if (mode === 'batch-increment') {
        const step = Number(config.step ?? config.linearStep ?? 1)
        await updatePoints(stationId, points, (point) => Number(point.value ?? 0) + step)
      } else if (mode === 'batch-toggle') {
        await updatePoints(stationId, points, (point) => (Number(point.value ?? 0) > 0.5 ? 0 : 1))
      } else {
        await simulateSlaveDataChanges(stationId, targetCommonAddress)
      }
      await options.syncFromBackend()
    } catch (error) {
      ElMessage.error(t('slave.app.messages.triggerSimulationFailed', { error: String(error) }))
    }
  }

  return { activePointSimulation, handleDataPointSimulate, stopPointWaveformSimulation }
}
