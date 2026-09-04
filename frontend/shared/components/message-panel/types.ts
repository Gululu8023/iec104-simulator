import { formatIec104TypeLabel, getInstalledIec104Capabilities } from '@shared/api/iec104'

export interface Message {
  id: string
  timestamp: number
  type: 'sent' | 'received' | 'error'
  content: string
  hexData?: string
  parsed?: Record<string, any>
  expanded?: boolean
  runtimeConnectionId?: string
  uiConnectionKey?: string
  slaveUiKey?: string
}

export interface StationInfo {
  id: string
  name: string
  groupName?: string
  slaveName?: string
  host?: string
  port?: number
  remoteAddress?: string
  status?: string
  isConnectionLevel?: boolean
  runtimeConnectionId?: string
  uiConnectionKey?: string
  slaveUiKey?: string
  stationConfigId?: string
  commonAddress?: number | null
}

export interface TypeFilterOption {
  value: string
  labelTypeName: string
  zhSourceTypeName: string
}

export interface TypeFilterGroup {
  label: string
  labelKey?: string
  options: TypeFilterOption[]
}

export interface MessageFocusFilter {
  connectionId?: string | null
  commonAddress?: number | null
  ioa?: number | null
  typeId?: number | null
}

export interface MessageFrameMeta {
  typeId: number | null
  commonAddress: number | null
  ioa: number | null
}

export interface FrameSummary {
  tag: string
  label: string
  color: string
  title: string
}

export interface ParsedRow {
  field: string
  value: string
  description: string
}

export function buildMessageTypeFilterGroups(): TypeFilterGroup[] {
  const definitions = [
    {
      labelKey: 'messagePanel.typeGroups.monitoringNoTimestamp',
      match: (role: string, family: string, timestamp: string) =>
        role === 'monitor' && family !== 'protection' && timestamp === 'none',
    },
    {
      labelKey: 'messagePanel.typeGroups.monitoringCp24',
      match: (role: string, family: string, timestamp: string) =>
        role === 'monitor' && family !== 'protection' && timestamp === 'cp24',
    },
    {
      labelKey: 'messagePanel.typeGroups.monitoringCp56',
      match: (role: string, family: string, timestamp: string) =>
        role === 'monitor' && family !== 'protection' && timestamp === 'cp56',
    },
    {
      labelKey: 'messagePanel.typeGroups.protection',
      match: (_role: string, family: string) => family === 'protection',
    },
    { labelKey: 'messagePanel.typeGroups.control', match: (role: string) => role === 'control' },
    {
      labelKey: 'messagePanel.typeGroups.initialization',
      match: (_role: string, family: string) => family === 'initialization',
    },
    {
      labelKey: 'messagePanel.typeGroups.system',
      match: (_role: string, family: string) => family === 'system_command',
    },
    {
      labelKey: 'messagePanel.typeGroups.security',
      match: (_role: string, family: string) => family === 'security',
    },
    {
      labelKey: 'messagePanel.typeGroups.parameter',
      match: (_role: string, family: string) => family === 'parameter',
    },
    {
      labelKey: 'messagePanel.typeGroups.fileTransfer',
      match: (_role: string, family: string) => family === 'file_transfer',
    },
  ]
  const capabilities = getInstalledIec104Capabilities()
  return definitions
    .map((definition) => ({
      label: definition.labelKey,
      labelKey: definition.labelKey,
      options: capabilities
        .filter((capability) =>
          definition.match(capability.point_role, capability.family, capability.timestamp_model),
        )
        .sort((left, right) => left.type_id - right.type_id)
        .map((capability) => ({
          value: String(capability.type_id),
          labelTypeName: capability.name,
          zhSourceTypeName: capability.name,
        })),
    }))
    .filter((group) => group.options.length > 0)
}

export function getMessageTypeFilterOptionLabel(option: TypeFilterOption): string {
  return formatIec104TypeLabel(Number(option.value))
}
