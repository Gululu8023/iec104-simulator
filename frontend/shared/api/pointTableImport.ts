import { invokeCommand } from './tauri'
import type {
  PointTableImportApplyResult,
  PointTableImportNormalizedPayload,
  PointTableImportMode,
  PointTableImportPreview,
  PointTableImportTarget,
  PointTableTemplateExportResponse,
} from './types'

export interface PointTableImportTargetOption {
  key: string
  label: string
  subtitleBadges?: string[] | null
  description?: string | null
  target: PointTableImportTarget
}

export const buildPointTableImportTargetKey = (target: PointTableImportTarget): string => {
  switch (target.kind) {
    case 'master-link-slave':
      return `master-link-slave:${target.slave_id}`
    case 'slave-station-slave':
      return `slave-station-slave:${target.slave_id}`
    default:
      return JSON.stringify(target)
  }
}

export async function previewPointTableImport(
  filePath: string,
  target: PointTableImportTarget,
  mode: PointTableImportMode,
): Promise<PointTableImportPreview> {
  return invokeCommand<PointTableImportPreview>('preview_point_table_import', {
    filePath,
    target,
    mode,
  })
}

export async function applyPointTableImport(
  target: PointTableImportTarget,
  payload: PointTableImportNormalizedPayload,
  mode: PointTableImportMode,
): Promise<PointTableImportApplyResult> {
  return invokeCommand<PointTableImportApplyResult>('apply_point_table_import', {
    target,
    payload,
    mode,
  })
}

export async function generatePointTableCsvTemplate(
  locale: string,
): Promise<PointTableTemplateExportResponse> {
  return invokeCommand<PointTableTemplateExportResponse>('generate_point_table_csv_template', {
    locale,
  })
}
