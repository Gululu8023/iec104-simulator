import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from 'vue'

export interface VirtualSpacerRow {
  __virtualSpacer: true
  __virtualKey: string
  __virtualHeight: number
}

export type VirtualTableRow<T> = T | VirtualSpacerRow

interface UseTableVirtualScrollOptions<T> {
  tableRef: Ref<any>
  sourceRows: ComputedRef<T[]>
  rowHeight: ComputedRef<number>
  threshold?: number
  overscan?: number
}

const DEFAULT_VIRTUAL_THRESHOLD = 200
const DEFAULT_VIRTUAL_OVERSCAN = 12

const toSafeInt = (value: number, fallback: number): number => {
  if (!Number.isFinite(value)) return fallback
  return Math.max(0, Math.trunc(value))
}

export const isVirtualSpacerRow = <T>(
  row: VirtualTableRow<T> | unknown,
): row is VirtualSpacerRow => {
  if (!row || typeof row !== 'object') return false
  return (row as VirtualSpacerRow).__virtualSpacer === true
}

export const useTableVirtualScroll = <T>(options: UseTableVirtualScrollOptions<T>) => {
  const threshold = Math.max(
    1,
    toSafeInt(options.threshold ?? DEFAULT_VIRTUAL_THRESHOLD, DEFAULT_VIRTUAL_THRESHOLD),
  )
  const overscan = Math.max(
    0,
    toSafeInt(options.overscan ?? DEFAULT_VIRTUAL_OVERSCAN, DEFAULT_VIRTUAL_OVERSCAN),
  )

  const scrollTop = ref(0)
  const viewportHeight = ref(0)
  const bodyWrapRef = ref<HTMLElement | null>(null)

  let resizeObserver: ResizeObserver | null = null

  const resolvedRowHeight = computed(() => Math.max(1, toSafeInt(options.rowHeight.value, 44)))
  const isVirtualEnabled = computed(() => options.sourceRows.value.length > threshold)

  const virtualStartIndex = computed(() => {
    if (!isVirtualEnabled.value) return 0
    const base = Math.floor(scrollTop.value / resolvedRowHeight.value)
    return Math.max(0, base - overscan)
  })

  const virtualEndIndex = computed(() => {
    if (!isVirtualEnabled.value) return options.sourceRows.value.length
    const visibleCount = Math.ceil(viewportHeight.value / resolvedRowHeight.value)
    const end = virtualStartIndex.value + Math.max(visibleCount, 1) + overscan * 2
    return Math.min(options.sourceRows.value.length, end)
  })

  const visibleRows = computed<T[]>(() => {
    if (!isVirtualEnabled.value) return options.sourceRows.value
    return options.sourceRows.value.slice(virtualStartIndex.value, virtualEndIndex.value)
  })

  const virtualTopPadding = computed(() => {
    if (!isVirtualEnabled.value) return 0
    return virtualStartIndex.value * resolvedRowHeight.value
  })

  const virtualBottomPadding = computed(() => {
    if (!isVirtualEnabled.value) return 0
    const remaining = options.sourceRows.value.length - virtualEndIndex.value
    return Math.max(0, remaining) * resolvedRowHeight.value
  })

  const renderRows = computed<Array<VirtualTableRow<T>>>(() => {
    if (!isVirtualEnabled.value) return options.sourceRows.value
    const rows: Array<VirtualTableRow<T>> = []
    if (virtualTopPadding.value > 0) {
      rows.push({
        __virtualSpacer: true,
        __virtualKey: '__virtual-spacer-top__',
        __virtualHeight: virtualTopPadding.value,
      })
    }
    rows.push(...visibleRows.value)
    if (virtualBottomPadding.value > 0) {
      rows.push({
        __virtualSpacer: true,
        __virtualKey: '__virtual-spacer-bottom__',
        __virtualHeight: virtualBottomPadding.value,
      })
    }
    return rows
  })

  const resolveBodyWrap = (): HTMLElement | null => {
    const tableRoot = options.tableRef.value?.$el as HTMLElement | undefined
    if (!tableRoot) return null
    const bodyWrap = tableRoot.querySelector(
      '.el-table__body-wrapper .el-scrollbar__wrap',
    ) as HTMLElement | null
    if (bodyWrap) return bodyWrap
    return tableRoot.querySelector('.el-scrollbar__wrap') as HTMLElement | null
  }

  const updateViewport = () => {
    const wrap = bodyWrapRef.value
    if (!wrap) return
    viewportHeight.value = Math.max(0, toSafeInt(wrap.clientHeight, 0))
    scrollTop.value = Math.max(0, toSafeInt(wrap.scrollTop, 0))
  }

  const onBodyScroll = (event: Event) => {
    const target = event.target as HTMLElement | null
    if (!target) return
    scrollTop.value = Math.max(0, toSafeInt(target.scrollTop, 0))
    if (viewportHeight.value === 0) {
      viewportHeight.value = Math.max(0, toSafeInt(target.clientHeight, 0))
    }
  }

  const detach = () => {
    if (bodyWrapRef.value) {
      bodyWrapRef.value.removeEventListener('scroll', onBodyScroll)
    }
    bodyWrapRef.value = null
    if (resizeObserver) {
      resizeObserver.disconnect()
      resizeObserver = null
    }
  }

  const attach = () => {
    detach()
    const wrap = resolveBodyWrap()
    if (!wrap) return
    bodyWrapRef.value = wrap
    wrap.addEventListener('scroll', onBodyScroll, { passive: true })
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => updateViewport())
      resizeObserver.observe(wrap)
    }
    updateViewport()
  }

  const refreshVirtualViewport = () => {
    nextTick(() => {
      attach()
      if (!isVirtualEnabled.value) {
        scrollTop.value = 0
      }
    })
  }

  onMounted(() => {
    refreshVirtualViewport()
  })

  onBeforeUnmount(() => {
    detach()
  })

  watch(
    [() => options.tableRef.value, options.sourceRows, options.rowHeight, isVirtualEnabled],
    () => {
      if (!isVirtualEnabled.value) {
        scrollTop.value = 0
      }
      refreshVirtualViewport()
    },
    { flush: 'post' },
  )

  const resolveRowClassName = (row: unknown, baseClass = ''): string => {
    const classes: string[] = []
    if (baseClass) classes.push(baseClass)
    if (isVirtualSpacerRow<T>(row)) classes.push('virtual-spacer-row')
    return classes.join(' ')
  }

  const resolveRowStyle = (row: unknown): Record<string, string> => {
    if (!isVirtualSpacerRow<T>(row)) return {}
    return {
      height: `${Math.max(0, toSafeInt(row.__virtualHeight, 0))}px`,
    }
  }

  const isSelectableRow = (row: unknown): boolean => !isVirtualSpacerRow<T>(row)

  return {
    isVirtualEnabled,
    renderRows,
    visibleRows,
    virtualStartIndex,
    virtualEndIndex,
    virtualTopPadding,
    virtualBottomPadding,
    refreshVirtualViewport,
    resolveRowClassName,
    resolveRowStyle,
    isSelectableRow,
    isVirtualSpacerRow,
  }
}
