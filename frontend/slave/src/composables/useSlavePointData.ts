import { shallowRef } from 'vue'

import {
  categoryByTypeId,
  getIec104Capability,
  resolveIec104TypeId,
  resolveIec104TypeName,
} from '@shared/api/iec104'
import { PointDeltaStore, type AppliedPointDelta, type PointSyncBatch } from '@shared/api/pointSync'
import type { BackendDataPoint } from '@shared/api/types'

export type IndexedSlaveRuntimePoint = {
  id: string
  point: BackendDataPoint
  order: number
  typeId: number
  dataType: string
}

export type SlaveRuntimePointQuery = {
  typeId: number
  commonAddress?: number | null
  slaveId?: number | null
}

const INDEX_APPLY_FRAME_BUDGET_MS = 4
const scopeKey = (typeId: number, value: number): string => `${typeId}:${value}`
const mappingKey = (commonAddress: number | null, address: number): string =>
  `${commonAddress ?? -1}:${address}`

const normalizeCommonAddress = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  return normalized >= 1 && normalized <= 65535 ? normalized : null
}

const normalizeSlaveId = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  return normalized >= 1 ? normalized : null
}

const normalizeControlIoa = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  return normalized >= 1 ? normalized : null
}

function addIndex(index: Map<string | number, Set<string>>, key: string | number, id: string) {
  const bucket = index.get(key)
  if (bucket) bucket.add(id)
  else index.set(key, new Set([id]))
}

function removeIndex(index: Map<string | number, Set<string>>, key: string | number, id: string) {
  const bucket = index.get(key)
  if (!bucket) return
  bucket.delete(id)
  if (bucket.size === 0) index.delete(key)
}

export function useSlavePointData() {
  const store = new PointDeltaStore<BackendDataPoint>()
  const revision = shallowRef(0)
  const stationId = shallowRef('')
  const lastUpserts = shallowRef<BackendDataPoint[]>([])
  const lastDelta = shallowRef<AppliedPointDelta<BackendDataPoint> | null>(null)
  const singlePointCount = shallowRef(0)
  const measuredCount = shallowRef(0)
  const mappingRevision = shallowRef(0)
  const records = new Map<string, IndexedSlaveRuntimePoint>()
  const byType = new Map<number, Set<string>>()
  const byCommonAddress = new Map<string, Set<string>>()
  const bySlave = new Map<string, Set<string>>()
  const bySlaveWithoutCommonAddress = new Map<string, Set<string>>()
  const withoutSlave = new Map<number, Set<string>>()
  const tabQueryCache = new Map<string, IndexedSlaveRuntimePoint[]>()
  let reverseSourceAddresses = new Map<string, number[]>()
  let controlTypes = new Map<string, string[]>()
  let monitorTypes = new Map<string, string[]>()
  let pointTypeByAddress = new Map<string, string>()
  let nextOrder = 0
  let applyRevision = 0

  const addRecordToIndexes = (record: IndexedSlaveRuntimePoint) => {
    const commonAddress = normalizeCommonAddress(record.point.common_address)
    const slaveId = normalizeSlaveId(record.point.slave_id)
    addIndex(byType, record.typeId, record.id)
    if (commonAddress != null) {
      addIndex(byCommonAddress, scopeKey(record.typeId, commonAddress), record.id)
    } else if (slaveId != null) {
      addIndex(bySlaveWithoutCommonAddress, scopeKey(record.typeId, slaveId), record.id)
    }
    if (slaveId != null) addIndex(bySlave, scopeKey(record.typeId, slaveId), record.id)
    else addIndex(withoutSlave, record.typeId, record.id)
  }

  const removeRecordFromIndexes = (record: IndexedSlaveRuntimePoint) => {
    const commonAddress = normalizeCommonAddress(record.point.common_address)
    const slaveId = normalizeSlaveId(record.point.slave_id)
    removeIndex(byType, record.typeId, record.id)
    if (commonAddress != null) {
      removeIndex(byCommonAddress, scopeKey(record.typeId, commonAddress), record.id)
    } else if (slaveId != null) {
      removeIndex(bySlaveWithoutCommonAddress, scopeKey(record.typeId, slaveId), record.id)
    }
    if (slaveId != null) removeIndex(bySlave, scopeKey(record.typeId, slaveId), record.id)
    else removeIndex(withoutSlave, record.typeId, record.id)
  }

  const sameIndexIdentity = (
    previous: IndexedSlaveRuntimePoint,
    point: BackendDataPoint,
    typeId: number,
  ) =>
    previous.typeId === typeId &&
    normalizeCommonAddress(previous.point.common_address) ===
      normalizeCommonAddress(point.common_address) &&
    normalizeSlaveId(previous.point.slave_id) === normalizeSlaveId(point.slave_id)

  const sameMappingIdentity = (left: BackendDataPoint, right: BackendDataPoint) =>
    resolveIec104TypeId(left) === resolveIec104TypeId(right) &&
    normalizeCommonAddress(left.common_address) === normalizeCommonAddress(right.common_address) &&
    left.address === right.address &&
    normalizeControlIoa(left.control_ioa) === normalizeControlIoa(right.control_ioa)

  const rebuildCounts = async (
    yieldWhenNeeded: () => Promise<void> | null,
    shouldContinue: () => boolean,
  ) => {
    let single = 0
    let measured = 0
    for (const record of records.values()) {
      const category = categoryByTypeId(record.typeId)
      if (category === 'single_point') single += 1
      else if (
        category === 'measurement' ||
        category === 'step_position' ||
        category === 'bitstring'
      ) {
        measured += 1
      }
      const pendingFrame = yieldWhenNeeded()
      if (pendingFrame) await pendingFrame
      if (!shouldContinue()) return false
    }
    if (!shouldContinue()) return false
    singlePointCount.value = single
    measuredCount.value = measured
    return true
  }

  const rebuildMappingIndexes = async (
    yieldWhenNeeded: () => Promise<void> | null,
    shouldContinue: () => boolean,
  ) => {
    const reverseSets = new Map<string, Set<number>>()
    const controlSets = new Map<string, Set<string>>()
    const monitorSets = new Map<string, Set<string>>()
    const typesByAddress = new Map<string, string>()
    for (const record of records.values()) {
      const point = record.point
      const commonAddress = normalizeCommonAddress(point.common_address)
      const addressKey = mappingKey(commonAddress, point.address)
      if (!typesByAddress.has(addressKey)) typesByAddress.set(addressKey, record.dataType)
      const capability = getIec104Capability(record.typeId)
      if (capability?.point_role === 'control') {
        const set = controlSets.get(addressKey) ?? new Set<string>()
        set.add(record.dataType)
        controlSets.set(addressKey, set)
      }
      const target = normalizeControlIoa(point.control_ioa)
      if (target == null) continue
      const targetKey = mappingKey(commonAddress, target)
      const addresses = reverseSets.get(targetKey) ?? new Set<number>()
      addresses.add(point.address)
      reverseSets.set(targetKey, addresses)
      if (capability?.point_role === 'monitor') {
        const set = monitorSets.get(targetKey) ?? new Set<string>()
        set.add(record.dataType)
        monitorSets.set(targetKey, set)
      }
      const pendingFrame = yieldWhenNeeded()
      if (pendingFrame) await pendingFrame
      if (!shouldContinue()) return false
    }
    if (!shouldContinue()) return false
    reverseSourceAddresses = new Map(
      [...reverseSets].map(([key, values]) => [key, [...values].sort((a, b) => a - b)]),
    )
    const normalizeTypes = (source: Map<string, Set<string>>) =>
      new Map([...source].map(([key, values]) => [key, [...values].sort()]))
    controlTypes = normalizeTypes(controlSets)
    monitorTypes = normalizeTypes(monitorSets)
    pointTypeByAddress = typesByAddress
    mappingRevision.value += 1
    return true
  }

  const resetIndexes = () => {
    records.clear()
    byType.clear()
    byCommonAddress.clear()
    bySlave.clear()
    bySlaveWithoutCommonAddress.clear()
    withoutSlave.clear()
    tabQueryCache.clear()
    nextOrder = 0
  }

  const applySyncBatch = async (
    nextStationId: string,
    batch: PointSyncBatch<BackendDataPoint>,
  ): Promise<AppliedPointDelta<BackendDataPoint> | null> => {
    if (stationId.value !== nextStationId) {
      clear(nextStationId)
    }
    const operationRevision = ++applyRevision
    const delta = await store.apply(batch)
    if (!delta || operationRevision !== applyRevision) return null
    let frameStartedAt = performance.now()
    const yieldWhenNeeded = (): Promise<void> | null => {
      if (performance.now() - frameStartedAt < INDEX_APPLY_FRAME_BUDGET_MS) return null
      return new Promise<void>((resolve) => requestAnimationFrame(() => resolve())).then(() => {
        frameStartedAt = performance.now()
      })
    }
    let membershipChanged = batch.reset
    let mappingChanged = batch.reset
    if (batch.reset) resetIndexes()
    else {
      for (const removal of delta.removed) {
        const previous = records.get(removal.id)
        if (!previous) continue
        removeRecordFromIndexes(previous)
        records.delete(removal.id)
        membershipChanged = true
        mappingChanged = true
        const pendingFrame = yieldWhenNeeded()
        if (pendingFrame) await pendingFrame
        if (operationRevision !== applyRevision) return null
      }
    }

    for (const upsert of delta.upserts) {
      const typeId = resolveIec104TypeId(upsert.point)
      const previous = records.get(upsert.id)
      if (previous && sameIndexIdentity(previous, upsert.point, typeId)) {
        if (!sameMappingIdentity(previous.point, upsert.point)) mappingChanged = true
        previous.point = upsert.point
        previous.dataType = resolveIec104TypeName(upsert.point)
        const pendingFrame = yieldWhenNeeded()
        if (pendingFrame) await pendingFrame
        if (operationRevision !== applyRevision) return null
        continue
      }
      if (previous) removeRecordFromIndexes(previous)
      const record: IndexedSlaveRuntimePoint = {
        id: upsert.id,
        point: upsert.point,
        order: previous?.order ?? nextOrder++,
        typeId,
        dataType: resolveIec104TypeName(upsert.point),
      }
      records.set(upsert.id, record)
      addRecordToIndexes(record)
      membershipChanged = true
      mappingChanged = true
      const pendingFrame = yieldWhenNeeded()
      if (pendingFrame) await pendingFrame
      if (operationRevision !== applyRevision) return null
    }

    if (membershipChanged) {
      tabQueryCache.clear()
      if (!(await rebuildCounts(yieldWhenNeeded, () => operationRevision === applyRevision))) {
        return null
      }
    }
    if (mappingChanged) {
      if (
        !(await rebuildMappingIndexes(yieldWhenNeeded, () => operationRevision === applyRevision))
      ) {
        return null
      }
    }
    lastUpserts.value = delta.upserts.map((entry) => entry.point)
    lastDelta.value = delta
    if (delta.reset || delta.upserts.length > 0 || delta.removed.length > 0) {
      revision.value += 1
    }
    return delta
  }

  function clear(nextStationId = ''): void {
    applyRevision += 1
    store.clear()
    resetIndexes()
    stationId.value = nextStationId
    lastUpserts.value = []
    lastDelta.value = null
    singlePointCount.value = 0
    measuredCount.value = 0
    reverseSourceAddresses = new Map()
    controlTypes = new Map()
    monitorTypes = new Map()
    pointTypeByAddress = new Map()
    mappingRevision.value += 1
    revision.value += 1
  }

  const idsToRecords = (ids: Iterable<string>): IndexedSlaveRuntimePoint[] =>
    [...ids]
      .map((id) => records.get(id))
      .filter((record): record is IndexedSlaveRuntimePoint => record != null)

  const getPointsForTab = (query: SlaveRuntimePointQuery): IndexedSlaveRuntimePoint[] => {
    const commonAddress = normalizeCommonAddress(query.commonAddress)
    const slaveId = normalizeSlaveId(query.slaveId)
    const queryKey = `${query.typeId}:${commonAddress ?? ''}:${slaveId ?? ''}`
    const cached = tabQueryCache.get(queryKey)
    if (cached) return cached
    let result: IndexedSlaveRuntimePoint[]
    if (commonAddress != null) {
      const ids = new Set(byCommonAddress.get(scopeKey(query.typeId, commonAddress)) ?? [])
      if (slaveId != null) {
        for (const id of bySlaveWithoutCommonAddress.get(scopeKey(query.typeId, slaveId)) ?? []) {
          ids.add(id)
        }
      }
      result = idsToRecords(ids)
    } else if (slaveId != null) {
      const ids = new Set(bySlave.get(scopeKey(query.typeId, slaveId)) ?? [])
      for (const id of withoutSlave.get(query.typeId) ?? []) ids.add(id)
      result = idsToRecords(ids)
    } else {
      result = idsToRecords(byType.get(query.typeId) ?? [])
    }
    tabQueryCache.set(queryKey, result)
    return result
  }

  const filterPoints = (predicate: (point: BackendDataPoint) => boolean): BackendDataPoint[] => {
    const result: BackendDataPoint[] = []
    for (const record of records.values()) {
      if (predicate(record.point)) result.push(record.point)
    }
    return result
  }

  return {
    cursor: () => store.cursor,
    revision,
    size: store.size,
    stationId,
    lastUpserts,
    lastDelta,
    singlePointCount,
    measuredCount,
    mappingRevision,
    applySyncBatch,
    clear,
    getPointsForTab,
    filterPoints,
    getReverseSourceAddresses: (commonAddress: number | null, address: number) =>
      reverseSourceAddresses.get(mappingKey(commonAddress, address)) ?? [],
    getControlTypes: (commonAddress: number | null, address: number) =>
      controlTypes.get(mappingKey(commonAddress, address)) ?? [],
    getMonitorTypes: (commonAddress: number | null, address: number) =>
      monitorTypes.get(mappingKey(commonAddress, address)) ?? [],
    getPointType: (commonAddress: number | null, address: number) =>
      pointTypeByAddress.get(mappingKey(commonAddress, address)),
  }
}

export type SlavePointData = ReturnType<typeof useSlavePointData>
