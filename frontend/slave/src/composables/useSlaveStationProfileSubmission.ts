import { ElMessage } from 'element-plus'
import type { Ref } from 'vue'

import { t } from '@shared/i18n'
import type {
  CreateStationRequest,
  LinkParams,
  RedundancyGroupMode,
  UpsertSlaveBundleRequest,
} from '@shared/api/types'
import type { DeviceNode } from '@/types/slave'
import type { StationProfileSubmitPayload } from './useSlaveStationProfileDialogs'
import {
  buildPointDefsFromConfigObjects,
  buildPointTemplatePayload,
} from '@/utils/pointDefinitions'
import {
  createSlave,
  updateSlave,
  updateSlaveLinkParams,
  updateSlaveRedundancyMode,
  updateSlaveStationConfig,
} from '@/api/slave'

type ServerConfig = { host: string; port: number; commonAddress: number }

type UseSlaveStationProfileSubmissionOptions = {
  selectedSlave: Ref<DeviceNode | null>
  selectedSlaveId: Ref<number | null>
  currentStationId: Ref<string>
  isRunning: Ref<boolean>
  stationName: Ref<string>
  serverConfig: ServerConfig
  linkParams: LinkParams
  redundancyMode: Ref<RedundancyGroupMode>
  resolveLatestSlaveNode: (node: DeviceNode | null) => DeviceNode | null
  resolveStationIdFromNode: (node: DeviceNode | null) => string
  findLatestListenerNode: (stationId: string) => DeviceNode | null
  handleStationChange: (stationId: string) => Promise<void>
  ensureStationDisconnectedForConfig: (
    station: DeviceNode | null,
    actionLabel: string,
  ) => Promise<boolean>
  createStation: (
    overrides?: Partial<CreateStationRequest>,
    options?: { successMessage?: string | null },
  ) => Promise<string | null>
  stopServer: () => Promise<boolean>
  refreshStations: () => Promise<unknown>
  refreshStationPointTypeSummary: () => Promise<void>
  syncFromBackend: () => Promise<void>
  syncUiForCurrentStation: () => void
  rebuildDeviceTree: () => void
}

export function useSlaveStationProfileSubmission(options: UseSlaveStationProfileSubmissionOptions) {
  let submitting = false

  const buildStationConfigRequest = (
    payload: StationProfileSubmitPayload,
  ): CreateStationRequest => ({
    name:
      String(payload.name ?? '').trim() ||
      options.stationName.value ||
      t('slave.station.defaultName'),
    protocol: 'iec104',
    station_type: 'Slave',
    host: String(payload.host ?? options.serverConfig.host ?? '0.0.0.0').trim() || '0.0.0.0',
    port: Math.max(
      1,
      Math.min(65535, Math.trunc(Number(payload.port ?? options.serverConfig.port ?? 2404))),
    ),
    common_address: Math.max(
      1,
      Math.min(
        65535,
        Math.trunc(Number(payload.commonAddress ?? options.serverConfig.commonAddress ?? 1)),
      ),
    ),
    enabled: true,
  })

  const refreshRuntimeViews = async () => {
    await options.refreshStationPointTypeSummary()
    await options.syncFromBackend()
    options.rebuildDeviceTree()
  }

  const assignLinkTransportParams = (source: LinkParams) => {
    options.linkParams.k_value = source.k_value
    options.linkParams.w_value = source.w_value
    options.linkParams.t0_seconds = source.t0_seconds
    options.linkParams.t1_seconds = source.t1_seconds
    options.linkParams.t2_seconds = source.t2_seconds
    options.linkParams.t3_seconds = source.t3_seconds
    options.linkParams.max_asdu_bytes = source.max_asdu_bytes
  }

  const submitLogicalSlave = async (payload: StationProfileSubmitPayload): Promise<void> => {
    const targetStationId = String(payload.stationId || options.currentStationId.value || '').trim()
    if (!targetStationId) {
      ElMessage.error(t('slave.app.messages.missingConnectionIdSaveSlave'))
      return
    }
    if (targetStationId !== options.currentStationId.value) {
      await options.handleStationChange(targetStationId)
    }

    if (
      payload.mode === 'edit' &&
      (await options.ensureStationDisconnectedForConfig(
        options.findLatestListenerNode(targetStationId),
        t('slave.app.actions.editSlaveConfig'),
      ))
    ) {
      return
    }

    const request: UpsertSlaveBundleRequest = {
      slave: {
        name: String(payload.name || '').trim() || t('slave.app.slaveFallback'),
        common_address: Math.max(
          1,
          Math.min(65535, Math.trunc(Number(payload.commonAddress) || 1)),
        ),
        enabled: true,
        station_policy_override: payload.stationPolicyOverride ?? {},
      },
      point_defs: payload.pointDefs ?? buildPointDefsFromConfigObjects(payload.configObjects ?? []),
    }

    if (payload.mode === 'create') {
      const response = await createSlave(targetStationId, request)
      options.selectedSlaveId.value = response.slave_id
      ElMessage.success(
        t('slave.app.messages.slaveCreated', { count: response.point_defs_updated }),
      )
    } else {
      const slaveId = Number(payload.slaveId ?? options.selectedSlaveId.value ?? 0)
      if (!Number.isFinite(slaveId) || slaveId <= 0) {
        ElMessage.error(t('slave.app.messages.missingSlaveIdUpdate'))
        return
      }
      const response = await updateSlave(slaveId, request)
      options.selectedSlaveId.value = response.slave_id
      ElMessage.success(t('slave.app.messages.slaveSaved', { count: response.point_defs_updated }))
    }
    await refreshRuntimeViews()
  }

  const submitListener = async (payload: StationProfileSubmitPayload): Promise<void> => {
    const targetSlave = options.resolveLatestSlaveNode(options.selectedSlave.value)
    const requestedStationId = String(payload.stationId || '').trim()
    const targetStationId =
      requestedStationId ||
      options.resolveStationIdFromNode(targetSlave) ||
      options.currentStationId.value

    if (
      payload.mode === 'edit' &&
      targetStationId &&
      targetStationId !== options.currentStationId.value
    ) {
      await options.handleStationChange(targetStationId)
    }
    const targetForConnectionCheck = targetStationId
      ? options.findLatestListenerNode(targetStationId)
      : targetSlave
    if (
      payload.mode === 'edit' &&
      (await options.ensureStationDisconnectedForConfig(
        targetForConnectionCheck,
        t('slave.app.actions.modifyStationConfig'),
      ))
    ) {
      return
    }
    if (options.isRunning.value && !(await options.stopServer())) return

    const previous = {
      stationName: options.stationName.value,
      serverConfig: { ...options.serverConfig },
      linkParams: { ...options.linkParams },
      redundancyMode: options.redundancyMode.value,
    }
    options.stationName.value = payload.name
    options.serverConfig.commonAddress = payload.commonAddress
    if (payload.host) options.serverConfig.host = payload.host
    if (typeof payload.port === 'number') options.serverConfig.port = payload.port
    if (payload.linkParams) assignLinkTransportParams(payload.linkParams)
    if (payload.redundancyMode) options.redundancyMode.value = payload.redundancyMode

    let saved = false
    let effectiveStationId = options.currentStationId.value
    if (payload.mode === 'create') {
      const createdStationId = await options.createStation(
        {
          point_template: buildPointTemplatePayload(
            payload.configObjects,
            payload.useDefaultTemplate,
          ),
        },
        { successMessage: null },
      )
      saved = Boolean(createdStationId)
      if (createdStationId) effectiveStationId = createdStationId
    } else {
      effectiveStationId = targetStationId || options.currentStationId.value
      if (!effectiveStationId) {
        ElMessage.error(t('slave.app.messages.missingSlaveIdSaveConfig'))
      } else {
        const response = await updateSlaveStationConfig(effectiveStationId, {
          config: buildStationConfigRequest(payload),
        })
        options.stationName.value = response.station.name
        options.serverConfig.host = response.station.host
        options.serverConfig.port = response.station.port
        options.serverConfig.commonAddress = response.station.common_address
        ElMessage.success(
          t('slave.app.messages.stationConfigSaved', { count: response.point_defs_updated }),
        )
        saved = true
      }
    }

    if (saved && effectiveStationId) {
      try {
        if (payload.linkParams)
          await updateSlaveLinkParams(effectiveStationId, { ...options.linkParams })
        if (payload.redundancyMode) {
          await updateSlaveRedundancyMode(effectiveStationId, payload.redundancyMode)
        }
      } catch (error) {
        ElMessage.error(t('slave.app.messages.saveLinkParamsFailed', { error: String(error) }))
        saved = false
      }
    }

    if (!saved) {
      options.stationName.value = previous.stationName
      Object.assign(options.serverConfig, previous.serverConfig)
      assignLinkTransportParams(previous.linkParams)
      options.redundancyMode.value = previous.redundancyMode
      await options.refreshStations()
      options.syncUiForCurrentStation()
      await options.syncFromBackend()
      options.rebuildDeviceTree()
      return
    }

    await options.refreshStations()
    options.syncUiForCurrentStation()
    await refreshRuntimeViews()
  }

  const handleStationProfileSubmit = (payload: StationProfileSubmitPayload) => {
    void (async () => {
      if (submitting) {
        ElMessage.warning(t('slave.app.messages.submitInProgress'))
        return
      }
      submitting = true
      try {
        if (payload.scope === 'slave') await submitLogicalSlave(payload)
        else await submitListener(payload)
      } catch (error) {
        ElMessage.error(t('slave.app.messages.submitStationConfigFailed', { error: String(error) }))
      } finally {
        submitting = false
      }
    })()
  }

  return { handleStationProfileSubmit }
}
