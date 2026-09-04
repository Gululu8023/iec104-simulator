import { computed } from 'vue'

import { confirmUnsavedDialogClose } from './dialogConfirm'

type DialogCloseGuardOptions = {
  isDirty?: () => boolean
  isBlocked?: () => boolean
  onClose: () => void
}

export const useDialogCloseGuard = ({
  isDirty = () => false,
  isBlocked = () => false,
  onClose,
}: DialogCloseGuardOptions) => {
  const closeEnabled = computed(() => !isBlocked())

  const canClose = async (): Promise<boolean> => {
    if (!closeEnabled.value) return false
    if (!isDirty()) return true
    return confirmUnsavedDialogClose()
  }

  const requestClose = async (): Promise<boolean> => {
    if (!(await canClose())) return false
    onClose()
    return true
  }

  const handleBeforeClose = async (done: () => void): Promise<void> => {
    if (!(await canClose())) return
    onClose()
    done()
  }

  return {
    canClose,
    closeEnabled,
    handleBeforeClose,
    requestClose,
  }
}
