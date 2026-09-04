import { shallowRef } from 'vue'

import { resolveIec104TypeId, resolveIec104TypeName } from '@shared/api/iec104'
import { PointDeltaStore, type AppliedPointDelta, type PointSyncBatch } from '@shared/api/pointSync'
import type { BackendDataPoint, PointDef } from '@shared/api/types'

export type IndexedMasterRuntimePoint = {
  id: string
  point: BackendDataPoint
  order: number
  typeId: number
  dataType: string
}

export type IndexedMasterPointDef = {
  pointDef: PointDef
  order: number
  typeId: number
  dataType: string
}

export type MasterRuntimePointQuery = {
  typeIds: readonly number[]
  slaveId?: string | number | null
  linkProfileId?: string | number | null
  connectionId?: string | null
}

const INDEX_APPLY_FRAME_BUDGET_MS = 4
const indexKey = (typeId: number, identity: string | number): string => `${typeId}:${identity}`

function addIndexEntry(index: Map<string, Set<string>>, key: string, id: string): void {
  const bucket = index.get(key)
  if (bucket) bucket.add(id)
  else index.set(key, new Set([id]))
}

function removeIndexEntry(index: Map<string, Set<string>>, key: string, id: string): void {
  const bucket = index.get(key)
  if (!bucket) return
  bucket.delete(id)
  if (bucket.size === 0) index.delete(key)
}

function appendPointDefIndex(
  index: Map<number, IndexedMasterPointDef[]>,
  key: number,
  entry: IndexedMasterPointDef,
): void {
  const bucket = index.get(key)
  if (bucket) bucket.push(entry)
  else index.set(key, [entry])
}

export function useMasterPointData() {
  const store = new PointDeltaStore<BackendDataPoint>()
  const revision = shallowRef(0)
  const summaryRevision = shallowRef(0)
  const lastUpserts = shallowRef<BackendDataPoint[]>([])
  const lastDelta = shallowRef<AppliedPointDelta<BackendDataPoint> | null>(null)
  const records = new Map<string, IndexedMasterRuntimePoint>()
  const bySlave = new Map<string, Set<string>>()
  const byLink = new Map<string, Set<string>>()
  const byConnection = new Map<string, Set<string>>()
  const bySlaveAddress = new Map<string, Set<string>>()
  const byLinkAddress = new Map<string, Set<string>>()
  const byConnectionAddress = new Map<string, Set<string>>()
  const runtimeQueryCache = new Map<string, IndexedMasterRuntimePoint[]>()
  const knownTypeIds = new Set<number>()
  const typeCounts = new Map<number, number>()
  const profilePointDefSources = new Map<string, PointDef[]>()
  const profilePointDefsByProfile = new Map<string, Map<number, IndexedMasterPointDef[]>>()
  const profilePointDefsByAddress = new Map<string, Map<number, IndexedMasterPointDef>>()
  const profilesWithImportedPointDefs = new Set<string>()
  const builtInPointDefTypeIdsByProfile = new Map<string, Set<number>>()
  let nextOrder = 0
  let applyRevision = 0

  const addRecordToIndexes = (record: IndexedMasterRuntimePoint) => {
    const point = record.point
    addIndexEntry(bySlave, indexKey(record.typeId, Number(point.slave_id ?? 0)), record.id)
    addIndexEntry(byLink, indexKey(record.typeId, Number(point.link_profile_id ?? 0)), record.id)
    addIndexEntry(
      byConnection,
      indexKey(record.typeId, String(point.connection_id ?? '')),
      record.id,
    )
    addIndexEntry(
      bySlaveAddress,
      `${indexKey(record.typeId, Number(point.slave_id ?? 0))}:${point.address}`,
      record.id,
    )
    addIndexEntry(
      byLinkAddress,
      `${indexKey(record.typeId, Number(point.link_profile_id ?? 0))}:${point.address}`,
      record.id,
    )
    addIndexEntry(
      byConnectionAddress,
      `${indexKey(record.typeId, String(point.connection_id ?? ''))}:${point.address}`,
      record.id,
    )
    typeCounts.set(record.typeId, (typeCounts.get(record.typeId) ?? 0) + 1)
    knownTypeIds.add(record.typeId)
  }

  const removeRecordFromIndexes = (record: IndexedMasterRuntimePoint) => {
    const point = record.point
    removeIndexEntry(bySlave, indexKey(record.typeId, Number(point.slave_id ?? 0)), record.id)
    removeIndexEntry(byLink, indexKey(record.typeId, Number(point.link_profile_id ?? 0)), record.id)
    removeIndexEntry(
      byConnection,
      indexKey(record.typeId, String(point.connection_id ?? '')),
      record.id,
    )
    removeIndexEntry(
      bySlaveAddress,
      `${indexKey(record.typeId, Number(point.slave_id ?? 0))}:${point.address}`,
      record.id,
    )
    removeIndexEntry(
      byLinkAddress,
      `${indexKey(record.typeId, Number(point.link_profile_id ?? 0))}:${point.address}`,
      record.id,
    )
    removeIndexEntry(
      byConnectionAddress,
      `${indexKey(record.typeId, String(point.connection_id ?? ''))}:${point.address}`,
      record.id,
    )
    const nextCount = (typeCounts.get(record.typeId) ?? 1) - 1
    if (nextCount > 0) typeCounts.set(record.typeId, nextCount)
    else {
      typeCounts.delete(record.typeId)
      knownTypeIds.delete(record.typeId)
    }
  }

  const sameIndexIdentity = (
    previous: IndexedMasterRuntimePoint,
    point: BackendDataPoint,
    typeId: number,
  ) =>
    previous.typeId === typeId &&
    Number(previous.point.slave_id ?? 0) === Number(point.slave_id ?? 0) &&
    Number(previous.point.link_profile_id ?? 0) === Number(point.link_profile_id ?? 0) &&
    String(previous.point.connection_id ?? '') === String(point.connection_id ?? '')

  const applySyncBatch = async (
    batch: PointSyncBatch<BackendDataPoint>,
  ): Promise<AppliedPointDelta<BackendDataPoint> | null> => {
    const operationRevision = ++applyRevision
    const delta = await store.apply(batch)
    if (!delta || operationRevision !== applyRevision) return null
    let membershipChanged = batch.reset
    let frameStartedAt = performance.now()
    const yieldWhenNeeded = (): Promise<void> | null => {
      if (performance.now() - frameStartedAt < INDEX_APPLY_FRAME_BUDGET_MS) return null
      return new Promise<void>((resolve) => requestAnimationFrame(() => resolve())).then(() => {
        frameStartedAt = performance.now()
      })
    }

    if (batch.reset) {
      records.clear()
      bySlave.clear()
      byLink.clear()
      byConnection.clear()
      bySlaveAddress.clear()
      byLinkAddress.clear()
      byConnectionAddress.clear()
      runtimeQueryCache.clear()
      knownTypeIds.clear()
      typeCounts.clear()
      nextOrder = 0
    } else {
      for (const removal of delta.removed) {
        const previous = records.get(removal.id)
        if (!previous) continue
        removeRecordFromIndexes(previous)
        records.delete(removal.id)
        membershipChanged = true
        const pendingFrame = yieldWhenNeeded()
        if (pendingFrame) await pendingFrame
        if (operationRevision !== applyRevision) return null
      }
    }

    for (const upsert of delta.upserts) {
      const typeId = resolveIec104TypeId(upsert.point)
      const previous = records.get(upsert.id)
      if (previous && sameIndexIdentity(previous, upsert.point, typeId)) {
        previous.point = upsert.point
        previous.dataType = resolveIec104TypeName(upsert.point)
        const pendingFrame = yieldWhenNeeded()
        if (pendingFrame) await pendingFrame
        if (operationRevision !== applyRevision) return null
        continue
      }
      if (previous) removeRecordFromIndexes(previous)
      const record: IndexedMasterRuntimePoint = {
        id: upsert.id,
        point: upsert.point,
        order: previous?.order ?? nextOrder++,
        typeId,
        dataType: resolveIec104TypeName(upsert.point),
      }
      records.set(upsert.id, record)
      addRecordToIndexes(record)
      membershipChanged = true
      const pendingFrame = yieldWhenNeeded()
      if (pendingFrame) await pendingFrame
      if (operationRevision !== applyRevision) return null
    }

    if (membershipChanged) {
      runtimeQueryCache.clear()
      summaryRevision.value += 1
    }
    lastUpserts.value = delta.upserts.map((entry) => entry.point)
    lastDelta.value = delta
    if (delta.reset || delta.upserts.length > 0 || delta.removed.length > 0) {
      revision.value += 1
    }
    return delta
  }

  const clear = () => {
    applyRevision += 1
    store.clear()
    records.clear()
    bySlave.clear()
    byLink.clear()
    byConnection.clear()
    bySlaveAddress.clear()
    byLinkAddress.clear()
    byConnectionAddress.clear()
    runtimeQueryCache.clear()
    knownTypeIds.clear()
    typeCounts.clear()
    lastUpserts.value = []
    lastDelta.value = null
    nextOrder = 0
    summaryRevision.value += 1
    revision.value += 1
  }

  const getRuntimePoints = (query: MasterRuntimePointQuery): IndexedMasterRuntimePoint[] => {
    const queryKey = `${query.typeIds.join(',')}|${query.slaveId ?? ''}|${query.linkProfileId ?? ''}|${query.connectionId ?? ''}`
    const cached = runtimeQueryCache.get(queryKey)
    if (cached) return cached
    const ids = new Set<string>()
    for (const typeId of query.typeIds) {
      const buckets = [
        query.slaveId == null ? undefined : bySlave.get(indexKey(typeId, Number(query.slaveId))),
        query.linkProfileId == null
          ? undefined
          : byLink.get(indexKey(typeId, Number(query.linkProfileId))),
        query.connectionId == null
          ? undefined
          : byConnection.get(indexKey(typeId, String(query.connectionId))),
      ]
      for (const bucket of buckets) {
        if (!bucket) continue
        for (const id of bucket) ids.add(id)
      }
    }
    const result = [...ids]
      .map((id) => records.get(id))
      .filter((entry): entry is IndexedMasterRuntimePoint => entry != null)
    runtimeQueryCache.set(queryKey, result)
    return result
  }

  const getTypeCounts = (
    slaveId: string | number | null | undefined,
    connectionId: string | null | undefined,
  ): Record<string, number> => {
    const result: Record<string, number> = {}
    for (const typeId of knownTypeIds) {
      const ids = new Set<string>()
      if (slaveId != null) {
        for (const id of bySlave.get(indexKey(typeId, Number(slaveId))) ?? []) ids.add(id)
      }
      if (connectionId != null) {
        for (const id of byConnection.get(indexKey(typeId, String(connectionId))) ?? []) {
          ids.add(id)
        }
      }
      if (ids.size === 0) continue
      const first = records.get(ids.values().next().value as string)
      if (first) result[first.dataType] = ids.size
    }
    return result
  }

  const getRuntimePointAt = (
    query: MasterRuntimePointQuery,
    address: number,
  ): IndexedMasterRuntimePoint | null => {
    let matched: IndexedMasterRuntimePoint | null = null
    for (const typeId of query.typeIds) {
      const buckets = [
        query.slaveId == null
          ? undefined
          : bySlaveAddress.get(`${indexKey(typeId, Number(query.slaveId))}:${address}`),
        query.linkProfileId == null
          ? undefined
          : byLinkAddress.get(`${indexKey(typeId, Number(query.linkProfileId))}:${address}`),
        query.connectionId == null
          ? undefined
          : byConnectionAddress.get(`${indexKey(typeId, String(query.connectionId))}:${address}`),
      ]
      for (const bucket of buckets) {
        if (!bucket) continue
        for (const id of bucket) {
          const record = records.get(id)
          if (record && (matched == null || record.order > matched.order)) matched = record
        }
      }
    }
    return matched
  }

  const updateProfilePointDefs = (source: Record<string, PointDef[]>) => {
    for (const profileId of profilePointDefSources.keys()) {
      if (profileId in source) continue
      profilePointDefSources.delete(profileId)
      profilePointDefsByProfile.delete(profileId)
      profilePointDefsByAddress.delete(profileId)
      profilesWithImportedPointDefs.delete(profileId)
      builtInPointDefTypeIdsByProfile.delete(profileId)
    }
    for (const [profileId, pointDefs] of Object.entries(source)) {
      if (profilePointDefSources.get(profileId) === pointDefs) continue
      profilePointDefSources.set(profileId, pointDefs)
      const byType = new Map<number, IndexedMasterPointDef[]>()
      const byAddress = new Map<number, IndexedMasterPointDef>()
      const builtInTypeIds = new Set<number>()
      let hasImported = false
      pointDefs.forEach((pointDef, order) => {
        const entry: IndexedMasterPointDef = {
          pointDef,
          order,
          typeId: resolveIec104TypeId(pointDef),
          dataType: resolveIec104TypeName(pointDef),
        }
        appendPointDefIndex(byType, entry.typeId, entry)
        byAddress.set(pointDef.address, entry)
        if (pointDef.source === 'imported' || pointDef.source === 'manual') hasImported = true
        if (pointDef.source === 'built_in') builtInTypeIds.add(entry.typeId)
      })
      profilePointDefsByProfile.set(profileId, byType)
      profilePointDefsByAddress.set(profileId, byAddress)
      if (hasImported) profilesWithImportedPointDefs.add(profileId)
      else profilesWithImportedPointDefs.delete(profileId)
      builtInPointDefTypeIdsByProfile.set(profileId, builtInTypeIds)
    }
  }

  const getProfilePointDefs = (
    profileId: string | number,
    typeIds: readonly number[],
  ): IndexedMasterPointDef[] => {
    const candidates: IndexedMasterPointDef[] = []
    const byType = profilePointDefsByProfile.get(String(profileId))
    for (const typeId of typeIds) candidates.push(...(byType?.get(typeId) ?? []))
    return candidates
  }

  const getProfilePointDefAt = (
    profileId: string | number,
    typeIds: ReadonlySet<number>,
    address: number,
  ): IndexedMasterPointDef | null => {
    const pointDef = profilePointDefsByAddress.get(String(profileId))?.get(address)
    return pointDef && typeIds.has(pointDef.typeId) ? pointDef : null
  }

  const findPoint = (predicate: (point: BackendDataPoint) => boolean): BackendDataPoint | null => {
    for (const record of records.values()) {
      if (predicate(record.point)) return record.point
    }
    return null
  }

  return {
    cursor: () => store.cursor,
    revision,
    size: store.size,
    summaryRevision,
    lastUpserts,
    lastDelta,
    applySyncBatch,
    clear,
    getRuntimePoints,
    getRuntimePointAt,
    updateProfilePointDefs,
    getProfilePointDefs,
    getProfilePointDefAt,
    profileHasImportedPointDefs: (profileId: string | number) =>
      profilesWithImportedPointDefs.has(String(profileId)),
    profileHasBuiltInPointDefs: (profileId: string | number, typeIds: readonly number[]) => {
      const builtInTypeIds = builtInPointDefTypeIdsByProfile.get(String(profileId))
      return typeIds.some((typeId) => builtInTypeIds?.has(typeId))
    },
    getTypeCounts,
    findPoint,
  }
}

export type MasterPointData = ReturnType<typeof useMasterPointData>
