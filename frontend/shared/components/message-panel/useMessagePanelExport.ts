import type { Ref } from 'vue'

import { ElMessage } from 'element-plus'

import { invokeCommand } from '@shared/api/tauri'
import type { ExportCaptureResponse } from '@shared/api/types'
import { t } from '@shared/i18n'
import { translateApiError } from '@shared/i18n/errors'
import { encodeTextToBase64, saveExportFile } from '@shared/ui/saveExportFile'

import type { Message } from './types'

interface UseMessagePanelExportOptions {
  stationId: Ref<string>
  stationKind: Ref<'master' | 'slave'>
  messages: Ref<Message[]>
  filteredMessages: Ref<Message[]>
  connectionFilter: Ref<string>
  resolveConnectionIdFromUiKey: (uiConnectionKey: string) => string
  formatDetailTime: (timestamp: number) => string
  getDirectionLabel: (type: string) => string
  getStationName: (msg?: Message) => string
  formatHexData: (hexData: string) => string
}

export function useMessagePanelExport(options: UseMessagePanelExportOptions) {
  const localizeCaptureFileName = (fileName: string) => {
    const prefixKey =
      options.stationKind.value === 'master'
        ? 'messagePanel.export.captureMasterPrefix'
        : 'messagePanel.export.captureSlavePrefix'
    return String(fileName || '').replace(/^iec104-(master|slave)/, t(prefixKey))
  }

  const persistExportFile = async (
    dataBase64: string,
    fileName: string,
    successMessage: string,
  ) => {
    return saveExportFile({
      dataBase64,
      fileName,
      dialogTitle: t('messagePanel.export.saveTitle'),
      successMessage,
    })
  }

  const escapeCsvCell = (value: string) => `"${String(value ?? '').replace(/"/g, '""')}"`

  const exportCSV = async () => {
    const headers = ['时间', '方向', '从站', '描述', '原始内容']
    const rows = options.filteredMessages.value.map((msg) => [
      options.formatDetailTime(msg.timestamp),
      options.getDirectionLabel(msg.type),
      options.getStationName(msg),
      msg.content,
      msg.hexData || '',
    ])

    const csvContent = [
      headers.map(escapeCsvCell).join(','),
      ...rows.map((row) => row.map((cell) => escapeCsvCell(cell)).join(',')),
    ].join('\n')

    await persistExportFile(
      encodeTextToBase64(csvContent),
      t('messagePanel.export.csvFileName'),
      t('messagePanel.export.csvSuccess'),
    )
  }

  const exportTXT = async () => {
    const content = options.filteredMessages.value
      .map(
        (msg) =>
          `[${options.formatDetailTime(msg.timestamp)}] [${options.getDirectionLabel(msg.type)}] ${options.getStationName(msg)}: ${msg.content}\nHEX: ${options.formatHexData(msg.hexData || '')}`,
      )
      .join('\n\n')

    await persistExportFile(
      encodeTextToBase64(content),
      t('messagePanel.export.txtFileName'),
      t('messagePanel.export.txtSuccess'),
    )
  }

  const exportCapture = async (format: 'pcap' | 'pcapng') => {
    if (!options.stationId.value) {
      ElMessage.error(t('messagePanel.export.missingStationId'))
      return
    }

    const command =
      options.stationKind.value === 'master' ? 'export_master_capture' : 'export_slave_capture'
    const connectionId = options.connectionFilter.value
      ? options.resolveConnectionIdFromUiKey(options.connectionFilter.value)
      : null
    if (options.connectionFilter.value && !connectionId) {
      ElMessage.warning(t('messagePanel.export.connectionUnavailable'))
      return
    }

    try {
      const payload = await invokeCommand<ExportCaptureResponse>(command, {
        stationId: options.stationId.value,
        format,
        connectionId,
      })
      await persistExportFile(
        payload.data_base64,
        localizeCaptureFileName(payload.file_name),
        t('messagePanel.export.captureSuccess'),
      )
    } catch (error) {
      ElMessage.error(t('messagePanel.export.captureFailed', { error: translateApiError(error) }))
    }
  }

  const handleExport = async (command: string) => {
    if (command === 'csv' || command === 'txt') {
      if (options.filteredMessages.value.length === 0) {
        ElMessage.warning(t('messagePanel.export.noMessageData'))
        return
      }
    }

    if (command === 'pcap' || command === 'pcapng') {
      const selectedConnectionId = options.connectionFilter.value
        ? options.resolveConnectionIdFromUiKey(options.connectionFilter.value)
        : ''
      const hasCaptureMessages = selectedConnectionId
        ? options.messages.value.some(
            (msg) => String(msg.runtimeConnectionId || '').trim() === selectedConnectionId,
          )
        : options.messages.value.length > 0

      if (!hasCaptureMessages) {
        ElMessage.warning(t('messagePanel.export.noCaptureData'))
        return
      }
    }

    switch (command) {
      case 'csv':
        await exportCSV()
        break
      case 'txt':
        await exportTXT()
        break
      case 'pcap':
        await exportCapture('pcap')
        break
      case 'pcapng':
        await exportCapture('pcapng')
        break
      default:
        break
    }
  }

  return {
    handleExport,
  }
}
