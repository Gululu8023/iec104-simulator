import type { Ref } from 'vue'

import { ElMessage } from 'element-plus'

import type {
  MessageDetailParseViewMode,
  SimulationProfile,
  SlaveCommandMismatchPolicy,
  SlaveIoaDisplayFormat,
} from '@shared/api/types'
import { t } from '@shared/i18n'
import {
  loadMessageDetailViewModePreference,
  saveMessageDetailViewModePreference,
} from '@shared/ui/messageDetailViewMode'

import {
  getSlaveCommandMismatchPolicy,
  getSlaveIoaDisplayFormat,
  getSlaveSimulationProfile,
  updateSlaveCommandMismatchPolicy,
  updateSlaveIoaDisplayFormat,
  updateSlaveSimulationProfile,
} from '../api/slave'

interface UseSlaveDialogsOptions {
  currentStationId: Ref<string>
  showGlobalSettingsDialog: Ref<boolean>
  commandMismatchPolicy: Ref<SlaveCommandMismatchPolicy>
  ioaDisplayFormat: Ref<SlaveIoaDisplayFormat>
  messageDetailViewMode: Ref<MessageDetailParseViewMode>
  commandMismatchPolicySaving: Ref<boolean>
  showAboutDialog: Ref<boolean>
  simulationProfile: SimulationProfile
}

interface SaveSlaveGlobalSettingsPayload {
  policy: SlaveCommandMismatchPolicy
  format: SlaveIoaDisplayFormat
  messageDetailViewMode: MessageDetailParseViewMode
  simulationIntervalMs: number
  simulationAutoBroadcast: boolean
}

const normalizeSimulationIntervalMs = (value: number): number =>
  Math.max(1000, Math.min(60000, Math.trunc(Number(value) || 1000)))

export function useSlaveDialogs(options: UseSlaveDialogsOptions) {
  const loadSlaveGlobalSettings = async (params: { silent?: boolean } = {}): Promise<boolean> => {
    const { silent = false } = params
    options.messageDetailViewMode.value = loadMessageDetailViewModePreference('slave')
    try {
      const requests: Array<Promise<unknown>> = [
        getSlaveCommandMismatchPolicy(),
        getSlaveIoaDisplayFormat(),
      ]
      if (options.currentStationId.value) {
        requests.push(getSlaveSimulationProfile(options.currentStationId.value))
      }

      const [policy, format, profile] = await Promise.all(requests)
      options.commandMismatchPolicy.value = policy as SlaveCommandMismatchPolicy
      options.ioaDisplayFormat.value = format as SlaveIoaDisplayFormat
      if (profile) {
        const simulation = profile as SimulationProfile
        options.simulationProfile.enabled = simulation.enabled
        options.simulationProfile.interval_ms = normalizeSimulationIntervalMs(
          simulation.interval_ms,
        )
        options.simulationProfile.delta_ratio = simulation.delta_ratio
        options.simulationProfile.min_value = simulation.min_value
        options.simulationProfile.max_value = simulation.max_value
        options.simulationProfile.seed = simulation.seed
        options.simulationProfile.auto_broadcast = simulation.auto_broadcast
      }
      return true
    } catch (error) {
      if (!silent) {
        ElMessage.error(t('settingsMessages.loadSlaveGlobalSettingsFailed', { error }))
      }
      return false
    }
  }

  const openGlobalSettingsDialog = async () => {
    const loaded = await loadSlaveGlobalSettings()
    if (!loaded) return
    options.showGlobalSettingsDialog.value = true
  }

  const handleSaveGlobalSettings = async (payload: SaveSlaveGlobalSettingsPayload) => {
    if (options.commandMismatchPolicySaving.value) return
    options.commandMismatchPolicySaving.value = true
    try {
      const normalizedIntervalMs = normalizeSimulationIntervalMs(payload.simulationIntervalMs)
      const requests: Array<Promise<unknown>> = [
        updateSlaveCommandMismatchPolicy(payload.policy),
        updateSlaveIoaDisplayFormat(payload.format),
      ]
      if (options.currentStationId.value) {
        requests.push(
          updateSlaveSimulationProfile(options.currentStationId.value, {
            ...options.simulationProfile,
            interval_ms: normalizedIntervalMs,
            auto_broadcast: payload.simulationAutoBroadcast,
          }),
        )
      }
      await Promise.all(requests)
      options.commandMismatchPolicy.value = payload.policy
      options.ioaDisplayFormat.value = payload.format
      options.messageDetailViewMode.value = payload.messageDetailViewMode
      saveMessageDetailViewModePreference('slave', payload.messageDetailViewMode)
      options.simulationProfile.interval_ms = normalizedIntervalMs
      options.simulationProfile.auto_broadcast = payload.simulationAutoBroadcast
      options.showGlobalSettingsDialog.value = false
      ElMessage.success(t('settingsMessages.globalSettingsSaved'))
    } catch (error) {
      ElMessage.error(t('settingsMessages.saveGlobalSettingsFailed', { error }))
    } finally {
      options.commandMismatchPolicySaving.value = false
    }
  }

  const showAboutDialog = async () => {
    options.showAboutDialog.value = true
  }

  return {
    loadSlaveGlobalSettings,
    openGlobalSettingsDialog,
    handleSaveGlobalSettings,
    showAboutDialog,
  }
}
