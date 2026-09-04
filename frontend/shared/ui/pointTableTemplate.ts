import { ElMessage } from 'element-plus'

import { generatePointTableCsvTemplate } from '@shared/api/pointTableImport'
import { currentLocale, t } from '@shared/i18n'
import { translateApiError } from '@shared/i18n/errors'
import { saveExportFile } from '@shared/ui/saveExportFile'

export async function downloadPointTableTemplate(): Promise<void> {
  try {
    const template = await generatePointTableCsvTemplate(currentLocale.value)
    await saveExportFile({
      dataBase64: template.data_base64,
      fileName: template.file_name,
      dialogTitle: t('pointTableTemplate.saveTitle'),
      successMessage: t('pointTableTemplate.saved'),
    })
  } catch (error) {
    ElMessage.error(t('pointTableTemplate.saveFailed', { error: translateApiError(error) }))
  }
}
