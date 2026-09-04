import { ElMessage } from 'element-plus'
import { save } from '@tauri-apps/plugin-dialog'

import { invokeCommand } from '@shared/api/tauri'

export function encodeBytesToBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunkSize = 0x8000
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}

export function encodeTextToBase64(content: string): string {
  return encodeBytesToBase64(new TextEncoder().encode(content))
}

export async function saveExportFile(options: {
  dataBase64: string
  fileName: string
  dialogTitle: string
  successMessage: string
}): Promise<boolean> {
  const selected = await save({
    title: options.dialogTitle,
    defaultPath: options.fileName,
  })
  if (typeof selected !== 'string' || !selected.trim()) return false

  await invokeCommand<void>('save_export_file', {
    filePath: selected.trim(),
    dataBase64: options.dataBase64,
  })
  ElMessage.success(options.successMessage)
  return true
}
