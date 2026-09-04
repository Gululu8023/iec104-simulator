import { invokeCommand } from '@shared/api/tauri'
import type { PointSyncBatch, PointSyncCursor } from '@shared/api/pointSync'
import type {
  BackendConnectionInfo,
  BackendDataPoint,
  BackendMasterMessage,
  BackendSoeEvent,
  ConnectRequest,
  CreateStationRequest,
  Iec104CommandRequest,
  Iec104CommandResult,
  LinkProfileResponse,
  LinkSlaveResponse,
  MasterFileTransferRequest,
  MasterFileTransferLimits,
  MasterFileTransferSession,
  PointDef,
  UpsertLinkProfileRequest,
  UpsertLinkSlaveBundleRequest,
  UpsertLinkSlaveBundleResponse,
} from '@shared/api/types'

export async function createMasterStation(request: CreateStationRequest): Promise<string> {
  return invokeCommand<string>('create_master_station', { request })
}

export async function startMasterStation(stationId: string): Promise<void> {
  return invokeCommand<void>('start_master_station', { stationId })
}

export async function stopMasterStation(stationId: string): Promise<void> {
  await invokeCommand<void>('stop_master_station', { stationId })
}

export async function connectToSlave(stationId: string, request: ConnectRequest): Promise<string> {
  return invokeCommand<string>('connect_to_slave', { stationId, request })
}

export async function disconnectFromSlave(stationId: string, connectionId: string): Promise<void> {
  return invokeCommand<void>('disconnect_from_slave', {
    stationId,
    connectionId,
  })
}

export async function startMasterDataTransfer(
  stationId: string,
  connectionId: string,
): Promise<void> {
  await invokeCommand<void>('start_master_data_transfer', {
    stationId,
    connectionId,
  })
}

export async function stopMasterDataTransfer(
  stationId: string,
  connectionId: string,
): Promise<void> {
  await invokeCommand<void>('stop_master_data_transfer', {
    stationId,
    connectionId,
  })
}

export async function startMasterFileTransfer(
  stationId: string,
  request: MasterFileTransferRequest,
): Promise<MasterFileTransferSession> {
  return invokeCommand<MasterFileTransferSession>('start_master_file_transfer', {
    stationId,
    request,
  })
}

export async function getMasterFileTransferLimits(
  stationId: string,
  connectionId: string,
): Promise<MasterFileTransferLimits> {
  return invokeCommand<MasterFileTransferLimits>('get_master_file_transfer_limits', {
    stationId,
    connectionId,
  })
}

export async function getMasterFileTransferSession(
  stationId: string,
  connectionId: string,
): Promise<MasterFileTransferSession | null> {
  return invokeCommand<MasterFileTransferSession | null>('get_master_file_transfer_session', {
    stationId,
    connectionId,
  })
}

export async function cancelMasterFileTransfer(
  stationId: string,
  connectionId: string,
): Promise<MasterFileTransferSession> {
  return invokeCommand<MasterFileTransferSession>('cancel_master_file_transfer', {
    stationId,
    connectionId,
  })
}

export async function sendIec104Command(
  stationId: string,
  request: Iec104CommandRequest,
): Promise<Iec104CommandResult> {
  return invokeCommand<Iec104CommandResult>('send_iec104_command', {
    stationId,
    request,
  })
}

export async function sendTestFrame(stationId: string, connectionId: string): Promise<void> {
  return invokeCommand<void>('send_test_frame', {
    stationId,
    connectionId,
  })
}

export async function getMasterConnections(stationId: string): Promise<BackendConnectionInfo[]> {
  return invokeCommand<BackendConnectionInfo[]>('get_master_connections', { stationId })
}

export async function syncMasterDataPoints(
  stationId: string,
  cursor: PointSyncCursor | null,
): Promise<PointSyncBatch<BackendDataPoint>> {
  return invokeCommand<PointSyncBatch<BackendDataPoint>>('sync_master_data_points', {
    stationId,
    cursor,
  })
}

export async function getMasterMessages(
  stationId: string,
  sinceId?: number | null,
  limit?: number | null,
): Promise<BackendMasterMessage[]> {
  return invokeCommand<BackendMasterMessage[]>('get_master_messages', {
    stationId,
    sinceId: sinceId ?? null,
    limit: limit ?? null,
  })
}

export async function setMasterMessageTracking(stationId: string, enabled: boolean): Promise<void> {
  return invokeCommand<void>('set_master_message_tracking', {
    stationId,
    enabled,
  })
}

export async function getMasterSoeEvents(
  stationId: string,
  sinceId?: number | null,
  limit?: number | null,
): Promise<BackendSoeEvent[]> {
  return invokeCommand<BackendSoeEvent[]>('get_master_soe_events', {
    stationId,
    sinceId: sinceId ?? null,
    limit: limit ?? null,
  })
}

export async function clearMasterSoeEvents(stationId: string): Promise<void> {
  return invokeCommand<void>('clear_master_soe_events', { stationId })
}

export async function createLinkProfile(
  stationId: string,
  request: UpsertLinkProfileRequest,
): Promise<LinkProfileResponse> {
  return invokeCommand<LinkProfileResponse>('create_link_profile', { stationId, request })
}

export async function updateLinkProfile(
  profileId: number,
  request: UpsertLinkProfileRequest,
): Promise<LinkProfileResponse> {
  return invokeCommand<LinkProfileResponse>('update_link_profile', { profileId, request })
}

export async function deleteLinkProfile(profileId: number): Promise<void> {
  return invokeCommand<void>('delete_link_profile', { profileId })
}

export async function listLinkProfiles(stationId: string): Promise<LinkProfileResponse[]> {
  return invokeCommand<LinkProfileResponse[]>('list_link_profiles', { stationId })
}

export async function createLinkSlave(
  connectionId: number,
  request: UpsertLinkSlaveBundleRequest,
): Promise<UpsertLinkSlaveBundleResponse> {
  return invokeCommand<UpsertLinkSlaveBundleResponse>('create_link_slave', {
    connectionId,
    request,
  })
}

export async function updateLinkSlave(
  slaveId: number,
  request: UpsertLinkSlaveBundleRequest,
): Promise<UpsertLinkSlaveBundleResponse> {
  return invokeCommand<UpsertLinkSlaveBundleResponse>('update_link_slave', {
    slaveId,
    request,
  })
}

export async function deleteLinkSlave(slaveId: number): Promise<void> {
  return invokeCommand<void>('delete_link_slave', { slaveId })
}

export async function listLinkSlaves(connectionId: number): Promise<LinkSlaveResponse[]> {
  return invokeCommand<LinkSlaveResponse[]>('list_link_slaves', { connectionId })
}

export async function getLinkSlavePointDefs(slaveId: number): Promise<PointDef[]> {
  return invokeCommand<PointDef[]>('get_link_slave_point_defs', { slaveId })
}

export async function getLinkSlaveAsduAliases(slaveId: number): Promise<Record<string, string>> {
  return invokeCommand<Record<string, string>>('get_link_slave_asdu_aliases', {
    slaveId,
  })
}

export async function connectLinkProfile(
  stationId: string,
  linkProfileId: number,
  autoStartDataTransfer?: boolean | null,
): Promise<string> {
  return invokeCommand<string>('connect_link_profile', {
    stationId,
    linkProfileId,
    autoStartDataTransfer: autoStartDataTransfer ?? null,
  })
}
