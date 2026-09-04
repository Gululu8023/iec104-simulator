import type { Ref } from 'vue'

import type { SlaveResponse } from '@shared/api/types'

import type { DeviceNode } from '../types/slave'

interface DeviceTreeExpose {
  deviceTreeData?: DeviceNode[]
}

interface UseSlaveSelectionOptions {
  currentStationId: Ref<string>
  deviceTreeRef: Ref<DeviceTreeExpose | undefined>
  selectedSlave: Ref<DeviceNode | null>
  selectedSlaveId: Ref<number | null>
  slavesByStation: Ref<Record<string, SlaveResponse[]>>
}

export function useSlaveSelection(options: UseSlaveSelectionOptions) {
  const getTreeNodes = (): DeviceNode[] =>
    (options.deviceTreeRef.value?.deviceTreeData ?? []) as DeviceNode[]

  const clearSelection = () => {
    options.selectedSlave.value = null
    options.selectedSlaveId.value = null
  }

  const resolveStationIdFromNode = (node: DeviceNode | null | undefined): string => {
    if (!node) return ''
    if (node.type === 'listener') return node.id
    if (node.type === 'slave') return String(node.parentStationId || node.stationId || '')
    if (node.parentStationId) return node.parentStationId

    for (const listenerNode of getTreeNodes()) {
      if (listenerNode.type !== 'listener' || !Array.isArray(listenerNode.children)) continue
      for (const slaveNode of listenerNode.children) {
        if (slaveNode.id === node.id) return listenerNode.id
        if (
          Array.isArray(slaveNode.children) &&
          slaveNode.children.some((child) => child.id === node.id)
        ) {
          return listenerNode.id
        }
      }
    }
    return ''
  }

  const resolveSlaveIdFromNode = (node: DeviceNode | null | undefined): number | null => {
    if (!node) return null
    if (node.slaveId != null) {
      const numericId = Number(node.slaveId)
      return Number.isFinite(numericId) && numericId > 0 ? numericId : null
    }
    if (node.type === 'slave' && node.id) {
      const numericId = Number(node.id)
      return Number.isFinite(numericId) && numericId > 0 ? numericId : null
    }
    if (node.parentSlaveId) {
      const numericId = Number(node.parentSlaveId)
      return Number.isFinite(numericId) && numericId > 0 ? numericId : null
    }
    return null
  }

  const findLatestListenerNode = (stationId: string): DeviceNode | null => {
    if (!stationId) return null
    return getTreeNodes().find((node) => node.type === 'listener' && node.id === stationId) ?? null
  }

  const findLatestSlaveNode = (slaveId: number, stationId?: string): DeviceNode | null => {
    if (!Number.isFinite(slaveId)) return null
    const numericId = Number(slaveId)
    const listeners = stationId
      ? getTreeNodes().filter((node) => node.type === 'listener' && node.id === stationId)
      : getTreeNodes().filter((node) => node.type === 'listener')

    for (const listener of listeners) {
      for (const child of listener.children ?? []) {
        if (child.type !== 'slave') continue
        if (Number(child.slaveId) === numericId || Number(child.id) === numericId) {
          return child
        }
      }
    }
    return null
  }

  const resolveLatestSlaveNode = (node: DeviceNode | null | undefined): DeviceNode | null => {
    if (node?.type === 'slave') {
      const slaveId = resolveSlaveIdFromNode(node)
      if (slaveId != null) {
        return findLatestSlaveNode(slaveId, resolveStationIdFromNode(node)) ?? node
      }
      return node
    }

    const stationId = resolveStationIdFromNode(node) || options.currentStationId.value
    if (!stationId) return null
    return findLatestListenerNode(stationId) ?? (node?.type === 'listener' ? node : null)
  }

  const selectNode = (node: DeviceNode) => {
    options.selectedSlave.value = node
    const slaveId = resolveSlaveIdFromNode(node)
    if (slaveId != null) {
      options.selectedSlaveId.value = slaveId
    }
    return {
      stationId: resolveStationIdFromNode(node),
      slaveId,
    }
  }

  const syncUiForCurrentStation = () => {
    if (!options.currentStationId.value) {
      clearSelection()
      return
    }

    const selectedStationId = options.currentStationId.value
    const slaveCandidates = options.slavesByStation.value[selectedStationId] ?? []
    const hasValidSelectedSlaveId =
      options.selectedSlaveId.value != null &&
      slaveCandidates.some((item) => item.id === options.selectedSlaveId.value)
    if (!hasValidSelectedSlaveId) {
      options.selectedSlaveId.value = null
    }

    const latestListener = findLatestListenerNode(selectedStationId)
    const shouldKeepListenerSelected =
      options.selectedSlave.value?.type === 'listener' &&
      options.selectedSlave.value.id === selectedStationId
    if (shouldKeepListenerSelected && latestListener) {
      options.selectedSlave.value = latestListener
      return
    }

    const latestSlaveNode =
      options.selectedSlaveId.value != null
        ? findLatestSlaveNode(options.selectedSlaveId.value, selectedStationId)
        : null
    if (latestSlaveNode) {
      options.selectedSlave.value = latestSlaveNode
      return
    }

    options.selectedSlave.value = null
  }

  return {
    clearSelection,
    selectNode,
    resolveStationIdFromNode,
    resolveSlaveIdFromNode,
    findLatestListenerNode,
    findLatestSlaveNode,
    resolveLatestSlaveNode,
    syncUiForCurrentStation,
  }
}
