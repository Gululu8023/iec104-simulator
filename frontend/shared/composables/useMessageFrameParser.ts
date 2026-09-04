import { computed, ref } from 'vue'

import { parseMessageFrame } from '@shared/api/messageParser'
import type { MessageFrameParseResult } from '@shared/api/types'
import { t } from '@shared/i18n'

const MAX_PARSE_CACHE_ENTRIES = 128

type CacheEntry = { result: MessageFrameParseResult; aliases: Set<string> }

const parseResultCache = new Map<string, CacheEntry>()
const cacheAliases = new Map<string, string>()
const pendingRequests = new Map<string, Promise<MessageFrameParseResult>>()
let cacheGeneration = 0

export function clearMessageFrameParseCache() {
  cacheGeneration += 1
  parseResultCache.clear()
  cacheAliases.clear()
  pendingRequests.clear()
}

const normalizeHexKey = (value: string): string | null => {
  const normalized = String(value ?? '')
    .trim()
    .replace(/\s+/g, '')
    .toUpperCase()
  return !normalized || normalized.length % 2 !== 0 || /[^0-9A-F]/.test(normalized)
    ? null
    : `hex:${normalized}`
}

const normalizeInputKey = (value: string): string =>
  normalizeHexKey(value) ?? `text:${String(value ?? '').trim()}`

const touchCacheEntry = (key: string): CacheEntry | null => {
  const entry = parseResultCache.get(key)
  if (!entry) return null
  parseResultCache.delete(key)
  parseResultCache.set(key, entry)
  return entry
}

const readCache = (alias: string): MessageFrameParseResult | null => {
  const canonical = cacheAliases.get(alias) ?? alias
  return touchCacheEntry(canonical)?.result ?? null
}

const writeCache = (inputText: string, result: MessageFrameParseResult) => {
  const canonical = normalizeHexKey(result.summary.frame_hex) ?? normalizeInputKey(inputText)
  const aliases = new Set(
    [
      normalizeInputKey(inputText),
      normalizeHexKey(result.summary.input_hex),
      normalizeHexKey(result.summary.frame_hex),
    ].filter((key): key is string => Boolean(key)),
  )
  const previous = parseResultCache.get(canonical)
  if (previous) {
    for (const alias of previous.aliases) cacheAliases.delete(alias)
    parseResultCache.delete(canonical)
  }
  parseResultCache.set(canonical, { result, aliases })
  for (const alias of aliases) cacheAliases.set(alias, canonical)

  while (parseResultCache.size > MAX_PARSE_CACHE_ENTRIES) {
    const oldestKey = parseResultCache.keys().next().value as string | undefined
    if (!oldestKey) break
    const oldest = parseResultCache.get(oldestKey)
    if (oldest) for (const alias of oldest.aliases) cacheAliases.delete(alias)
    parseResultCache.delete(oldestKey)
  }
}

async function fetchAndCacheParseResult(inputText: string): Promise<MessageFrameParseResult> {
  const inputKey = normalizeInputKey(inputText)
  const cached = readCache(inputKey)
  if (cached) return cached
  const existing = pendingRequests.get(inputKey)
  if (existing) return existing
  const generation = cacheGeneration
  const request = parseMessageFrame(inputText)
    .then((result) => {
      if (generation === cacheGeneration) writeCache(inputText, result)
      return result
    })
    .finally(() => {
      if (pendingRequests.get(inputKey) === request) pendingRequests.delete(inputKey)
    })
  pendingRequests.set(inputKey, request)
  return request
}

export function getCachedMessageFrameParseResult(
  hexData?: string | null,
): MessageFrameParseResult | null {
  const key = normalizeHexKey(String(hexData ?? ''))
  return key ? readCache(key) : null
}

export function getMessageFrameParseErrorMessage(
  result: MessageFrameParseResult | null,
): string | null {
  const error = result?.error
  if (!error) return null
  if (error.code === 'INPUT_TOO_LARGE') return t('messageParser.errors.inputTooLarge')
  if (error.code === 'DECODED_INPUT_TOO_LARGE')
    return t('messageParser.errors.decodedInputTooLarge')
  return error.message
}

export function useMessageFrameParserSession(initialInput = '') {
  const inputText = ref(initialInput)
  const loading = ref(false)
  const result = ref<MessageFrameParseResult | null>(null)
  const parseError = ref<string | null>(null)
  const lastResolvedInput = ref('')
  let requestVersion = 0
  const hasResult = computed(() => result.value != null)

  const parse = async (nextInput = inputText.value) => {
    const version = ++requestVersion
    const normalizedInput = String(nextInput ?? '').trim()
    inputText.value = nextInput
    parseError.value = null
    if (!normalizedInput) {
      result.value = null
      lastResolvedInput.value = ''
      return null
    }
    loading.value = true
    try {
      const nextResult = await fetchAndCacheParseResult(nextInput)
      if (version === requestVersion) {
        result.value = nextResult
        lastResolvedInput.value = nextInput
      }
      return nextResult
    } catch (error) {
      if (version === requestVersion) {
        result.value = null
        parseError.value = error instanceof Error ? error.message : String(error)
      }
      throw error
    } finally {
      if (version === requestVersion) loading.value = false
    }
  }

  const hydrateFromHex = (hexData?: string | null) => {
    const cached = getCachedMessageFrameParseResult(hexData)
    if (!cached) return false
    requestVersion += 1
    loading.value = false
    result.value = cached
    parseError.value = null
    lastResolvedInput.value = cached.summary.frame_hex
    return true
  }

  const reset = () => {
    requestVersion += 1
    loading.value = false
    result.value = null
    parseError.value = null
    lastResolvedInput.value = ''
  }

  return {
    inputText,
    loading,
    result,
    parseError,
    hasResult,
    lastResolvedInput,
    hydrateFromHex,
    parse,
    reset,
  }
}
