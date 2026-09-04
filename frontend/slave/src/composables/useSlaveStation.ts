import { reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { deleteStation, listStations } from '@shared/api/common'
import { t } from '@shared/i18n'
import {
  createSlaveStation,
  getSlaveLinkParams,
  getSlaveRedundancyMode,
  getSlaveSimulationProfile,
  getSlaveStationConfig,
  startSlaveStation,
  stopSlaveStation,
} from '../api/slave'
import type {
  CreateStationRequest,
  LinkParams,
  RedundancyGroupMode,
  SimulationProfile,
  StationSummary,
} from '@shared/api/types'

type ServerConfig = {
  host: string
  port: number
  commonAddress: number
  protocol: 'iec104'
}

export function useSlaveStation() {
  const isRunning = ref(false)
  const currentStationId = ref('')
  const stationName = ref(t('slave.station.defaultName'))
  const stations = ref<StationSummary[]>([])

  const serverConfig = reactive<ServerConfig>({
    host: '0.0.0.0',
    port: 2404,
    commonAddress: 1,
    protocol: 'iec104',
  })

  const linkParams = reactive<LinkParams>({
    k_value: 12,
    w_value: 8,
    t0_seconds: 30,
    t1_seconds: 15,
    t2_seconds: 10,
    t3_seconds: 20,
    max_asdu_bytes: 249,
    select_timeout_seconds: 15,
    enable_sq1_upload: false,
    enable_soe: false,
    unknown_typeid_negative_ack: true,
  })

  const simulationProfile = reactive<SimulationProfile>({
    enabled: false,
    interval_ms: 1000,
    delta_ratio: 0.05,
    min_value: -1000,
    max_value: 1000,
    seed: 1,
    auto_broadcast: true,
  })

  const redundancyMode = ref<RedundancyGroupMode>('single')

  const buildCreateSlaveStationRequest = (
    overrides: Partial<CreateStationRequest> = {},
  ): CreateStationRequest => ({
    name: stationName.value || t('slave.station.defaultName'),
    protocol: serverConfig.protocol,
    station_type: 'Slave',
    host: serverConfig.host,
    port: serverConfig.port,
    common_address: serverConfig.commonAddress,
    enabled: true,
    ...overrides,
  })

  const syncStationConfigFromBackend = async (stationId: string) => {
    try {
      const config = await getSlaveStationConfig(stationId)
      stationName.value = config.name
      serverConfig.host = config.host
      serverConfig.port = config.port
      serverConfig.commonAddress = config.common_address
      serverConfig.protocol = config.protocol
    } catch (error) {
      ElMessage.warning(t('slave.station.messages.loadConfigFailed', { error: String(error) }))
    }
  }

  const syncLinkParamsFromBackend = async (stationId: string) => {
    try {
      const params = await getSlaveLinkParams(stationId)
      linkParams.k_value = params.k_value
      linkParams.w_value = params.w_value
      linkParams.t0_seconds = params.t0_seconds
      linkParams.t1_seconds = params.t1_seconds
      linkParams.t2_seconds = params.t2_seconds
      linkParams.t3_seconds = params.t3_seconds
      linkParams.max_asdu_bytes = params.max_asdu_bytes
      linkParams.select_timeout_seconds = params.select_timeout_seconds
      linkParams.enable_sq1_upload = params.enable_sq1_upload
      linkParams.enable_soe = params.enable_soe
      linkParams.unknown_typeid_negative_ack = params.unknown_typeid_negative_ack
    } catch (error) {
      ElMessage.warning(t('slave.station.messages.loadLinkParamsFailed', { error: String(error) }))
    }
  }

  const syncSimulationProfileFromBackend = async (stationId: string) => {
    try {
      const profile = await getSlaveSimulationProfile(stationId)
      simulationProfile.enabled = profile.enabled
      simulationProfile.interval_ms = profile.interval_ms
      simulationProfile.delta_ratio = profile.delta_ratio
      simulationProfile.min_value = profile.min_value
      simulationProfile.max_value = profile.max_value
      simulationProfile.seed = profile.seed
      simulationProfile.auto_broadcast = profile.auto_broadcast
    } catch (error) {
      ElMessage.warning(
        t('slave.station.messages.loadSimulationProfileFailed', { error: String(error) }),
      )
    }
  }

  const syncRedundancyModeFromBackend = async (stationId: string) => {
    try {
      redundancyMode.value = await getSlaveRedundancyMode(stationId)
    } catch (error) {
      ElMessage.warning(
        t('slave.station.messages.loadRedundancyModeFailed', { error: String(error) }),
      )
    }
  }

  const sortStationsByDisplayOrder = (rows: StationSummary[]): StationSummary[] => {
    return [...rows].sort((left, right) => {
      const leftId = Number.parseInt(left.station_id, 10)
      const rightId = Number.parseInt(right.station_id, 10)

      if (Number.isFinite(leftId) && Number.isFinite(rightId)) {
        return leftId - rightId
      }

      return left.station_id.localeCompare(right.station_id, 'zh-CN', { numeric: true })
    })
  }

  const refreshStations = async () => {
    const previousRows = stations.value
    try {
      let rows = await listStations('slave')

      // 保护性兜底：当前已选从站存在时，后端异常返回空列表不应立刻把 UI 清空。
      if (rows.length === 0 && currentStationId.value) {
        try {
          const config = await getSlaveStationConfig(currentStationId.value)
          const previous = previousRows.find((item) => item.station_id === currentStationId.value)
          rows = [
            {
              station_id: currentStationId.value,
              name: config.name,
              station_type: config.station_type,
              host: config.host,
              port: config.port,
              common_address: config.common_address,
              enabled: config.enabled,
              status: previous?.status ?? (isRunning.value ? 'Running' : 'Stopped'),
            },
          ]
        } catch {
          if (previousRows.length > 0) {
            rows = [...previousRows]
          }
        }
      }

      rows = sortStationsByDisplayOrder(rows)
      stations.value = rows

      const selected = rows.find((s) => s.station_id === currentStationId.value)
      if (selected) {
        stationName.value = selected.name
        serverConfig.host = selected.host
        serverConfig.port = selected.port
        serverConfig.commonAddress = selected.common_address
        isRunning.value = selected.status === 'Running'
      } else if (rows.length > 0 && currentStationId.value) {
        const fallback = rows[0]
        currentStationId.value = fallback.station_id
        stationName.value = fallback.name
        serverConfig.host = fallback.host
        serverConfig.port = fallback.port
        serverConfig.commonAddress = fallback.common_address
        isRunning.value = fallback.status === 'Running'
      } else if (rows.length === 0) {
        isRunning.value = false
      }

      return rows
    } catch (error) {
      ElMessage.error(t('slave.station.messages.loadStationsFailed', { error: String(error) }))
      return stations.value
    }
  }

  const selectStation = async (stationId: string) => {
    if (!stationId) return
    currentStationId.value = stationId

    const row = stations.value.find((s) => s.station_id === stationId)
    if (row) {
      stationName.value = row.name
      serverConfig.host = row.host
      serverConfig.port = row.port
      serverConfig.commonAddress = row.common_address
      isRunning.value = row.status === 'Running'
    }

    await syncStationConfigFromBackend(stationId)
    await syncLinkParamsFromBackend(stationId)
    await syncSimulationProfileFromBackend(stationId)
    await syncRedundancyModeFromBackend(stationId)
  }

  const initializeSlaveStation = async () => {
    await refreshStations()
    if (!currentStationId.value && stations.value.length === 0) {
      await refreshStations()
    }
    if (currentStationId.value) {
      await selectStation(currentStationId.value)
      return
    }

    const first = stations.value[0]
    if (first) {
      await selectStation(first.station_id)
      return
    }

    currentStationId.value = ''
    isRunning.value = false
  }

  const createStation = async (
    overrides: Partial<CreateStationRequest> = {},
    options: { successMessage?: string | null } = {},
  ) => {
    try {
      const stationId = await createSlaveStation(buildCreateSlaveStationRequest(overrides))
      currentStationId.value = stationId
      await refreshStations()
      await selectStation(stationId)
      if (options.successMessage !== null) {
        ElMessage.success(options.successMessage ?? t('slave.station.messages.stationCreated'))
      }
      return stationId
    } catch (error) {
      ElMessage.error(t('slave.station.messages.createStationFailed', { error: String(error) }))
      return null
    }
  }

  const toggleServer = async () => {
    if (!currentStationId.value) {
      ElMessage.warning(t('slave.station.messages.selectStationFirst'))
      return
    }

    try {
      if (isRunning.value) {
        await stopSlaveStation(currentStationId.value)
        isRunning.value = false
        await refreshStations()
        ElMessage.success(t('slave.station.messages.serviceStopped'))
      } else {
        await startSlaveStation(currentStationId.value)
        isRunning.value = true
        await refreshStations()
        ElMessage.success(t('slave.station.messages.serviceStarted'))
      }
    } catch (error) {
      ElMessage.error(t('slave.station.messages.operationFailed', { error: String(error) }))
    }
  }

  const startServer = async () => {
    if (!currentStationId.value) {
      ElMessage.warning(t('slave.station.messages.selectStationFirst'))
      return false
    }
    if (isRunning.value) {
      return true
    }

    try {
      await startSlaveStation(currentStationId.value)
      isRunning.value = true
      await refreshStations()
      ElMessage.success(t('slave.station.messages.serviceStarted'))
      return true
    } catch (error) {
      ElMessage.error(t('slave.station.messages.startFailed', { error: String(error) }))
      return false
    }
  }

  const stopServer = async () => {
    if (!currentStationId.value) {
      ElMessage.warning(t('slave.station.messages.selectStationFirst'))
      return false
    }
    if (!isRunning.value) {
      return true
    }

    try {
      await stopSlaveStation(currentStationId.value)
      isRunning.value = false
      await refreshStations()
      ElMessage.success(t('slave.station.messages.serviceStopped'))
      return true
    } catch (error) {
      ElMessage.error(t('slave.station.messages.stopFailed', { error: String(error) }))
      return false
    }
  }

  const deleteCurrentStation = async () => {
    if (!currentStationId.value) {
      ElMessage.warning(t('slave.station.messages.noStationToDelete'))
      return false
    }

    try {
      if (isRunning.value) {
        await stopServer()
      }
      await deleteStation(currentStationId.value)
      const deleted = currentStationId.value
      currentStationId.value = ''
      isRunning.value = false
      await refreshStations()
      const next = stations.value.find((s) => s.station_id !== deleted) ?? stations.value[0]
      if (next) {
        await selectStation(next.station_id)
      }
      ElMessage.success(t('slave.station.messages.stationDeleted'))
      return true
    } catch (error) {
      ElMessage.error(t('slave.station.messages.deleteStationFailed', { error: String(error) }))
      return false
    }
  }

  return {
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
  }
}
