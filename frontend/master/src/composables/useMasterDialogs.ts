import { watch, type Ref } from 'vue'

import { ElMessage } from 'element-plus'

import type {
  MessageDetailParseViewMode,
  MasterFileTransferSession,
  ResetProcessMode,
  SlaveIoaDisplayFormat,
} from '@shared/api/types'
import { t } from '@shared/i18n'
import {
  loadMessageDetailViewModePreference,
  saveMessageDetailViewModePreference,
} from '@shared/ui/messageDetailViewMode'

interface UseMasterDialogsOptions {
  showFileTransferDialog: Ref<boolean>
  fileTransferSession: Ref<MasterFileTransferSession | null>
  showGlobalSettingsDialog: Ref<boolean>
  ioaDisplayFormat: Ref<SlaveIoaDisplayFormat>
  resetProcessMode: Ref<ResetProcessMode>
  messageDetailViewMode: Ref<MessageDetailParseViewMode>
  showAboutDialog: Ref<boolean>
  addLog: (level: string, message: string) => void
}

interface SaveMasterGlobalSettingsPayload {
  format: SlaveIoaDisplayFormat
  resetProcessMode: ResetProcessMode
  messageDetailViewMode: MessageDetailParseViewMode
}

const MASTER_GLOBAL_SETTINGS_STORAGE_KEY = 'iec104-simulator.master.global-settings'

export function useMasterDialogs(options: UseMasterDialogsOptions) {
  const loadMasterGlobalSettings = () => {
    try {
      options.messageDetailViewMode.value = loadMessageDetailViewModePreference('master')
      const raw = window.localStorage.getItem(MASTER_GLOBAL_SETTINGS_STORAGE_KEY)
      if (!raw) return
      const parsed = JSON.parse(raw) as {
        format?: SlaveIoaDisplayFormat
        resetProcessMode?: ResetProcessMode
      }
      if (parsed.format === 'dec' || parsed.format === 'hex') {
        options.ioaDisplayFormat.value = parsed.format
      }
      if (
        parsed.resetProcessMode === 'general-reset' ||
        parsed.resetProcessMode === 'clear-event-buffer'
      ) {
        options.resetProcessMode.value = parsed.resetProcessMode
      }
    } catch (error) {
      options.addLog('warning', t('settingsMessages.loadMasterGlobalSettingsFailed', { error }))
    }
  }

  const persistMasterGlobalSettings = () => {
    try {
      saveMessageDetailViewModePreference('master', options.messageDetailViewMode.value)
      window.localStorage.setItem(
        MASTER_GLOBAL_SETTINGS_STORAGE_KEY,
        JSON.stringify({
          format: options.ioaDisplayFormat.value,
          resetProcessMode: options.resetProcessMode.value,
        }),
      )
    } catch (error) {
      options.addLog('warning', t('settingsMessages.saveMasterGlobalSettingsFailed', { error }))
    }
  }

  const openGlobalSettingsDialog = () => {
    options.showGlobalSettingsDialog.value = true
  }

  const handleSaveGlobalSettings = (payload: SaveMasterGlobalSettingsPayload) => {
    options.ioaDisplayFormat.value = payload.format
    options.resetProcessMode.value = payload.resetProcessMode
    options.messageDetailViewMode.value = payload.messageDetailViewMode
    persistMasterGlobalSettings()
    options.showGlobalSettingsDialog.value = false
    ElMessage.success(t('settingsMessages.globalSettingsSaved'))
  }

  const showAboutDialog = async () => {
    options.showAboutDialog.value = true
  }

  watch(options.showFileTransferDialog, (visible) => {
    if (!visible) {
      options.fileTransferSession.value = null
    }
  })

  return {
    loadMasterGlobalSettings,
    openGlobalSettingsDialog,
    handleSaveGlobalSettings,
    showAboutDialog,
  }
}
