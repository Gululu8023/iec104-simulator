import { computed, watch, type Ref } from 'vue'

import { t } from '@shared/i18n'

import type { SlaveConnection } from '../types/master'

type ReadonlyRef<T> = {
  readonly value: T
}

interface UseMasterSelectionOptions {
  selectedSlave: Ref<SlaveConnection | null>
  slaveProfiles: ReadonlyRef<SlaveConnection[]>
  setActiveConnection: (connectionId: string | null) => void
  addLog: (level: string, message: string) => void
}

export function useMasterSelection(options: UseMasterSelectionOptions) {
  const selectedLinkProfileId = computed(() => {
    const targetId = Number(options.selectedSlave.value?.linkProfileId ?? 0)
    return Number.isFinite(targetId) && targetId > 0 ? targetId : 0
  })

  const hasSelectedLinkProfile = computed(() => selectedLinkProfileId.value > 0)

  const selectSlave = (slave: SlaveConnection | null, params: { logSelection?: boolean } = {}) => {
    const { logSelection = false } = params
    options.selectedSlave.value = slave
    options.setActiveConnection(slave?.connectionId ?? null)
    if (logSelection && slave) {
      options.addLog('info', t('master.messages.selectedSlave', { name: slave.name }))
    }
  }

  const clearSelectedSlave = () => {
    selectSlave(null)
  }

  const syncSelectedSlaveRuntimeState = () => {
    const current = options.selectedSlave.value
    if (!current) return

    const next = options.slaveProfiles.value.find(
      (profile) => profile.profileId === current.profileId,
    )
    if (!next) return

    selectSlave(next)
  }

  const reselectSlaveByProfileId = (profileId: number) => {
    const next =
      options.slaveProfiles.value.find((profile) => profile.profileId === profileId) ?? null
    if (!next) return
    selectSlave(next)
  }

  const handleSlaveSelect = (slave: SlaveConnection | null) => {
    selectSlave(slave, { logSelection: true })
  }

  watch(
    () => options.slaveProfiles.value,
    () => {
      syncSelectedSlaveRuntimeState()
    },
  )

  return {
    selectedLinkProfileId,
    hasSelectedLinkProfile,
    selectSlave,
    clearSelectedSlave,
    syncSelectedSlaveRuntimeState,
    reselectSlaveByProfileId,
    handleSlaveSelect,
  }
}
