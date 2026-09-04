import { ref, type Ref } from 'vue'

import { t } from '@shared/i18n'

export function useMasterMessages(
  messagePanelRef: Ref<any>,
  addLog: (level: string, message: string) => void,
) {
  const isMessagePanelHidden = ref(true)

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

  const handleMessageSelect = (message: any) => {
    addLog('info', t('master.messages.selectedMessage', { type: message.type }))
  }

  return {
    isMessagePanelHidden,
    handleMessagePanelHidden,
    showMessagePanel,
    toggleMessagePanel,
    handleMessageSelect,
  }
}
