import { computed, type Ref } from 'vue'
import type { SlaveContentTabData as TabData } from '@/types/slaveContentPanel'
import {
  type ActivePointSimulationSummary,
  type DataPointSimulationPayload,
  useSlavePointEditor,
} from './useSlavePointEditor'

type UseSlaveContentActionsOptions = {
  currentTab: Ref<TabData | null | undefined>
  currentStationId: Ref<string>
  activePointSimulation: Ref<ActivePointSimulationSummary | null | undefined>
  pointEditor: Omit<Parameters<typeof useSlavePointEditor>[0], 'currentTabSimulation'>
}

export type { ActivePointSimulationSummary, DataPointSimulationPayload }

export function useSlaveContentActions(options: UseSlaveContentActionsOptions) {
  const currentTabSimulation = computed(() => {
    const activeSimulation = options.activePointSimulation.value
    const tab = options.currentTab.value
    if (!activeSimulation || !tab?.parentStationId) return null
    if (activeSimulation.stationId !== options.currentStationId.value) return null
    if (activeSimulation.stationId !== tab.parentStationId) return null
    return activeSimulation
  })

  const pointEditor = useSlavePointEditor({
    ...options.pointEditor,
    currentTabSimulation,
  })

  return {
    currentTabSimulation,
    ...pointEditor,
  }
}
