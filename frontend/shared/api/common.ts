import { invokeCommand } from './tauri'
import { installIec104CapabilityCatalog } from './iec104/catalog'
import type { AppInfo, AppLocale, Iec104Capability, StationStats, StationSummary } from './types'

let iec104Capabilities: Iec104Capability[] = []

export async function loadIec104Capabilities(): Promise<Iec104Capability[]> {
  iec104Capabilities = await invokeCommand<Iec104Capability[]>('get_iec104_capabilities')
  installIec104CapabilityCatalog(iec104Capabilities)
  return iec104Capabilities
}

export function getLoadedIec104Capabilities(): readonly Iec104Capability[] {
  return iec104Capabilities
}

export async function deleteStation(stationId: string): Promise<void> {
  await invokeCommand<void>('delete_station', { stationId })
}

export async function listStations(stationType?: 'master' | 'slave'): Promise<StationSummary[]> {
  return invokeCommand<StationSummary[]>('list_stations', {
    stationType: stationType ?? null,
  })
}

export async function getStationStats(): Promise<StationStats> {
  return invokeCommand<StationStats>('get_station_stats')
}

export async function stopAllStations(): Promise<void> {
  return invokeCommand<void>('stop_all_stations')
}

export async function getAppInfo(): Promise<AppInfo> {
  return invokeCommand<AppInfo>('get_app_info')
}

export async function getUiLanguage(): Promise<AppLocale> {
  return invokeCommand<AppLocale>('get_ui_language')
}

export async function updateUiLanguage(locale: AppLocale): Promise<void> {
  await invokeCommand<void>('update_ui_language', { locale })
}
