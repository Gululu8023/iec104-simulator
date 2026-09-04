import { invoke } from '@tauri-apps/api/core'

import type { ApiError } from './types'

const TOP_LEVEL_ARG_ALIASES: Record<string, string> = {
  station_id: 'stationId',
  station_type: 'stationType',
  connection_id: 'connectionId',
  profile_id: 'profileId',
  since_id: 'sinceId',
  file_path: 'filePath',
}

function normalizeInvokeArgs(args: Record<string, unknown>): Record<string, unknown> {
  if (!args || Object.keys(args).length === 0) return {}

  const normalized: Record<string, unknown> = { ...args }
  for (const [fromKey, toKey] of Object.entries(TOP_LEVEL_ARG_ALIASES)) {
    if (normalized[toKey] === undefined && normalized[fromKey] !== undefined) {
      normalized[toKey] = normalized[fromKey]
      delete normalized[fromKey]
    }
  }
  return normalized
}

export async function invokeCommand<T>(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const normalizedArgs = normalizeInvokeArgs(args)
  const startedAt = Date.now()

  try {
    return await invoke<T>(cmd, normalizedArgs)
  } catch (error) {
    console.error('[tauri.invoke] failed', {
      cmd,
      args: normalizedArgs,
      durationMs: Date.now() - startedAt,
      error,
    })
    throw toInvokeError(error)
  }
}

type InvokeError = Error & {
  code?: string
  detail?: string | null
  params?: Record<string, unknown> | null
  apiError?: ApiError
}

function toInvokeError(error: unknown): Error {
  if (error instanceof Error) return error
  if (!error || typeof error !== 'object') return new Error(String(error || 'Unknown error'))

  const apiError = error as Partial<ApiError>

  const message = apiError.message?.trim() || apiError.code || 'Unknown error'
  const wrapped = new Error(message) as InvokeError
  wrapped.code = apiError.code
  wrapped.detail = apiError.detail ?? null
  wrapped.params = apiError.params ?? null
  wrapped.apiError = apiError as ApiError
  return wrapped
}
