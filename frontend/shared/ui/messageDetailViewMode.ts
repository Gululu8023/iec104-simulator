import type { MessageDetailParseViewMode } from '@shared/api/types'

type MessageDetailPreferenceScope = 'master' | 'slave'

const STORAGE_KEYS: Record<MessageDetailPreferenceScope, string> = {
  master: 'iec104-simulator.master.message-detail-view-mode',
  slave: 'iec104-simulator.slave.message-detail-view-mode',
}

export const DEFAULT_MESSAGE_DETAIL_VIEW_MODE: MessageDetailParseViewMode = 'table'

export function loadMessageDetailViewModePreference(
  scope: MessageDetailPreferenceScope,
): MessageDetailParseViewMode {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEYS[scope])
    return raw === 'tree' || raw === 'table' ? raw : DEFAULT_MESSAGE_DETAIL_VIEW_MODE
  } catch {
    return DEFAULT_MESSAGE_DETAIL_VIEW_MODE
  }
}

export function saveMessageDetailViewModePreference(
  scope: MessageDetailPreferenceScope,
  mode: MessageDetailParseViewMode,
): void {
  try {
    window.localStorage.setItem(STORAGE_KEYS[scope], mode)
  } catch {
    // UI preference persistence failure should not block normal workflows.
  }
}
