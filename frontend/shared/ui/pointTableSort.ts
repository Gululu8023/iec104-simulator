export type PointTableSortProp = 'address' | 'name' | 'quality' | 'reportCount' | 'timestamp'

export type PointTableSortOrder = 'ascending' | 'descending'

type PointTableSortableRow = {
  address: number
  name?: string
  quality?: string
  reportCount?: number | null
  timestamp?: string | number | null
}

const qualityRank: Record<string, number> = {
  invalid: 0,
  questionable: 1,
  good: 2,
}

const normalizeTimestamp = (value: string | number | null | undefined): number | null => {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null
  if (!value) return null
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : null
}

export const getPointTableSortValue = (
  row: PointTableSortableRow,
  prop: PointTableSortProp,
): string | number | null => {
  switch (prop) {
    case 'address':
      return Number(row.address)
    case 'name':
      return String(row.name ?? '')
    case 'quality':
      return qualityRank[String(row.quality ?? '')] ?? Number.MAX_SAFE_INTEGER
    case 'reportCount': {
      const value = Number(row.reportCount)
      return row.reportCount == null || !Number.isFinite(value) ? null : value
    }
    case 'timestamp':
      return normalizeTimestamp(row.timestamp)
  }
}

const compareSortValues = (left: string | number | null, right: string | number | null): number => {
  if (left == null) return right == null ? 0 : 1
  if (right == null) return -1
  if (typeof left === 'string' && typeof right === 'string') {
    return left.localeCompare(right, undefined, { numeric: true, sensitivity: 'base' })
  }
  return Number(left) - Number(right)
}

export const sortPointTableRows = <T extends PointTableSortableRow>(
  rows: readonly T[],
  prop: PointTableSortProp,
  order: PointTableSortOrder,
): T[] => {
  const direction = order === 'descending' ? -1 : 1
  return [...rows].sort((left, right) => {
    const compared = compareSortValues(
      getPointTableSortValue(left, prop),
      getPointTableSortValue(right, prop),
    )
    return compared === 0 ? left.address - right.address : compared * direction
  })
}

export const sortPointTableRowsInFrames = async <T extends PointTableSortableRow>(
  rows: readonly T[],
  prop: PointTableSortProp,
  order: PointTableSortOrder,
  shouldContinue: () => boolean,
  frameBudgetMs = 4,
): Promise<T[] | null> => {
  if (rows.length < 2) return [...rows]

  type SortEntry = { row: T; value: string | number | null; index: number }
  const direction = order === 'descending' ? -1 : 1
  const compareEntries = (left: SortEntry, right: SortEntry) => {
    const compared = compareSortValues(left.value, right.value)
    if (compared !== 0) return compared * direction
    const addressCompared = left.row.address - right.row.address
    return addressCompared !== 0 ? addressCompared : left.index - right.index
  }
  let frameStartedAt = performance.now()
  const yieldWhenNeeded = async () => {
    if (performance.now() - frameStartedAt < frameBudgetMs) return true
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    frameStartedAt = performance.now()
    return shouldContinue()
  }

  let source: SortEntry[] = []
  for (let index = 0; index < rows.length; index += 1) {
    source.push({ row: rows[index], value: getPointTableSortValue(rows[index], prop), index })
    if (performance.now() - frameStartedAt >= frameBudgetMs && !(await yieldWhenNeeded())) {
      return null
    }
  }

  let target = new Array<SortEntry>(source.length)
  for (let width = 1; width < source.length; width *= 2) {
    for (let start = 0; start < source.length; start += width * 2) {
      const middle = Math.min(start + width, source.length)
      const end = Math.min(start + width * 2, source.length)
      let left = start
      let right = middle
      let output = start
      while (left < middle || right < end) {
        if (right >= end || (left < middle && compareEntries(source[left], source[right]) <= 0)) {
          target[output++] = source[left++]
        } else {
          target[output++] = source[right++]
        }
        if (performance.now() - frameStartedAt >= frameBudgetMs && !(await yieldWhenNeeded())) {
          return null
        }
      }
    }
    const previous = source
    source = target
    target = previous
  }

  const result: T[] = []
  for (const entry of source) {
    result.push(entry.row)
    if (performance.now() - frameStartedAt >= frameBudgetMs && !(await yieldWhenNeeded())) {
      return null
    }
  }
  return result
}
