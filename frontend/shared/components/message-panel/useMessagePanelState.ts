import { computed, nextTick, ref, type Ref } from 'vue'
import type { Message } from './types'

type UseMessagePanelStateOptions = {
  messages: Ref<Message[]>
  filteredMessages: Ref<Message[]>
  emitMessageSelect: (message: Message) => void
  emitPanelHidden: (hidden: boolean) => void
}

const VIRTUAL_ROW_HEIGHT_PX = 28
const VIRTUAL_OVERSCAN_ROWS = 12

export function useMessagePanelState(options: UseMessagePanelStateOptions) {
  const isHidden = ref(false)
  const messageListScrollbarRef = ref<any>(null)
  const showDetailDialog = ref(false)
  const selectedMessageIndex = ref(-1)
  const selectedMessage = ref<Message | null>(null)
  const autoScroll = ref(true)
  const virtualScrollTop = ref(0)
  const virtualViewportHeight = ref(0)
  const mouseDownPos = ref({ x: 0, y: 0 })
  const isDragging = ref(false)

  const virtualStartIndex = computed(() => {
    const base = Math.floor(virtualScrollTop.value / VIRTUAL_ROW_HEIGHT_PX)
    return Math.max(0, base - VIRTUAL_OVERSCAN_ROWS)
  })

  const virtualEndIndex = computed(() => {
    const visible = Math.ceil(virtualViewportHeight.value / VIRTUAL_ROW_HEIGHT_PX)
    const end = virtualStartIndex.value + visible + VIRTUAL_OVERSCAN_ROWS * 2
    return Math.min(options.filteredMessages.value.length, end)
  })

  const virtualTopPadding = computed(() => virtualStartIndex.value * VIRTUAL_ROW_HEIGHT_PX)
  const virtualBottomPadding = computed(() => {
    const remaining = options.filteredMessages.value.length - virtualEndIndex.value
    return Math.max(0, remaining) * VIRTUAL_ROW_HEIGHT_PX
  })
  const virtualMessages = computed(() =>
    options.filteredMessages.value.slice(virtualStartIndex.value, virtualEndIndex.value),
  )

  const getMessageListWrap = (): HTMLElement | null => {
    const wrap = messageListScrollbarRef.value?.wrapRef as HTMLElement | undefined
    return wrap ?? null
  }

  const updateVirtualViewport = () => {
    const wrap = getMessageListWrap()
    if (!wrap) return
    virtualViewportHeight.value = wrap.clientHeight
  }

  const handleMessageListScroll = ({ scrollTop }: { scrollLeft: number; scrollTop: number }) => {
    virtualScrollTop.value = Number(scrollTop) || 0
    if (virtualViewportHeight.value === 0) {
      updateVirtualViewport()
    }
  }

  const scrollToBottom = () => {
    nextTick(() => {
      const wrap = getMessageListWrap()
      if (!wrap) return
      wrap.scrollTop = wrap.scrollHeight
      virtualScrollTop.value = wrap.scrollTop
      updateVirtualViewport()
    })
  }

  const scrollToMessageIndex = (index: number) => {
    nextTick(() => {
      const wrap = getMessageListWrap()
      if (!wrap) return
      const target = Math.max(
        0,
        index * VIRTUAL_ROW_HEIGHT_PX - Math.max(0, Math.floor(wrap.clientHeight / 2)),
      )
      wrap.scrollTop = target
      virtualScrollTop.value = wrap.scrollTop
      updateVirtualViewport()
    })
  }

  const toggleAutoScroll = () => {
    autoScroll.value = !autoScroll.value
    if (autoScroll.value) {
      scrollToBottom()
    }
  }

  const clearMessages = () => {
    options.messages.value = []
    selectedMessageIndex.value = -1
    selectedMessage.value = null
    // ElMessage.success('报文已清空')
  }

  const removeMessagesByConnectionIds = (connectionIds: string[]) => {
    const targetIds = new Set(
      connectionIds.map((item) => String(item ?? '').trim()).filter((item) => item.length > 0),
    )
    if (targetIds.size === 0) return

    options.messages.value = options.messages.value.filter((message) => {
      const connectionId = String(message.runtimeConnectionId ?? '').trim()
      return !connectionId || !targetIds.has(connectionId)
    })

    if (!selectedMessage.value) return

    const nextIndex = options.messages.value.findIndex(
      (message) => message.id === selectedMessage.value?.id,
    )
    if (nextIndex >= 0) {
      selectedMessageIndex.value = nextIndex
      selectedMessage.value = options.messages.value[nextIndex]
      return
    }

    selectedMessageIndex.value = -1
    selectedMessage.value = null
  }

  const removeMessagesByUiConnectionKey = (uiConnectionKey: string) => {
    const target = String(uiConnectionKey ?? '').trim()
    if (!target) return
    options.messages.value = options.messages.value.filter(
      (message) => String(message.uiConnectionKey ?? '').trim() !== target,
    )
    if (selectedMessage.value?.uiConnectionKey === target) {
      selectedMessageIndex.value = -1
      selectedMessage.value = null
    }
  }

  const hidePanel = () => {
    isHidden.value = true
    options.emitPanelHidden(true)
  }

  const showPanel = () => {
    isHidden.value = false
    options.emitPanelHidden(false)
  }

  const handleMouseDown = (event: MouseEvent, index: number) => {
    mouseDownPos.value = { x: event.clientX, y: event.clientY }
    isDragging.value = false

    if (event.detail === 2) {
      selectedMessage.value = options.filteredMessages.value[index]
      selectedMessageIndex.value = index
      showDetailDialog.value = true
    } else {
      selectedMessageIndex.value = index
    }
  }

  const handleMouseUp = (_event: MouseEvent, index: number) => {
    if (!isDragging.value) {
      selectedMessageIndex.value = index
      options.emitMessageSelect(options.filteredMessages.value[index])
    }
    isDragging.value = false
  }

  const handleMouseMove = (event: MouseEvent) => {
    if (
      Math.abs(event.clientX - mouseDownPos.value.x) > 5 ||
      Math.abs(event.clientY - mouseDownPos.value.y) > 5
    ) {
      isDragging.value = true
    }
  }

  const closeDetailDialog = () => {
    showDetailDialog.value = false
    selectedMessage.value = null
  }

  const navigateMessage = (direction: number) => {
    const nextIndex = selectedMessageIndex.value + direction
    if (nextIndex >= 0 && nextIndex < options.filteredMessages.value.length) {
      selectedMessageIndex.value = nextIndex
      selectedMessage.value = options.filteredMessages.value[nextIndex]
    }
  }

  const addMessages = (messages: Partial<Message>[]) => {
    if (messages.length === 0) return

    const receivedAt = Date.now()
    const nextMessages = messages.map<Message>((message, index) => ({
      id: `${receivedAt}-${index}`,
      timestamp: receivedAt,
      type: 'received',
      content: '',
      ...message,
    }))
    options.messages.value = [...options.messages.value, ...nextMessages].slice(-1000)

    if (autoScroll.value) {
      scrollToBottom()
    }
  }

  const addMessage = (message: Partial<Message>) => {
    const nextMessage: Message = {
      id: Date.now().toString(),
      timestamp: Date.now(),
      type: 'received',
      content: '',
      ...message,
    }

    options.messages.value.push(nextMessage)
    if (options.messages.value.length > 1000) {
      options.messages.value = options.messages.value.slice(-1000)
    }

    if (autoScroll.value) {
      scrollToBottom()
    }
  }

  const replaceMessages = (nextMessages: Partial<Message>[]) => {
    options.messages.value = nextMessages.slice(-1000).map((message) => ({
      id: Date.now().toString(),
      timestamp: Date.now(),
      type: 'received',
      content: '',
      ...message,
    }))

    if (selectedMessageIndex.value >= options.messages.value.length) {
      selectedMessageIndex.value = -1
      selectedMessage.value = null
    }
  }

  const getMessageCount = () => options.messages.value.length

  const getLastMessageId = () => {
    const last = options.messages.value[options.messages.value.length - 1]
    return last?.id ?? null
  }

  return {
    autoScroll,
    closeDetailDialog,
    addMessage,
    addMessages,
    clearMessages,
    removeMessagesByConnectionIds,
    removeMessagesByUiConnectionKey,
    getLastMessageId,
    getMessageCount,
    handleMessageListScroll,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
    hidePanel,
    isHidden,
    messageListScrollbarRef,
    messages: options.messages,
    navigateMessage,
    replaceMessages,
    scrollToBottom,
    scrollToMessageIndex,
    selectedMessage,
    selectedMessageIndex,
    showDetailDialog,
    showPanel,
    toggleAutoScroll,
    updateVirtualViewport,
    virtualBottomPadding,
    virtualMessages,
    virtualScrollTop,
    virtualStartIndex,
    virtualTopPadding,
    virtualViewportHeight,
  }
}
