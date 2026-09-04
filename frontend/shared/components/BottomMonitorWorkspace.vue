<template>
  <section v-show="!isMessagePanelHidden" ref="workspaceRef" class="bottom-monitor-workspace">
    <MessagePanel
      ref="messagePanelRef"
      class="bottom-monitor-workspace__message"
      :stationList="stationList"
      :stationId="stationId"
      :stationKind="stationKind"
      :detail-parse-view-mode="detailParseViewMode"
      @panel-hidden="handleMessagePanelHidden"
      @message-select="emit('messageSelect', $event)"
      @messages-cleared="handleMessagesCleared"
    />

    <template v-if="!isSoePanelHidden">
      <div class="bottom-monitor-workspace__split" @mousedown="startResizeSoe"></div>
      <div class="bottom-monitor-workspace__soe-shell" :style="{ width: `${clampedSoeWidth}px` }">
        <SoePanel
          :events="soeEvents"
          :station-list="stationList"
          :stationKind="stationKind"
          @select="emit('soeSelect', $event)"
          @clear="emit('clearSoe')"
          @hide="hideSoePanel"
        />
      </div>
    </template>
  </section>

  <MessageParserDialog ref="messageParserDialogRef" />
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import type { BackendSoeEvent, MessageDetailParseViewMode } from '@shared/api/types'
import MessageParserDialog from './MessageParserDialog.vue'
import MessagePanel from './MessagePanel.vue'
import SoePanel from './SoePanel.vue'

interface StationInfo {
  id: string
  name: string
  groupName?: string
  slaveName?: string
  host?: string
  port?: number
  remoteAddress?: string
  status?: string
  isConnectionLevel?: boolean
  runtimeConnectionId?: string
  uiConnectionKey?: string
  slaveUiKey?: string
  stationConfigId?: string
  commonAddress?: number | null
}

const props = defineProps<{
  stationList?: StationInfo[]
  stationId: string
  stationKind: 'master' | 'slave'
  soeEvents: BackendSoeEvent[]
  detailParseViewMode: MessageDetailParseViewMode
}>()

const emit = defineEmits<{
  messageSelect: [message: any]
  panelHidden: [hidden: boolean]
  soeVisibilityChange: [visible: boolean]
  soeSelect: [event: BackendSoeEvent]
  clearSoe: []
}>()

const MESSAGE_MIN_WIDTH = 140
const SOE_MIN_WIDTH = 430
const SOE_DEFAULT_WIDTH = 440
const SOE_MAX_WIDTH = 560

const workspaceRef = ref<HTMLElement | null>(null)
const messagePanelRef = ref<any>(null)
const messageParserDialogRef = ref<{
  open?: (inputText?: string) => void
  close?: () => void
} | null>(null)
const isMessagePanelHidden = ref(true)
const isSoePanelHidden = ref(true)
const isSoePanelVisible = computed(() => !isMessagePanelHidden.value && !isSoePanelHidden.value)
const soePanelWidth = ref(SOE_DEFAULT_WIDTH)
const workspaceWidth = ref(0)
const messageCache = ref<any[]>([])
let resizeObserver: ResizeObserver | null = null

const resolveMaxSoeWidth = () => {
  const base = workspaceWidth.value > 0 ? workspaceWidth.value : window.innerWidth
  return Math.max(0, Math.min(SOE_MAX_WIDTH, Math.floor(base * 0.48), base - MESSAGE_MIN_WIDTH))
}

const resolveMinSoeWidth = () => {
  return Math.min(SOE_MIN_WIDTH, resolveMaxSoeWidth())
}

const clampSoeWidth = (width: number) => {
  const minWidth = resolveMinSoeWidth()
  const maxWidth = Math.max(minWidth, resolveMaxSoeWidth())
  return Math.max(minWidth, Math.min(maxWidth, width))
}

const clampedSoeWidth = computed(() => {
  return clampSoeWidth(soePanelWidth.value)
})

const updateWorkspaceWidth = () => {
  workspaceWidth.value = workspaceRef.value?.clientWidth ?? 0
}

const syncMessagePanelFromCache = () => {
  const panel = messagePanelRef.value
  if (!panel?.replaceMessages) return

  const cachedLastId =
    messageCache.value.length > 0
      ? (messageCache.value[messageCache.value.length - 1].id ?? null)
      : null
  const currentCount = Number(panel.getMessageCount?.() ?? 0)
  const currentLastId = panel.getLastMessageId?.() ?? null
  if (currentCount === messageCache.value.length && currentLastId === cachedLastId) {
    return
  }

  panel.replaceMessages(messageCache.value)
}

const handleMessagePanelHidden = (hidden: boolean) => {
  if (isMessagePanelHidden.value === hidden) return
  isMessagePanelHidden.value = hidden
  emit('panelHidden', hidden)
}

const startResizeSoe = (event: MouseEvent) => {
  const startX = event.clientX
  const startWidth = clampedSoeWidth.value

  const handleMouseMove = (moveEvent: MouseEvent) => {
    const nextWidth = startWidth - (moveEvent.clientX - startX)
    soePanelWidth.value = clampSoeWidth(nextWidth)
  }

  const handleMouseUp = () => {
    document.removeEventListener('mousemove', handleMouseMove)
    document.removeEventListener('mouseup', handleMouseUp)
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
  }

  document.addEventListener('mousemove', handleMouseMove)
  document.addEventListener('mouseup', handleMouseUp)
  document.body.style.cursor = 'ew-resize'
  document.body.style.userSelect = 'none'
}

const addMessage = (message: any) => {
  messageCache.value = [...messageCache.value, message].slice(-1000)
  messagePanelRef.value?.addMessage?.(message)
}

const addMessages = (messages: any[]) => {
  if (messages.length === 0) return
  messageCache.value = [...messageCache.value, ...messages].slice(-1000)
  messagePanelRef.value?.addMessages?.(messages)
}

const clearMessages = () => {
  messageCache.value = []
  messagePanelRef.value?.clearMessages?.()
}

const handleMessagesCleared = (uiConnectionKey: string | null) => {
  if (!uiConnectionKey) {
    messageCache.value = []
    return
  }
  messageCache.value = messageCache.value.filter(
    (message) => String(message.uiConnectionKey ?? '').trim() !== uiConnectionKey,
  )
}

const removeMessagesByConnectionIds = (connectionIds: string[]) => {
  const targetIds = new Set(
    connectionIds.map((item) => String(item ?? '').trim()).filter((item) => item.length > 0),
  )
  if (targetIds.size === 0) return

  messageCache.value = messageCache.value.filter((message) => {
    const connectionId = String(message.runtimeConnectionId ?? '').trim()
    return !connectionId || !targetIds.has(connectionId)
  })
  messagePanelRef.value?.removeMessagesByConnectionIds?.([...targetIds])
}

const hidePanel = () => {
  handleMessagePanelHidden(true)
  messagePanelRef.value?.hidePanel?.()
}

const showPanel = () => {
  handleMessagePanelHidden(false)
  nextTick(() => {
    messagePanelRef.value?.showPanel?.()
    syncMessagePanelFromCache()
    updateWorkspaceWidth()
  })
}

const setSearchText = (text: string) => {
  showPanel()
  messagePanelRef.value?.setSearchText?.(text)
}

const applyFocusFilter = (focus: Record<string, unknown>) => {
  showPanel()
  messagePanelRef.value?.applyFocusFilter?.(focus)
}

const clearFocusFilter = () => {
  messagePanelRef.value?.clearFocusFilter?.()
}

const hideSoePanel = () => {
  isSoePanelHidden.value = true
}

const showSoePanel = () => {
  showPanel()
  isSoePanelHidden.value = false
  updateWorkspaceWidth()
}

const toggleSoePanel = () => {
  if (isMessagePanelHidden.value) {
    showSoePanel()
    return
  }
  if (isSoePanelHidden.value) {
    showSoePanel()
    return
  }
  hideSoePanel()
}

const openMessageParserDialog = (inputText = '') => {
  messageParserDialogRef.value?.open?.(inputText)
}

const closeMessageParserDialog = () => {
  messageParserDialogRef.value?.close?.()
}

onMounted(() => {
  emit('panelHidden', isMessagePanelHidden.value)
  emit('soeVisibilityChange', isSoePanelVisible.value)
  updateWorkspaceWidth()
  nextTick(() => {
    syncMessagePanelFromCache()
  })
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => {
      updateWorkspaceWidth()
      soePanelWidth.value = clampedSoeWidth.value
    })
    if (workspaceRef.value) {
      resizeObserver.observe(workspaceRef.value)
    }
  } else {
    window.addEventListener('resize', updateWorkspaceWidth)
  }
})

watch(isSoePanelVisible, (visible) => emit('soeVisibilityChange', visible))

onUnmounted(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  window.removeEventListener('resize', updateWorkspaceWidth)
})

defineExpose({
  addMessage,
  addMessages,
  clearMessages,
  removeMessagesByConnectionIds,
  hidePanel,
  showPanel,
  setSearchText,
  applyFocusFilter,
  clearFocusFilter,
  toggleSoePanel,
  showSoePanel,
  hideSoePanel,
  openMessageParserDialog,
  closeMessageParserDialog,
})
</script>

<style scoped>
.bottom-monitor-workspace {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: #fff;
}

.bottom-monitor-workspace__message {
  flex: 1 1 160px;
  min-width: 160px;
}

.bottom-monitor-workspace__split {
  position: relative;
  width: 1px;
  flex-shrink: 0;
  background: transparent;
  cursor: ew-resize;
}

.bottom-monitor-workspace__split::after {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: -6px;
  right: -6px;
  cursor: ew-resize;
}

.bottom-monitor-workspace__split:hover {
  background: #3b82f6;
}

.bottom-monitor-workspace__soe-shell {
  flex-shrink: 0;
  min-width: 0;
  height: 100%;
}
</style>
