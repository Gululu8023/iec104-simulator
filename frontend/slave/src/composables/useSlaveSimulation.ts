import { ref, type Ref } from 'vue'
import { ElMessage } from 'element-plus'
import { t } from '@shared/i18n'
import { triggerSlaveScenario } from '../api/slave'

type StatsState = {
  pendingQueue: number
}

export function useSlaveSimulation(
  messagePanelRef: Ref<any>,
  currentStationId: Ref<string>,
  selectedSlave: Ref<any>,
  slaveStats: StatsState,
) {
  const isMessagePanelHidden = ref(true)

  const normalizeOptionalCommonAddress = (value: unknown): number | undefined => {
    const numeric = Number(value)
    if (!Number.isFinite(numeric)) return undefined
    const normalized = Math.trunc(numeric)
    if (normalized < 1 || normalized > 65535) return undefined
    return normalized
  }

  const handleMessagePanelHidden = (hidden: boolean) => {
    isMessagePanelHidden.value = hidden
  }

  const showMessagePanel = () => {
    if (messagePanelRef.value) {
      messagePanelRef.value.showPanel()
      isMessagePanelHidden.value = false
    }
  }

  const toggleMessagePanel = () => {
    if (isMessagePanelHidden.value) {
      showMessagePanel()
      return
    }
    if (messagePanelRef.value) {
      messagePanelRef.value.hidePanel()
    }
  }

  const handleAvalancheTest = async (commonAddress?: number) => {
    if (!currentStationId.value) {
      ElMessage.warning(t('slave.simulationMessages.initFirst'))
      return
    }
    if (!selectedSlave.value) {
      ElMessage.warning(t('slave.simulationMessages.selectSlaveFirst'))
      return
    }

    slaveStats.pendingQueue += 1
    try {
      const targetCommonAddress =
        normalizeOptionalCommonAddress(commonAddress) ??
        normalizeOptionalCommonAddress(selectedSlave.value?.commonAddress)
      const changed = await triggerSlaveScenario(currentStationId.value, {
        type: 'avalanche',
        common_address: targetCommonAddress,
      })
      if (changed === 0) {
        ElMessage.warning(t('slave.simulationMessages.avalancheNoTelemetry'))
        return
      }
      ElMessage.success(t('slave.simulationMessages.avalancheTriggered', { count: changed }))
    } catch (error) {
      ElMessage.error(t('slave.simulationMessages.avalancheFailed', { error: String(error) }))
    } finally {
      slaveStats.pendingQueue = Math.max(0, slaveStats.pendingQueue - 1)
    }
  }

  return {
    isMessagePanelHidden,
    handleMessagePanelHidden,
    showMessagePanel,
    toggleMessagePanel,
    handleAvalancheTest,
  }
}
