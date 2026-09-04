import { nextTick, type Ref } from 'vue'

import type { SlaveResponse, StationSummary } from '@shared/api/types'
import { compareIec104TypeForDisplay, normalizeIecObjectDisplayName } from '@shared/api/iec104'
import { replayExpandedTreeKeys, syncExpandedTreeKeys } from '@shared/ui/treeExpansion'

import type { DeviceNode } from '../types/slave'
import { createEmptyPointTypeSummary, type PointTypeSummary } from '../types/pointTypeSummary'

interface SlaveTreeModelSource {
  stations?: StationSummary[]
  stationId?: string
  selectedSlaveStatus?: DeviceNode['status'] | null
  stationRuntimeStatusMap?: Record<string, DeviceNode['status']>
  slavesByStation?: Record<string, SlaveResponse[]>
  slavePointTypeSummary?: Record<number, PointTypeSummary>
}

interface UseSlaveTreeModelOptions {
  treeRef: Ref<any>
  deviceTreeData: Ref<DeviceNode[]>
  selectedSlave: Ref<DeviceNode | null>
  expandedNodeKeys: Ref<string[]>
  hasAppliedInitialAutoExpand: Ref<boolean>
  stationRuntimeStatusCache: Ref<Record<string, DeviceNode['status']>>
  source: SlaveTreeModelSource
}

const buildTypeNode = (
  stationId: string,
  slaveId: number,
  slaveName: string,
  commonAddress: number,
  dataType: string,
  label: string,
  count: number,
): DeviceNode => ({
  id: `${stationId}-${slaveId}-${dataType}`,
  label,
  type: 'type-id',
  dataType,
  count,
  stationId,
  parentStationId: stationId,
  parentSlaveId: String(slaveId),
  slaveId,
  slaveCommonAddress: commonAddress,
  parentSlaveName: slaveName,
})

const mapStationStatusToNodeStatus = (
  status: StationSummary['status'] | undefined,
): DeviceNode['status'] => {
  switch (status) {
    case 'Running':
      return 'running'
    case 'Starting':
      return 'starting'
    case 'Stopping':
      return 'stopping'
    case 'Error':
      return 'error'
    case 'Stopped':
    default:
      return 'stopped'
  }
}

export function useSlaveTreeModel(options: UseSlaveTreeModelOptions) {
  const resolveStationNodeStatus = (
    stationId: string,
    stationStatus: StationSummary['status'] | undefined,
  ): DeviceNode['status'] => {
    const normalizedStationId = String(stationId || '').trim()
    const defaultStatus = mapStationStatusToNodeStatus(stationStatus)
    const selectedStationId = String(options.source.stationId || '').trim()
    if (!normalizedStationId) {
      return defaultStatus
    }

    const runtimeStatus = options.source.stationRuntimeStatusMap?.[normalizedStationId]
    if (runtimeStatus) {
      options.stationRuntimeStatusCache.value[normalizedStationId] = runtimeStatus
      return runtimeStatus
    }

    if (selectedStationId && normalizedStationId === selectedStationId) {
      const selectedStatus = options.source.selectedSlaveStatus ?? defaultStatus
      options.stationRuntimeStatusCache.value[normalizedStationId] = selectedStatus
      return selectedStatus
    }

    if (defaultStatus !== 'running') {
      delete options.stationRuntimeStatusCache.value[normalizedStationId]
      return defaultStatus
    }

    const cachedStatus = options.stationRuntimeStatusCache.value[normalizedStationId]
    if (cachedStatus === 'connected' || cachedStatus === 'connecting' || cachedStatus === 'error') {
      return cachedStatus
    }

    options.stationRuntimeStatusCache.value[normalizedStationId] = defaultStatus
    return defaultStatus
  }

  const mergeSlaveChildren = (
    stationId: string,
    slaveNode: DeviceNode,
    summary: PointTypeSummary,
    slaveId: number,
    slaveName: string,
    commonAddress: number,
  ) => {
    if (!slaveNode.children) slaveNode.children = []

    const { countByType, displayNameByType } = summary
    const existingChildMap = new Map(slaveNode.children.map((child) => [child.dataType!, child]))

    let changed = false
    const validTypeSet = new Set<string>()

    for (const [dataType, count] of Object.entries(countByType)) {
      const numericCount = Number(count)
      if (numericCount <= 0) continue
      validTypeSet.add(dataType)

      const nextLabel = normalizeIecObjectDisplayName(displayNameByType[dataType])
      const existing = existingChildMap.get(dataType)
      if (existing) {
        if (existing.count !== numericCount) {
          existing.count = numericCount
          changed = true
        }
        if (existing.label !== nextLabel) {
          existing.label = nextLabel
          changed = true
        }
      } else {
        slaveNode.children.push(
          buildTypeNode(
            stationId,
            slaveId,
            slaveName,
            commonAddress,
            dataType,
            nextLabel,
            numericCount,
          ),
        )
        changed = true
      }
    }

    const filteredChildren = slaveNode.children.filter(
      (child) => child.type === 'type-id' && validTypeSet.has(String(child.dataType || '')),
    )
    if (filteredChildren.length !== slaveNode.children.length) {
      slaveNode.children = filteredChildren
      changed = true
    }

    if (changed) {
      slaveNode.children.sort((left, right) =>
        compareIec104TypeForDisplay(String(left.dataType || ''), String(right.dataType || '')),
      )
    }
  }

  const syncSelectedSlaveReference = () => {
    const currentSelected = options.selectedSlave.value
    if (!currentSelected) return

    if (currentSelected.type === 'listener') {
      const updatedListener =
        options.deviceTreeData.value.find(
          (item) => item.type === 'listener' && item.id === currentSelected.id,
        ) ?? null
      if (updatedListener) {
        options.selectedSlave.value = updatedListener
      }
      return
    }

    if (currentSelected.type !== 'slave') return

    const selectedSlaveId = Number(currentSelected.slaveId)
    if (!Number.isFinite(selectedSlaveId)) return

    for (const listenerNode of options.deviceTreeData.value) {
      const updatedSlave = (listenerNode.children ?? []).find(
        (item) => item.type === 'slave' && Number(item.slaveId) === selectedSlaveId,
      )
      if (updatedSlave) {
        options.selectedSlave.value = updatedSlave
        return
      }
    }
  }

  const rebuildDeviceTreeFromBackend = () => {
    const stations = options.source.stations ?? []
    if (stations.length === 0) {
      options.deviceTreeData.value = []
      options.selectedSlave.value = null
      options.expandedNodeKeys.value = []
      options.hasAppliedInitialAutoExpand.value = false
      options.stationRuntimeStatusCache.value = {}
      return
    }

    const selectedStationId = String(options.source.stationId || '')
    const slavesByStation = options.source.slavesByStation ?? {}
    const slaveSummaryMap = options.source.slavePointTypeSummary ?? {}
    const stationIds = new Set(stations.map((station) => station.station_id))

    for (const id of Object.keys(options.stationRuntimeStatusCache.value)) {
      if (!stationIds.has(id)) delete options.stationRuntimeStatusCache.value[id]
    }

    options.expandedNodeKeys.value = syncExpandedTreeKeys(
      options.expandedNodeKeys.value,
      stationIds,
      {
        fallbackKey: selectedStationId,
        applyFallbackWhenEmpty: !options.hasAppliedInitialAutoExpand.value,
      },
    )
    if (!options.hasAppliedInitialAutoExpand.value) {
      options.hasAppliedInitialAutoExpand.value = true
    }

    const existingMap = new Map(options.deviceTreeData.value.map((node) => [node.id, node]))
    let structureChanged = false

    for (let index = options.deviceTreeData.value.length - 1; index >= 0; index -= 1) {
      if (!stationIds.has(options.deviceTreeData.value[index].id)) {
        options.deviceTreeData.value.splice(index, 1)
        structureChanged = true
      }
    }

    for (let index = 0; index < stations.length; index += 1) {
      const station = stations[index]
      const stationId = station.station_id
      const existing = existingMap.get(stationId)

      if (existing) {
        existing.label = `${station.name} (${station.host}:${station.port})`
        existing.name = station.name
        existing.stationId = stationId
        existing.status = resolveStationNodeStatus(stationId, station.status)
        existing.address = station.host
        existing.port = station.port
        existing.type = 'listener'
        if (!Array.isArray(existing.children)) existing.children = []

        const currentIndex = options.deviceTreeData.value.indexOf(existing)
        if (currentIndex !== -1 && currentIndex !== index) {
          options.deviceTreeData.value.splice(currentIndex, 1)
          options.deviceTreeData.value.splice(index, 0, existing)
        }
      } else {
        const nextNode: DeviceNode = {
          id: stationId,
          label: `${station.name} (${station.host}:${station.port})`,
          name: station.name,
          stationId,
          type: 'listener',
          status: resolveStationNodeStatus(stationId, station.status),
          address: station.host,
          port: station.port,
          children: [],
        }
        options.deviceTreeData.value.splice(index, 0, nextNode)
        structureChanged = true
      }
    }

    for (const station of stations) {
      const stationId = station.station_id
      const listenerNode =
        options.deviceTreeData.value.find(
          (node) => node.type === 'listener' && node.id === stationId,
        ) ?? null
      if (!listenerNode) continue
      if (!Array.isArray(listenerNode.children)) {
        listenerNode.children = []
      }

      const slaveRows = (slavesByStation[stationId] ?? [])
        .slice()
        .sort((left, right) => left.common_address - right.common_address)
      const validSlaveKeys = new Set(slaveRows.map((slave) => String(slave.id)))

      listenerNode.children = listenerNode.children.filter((child) => {
        if (child.type !== 'slave') return false
        return validSlaveKeys.has(String(child.slaveId ?? child.id))
      })

      for (let index = 0; index < slaveRows.length; index += 1) {
        const slaveRow = slaveRows[index]
        const slaveId = Number(slaveRow.id)
        const nodeId = `slave-${stationId}-${slaveId}`
        let slaveNode =
          listenerNode.children.find((child) => Number(child.slaveId) === slaveId) ?? null

        if (!slaveNode) {
          slaveNode = {
            id: nodeId,
            label: slaveRow.name,
            name: slaveRow.name,
            type: 'slave',
            stationId,
            parentStationId: stationId,
            slaveId,
            commonAddress: slaveRow.common_address,
            status: listenerNode.status,
            children: [],
          } as DeviceNode
          listenerNode.children.splice(index, 0, slaveNode)
          structureChanged = true
        } else {
          slaveNode.id = nodeId
          slaveNode.label = slaveRow.name
          slaveNode.name = slaveRow.name
          slaveNode.type = 'slave'
          slaveNode.stationId = stationId
          slaveNode.parentStationId = stationId
          slaveNode.slaveId = slaveId
          slaveNode.commonAddress = slaveRow.common_address
          slaveNode.status = listenerNode.status
          if (!Array.isArray(slaveNode.children)) slaveNode.children = []

          const currentIndex = listenerNode.children.indexOf(slaveNode)
          if (currentIndex !== index) {
            listenerNode.children.splice(currentIndex, 1)
            listenerNode.children.splice(index, 0, slaveNode)
            structureChanged = true
          }
        }

        const slaveSummary = slaveSummaryMap[slaveId] ?? createEmptyPointTypeSummary()
        if (Object.keys(slaveSummary.countByType).length === 0) {
          slaveNode.children = []
        } else {
          mergeSlaveChildren(
            stationId,
            slaveNode,
            slaveSummary,
            slaveId,
            slaveRow.name,
            slaveRow.common_address,
          )
        }
      }
    }

    options.deviceTreeData.value.sort((left, right) =>
      String(left.id).localeCompare(String(right.id), 'zh-CN', { numeric: true }),
    )

    syncSelectedSlaveReference()

    if (structureChanged) {
      nextTick(() => {
        replayExpandedTreeKeys(options.treeRef.value, options.expandedNodeKeys.value)
      })
    }
  }

  return {
    resolveStationNodeStatus,
    rebuildDeviceTreeFromBackend,
  }
}
