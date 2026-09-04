import { onUnmounted, type Ref } from 'vue'

import type { SlaveContentTabData } from '@/types/slaveContentPanel'

type UseSlaveContentTabsOptions = {
  activeTabs: Ref<SlaveContentTabData[]>
  activeTabName: Ref<string>
  loadTab: (tab: SlaveContentTabData, revision: number) => Promise<boolean>
}

export function useSlaveContentTabs(options: UseSlaveContentTabsOptions) {
  const pendingLoads = new Set<string>()
  let paintFrame: number | null = null
  let loadFrame: number | null = null

  const schedulePump = () => {
    if (paintFrame != null || loadFrame != null) return
    paintFrame = requestAnimationFrame(() => {
      paintFrame = null
      loadFrame = requestAnimationFrame(() => void flush())
    })
  }

  const flush = async () => {
    const activeId = options.activeTabName.value
    const tabIds = [...pendingLoads].sort((left, right) => {
      if (left === activeId) return -1
      if (right === activeId) return 1
      return 0
    })
    pendingLoads.clear()

    for (const tabId of tabIds) {
      const tab = options.activeTabs.value.find((candidate) => candidate.id === tabId)
      if (!tab) continue
      const revision = tab.dataRevision
      if (!(await options.loadTab(tab, revision))) continue
      if (!options.activeTabs.value.includes(tab) || tab.dataRevision !== revision) continue
      tab.loading = false
    }

    loadFrame = null
    if (pendingLoads.size > 0) schedulePump()
  }

  const scheduleTabLoad = (tab: SlaveContentTabData) => {
    if (pendingLoads.has(tab.id)) return
    tab.dataRevision += 1
    pendingLoads.add(tab.id)
    schedulePump()
  }

  const cancelTabLoad = (tabId: string) => {
    const tab = options.activeTabs.value.find((candidate) => candidate.id === tabId)
    if (tab) tab.dataRevision += 1
    pendingLoads.delete(tabId)
  }

  const cancelAllTabLoads = () => {
    for (const tab of options.activeTabs.value) tab.dataRevision += 1
    pendingLoads.clear()
    if (paintFrame != null) cancelAnimationFrame(paintFrame)
    if (loadFrame != null) cancelAnimationFrame(loadFrame)
    paintFrame = null
    loadFrame = null
  }

  onUnmounted(cancelAllTabLoads)

  return {
    scheduleTabLoad,
    cancelTabLoad,
    cancelAllTabLoads,
  }
}
