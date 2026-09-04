import { invokeCommand } from '@shared/api/tauri'
import type { PointSyncBatch, PointSyncCursor } from '@shared/api/pointSync'
import type {
  BackendConnectionInfo,
  BackendDataPoint,
  BackendSoeEvent,
  BackendSlaveMessage,
  CreateStationRequest,
  InitializationProfile,
  LinkParams,
  PointDef,
  RedundancyGroupMode,
  SlaveCommandMismatchPolicy,
  SlaveIoaDisplayFormat,
  StartSlaveSelectedSimulationRequest,
  SlaveStationPolicy,
  StationConfigResponse,
  SimulationProfile,
  SimulationScenario,
  UpdateDataPointRequest,
  UpdateSlaveStationConfigBundleRequest,
  UpdateSlaveStationConfigBundleResponse,
  UpsertSlaveBundleRequest,
  UpsertSlaveBundleResponse,
  SlaveResponse,
} from '@shared/api/types'

export async function createSlaveStation(request: CreateStationRequest): Promise<string> {
  return invokeCommand<string>('create_slave_station', { request })
}

export async function startSlaveStation(stationId: string): Promise<void> {
  return invokeCommand<void>('start_slave_station', { stationId })
}

export async function stopSlaveStation(stationId: string): Promise<void> {
  await invokeCommand<void>('stop_slave_station', { stationId })
}

export async function getSlaveStationConfig(stationId: string): Promise<StationConfigResponse> {
  return invokeCommand<StationConfigResponse>('get_slave_station_config', { stationId })
}

export async function updateSlaveStationConfig(
  stationId: string,
  request: UpdateSlaveStationConfigBundleRequest,
): Promise<UpdateSlaveStationConfigBundleResponse> {
  return invokeCommand<UpdateSlaveStationConfigBundleResponse>('update_slave_station_config', {
    stationId,
    request,
  })
}

export async function getSlaveConnections(stationId: string): Promise<BackendConnectionInfo[]> {
  return invokeCommand<BackendConnectionInfo[]>('get_slave_connections', { stationId })
}

export async function getSlaveStationTimeOffset(stationId: string): Promise<number> {
  return invokeCommand<number>('get_slave_station_time_offset', { stationId })
}

export async function syncSlaveDataPoints(
  stationId: string,
  cursor: PointSyncCursor | null,
): Promise<PointSyncBatch<BackendDataPoint>> {
  return invokeCommand<PointSyncBatch<BackendDataPoint>>('sync_slave_data_points', {
    stationId,
    cursor,
  })
}

export async function updateSlaveDataPoint(
  stationId: string,
  request: UpdateDataPointRequest,
): Promise<void> {
  return invokeCommand<void>('update_slave_data_point', { stationId, request })
}

export async function bulkUpdateSlaveDataPoints(
  stationId: string,
  requests: UpdateDataPointRequest[],
): Promise<number> {
  return invokeCommand<number>('bulk_update_slave_data_points', { stationId, requests })
}

export async function simulateSlaveDataChanges(
  stationId: string,
  commonAddress?: number,
): Promise<void> {
  return invokeCommand<void>('simulate_slave_data_changes', { stationId, commonAddress })
}

export async function forceUploadSlaveData(
  stationId: string,
  commonAddress: number,
): Promise<number> {
  return invokeCommand<number>('force_upload_slave_data', { stationId, commonAddress })
}

export async function startSlaveSimulation(stationId: string): Promise<void> {
  await invokeCommand<void>('start_slave_simulation', { stationId })
}

export async function stopSlaveSimulation(stationId: string): Promise<void> {
  await invokeCommand<void>('stop_slave_simulation', { stationId })
}

export async function startSlaveSelectedPointSimulation(
  stationId: string,
  request: StartSlaveSelectedSimulationRequest,
): Promise<void> {
  await invokeCommand<void>('start_slave_selected_point_simulation', {
    stationId,
    request,
  })
}

export async function stopSlaveSelectedPointSimulation(stationId: string): Promise<void> {
  await invokeCommand<void>('stop_slave_selected_point_simulation', { stationId })
}

export async function getSlaveSimulationProfile(stationId: string): Promise<SimulationProfile> {
  return invokeCommand<SimulationProfile>('get_slave_simulation_profile', { stationId })
}

export async function updateSlaveSimulationProfile(
  stationId: string,
  profile: SimulationProfile,
): Promise<void> {
  return invokeCommand<void>('update_slave_simulation_profile', {
    stationId,
    request: { profile },
  })
}

export async function updateSlaveInitializationProfile(
  stationId: string,
  profile: InitializationProfile,
): Promise<void> {
  await invokeCommand<void>('update_slave_initialization_profile', {
    stationId,
    request: { profile },
  })
}

export async function triggerSlaveScenario(
  stationId: string,
  scenario: SimulationScenario,
): Promise<number> {
  return invokeCommand<number>('trigger_slave_scenario', {
    stationId,
    request: { scenario },
  })
}

export async function getSlaveMessages(
  stationId: string,
  sinceId: number | null = null,
  limit = 200,
): Promise<BackendSlaveMessage[]> {
  return invokeCommand<BackendSlaveMessage[]>('get_slave_messages', {
    stationId,
    sinceId,
    limit,
  })
}

export async function setSlaveMessageTracking(stationId: string, enabled: boolean): Promise<void> {
  return invokeCommand<void>('set_slave_message_tracking', {
    stationId,
    enabled,
  })
}

export async function getSlaveLinkParams(stationId: string): Promise<LinkParams> {
  return invokeCommand<LinkParams>('get_slave_link_params', { stationId })
}

export async function getSlaveRedundancyMode(stationId: string): Promise<RedundancyGroupMode> {
  return invokeCommand<RedundancyGroupMode>('get_slave_redundancy_mode', { stationId })
}

export async function getSlaveCommandMismatchPolicy(): Promise<SlaveCommandMismatchPolicy> {
  return invokeCommand<SlaveCommandMismatchPolicy>('get_slave_command_mismatch_policy')
}

export async function getSlaveIoaDisplayFormat(): Promise<SlaveIoaDisplayFormat> {
  return invokeCommand<SlaveIoaDisplayFormat>('get_slave_ioa_display_format')
}

export async function getSlaveStationPolicyDefaults(): Promise<SlaveStationPolicy> {
  return invokeCommand<SlaveStationPolicy>('get_slave_station_policy_defaults')
}

export async function updateSlaveLinkParams(stationId: string, params: LinkParams): Promise<void> {
  return invokeCommand<void>('update_slave_link_params', { stationId, params })
}

export async function updateSlaveRedundancyMode(
  stationId: string,
  mode: RedundancyGroupMode,
): Promise<void> {
  await invokeCommand<void>('update_slave_redundancy_mode', { stationId, mode })
}

export async function updateSlaveCommandMismatchPolicy(
  policy: SlaveCommandMismatchPolicy,
): Promise<void> {
  await invokeCommand<void>('update_slave_command_mismatch_policy', { policy })
}

export async function updateSlaveIoaDisplayFormat(format: SlaveIoaDisplayFormat): Promise<void> {
  await invokeCommand<void>('update_slave_ioa_display_format', { format })
}

export async function updateSlaveStationPolicyDefaults(
  defaults: SlaveStationPolicy,
): Promise<void> {
  await invokeCommand<void>('update_slave_station_policy_defaults', { defaults })
}

export async function createSlave(
  stationId: string,
  request: UpsertSlaveBundleRequest,
): Promise<UpsertSlaveBundleResponse> {
  return invokeCommand<UpsertSlaveBundleResponse>('create_station_slave', {
    stationId,
    request,
  })
}

export async function getSlaveSoeEvents(
  stationId: string,
  sinceId: number | null = null,
  limit = 200,
): Promise<BackendSoeEvent[]> {
  return invokeCommand<BackendSoeEvent[]>('get_slave_soe_events', {
    stationId,
    sinceId,
    limit,
  })
}

export async function clearSlaveSoeEvents(stationId: string): Promise<void> {
  return invokeCommand<void>('clear_slave_soe_events', { stationId })
}

export async function updateSlave(
  slaveId: number,
  request: UpsertSlaveBundleRequest,
): Promise<UpsertSlaveBundleResponse> {
  return invokeCommand<UpsertSlaveBundleResponse>('update_station_slave', {
    slaveId,
    request,
  })
}

export async function deleteSlave(slaveId: number): Promise<void> {
  return invokeCommand<void>('delete_station_slave', { slaveId })
}

export async function listSlaves(stationId: string): Promise<SlaveResponse[]> {
  return invokeCommand<SlaveResponse[]>('list_station_slaves', { stationId })
}

export async function getStationSlavePointDefs(slaveId: number): Promise<PointDef[]> {
  return invokeCommand<PointDef[]>('get_station_slave_point_defs', { slaveId })
}

export async function replaceStationSlavePointDefs(
  slaveId: number,
  defs: PointDef[],
): Promise<number> {
  return invokeCommand<number>('replace_station_slave_point_defs', { slaveId, defs })
}

export async function getStationSlaveAsduAliases(slaveId: number): Promise<Record<string, string>> {
  return invokeCommand<Record<string, string>>('get_station_slave_asdu_aliases', {
    slaveId,
  })
}

export async function upsertStationSlaveAsduAlias(
  slaveId: number,
  asduType: string,
  alias: string,
): Promise<Record<string, string>> {
  return invokeCommand<Record<string, string>>('upsert_station_slave_asdu_alias', {
    slaveId,
    asduType,
    alias,
  })
}
