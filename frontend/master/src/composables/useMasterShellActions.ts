import { ElMessage } from 'element-plus'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { Ref } from 'vue'
import { stopAllStations } from '@shared/api/common'
import { resolveHelpDocKeyFromAction, type HelpDocKey } from '@shared/content/helpDocs'
import { t } from '@shared/i18n'
import { confirmExitApp } from '@shared/ui/dialogConfirm'
import { downloadPointTableTemplate } from '@shared/ui/pointTableTemplate'

type UseMasterShellActionsOptions = {
  logs: Ref<unknown[]>
  leftPanelWidth: Ref<number>
  mainPanelsHeight: Ref<number>
  loadMasterGlobalSettings: () => void
  initializeMasterStation: () => Promise<void>
  hideMessagePanel: () => void
  refreshProfiles: () => Promise<void>
  syncFromBackend: () => Promise<void>
  startRuntimeSync: () => Promise<void>
  stopRuntimeSync: () => void
  openGlobalSettingsDialog: () => void
  openMessageParserDialog: () => void
  openHelpDocument: (key: HelpDocKey) => void
  showAboutDialog: () => Promise<void> | void
  addLog: (level: string, message: string) => void
}

export function useMasterShellActions(options: UseMasterShellActionsOptions) {
  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      options.addLog('info', t('shell.copied', { text }))
    } catch {
      ElMessage.warning(t('shell.copyFailed'))
    }
  }

  const handleProfileDialogBadgeClick = (index: number, badge: string) => {
    if (index > 1) return
    const normalized = String(badge || '').trim()
    if (!normalized) return
    void copyToClipboard(normalized)
  }

  const handleLinkDialogBadgeClick = (_index: number, badge: string) => {
    const normalized = String(badge || '').trim()
    if (!normalized) return
    void copyToClipboard(normalized)
  }

  const bootstrap = async () => {
    options.loadMasterGlobalSettings()
    await options.initializeMasterStation()
    options.hideMessagePanel()
    await options.refreshProfiles()
    await options.syncFromBackend()
    await options.startRuntimeSync()
  }

  const cleanup = () => {
    options.stopRuntimeSync()
  }

  const clearLogs = () => {
    options.logs.value = []
  }

  const handleMenuAction = (action: string) => {
    const helpDocKey = resolveHelpDocKeyFromAction(action)
    if (helpDocKey) {
      options.openHelpDocument(helpDocKey)
      return
    }

    switch (action) {
      case 'exit':
        void (async () => {
          const confirmed = await confirmExitApp()
          if (!confirmed) return
          try {
            await stopAllStations()
          } catch (error) {
            options.addLog('warning', t('shell.stopStationsBeforeExitFailed', { error }))
          }
          try {
            await getCurrentWindow().close()
          } catch (error) {
            ElMessage.error(t('shell.exitFailed', { error }))
          }
        })()
        return
      case 'clear-logs':
        clearLogs()
        options.addLog('info', t('shell.clearLogs'))
        return
      case 'reset-layout':
        options.leftPanelWidth.value = 280
        options.mainPanelsHeight.value = 550
        options.addLog('info', t('shell.resetLayout'))
        return
      case 'about':
        void options.showAboutDialog()
        return
      case 'global-settings':
        options.openGlobalSettingsDialog()
        return
      case 'message-parser':
        options.openMessageParserDialog()
        return
      case 'download-point-table-template':
        void downloadPointTableTemplate()
        return
      default:
        options.addLog('warning', t('shell.unknownMenuAction', { action }))
    }
  }

  return {
    bootstrap,
    cleanup,
    handleLinkDialogBadgeClick,
    handleMenuAction,
    handleProfileDialogBadgeClick,
  }
}
