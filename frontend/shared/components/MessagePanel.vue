<template>
  <div class="message-panel" v-show="!isHidden">
    <!-- 单行面板头部 (Single Header Bar) -->
    <div class="panel-header">
      <!-- 左: 标题 + 计数徽章 -->
      <div class="header-left">
        <el-icon class="header-icon"><ChatDotRound /></el-icon>
        <span class="panel-title">{{ t('messagePanel.title') }}</span>
        <span class="panel-badge">{{ filteredMessages.length }}/{{ messages.length }}</span>
      </div>

      <!-- 中: 搜索 + 筛选器 (Ghost Button + Popover) -->
      <div class="header-center">
        <input
          v-model="searchText"
          type="text"
          class="filter-input"
          :placeholder="t('messagePanel.searchPlaceholder')"
          @input="filterMessages"
        />

        <!-- 方向筛选 Popover -->
        <el-popover
          trigger="click"
          placement="bottom-start"
          popper-class="app-menu-dropdown message-panel-dropdown"
        >
          <template #reference>
            <button class="filter-btn" :class="{ active: directionFilter.length > 0 }">
              {{ getDirectionFilterLabel() }} ▾
            </button>
          </template>
          <el-scrollbar class="filter-options filter-options-multi" max-height="400px">
            <label
              class="filter-checkbox el-dropdown-menu__item"
              :class="{ 'is-active': directionFilter.includes('sent') }"
            >
              <input
                type="checkbox"
                value="sent"
                v-model="directionFilter"
                @change="filterMessages"
              />
              <span class="filter-option-label" :title="t('messagePanel.directions.sent')">
                {{ t('messagePanel.directions.sent') }}
              </span>
            </label>
            <label
              class="filter-checkbox el-dropdown-menu__item"
              :class="{ 'is-active': directionFilter.includes('received') }"
            >
              <input
                type="checkbox"
                value="received"
                v-model="directionFilter"
                @change="filterMessages"
              />
              <span class="filter-option-label" :title="t('messagePanel.directions.received')">
                {{ t('messagePanel.directions.received') }}
              </span>
            </label>
            <label
              class="filter-checkbox el-dropdown-menu__item"
              :class="{ 'is-active': directionFilter.includes('error') }"
            >
              <input
                type="checkbox"
                value="error"
                v-model="directionFilter"
                @change="filterMessages"
              />
              <span class="filter-option-label" :title="t('messagePanel.directions.error')">
                {{ t('messagePanel.directions.error') }}
              </span>
            </label>
          </el-scrollbar>
        </el-popover>

        <!-- 从站筛选 Popover -->
        <el-popover
          v-if="showStationFilter"
          trigger="click"
          placement="bottom-start"
          popper-class="app-menu-dropdown message-panel-dropdown"
        >
          <template #reference>
            <button class="filter-btn" :class="{ active: stationFilter.length > 0 }">
              {{ getStationFilterLabel() }} ▾
            </button>
          </template>
          <el-scrollbar class="filter-options filter-options-multi" max-height="400px">
            <label
              v-for="station in selectableStations"
              :key="station.id"
              class="filter-checkbox el-dropdown-menu__item"
              :class="{ 'is-active': stationFilter.includes(station.id) }"
            >
              <input
                type="checkbox"
                :value="station.id"
                v-model="stationFilter"
                @change="filterMessages"
              />
              <span class="filter-option-label" :title="station.slaveName || station.name">
                {{ station.slaveName || station.name }}
              </span>
            </label>
          </el-scrollbar>
        </el-popover>

        <!-- 类型筛选 Popover -->
        <el-popover
          trigger="click"
          placement="bottom-start"
          :fallback-placements="['bottom-start']"
          popper-class="app-menu-dropdown message-panel-dropdown"
          @show="updateTypeFilterMaxHeight"
        >
          <template #reference>
            <button
              ref="typeFilterButtonRef"
              class="filter-btn"
              :class="{ active: typeFilter.length > 0 }"
              @click="updateTypeFilterMaxHeight"
            >
              {{
                typeFilter.length > 0
                  ? t('messagePanel.filters.typeWithCount', { count: typeFilter.length })
                  : t('messagePanel.filters.type')
              }}
              ▾
            </button>
          </template>
          <el-scrollbar
            class="filter-options filter-options-multi"
            :max-height="`${typeFilterMaxHeight}px`"
          >
            <template v-for="group in typeFilterGroups" :key="group.labelKey || group.label">
              <div class="filter-group-label" :title="group.label">{{ group.label }}</div>
              <label
                v-for="option in group.options"
                :key="option.value"
                class="filter-checkbox el-dropdown-menu__item"
                :class="{ 'is-active': typeFilter.includes(option.value) }"
              >
                <input
                  type="checkbox"
                  :value="option.value"
                  v-model="typeFilter"
                  @change="filterMessages"
                />
                <span class="filter-option-label" :title="getTypeFilterOptionLabel(option)">
                  {{ getTypeFilterOptionLabel(option) }}
                </span>
              </label>
            </template>
          </el-scrollbar>
        </el-popover>
      </div>

      <!-- 右: 操作图标 + 关闭 -->
      <div class="header-right">
        <el-dropdown
          trigger="click"
          placement="bottom-start"
          popper-class="app-menu-dropdown message-panel-dropdown"
          @command="selectConnectionFilter"
        >
          <button class="filter-btn connection-filter-btn" :class="{ active: connectionFilter }">
            <span class="connection-filter-label" :title="getConnectionFilterLabel()">
              {{ getConnectionFilterLabel() }}
            </span>
            <span>▾</span>
          </button>
          <template #dropdown>
            <el-dropdown-menu class="filter-options">
              <el-dropdown-item command="" :class="{ 'is-active': !connectionFilter }">
                <span class="filter-option-label" :title="t('messagePanel.filters.allConnections')">
                  {{ t('messagePanel.filters.allConnections') }}
                </span>
              </el-dropdown-item>
              <el-dropdown-item
                v-for="station in connectionOptions"
                :key="station.uiConnectionKey || station.id"
                :command="String(station.uiConnectionKey || station.id)"
                :class="{
                  'is-active':
                    connectionFilter === String(station.uiConnectionKey || station.id).trim(),
                }"
              >
                <span class="filter-option-label" :title="getConnectionOptionLabel(station)">
                  {{ getConnectionOptionLabel(station) }}
                </span>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-tooltip
          :content="
            autoScroll
              ? t('messagePanel.tooltips.pauseScroll')
              : t('messagePanel.tooltips.autoScroll')
          "
          placement="bottom"
          :enterable="false"
        >
          <button class="icon-btn" :class="{ active: autoScroll }" @click="toggleAutoScroll">
            <el-icon><VideoPause v-if="autoScroll" /><VideoPlay v-else /></el-icon>
          </button>
        </el-tooltip>
        <el-tooltip
          :content="t('messagePanel.tooltips.clearMessages')"
          placement="bottom"
          :enterable="false"
        >
          <button class="icon-btn" @click="handleClearMessages">
            <el-icon><Delete /></el-icon>
          </button>
        </el-tooltip>
        <el-dropdown
          trigger="click"
          @command="handleExport"
          popper-class="app-menu-dropdown message-panel-dropdown"
        >
          <span class="dropdown-trigger">
            <el-tooltip
              :content="t('messagePanel.tooltips.export')"
              placement="bottom"
              :enterable="false"
            >
              <button class="icon-btn">
                <el-icon><Download /></el-icon>
              </button>
            </el-tooltip>
          </span>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="csv">{{ t('messagePanel.export.csv') }}</el-dropdown-item>
              <el-dropdown-item command="txt">{{ t('messagePanel.export.txt') }}</el-dropdown-item>
              <el-dropdown-item command="pcap" divided>{{
                t('messagePanel.export.pcap')
              }}</el-dropdown-item>
              <el-dropdown-item command="pcapng">{{
                t('messagePanel.export.pcapng')
              }}</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <button class="icon-btn close-btn" @click="hidePanel">
          <el-icon><Close /></el-icon>
        </button>
      </div>
    </div>

    <!-- 消息列表（虚拟滚动） -->
    <el-scrollbar
      ref="messageListScrollbarRef"
      class="message-list"
      @scroll="handleMessageListScroll"
    >
      <template v-if="filteredMessages.length > 0">
        <div class="virtual-spacer" :style="{ height: `${virtualTopPadding}px` }"></div>

        <div
          v-for="row in virtualMessageRows"
          :key="row.message.id"
          class="message-item"
          :class="{
            [row.message.type]: true,
            selected: selectedMessageIndex === virtualStartIndex + row.localIndex,
          }"
          @mousedown="handleMouseDown($event, virtualStartIndex + row.localIndex)"
          @mouseup="handleMouseUp($event, virtualStartIndex + row.localIndex)"
          @mousemove="handleMouseMove"
        >
          <span class="message-time">{{ row.time }}</span>
          <span class="message-station" :title="row.stationName">{{ row.stationName }}</span>
          <span class="message-direction" :class="row.message.type">{{ row.direction }}</span>
          <div class="message-hex font-mono" v-html="row.highlightedHex"></div>
          <span
            v-if="row.message.hexData"
            class="frame-tag"
            :class="row.summary.color"
            :title="row.summary.title"
            >{{ row.summary.tag }}{{ row.summary.label ? ':' + row.summary.label : '' }}</span
          >
        </div>

        <div class="virtual-spacer" :style="{ height: `${virtualBottomPadding}px` }"></div>
      </template>

      <div v-else class="empty-message">{{ t('messagePanel.empty') }}</div>
    </el-scrollbar>

    <MessageDetailDialog
      v-model:visible="showDetailDialog"
      :before-close="handleDetailDialogBeforeClose"
      :selected-message-index="selectedMessageIndex"
      :filtered-message-count="filteredMessages.length"
      :selected-message="selectedMessage"
      :selected-message-route="selectedMessageRoute"
      :selected-frame-summary="selectedFrameSummary"
      :hex-data-expanded="hexDataExpanded"
      :should-show-detail-tree="shouldShowDetailTree"
      :detail-parse-result="detailParseResult"
      :detail-parse-error="detailParseError"
      :detail-parse-loading="detailParseLoading"
      :is-tree-detail-mode="isTreeDetailMode"
      :navigate-message="navigateMessage"
      :copy-hex-data="copyHexData"
      :toggle-hex-data-expanded="toggleHexDataExpanded"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, toRef, watch } from 'vue'
import {
  ChatDotRound,
  Delete,
  Download,
  Close,
  VideoPause,
  VideoPlay,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { currentLocale, t } from '@shared/i18n'
import MessageDetailDialog from './message-panel/MessageDetailDialog.vue'
import { formatHexData, formatHighlightedHex, getFrameSummary } from './message-panel/parser'
import type { MessageDetailParseViewMode } from '@shared/api/types'
import {
  clearMessageFrameParseCache,
  getMessageFrameParseErrorMessage,
  useMessageFrameParserSession,
} from '@shared/composables/useMessageFrameParser'
import { formatDetailTime, getDirectionLabel } from './message-panel/frameFormatters'
import { useMessagePanelFilters } from './message-panel/useMessagePanelFilters'
import { useMessagePanelExport } from './message-panel/useMessagePanelExport'
import { useMessagePanelState } from './message-panel/useMessagePanelState'
import type { Message, MessageFocusFilter, StationInfo } from './message-panel/types'

// Props定义
const props = defineProps<{
  stationList?: StationInfo[]
  stationId: string
  stationKind: 'master' | 'slave'
  detailParseViewMode: MessageDetailParseViewMode
}>()

const messages = ref<Message[]>([])
const TYPE_FILTER_MAX_HEIGHT = 400
const TYPE_FILTER_BOTTOM_RESERVE = 32
const typeFilterButtonRef = ref<HTMLButtonElement | null>(null)
const typeFilterMaxHeight = ref(TYPE_FILTER_MAX_HEIGHT)

const updateTypeFilterMaxHeight = () => {
  const triggerBottom = typeFilterButtonRef.value?.getBoundingClientRect().bottom
  if (triggerBottom == null) return

  const availableHeight = Math.floor(
    window.innerHeight - triggerBottom - TYPE_FILTER_BOTTOM_RESERVE,
  )
  typeFilterMaxHeight.value = Math.max(1, Math.min(TYPE_FILTER_MAX_HEIGHT, availableHeight))
}

const {
  connectionFilter,
  connectionOptions,
  directionFilter,
  externalFocusFilter,
  filteredMessages,
  getConnectionFilterLabel,
  getConnectionOptionLabel,
  getDirectionFilterLabel,
  getStationFilterLabel,
  getStationName,
  getTypeFilterOptionLabel,
  selectableStations,
  matchesExternalFocus,
  resetStandardFilters,
  resolveConnectionIdFromUiKey,
  searchText,
  selectConnectionFilter,
  showStationFilter,
  stationFilter,
  typeFilter,
  typeFilterGroups,
} = useMessagePanelFilters({
  stationList: toRef(props, 'stationList'),
  messages,
})

// 事件定义
const emit = defineEmits<{
  messageSelect: [message: Message]
  panelHidden: [hidden: boolean]
  messagesCleared: [uiConnectionKey: string | null]
}>()
const {
  autoScroll,
  addMessage,
  addMessages,
  clearMessages,
  closeDetailDialog,
  getLastMessageId,
  getMessageCount,
  handleMessageListScroll,
  handleMouseDown,
  handleMouseMove,
  handleMouseUp,
  hidePanel,
  isHidden,
  messageListScrollbarRef,
  navigateMessage,
  removeMessagesByConnectionIds: removeMessagesByConnectionIdsInternal,
  removeMessagesByUiConnectionKey,
  replaceMessages,
  scrollToMessageIndex,
  selectedMessage,
  selectedMessageIndex,
  showDetailDialog,
  showPanel,
  toggleAutoScroll,
  updateVirtualViewport,
  virtualBottomPadding,
  virtualMessages,
  virtualStartIndex,
  virtualTopPadding,
} = useMessagePanelState({
  messages,
  filteredMessages,
  emitMessageSelect: (message) => emit('messageSelect', message),
  emitPanelHidden: (hidden) => emit('panelHidden', hidden),
})
const handleDetailDialogBeforeClose = (done: () => void) => {
  closeDetailDialog()
  done()
}
void messageListScrollbarRef
const virtualMessageRows = computed(() => {
  void currentLocale.value
  return virtualMessages.value.map((message, localIndex) => ({
    message,
    localIndex,
    time: formatDetailTime(message.timestamp),
    stationName: getStationName(message),
    direction: getDirectionLabel(message.type),
    highlightedHex: formatHighlightedHex(message.hexData),
    summary: getFrameSummary(message.hexData),
  }))
})

const selectedFrameSummary = computed(() => {
  void currentLocale.value
  return getFrameSummary(selectedMessage.value?.hexData)
})

const selectedMessageRoute = computed(() => {
  void currentLocale.value
  const message = selectedMessage.value
  if (!message) return ''
  const master = t('frame.description.master')
  const peer = getStationName(message)
  const masterSends =
    props.stationKind === 'master' ? message.type !== 'received' : message.type === 'received'
  return masterSends ? `${master} → ${peer}` : `${peer} → ${master}`
})

const { handleExport } = useMessagePanelExport({
  stationId: toRef(props, 'stationId'),
  stationKind: toRef(props, 'stationKind'),
  messages,
  filteredMessages,
  connectionFilter,
  resolveConnectionIdFromUiKey,
  formatDetailTime,
  getDirectionLabel,
  getStationName,
  formatHexData,
})

const hexDataExpanded = ref(false)

const copyHexData = async () => {
  if (!selectedMessage.value?.hexData) return
  try {
    await navigator.clipboard.writeText(formatHexData(selectedMessage.value.hexData))
  } catch {
    ElMessage.warning(t('messagePanel.detail.copyFailed'))
  }
}

const toggleHexDataExpanded = () => {
  hexDataExpanded.value = !hexDataExpanded.value
}

const detailParser = useMessageFrameParserSession()
let detailParseVersion = 0

const detailParseResult = computed(() => detailParser.result.value)
const detailParseLoading = computed(() => detailParser.loading.value)
const detailParseError = computed(
  () =>
    detailParser.parseError.value || getMessageFrameParseErrorMessage(detailParser.result.value),
)
const isTreeDetailMode = computed(() => props.detailParseViewMode === 'tree')
const shouldShowDetailTree = computed(
  () =>
    isTreeDetailMode.value &&
    Boolean(selectedMessage.value?.hexData) &&
    Boolean(detailParseResult.value) &&
    detailParseResult.value?.status !== 'error',
)

const syncDetailParseState = async () => {
  detailParseVersion += 1
  const currentVersion = detailParseVersion

  if (!showDetailDialog.value) {
    detailParser.reset()
    return
  }

  const hexData = selectedMessage.value?.hexData
  if (!hexData) {
    detailParser.reset()
    return
  }

  if (detailParser.hydrateFromHex(hexData)) {
    return
  }

  try {
    await detailParser.parse(hexData)
  } catch {
    if (currentVersion !== detailParseVersion) return
  }
}

// 过滤消息
const filterMessages = () => {
  // 触发计算属性重新计算
}

onMounted(() => {
  updateVirtualViewport()
})

watch(
  () => filteredMessages.value.length,
  () => {
    if (selectedMessageIndex.value < 0) return
    if (selectedMessageIndex.value >= filteredMessages.value.length) {
      selectedMessageIndex.value = -1
      selectedMessage.value = null
    }
  },
)

watch(
  [
    () => showDetailDialog.value,
    () => props.detailParseViewMode,
    () => selectedMessage.value?.id,
    () => selectedMessage.value?.hexData,
  ],
  () => {
    hexDataExpanded.value = false
    void syncDetailParseState()
  },
)

// 设置搜索文本
const setSearchText = (text: string) => {
  externalFocusFilter.value = null
  searchText.value = text
  filterMessages()
  // 如果面板隐藏，则显示
  if (isHidden.value) {
    showPanel()
  }
}

const applyFocusFilter = (focus: MessageFocusFilter) => {
  const nextFocus: MessageFocusFilter = {
    connectionId: focus.connectionId ?? null,
    commonAddress: focus.commonAddress ?? null,
    ioa: focus.ioa ?? null,
    typeId: focus.typeId ?? null,
  }
  const matchedMessages = messages.value.filter((msg) => matchesExternalFocus(msg, nextFocus))
  if (matchedMessages.length === 0) {
    externalFocusFilter.value = null
    return
  }
  externalFocusFilter.value = nextFocus
  resetStandardFilters()
  if (isHidden.value) {
    showPanel()
  }
  nextTick(() => {
    const nextIndex = filteredMessages.value.length - 1
    if (nextIndex < 0) return
    selectedMessageIndex.value = nextIndex
    selectedMessage.value = filteredMessages.value[nextIndex]
    scrollToMessageIndex(nextIndex)
    emit('messageSelect', filteredMessages.value[nextIndex])
  })
}

const clearFocusFilter = () => {
  externalFocusFilter.value = null
}

const clearMessagesAndFocus = () => {
  clearMessages()
  clearMessageFrameParseCache()
  detailParser.reset()
  externalFocusFilter.value = null
}

const handleClearMessages = () => {
  if (!connectionFilter.value) {
    clearMessagesAndFocus()
    emit('messagesCleared', null)
    return
  }
  removeMessagesByUiConnectionKey(connectionFilter.value)
  clearMessageFrameParseCache()
  emit('messagesCleared', connectionFilter.value)
  externalFocusFilter.value = null
}

const removeMessagesByConnectionIds = (connectionIds: string[]) => {
  removeMessagesByConnectionIdsInternal(connectionIds)
  clearMessageFrameParseCache()
  externalFocusFilter.value = null
}

// 暴露方法给父组件
defineExpose({
  addMessage,
  addMessages,
  replaceMessages,
  getMessageCount,
  getLastMessageId,
  clearMessages: clearMessagesAndFocus,
  removeMessagesByConnectionIds,
  hidePanel,
  showPanel,
  setSearchText,
  applyFocusFilter,
  clearFocusFilter,
})
</script>

<style scoped>
.message-panel {
  background: white;
  display: flex;
  flex-direction: column;
  position: relative;
  flex: 1;
  min-height: 200px;
}

.panel-header {
  padding: 0 12px;
  border-bottom: 1px solid #e5e7eb;
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 36px;
  background: #fafafa;
  border-top: 1px solid #e1e4e8;
  gap: 16px;
}

/* 左侧：标题 + 徽章 */
.header-left {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.header-icon {
  font-size: 14px;
  color: #57606a;
}

.panel-title {
  font-size: 13px;
  font-weight: 600;
  color: #24292f;
  font-family: var(--font-mono);
}

.panel-badge {
  font-size: 11px;
  font-family: var(--font-mono);
  background: #e5e7eb;
  color: #374151;
  padding: 1px 6px;
  border-radius: 10px;
  font-weight: 500;
}

/* 中间：搜索 + 筛选器 */
.header-center {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  justify-content: flex-start;
}

/* 无边框搜索框 */
.filter-input {
  width: 160px;
  height: 26px;
  padding: 0 8px;
  border: 1px solid #d1d5db; /* Light gray border */
  border-radius: 4px;
  background: white; /* White bg */
  font-size: 12px;
  color: #374151; /* Darker text */
  outline: none;
  transition: all 0.2s;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05); /* Slight shadow */
}

.filter-input:focus {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.1);
}

.filter-input::placeholder {
  color: #9ca3af;
}

/* Ghost Button 筛选器 */
.filter-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  height: 28px;
  border: none;
  background: transparent;
  color: #374151; /* Deeper text color */
  font-size: 13px;
  cursor: pointer;
  border-radius: 4px;
  transition: all 0.2s;
  white-space: nowrap;
  font-weight: 500;
}

.filter-btn:hover {
  background: #e5e7eb; /* Slightly darker hover bg */
  color: #111827;
}

.filter-btn.active {
  color: var(--primary-color);
  background: #eff6ff;
}

/* Popover 筛选选项 */
.filter-options {
  width: max-content;
  min-width: 180px;
  max-width: min(300px, calc(100vw - 32px));
}

.filter-options :deep(.el-scrollbar__wrap) {
  overflow-x: hidden;
}

.filter-options :deep(.el-scrollbar__view) {
  display: flex;
  flex-direction: column;
  padding: 2px 0;
}

.filter-options-multi {
  min-width: 180px;
}

.filter-options-multi :deep(.el-scrollbar__view) {
  padding: 4px 0;
}

.filter-group-label {
  font-size: 11px;
  font-weight: 600;
  color: #9ca3af;
  text-transform: uppercase;
  padding: 8px 12px 4px 12px;
  margin-top: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.filter-group-label:first-child {
  margin-top: 0;
}

.filter-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  user-select: none;
}

.filter-checkbox input[type='checkbox'] {
  flex: 0 0 auto;
  accent-color: var(--primary-color);
}

.filter-option-label {
  display: block;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 右侧：图标按钮组 */
.header-right {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.connection-filter-btn {
  width: 220px;
  flex: 0 0 220px;
  justify-content: space-between;
  margin-right: 4px;
  padding-right: 12px;
}

.connection-filter-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: left;
}

.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: #57606a;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
}

.icon-btn:hover {
  background: #e5e7eb;
  color: #24292f;
}

.icon-btn.active {
  background: var(--primary-light, #eff6ff);
  color: var(--primary-color);
}

.icon-btn.close-btn:hover {
  background: #fef2f2;
  color: #dc2626;
}

.message-list {
  flex: 1;
  min-height: 0;
  background: #fafafa; /* 雾白背景 */
  font-family: Consolas, 'JetBrains Mono', monospace;
}

.message-list :deep(.el-scrollbar__wrap) {
  overflow-x: hidden;
  overscroll-behavior: contain;
}

.message-list :deep(.el-scrollbar__view) {
  min-height: 100%;
  box-sizing: border-box;
  padding: 4px 0;
}

.virtual-spacer {
  width: 100%;
}

.message-item {
  display: flex;
  align-items: center;
  padding: 4px 12px;
  height: 30px;
  box-sizing: border-box;
  cursor: pointer;
  transition: background-color 0.1s;
  font-size: 13px;
  font-family: var(--font-mono);
  border-bottom: 1px solid #eaecef; /* 浅色分割线 */
  color: #24292f;
}

.message-item:hover {
  background-color: #f6f8fa;
}

.message-item.selected {
  background-color: #e8f0fe; /* 选中浅蓝 */
}

/* 状态胶囊 (Capsule Badges) */
.message-direction {
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 600;
  text-align: center;
  width: 50px;
  flex-shrink: 0;
  margin-right: 16px;
}

/* 发送 TX (淡蓝底深蓝字) */
.message-item.sent .message-direction {
  background-color: #ddf4ff;
  color: #0969da;
  border: 1px solid rgba(9, 105, 218, 0.1);
}

/* 接收 RX (淡绿底深绿字) */
.message-item.received .message-direction {
  background-color: #dafbe1;
  color: #1a7f37;
  border: 1px solid rgba(26, 127, 55, 0.1);
}

/* 错误 ERR (淡红底深红字) */
.message-item.error .message-direction {
  background-color: #ffebe9;
  color: #cf222e;
  border: 1px solid rgba(207, 34, 46, 0.1);
}

.message-time {
  color: #6a737d; /* 弱化灰色 */
  width: 155px;
  flex-shrink: 0;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 12px;
  margin-right: 16px;
}

.message-station {
  color: #57606a;
  width: 90px;
  font-weight: 600;
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: left;
  margin-right: 16px;
}

.message-hex {
  color: #0550ae; /* 深藏青 */
  flex: 1;
  font-family: Consolas, 'JetBrains Mono', monospace;
  font-weight: normal;
  margin-left: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

/* 帧类型标签（右侧徽章） */
.frame-tag {
  flex-shrink: 0;
  margin-left: 8px;
  padding: 1px 8px;
  border-radius: 3px;
  font-size: 12px;
  font-family: var(--font-mono);
  font-weight: 600;
  white-space: nowrap;
  letter-spacing: 0.02em;
}

.frame-tag.frame-i {
  background: #dbeafe;
  color: #1d4ed8;
}

.frame-tag.frame-unknown {
  background: #ffebe9;
  color: #cf222e;
}

.frame-tag.frame-s {
  background: #d1fae5;
  color: #047857;
}

.frame-tag.frame-u {
  background: #fef3c7;
  color: #b45309;
}

.message-desc {
  flex: 1;
  color: #24292f;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty-message {
  text-align: center;
  color: #9ca3af;
  padding: 40px;
}

.conn-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.conn-section-header .conn-section-title {
  margin-bottom: 0;
}
</style>
