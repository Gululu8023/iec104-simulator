<template>
  <AppStatusBar>
    <!-- 左侧：状态信息 -->
    <template #status>
      <!-- 连接状态 -->
      <div class="status-indicator">
        <el-icon class="status-link-icon" :class="connectionStatusClass"><Connection /></el-icon>
        <div class="status-info">
          <span class="status-label">{{ t('master.dashboard.status.masterStatus') }}</span>
          <span class="status-value">{{ connectionStatusText }}</span>
        </div>
      </div>

      <!-- 目标地址 -->
      <div class="stat-item">
        <span class="stat-label">{{ t('master.dashboard.status.targetAddress') }}</span>
        <span class="stat-value font-mono">{{ targetAddress }}</span>
      </div>

      <!-- 异常告警提示条 -->
      <transition name="slide-down">
        <div v-if="alerts.length > 0" class="alert-bar" :class="alertLevel">
          <el-icon class="alert-icon"><WarningFilled /></el-icon>
          <span class="alert-text">{{ alerts[0].message }}</span>
          <span class="alert-count" v-if="alerts.length > 1">+{{ alerts.length - 1 }}</span>
          <button class="alert-close" @click="dismissAlert">×</button>
        </div>
      </transition>
    </template>

    <!-- 右侧：彩色大图标命令 -->
    <template #commands>
      <el-tooltip :content="tcpToggleHint" placement="bottom">
        <button
          class="command-btn connect-cmd"
          :class="{
            active: canToggleConnection,
            'is-disconnect': isSlaveConnectedOrConnecting,
            disabled: !canToggleConnection,
          }"
          :disabled="!canToggleConnection"
          @click="handleConnectionAction()"
        >
          <el-icon class="cmd-icon">
            <VideoPause v-if="isSlaveConnectedOrConnecting" />
            <VideoPlay v-else />
          </el-icon>
          <span class="cmd-text">{{ tcpToggleText }}</span>
        </button>
      </el-tooltip>

      <el-tooltip :content="t('master.dashboard.dataTransfer.tooltip')" placement="bottom">
        <button
          class="command-btn cs-cmd"
          :class="{
            active: isTransportConnected && !isDataTransferActionDisabled,
            disabled: isDataTransferActionDisabled,
            loading: isDataTransferTransitioning,
          }"
          :disabled="isDataTransferActionDisabled"
          @click="handleCommand('toggle-data-transfer')"
        >
          <el-icon class="cmd-icon">
            <Promotion />
          </el-icon>
          <span class="cmd-text">{{ dataTransferButtonText }}</span>
        </button>
      </el-tooltip>

      <!-- 总召唤 -->
      <el-tooltip :content="t('master.dashboard.tooltips.generalInterrogation')" placement="bottom">
        <button
          class="command-btn gi-cmd"
          :class="{
            active: isConnected,
            disabled: !canSendGeneralInterrogation,
            loading: loadingCommands?.gi,
          }"
          :disabled="!canSendGeneralInterrogation"
          @click="handleCommand('general-interrogation')"
        >
          <el-icon class="cmd-icon" :class="{ spinning: loadingCommands?.gi }"
            ><Download
          /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.generalInterrogation') }}</span>
        </button>
      </el-tooltip>

      <!-- 电度召唤 -->
      <el-tooltip :content="t('master.dashboard.tooltips.counterInterrogation')" placement="bottom">
        <button
          class="command-btn ci-cmd"
          :class="{
            active: isConnected,
            disabled: !canSendCounterInterrogation,
            loading: loadingCommands?.ci,
          }"
          :disabled="!canSendCounterInterrogation"
          @click="handleCommand('counter-interrogation')"
        >
          <el-icon class="cmd-icon" :class="{ spinning: loadingCommands?.ci }"
            ><Odometer
          /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.counterInterrogation') }}</span>
        </button>
      </el-tooltip>

      <el-tooltip :content="t('master.dashboard.tooltips.counterFreeze')" placement="bottom">
        <button
          class="command-btn ci-cmd"
          :class="{
            active: isConnected,
            disabled: !canSendCounterFreeze,
            loading: loadingCommands?.cf,
          }"
          :disabled="!canSendCounterFreeze"
          @click="handleCommand('counter-freeze')"
        >
          <el-icon class="cmd-icon" :class="{ spinning: loadingCommands?.cf }"
            ><Odometer
          /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.counterFreeze') }}</span>
        </button>
      </el-tooltip>

      <!-- 时钟同步 -->
      <el-tooltip :content="t('master.dashboard.tooltips.clockSync')" placement="bottom">
        <button
          class="command-btn cs-cmd"
          :class="{
            active: isConnected,
            disabled: !canSendClockSync,
            loading: loadingCommands?.cs,
          }"
          :disabled="!canSendClockSync"
          @click="handleCommand('clock-synchronization')"
        >
          <el-icon class="cmd-icon" :class="{ spinning: loadingCommands?.cs }"><Timer /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.clockSync') }}</span>
        </button>
      </el-tooltip>

      <el-tooltip :content="t('master.dashboard.tooltips.resetProcess')" placement="bottom">
        <button
          class="command-btn cs-cmd"
          :class="{
            active: isConnected,
            disabled: !canSendResetProcess,
            loading: loadingCommands?.rp,
          }"
          :disabled="!canSendResetProcess"
          @click="handleCommand('reset-process-command')"
        >
          <el-icon class="cmd-icon" :class="{ spinning: loadingCommands?.rp }"
            ><RefreshRight
          /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.resetProcess') }}</span>
        </button>
      </el-tooltip>

      <!-- 测试命令（ASDU） -->
      <el-tooltip :content="t('master.dashboard.tooltips.testCommand')" placement="bottom">
        <button
          class="command-btn test-cmd"
          :class="{ active: isConnected, disabled: !isConnected }"
          :disabled="!isConnected"
          @click="handleCommand('test-command')"
        >
          <el-icon class="cmd-icon"><Promotion /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.testCommand') }}</span>
        </button>
      </el-tooltip>

      <span class="command-divider" />

      <el-tooltip :content="t('master.dashboard.tooltips.readCommand')" placement="bottom">
        <button
          class="command-btn rc-cmd"
          :class="{ active: isConnected, disabled: !isConnected }"
          :disabled="!isConnected"
          @click="handleCommand('open-read-command')"
        >
          <el-icon class="cmd-icon"><Search /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.readCommand') }}</span>
        </button>
      </el-tooltip>

      <el-tooltip :content="t('master.dashboard.tooltips.remoteControl')" placement="bottom">
        <button
          class="command-btn rc-cmd"
          :class="{ active: isConnected, disabled: !isConnected }"
          :disabled="!isConnected"
          @click="handleCommand('open-remote-control')"
        >
          <el-icon class="cmd-icon"><Operation /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.remoteControl') }}</span>
        </button>
      </el-tooltip>

      <el-tooltip :content="t('master.dashboard.tooltips.remoteAdjust')" placement="bottom">
        <button
          class="command-btn rc-cmd"
          :class="{ active: isConnected, disabled: !isConnected }"
          :disabled="!isConnected"
          @click="handleCommand('open-remote-adjust')"
        >
          <el-icon class="cmd-icon"><SetUp /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.remoteAdjust') }}</span>
        </button>
      </el-tooltip>

      <el-tooltip :content="t('master.dashboard.tooltips.fileTransfer')" placement="bottom">
        <button
          class="command-btn file-cmd"
          :class="{ active: isConnected, disabled: !isConnected }"
          :disabled="!isConnected"
          @click="handleCommand('open-file-transfer')"
        >
          <el-icon class="cmd-icon"><Document /></el-icon>
          <span class="cmd-text">{{ t('master.dashboard.commands.fileTransfer') }}</span>
        </button>
      </el-tooltip>
    </template>
  </AppStatusBar>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  Timer,
  Download,
  Odometer,
  Promotion,
  RefreshRight,
  WarningFilled,
  Connection,
  VideoPause,
  VideoPlay,
  Operation,
  SetUp,
  Document,
  Search,
} from '@element-plus/icons-vue'
import AppStatusBar from '@shared/components/AppStatusBar.vue'
import { t } from '@shared/i18n'
import type { GlobalCommandType, SlaveConnection } from '@/types/master'
import {
  deriveUnifiedConnStatusLocalized,
  isBusinessLinkReadyState,
  isTransportConnectedState,
  normalizeDataTransferState,
} from '@shared/ui/connectionStatus'

const props = defineProps<{
  selectedSlave: SlaveConnection | null
  loadingCommands?: { gi: boolean; ci: boolean; cf: boolean; cs: boolean; rp: boolean }
}>()

const emit = defineEmits<{
  globalCommand: [command: GlobalCommandType]
  connectionAction: [action: 'connect' | 'disconnect']
}>()

// 当前从站
const currentSlave = computed(() => props.selectedSlave)

// 异常告警列表
interface Alert {
  id: string
  level: 'warning' | 'error'
  message: string
  timestamp: number
}
const alerts = ref<Alert[]>([])

// 告警级别 (error 优先)
const alertLevel = computed(() => {
  if (alerts.value.some((a) => a.level === 'error')) return 'error'
  return 'warning'
})

// 关闭告警
const dismissAlert = () => {
  alerts.value.shift()
}

const unifiedStatus = computed(() =>
  deriveUnifiedConnStatusLocalized(
    props.selectedSlave?.transportState,
    props.selectedSlave?.dataTransferState,
    null,
    'client',
  ),
)

// 是否已连接（业务链路 RUNNING/STARTED）
const isConnected = computed(() => {
  return isBusinessLinkReadyState(props.selectedSlave?.dataTransferState)
})

const isTransportConnected = computed(() => {
  return isTransportConnectedState(props.selectedSlave?.transportState)
})

const isSlaveConnectedOrConnecting = computed(() => {
  return (
    Boolean(props.selectedSlave?.reconnecting) || unifiedStatus.value.severity !== 'status-idle'
  )
})

const canConnect = computed(
  () => Boolean(props.selectedSlave) && !isSlaveConnectedOrConnecting.value,
)
const canDisconnect = computed(
  () => Boolean(props.selectedSlave) && isSlaveConnectedOrConnecting.value,
)
const canToggleConnection = computed(() => canConnect.value || canDisconnect.value)
const tcpToggleText = computed(() =>
  isSlaveConnectedOrConnecting.value
    ? t('master.dashboard.tcp.close')
    : t('master.dashboard.tcp.open'),
)
const tcpToggleHint = computed(() => {
  if (!props.selectedSlave) return t('master.dashboard.tcp.selectFirst')
  return isSlaveConnectedOrConnecting.value
    ? t('master.dashboard.tcp.closeHint')
    : t('master.dashboard.tcp.openHint')
})

const isDataTransferStarted = computed(() => {
  return isBusinessLinkReadyState(props.selectedSlave?.dataTransferState)
})

const isDataTransferTransitioning = computed(() => {
  const state = normalizeDataTransferState(props.selectedSlave?.dataTransferState)
  return state === 'starting' || state === 'stopping'
})

const isDataTransferActionDisabled = computed(() => {
  if (!props.selectedSlave?.connectionId) return true
  if (!isTransportConnected.value) return true
  return isDataTransferTransitioning.value
})

const canSendGeneralInterrogation = computed(() => isConnected.value && !props.loadingCommands?.gi)
const canSendCounterInterrogation = computed(() => isConnected.value && !props.loadingCommands?.ci)
const canSendCounterFreeze = computed(() => isConnected.value && !props.loadingCommands?.cf)
const canSendClockSync = computed(() => isConnected.value && !props.loadingCommands?.cs)
const canSendResetProcess = computed(() => isConnected.value && !props.loadingCommands?.rp)

const dataTransferButtonText = computed(() => {
  return isDataTransferStarted.value
    ? t('master.dashboard.dataTransfer.stop')
    : t('master.dashboard.dataTransfer.start')
})

// 连接状态样式
const connectionStatusClass = computed(() => {
  if (props.selectedSlave?.reconnecting) return 'status-processing'
  return unifiedStatus.value.severity
})

// 连接状态文本
const connectionStatusText = computed(() => {
  if (props.selectedSlave?.reconnecting) {
    const attempt = Number(props.selectedSlave.reconnectAttempts ?? 0)
    const maxAttempts = Number(props.selectedSlave.maxReconnectAttempts ?? 0)
    if (attempt > 0 && maxAttempts > 0) {
      return t('master.dashboard.status.reconnectingWithAttempts', { attempt, maxAttempts })
    }
    return t('master.dashboard.status.reconnecting')
  }
  return unifiedStatus.value.text
})

const targetAddress = computed(() => {
  if (!currentSlave.value) return '--'
  const host = String(currentSlave.value.host || '').trim()
  const port = Number(currentSlave.value.port)
  if (!host || !Number.isFinite(port)) return '--'
  return `${host}:${port}`
})

const handleConnectionAction = () => {
  const action: 'connect' | 'disconnect' = isSlaveConnectedOrConnecting.value
    ? 'disconnect'
    : 'connect'
  if (
    (action === 'connect' && !canConnect.value) ||
    (action === 'disconnect' && !canDisconnect.value)
  ) {
    return
  }
  emit('connectionAction', action)
}

// 命令处理
const handleCommand = (command: GlobalCommandType) => {
  if (command === 'toggle-data-transfer') {
    if (isDataTransferActionDisabled.value) return
    emit('globalCommand', command)
    return
  }
  if (command === 'general-interrogation' && !canSendGeneralInterrogation.value) return
  if (command === 'counter-interrogation' && !canSendCounterInterrogation.value) return
  if (command === 'counter-freeze' && !canSendCounterFreeze.value) return
  if (command === 'clock-synchronization' && !canSendClockSync.value) return
  if (
    (command === 'open-remote-control' ||
      command === 'open-remote-adjust' ||
      command === 'open-file-transfer') &&
    !isConnected.value
  ) {
    return
  }
  if (!isConnected.value) return
  emit('globalCommand', command)
}
</script>

<style scoped>
:deep(.command-btn.connect-cmd.active .cmd-icon) {
  color: #2563eb;
}

:deep(.command-btn.connect-cmd.is-disconnect .cmd-icon) {
  color: #ef4444;
}

/* 加载旋转动画 */
.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
