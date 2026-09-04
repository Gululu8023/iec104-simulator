import { computed, type Ref } from 'vue'

import { t } from '@shared/i18n'
import type {
  BackendConnectionInfo,
  BackendSoeEvent,
  PointDef,
  SlaveResponse,
  StationSummary,
} from '@shared/api/types'
import { normalizeIecObjectDisplayName, resolveIec104TypeName } from '@shared/api/iec104'
import { deriveUnifiedConnStatusLocalized } from '@shared/ui/connectionStatus'
import { getStationSlaveAsduAliases, getStationSlavePointDefs, listSlaves } from '@/api/slave'
import {
  createEmptyPointTypeSummary,
  type DataTypeCountMap,
  type DataTypeDisplayNameMap,
  type PointTypeSummary,
} from '@/types/pointTypeSummary'

type UseSlaveStationCatalogOptions = {
  stations: Ref<StationSummary[]>
  currentStationId: Ref<string>
  selectedSlaveId: Ref<number | null>
  slavesByStation: Ref<Record<string, SlaveResponse[]>>
  runtimeConnectionsByStation: Ref<Record<string, BackendConnectionInfo[]>>
  stationPointTypeSummary: Ref<Record<string, PointTypeSummary>>
  slavePointTypeSummary: Ref<Record<number, PointTypeSummary>>
  slaveAsduAliasMap: Ref<Record<number, Record<string, string>>>
  soeEventsByStation: Ref<Record<string, BackendSoeEvent[]>>
  lastSoeIdByStation: Ref<Record<string, number>>
}

function summarizePointTypes(
  defs: PointDef[],
  aliasesByType: Record<string, string>,
): PointTypeSummary {
  const countByType: DataTypeCountMap = {}
  const pointNamesByType = new Map<string, Set<string>>()
  for (const def of defs) {
    const dataType = resolveIec104TypeName(def)
    countByType[dataType] = (countByType[dataType] ?? 0) + 1
    const name = normalizeIecObjectDisplayName(def.name)
    if (!name) continue
    const names = pointNamesByType.get(dataType) ?? new Set<string>()
    names.add(name)
    pointNamesByType.set(dataType, names)
  }
  const displayNameByType: DataTypeDisplayNameMap = {}
  for (const dataType of Object.keys(countByType)) {
    const alias = normalizeIecObjectDisplayName(aliasesByType[dataType])
    const usesPointName =
      (countByType[dataType] ?? 0) > 1 && Boolean(pointNamesByType.get(dataType)?.has(alias))
    if (alias && !usesPointName) displayNameByType[dataType] = alias
  }
  return { countByType, displayNameByType }
}

export function useSlaveStationCatalog(options: UseSlaveStationCatalogOptions) {
  const refreshStationPointTypeSummary = async () => {
    const stationRows = options.stations.value ?? []
    if (stationRows.length === 0) {
      options.stationPointTypeSummary.value = {}
      options.slavesByStation.value = {}
      options.slavePointTypeSummary.value = {}
      options.slaveAsduAliasMap.value = {}
      options.runtimeConnectionsByStation.value = {}
      options.soeEventsByStation.value = {}
      options.lastSoeIdByStation.value = {}
      options.selectedSlaveId.value = null
      return
    }

    const nextSlaves: Record<string, SlaveResponse[]> = {}
    const nextSummaries: Record<number, PointTypeSummary> = {}
    const nextAliases: Record<number, Record<string, string>> = {}
    for (const station of stationRows) {
      try {
        const slaves = await listSlaves(station.station_id)
        nextSlaves[station.station_id] = slaves
        for (const slave of slaves) {
          let aliases: Record<string, string> = {}
          try {
            aliases = await getStationSlaveAsduAliases(slave.id)
          } catch {
            aliases = {}
          }
          nextAliases[slave.id] = aliases
          try {
            nextSummaries[slave.id] = summarizePointTypes(
              await getStationSlavePointDefs(slave.id),
              aliases,
            )
          } catch {
            nextSummaries[slave.id] = createEmptyPointTypeSummary()
          }
        }
      } catch {
        nextSlaves[station.station_id] = []
      }
    }

    options.stationPointTypeSummary.value = Object.fromEntries(
      stationRows.map((station) => [station.station_id, createEmptyPointTypeSummary()]),
    )
    options.slavesByStation.value = nextSlaves
    options.slavePointTypeSummary.value = nextSummaries
    options.slaveAsduAliasMap.value = nextAliases
    const validIds = new Set(stationRows.map((station) => station.station_id))
    const retainValidStations = <T>(values: Record<string, T>): Record<string, T> =>
      Object.fromEntries(Object.entries(values).filter(([stationId]) => validIds.has(stationId)))
    options.runtimeConnectionsByStation.value = retainValidStations(
      options.runtimeConnectionsByStation.value,
    )
    options.soeEventsByStation.value = retainValidStations(options.soeEventsByStation.value)
    options.lastSoeIdByStation.value = retainValidStations(options.lastSoeIdByStation.value)

    const activeSlaves = nextSlaves[options.currentStationId.value] ?? []
    if (
      activeSlaves.length === 0 ||
      (options.selectedSlaveId.value != null &&
        !activeSlaves.some((slave) => slave.id === options.selectedSlaveId.value))
    ) {
      options.selectedSlaveId.value = null
    }
  }

  const currentStationList = computed(() => {
    const list: any[] = []
    for (const station of options.stations.value) {
      const connections = options.runtimeConnectionsByStation.value[station.station_id]
      const connectionRows = connections?.length ? connections : [null]
      const connectionName = String(station.name || '').trim() || t('slave.app.unknownConnection')
      for (const connection of connectionRows) {
        const runtimeConnectionId = String(connection?.id ?? '').trim()
        const uiConnectionKey = runtimeConnectionId
          ? `slave-station:${station.station_id}:conn:${runtimeConnectionId}`
          : `slave-station:${station.station_id}:conn:offline`
        const status = connection
          ? deriveUnifiedConnStatusLocalized(
              connection.transport_state,
              connection.data_transfer_state,
              null,
              'server',
            ).text
          : station.status === 'Running'
            ? 'running'
            : 'stopped'
        list.push({
          id: uiConnectionKey,
          name: connectionName,
          groupName: connectionName,
          slaveName: connectionName,
          isConnectionLevel: true,
          host: station.host,
          port: station.port,
          remoteAddress: String(connection?.remote_addr ?? '').trim(),
          status,
          runtimeConnectionId,
          uiConnectionKey,
          stationConfigId: station.station_id,
          commonAddress: null,
        })
        for (const slave of options.slavesByStation.value[station.station_id] ?? []) {
          const slaveName = String(slave.name || '').trim()
          const name =
            connectionName && slaveName && connectionName !== slaveName
              ? `${connectionName} - ${slaveName}`
              : slaveName || connectionName || t('slave.app.unknownSlave')
          const slaveUiKey = `${uiConnectionKey}:slave:${slave.id}`
          list.push({
            id: slaveUiKey,
            name,
            groupName: connectionName,
            slaveName: slave.name,
            host: station.host,
            port: station.port,
            status,
            runtimeConnectionId,
            uiConnectionKey,
            slaveUiKey,
            stationConfigId: String(slave.id),
            commonAddress: slave.common_address,
          })
        }
      }
    }
    return list
  })

  return { currentStationList, refreshStationPointTypeSummary }
}
