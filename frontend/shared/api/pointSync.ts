import { shallowRef, type ShallowRef } from 'vue'

export type PointSyncScopeKind = 'configured_slave' | 'live_connection' | 'slave_common_address'

export interface PointSyncCursor {
  epoch: number
  revision: number
}

export interface PointSyncKey {
  scope_kind: PointSyncScopeKind
  scope_id: string
  common_address: number
  address: number
}

export interface PointSyncItem<T> {
  key: PointSyncKey
  point: T
}

export interface PointSyncBatch<T> {
  cursor: PointSyncCursor
  reset: boolean
  upserts: PointSyncItem<T>[]
  removed: PointSyncKey[]
}

export interface AppliedPointUpsert<T> {
  id: string
  key: PointSyncKey
  previous: T | undefined
  point: T
}

export interface AppliedPointRemoval<T> {
  id: string
  key: PointSyncKey
  previous: T | undefined
}

export interface AppliedPointDelta<T> {
  reset: boolean
  upserts: AppliedPointUpsert<T>[]
  removed: AppliedPointRemoval<T>[]
}

const APPLY_FRAME_BUDGET_MS = 4

export function pointSyncKeyId(key: PointSyncKey): string {
  return `${key.scope_kind}:${key.scope_id}:${key.common_address}:${key.address}`
}

export function hasReachedPointSyncCursor(
  current: PointSyncCursor | null,
  target: PointSyncCursor | null | undefined,
): boolean {
  return target == null || (current?.epoch === target.epoch && current.revision >= target.revision)
}

const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))

export class PointDeltaStore<T> {
  readonly revision: ShallowRef<number> = shallowRef(0)
  readonly size: ShallowRef<number> = shallowRef(0)

  private points = new Map<string, PointSyncItem<T>>()
  private currentCursor: PointSyncCursor | null = null
  private operationRevision = 0

  get cursor(): PointSyncCursor | null {
    return this.currentCursor
  }

  get(id: string): T | undefined {
    return this.points.get(id)?.point
  }

  getItem(id: string): PointSyncItem<T> | undefined {
    return this.points.get(id)
  }

  values(): IterableIterator<PointSyncItem<T>> {
    return this.points.values()
  }

  clear(): void {
    this.operationRevision += 1
    if (this.points.size === 0 && this.currentCursor == null) return
    this.points.clear()
    this.currentCursor = null
    this.size.value = 0
    this.revision.value += 1
  }

  async apply(batch: PointSyncBatch<T>): Promise<AppliedPointDelta<T> | null> {
    const operationRevision = ++this.operationRevision
    if (!batch.reset && batch.upserts.length === 0 && batch.removed.length === 0) {
      this.currentCursor = batch.cursor
      return { reset: false, upserts: [], removed: [] }
    }
    const nextPoints = batch.reset ? new Map<string, PointSyncItem<T>>() : this.points
    const upserts: AppliedPointUpsert<T>[] = []
    const removed: AppliedPointRemoval<T>[] = []
    const upsertIds = batch.reset ? new Set<string>() : null
    let frameStartedAt = performance.now()

    const yieldWhenNeeded = async () => {
      await nextFrame()
      frameStartedAt = performance.now()
      return operationRevision === this.operationRevision
    }

    if (batch.reset) {
      for (const previous of this.points.values()) {
        const id = pointSyncKeyId(previous.key)
        removed.push({ id, key: previous.key, previous: previous.point })
        if (performance.now() - frameStartedAt >= APPLY_FRAME_BUDGET_MS) {
          if (!(await yieldWhenNeeded())) return null
        }
      }
    }

    for (const item of batch.upserts) {
      const id = pointSyncKeyId(item.key)
      const previous = this.points.get(id)?.point
      nextPoints.set(id, item)
      upserts.push({ id, key: item.key, previous, point: item.point })
      upsertIds?.add(id)
      if (performance.now() - frameStartedAt >= APPLY_FRAME_BUDGET_MS) {
        if (!(await yieldWhenNeeded())) return null
      }
    }

    if (!batch.reset) {
      for (const key of batch.removed) {
        const id = pointSyncKeyId(key)
        const previous = nextPoints.get(id)?.point
        nextPoints.delete(id)
        removed.push({ id, key, previous })
        if (performance.now() - frameStartedAt >= APPLY_FRAME_BUDGET_MS) {
          if (!(await yieldWhenNeeded())) return null
        }
      }
    } else {
      for (let index = removed.length - 1; index >= 0; index -= 1) {
        if (upsertIds?.has(removed[index].id)) removed.splice(index, 1)
        if (performance.now() - frameStartedAt >= APPLY_FRAME_BUDGET_MS) {
          if (!(await yieldWhenNeeded())) return null
        }
      }
    }

    if (operationRevision !== this.operationRevision) return null
    this.points = nextPoints
    this.currentCursor = batch.cursor
    this.size.value = nextPoints.size
    this.revision.value += 1
    return { reset: batch.reset, upserts, removed }
  }
}
