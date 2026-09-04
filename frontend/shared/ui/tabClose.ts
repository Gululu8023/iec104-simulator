export type ClosableTabLike = {
  id: string
}

export function resolveAdjacentTabId<T extends ClosableTabLike>(
  tabs: readonly T[],
  closedTabId: string,
  activeTabId: string | null | undefined,
): string {
  const currentActiveId = String(activeTabId ?? '')
  if (!currentActiveId || currentActiveId !== closedTabId) return currentActiveId

  const closedIndex = tabs.findIndex((tab) => tab.id === closedTabId)
  if (closedIndex === -1) return currentActiveId

  const remainingTabs = tabs.filter((tab) => tab.id !== closedTabId)
  if (remainingTabs.length === 0) return ''

  const adjacentIndex = Math.min(closedIndex, remainingTabs.length - 1)
  return remainingTabs[adjacentIndex]?.id ?? ''
}
