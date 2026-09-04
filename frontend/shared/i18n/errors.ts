import type { ApiError } from '../api/types'
import { t } from './index'

export type ApiErrorInput = unknown

export const API_ERROR_TRANSLATION_KEYS: Record<string, string> = {
  APP_ERROR: 'errors.APP_ERROR',
  VALIDATION_ERROR: 'errors.VALIDATION_ERROR',
  DATABASE_ERROR: 'errors.DATABASE_ERROR',
  DB_READ_FAILED: 'errors.DB_READ_FAILED',
  DB_WRITE_FAILED: 'errors.DB_WRITE_FAILED',
  DB_TRANSACTION_FAILED: 'errors.DB_TRANSACTION_FAILED',
  USER_SETTING_PARSE_FAILED: 'errors.USER_SETTING_PARSE_FAILED',
  INVALID_LOCALE: 'errors.INVALID_LOCALE',
  INVALID_ASDU_TYPE: 'errors.INVALID_ASDU_TYPE',
  NOT_FOUND: 'errors.NOT_FOUND',
  CONNECTION_NOT_FOUND: 'errors.CONNECTION_NOT_FOUND',
  STATION_NOT_FOUND: 'errors.STATION_NOT_FOUND',
  PROFILE_NOT_FOUND: 'errors.PROFILE_NOT_FOUND',
  SLAVE_NOT_FOUND: 'errors.SLAVE_NOT_FOUND',
  TIMEOUT: 'errors.TIMEOUT',
  INTERNAL_ERROR: 'errors.INTERNAL_ERROR',
  NOT_CONNECTED: 'errors.NOT_CONNECTED',
  DUPLICATE_CONNECTION: 'errors.DUPLICATE_CONNECTION',
  PROTOCOL_STATE: 'errors.PROTOCOL_STATE',
  FILE_TRANSFER_BUSY: 'errors.FILE_TRANSFER_BUSY',
  UPLOAD_FILE_MISSING: 'errors.UPLOAD_FILE_MISSING',
  FILE_TOO_LARGE: 'errors.FILE_TOO_LARGE',
  CONFLICT_IOA_DUPLICATE: 'errors.CONFLICT_IOA_DUPLICATE',
  CONFLICT_IOA_EXISTING: 'errors.CONFLICT_IOA_EXISTING',
  CONFLICT_ASDU_ALIAS: 'errors.CONFLICT_ASDU_ALIAS',
  INVALID_CONTROL_PRESET: 'errors.INVALID_CONTROL_PRESET',
  CONFLICT_COA_DUPLICATE: 'errors.CONFLICT_COA_DUPLICATE',
  POINT_TABLE_IMPORT_FAILED: 'errors.POINT_TABLE_IMPORT_FAILED',
  OVERWRITE_REQUIRED: 'errors.OVERWRITE_REQUIRED',
  EMPTY_POINT_TABLE: 'errors.EMPTY_POINT_TABLE',
  UNSUPPORTED_POINT_TABLE_IMPORT_TARGET: 'errors.UNSUPPORTED_POINT_TABLE_IMPORT_TARGET',
  INVALID_EXPORT_PATH: 'errors.INVALID_EXPORT_PATH',
  INVALID_EXPORT_DATA: 'errors.INVALID_EXPORT_DATA',
  EXPORT_DIRECTORY_FAILED: 'errors.EXPORT_DIRECTORY_FAILED',
  EXPORT_WRITE_FAILED: 'errors.EXPORT_WRITE_FAILED',
  STATION_STATUS_FAILED: 'errors.STATION_STATUS_FAILED',
  STATION_STOP_FAILED: 'errors.STATION_STOP_FAILED',
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isApiError(value: unknown): value is ApiError {
  return isRecord(value) && typeof value.code === 'string' && typeof value.message === 'string'
}

export function normalizeApiError(error: ApiErrorInput): ApiError | null {
  if (!error) return null

  if (typeof error === 'string') {
    return { code: 'APP_ERROR', message: error }
  }

  if (isApiError(error)) {
    return error
  }

  if (error instanceof Error) {
    const wrapped = error as Error & {
      code?: unknown
      detail?: unknown
      params?: unknown
      apiError?: unknown
    }
    if (isApiError(wrapped.apiError)) {
      return wrapped.apiError
    }
    return {
      code: typeof wrapped.code === 'string' ? wrapped.code : 'APP_ERROR',
      message: error.message,
      detail: typeof wrapped.detail === 'string' ? wrapped.detail : null,
      params: isRecord(wrapped.params) ? wrapped.params : null,
    }
  }

  return null
}

export function translateApiError(error: ApiErrorInput, fallback = t('errors.unknown')): string {
  const apiError = normalizeApiError(error)
  if (!apiError) return fallback

  const messageKey = API_ERROR_TRANSLATION_KEYS[apiError.code]
  const params = {
    ...(apiError.params ?? {}),
    code: apiError.code,
    message: apiError.message,
    detail: apiError.detail ?? '',
  }
  const translatedMessage = messageKey ? t(messageKey, params) : apiError.message || fallback
  const detail = String(apiError.detail ?? '').trim()

  if (!detail || detail === translatedMessage) {
    return translatedMessage
  }

  return t('errors.withDetail', {
    message: translatedMessage,
    detail,
  })
}
