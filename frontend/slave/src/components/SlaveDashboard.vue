\
<template>
  <AppStatusBar>
    <!-- 左侧：状态信息 -->
    <template #status>
      <!-- 连接状态 -->
      <div class="status-indicator">
        <el-icon class="status-link-icon" :class="connectionStatus"><Connection /></el-icon>
        <div class="status-info">
          <span class="status-label">{{ t('slave.dashboard.statusLabel') }}</span>
          <span class="status-value">{{ statusText }}</span>
        </div>
      </div>

      <!-- 监听地址 -->
      <div class="stat-item">
        <span class="stat-label">{{ t('slave.dashboard.listenAddress') }}</span>
        <el-tooltip
          :content="
            canCopyListenAddress
              ? t('slave.dashboard.copyListenAddress')
              : t('slave.dashboard.selectSlaveFirst')
          "
          placement="bottom"
        >
          <span
            class="stat-value font-mono copyable-address"
            :class="{ disabled: !canCopyListenAddress }"
            @click="handleCopyListenAddress"
          >
            {{ listenAddress }}
          </span>
        </el-tooltip>
      </div>

      <template v-if="showPendingSplit">
        <div class="stat-item">
          <span class="stat-label">{{ t('slave.dashboard.txUnconfirmed') }}</span>
          <span class="stat-value">
            <span class="highlight-number" :class="{ warning: txUnconfirmedWarning }">{{
              txUnconfirmedDisplay
            }}</span>
            <span class="value-suffix">{{ t('slave.dashboard.iFrame') }}</span>
          </span>
        </div>
        <div class="stat-item">
          <span class="stat-label">{{ t('slave.dashboard.rxPending') }}</span>
          <span class="stat-value">
            <span class="highlight-number" :class="{ warning: rxPendingWarning }">{{
              rxPendingDisplay
            }}</span>
            <span class="value-suffix">{{ t('slave.dashboard.sFrame') }}</span>
          </span>
        </div>
      </template>
    </template>

    <!-- 右侧：从站模拟命令 -->
    <template #commands>
      <el-tooltip :content="listenToggleTooltip" placement="bottom">
        <button
          class="command-btn listen-cmd"
          :class="{
            active: canToggleListener,
            'is-running': isRunning,
            disabled: !canToggleListener,
          }"
          :disabled="!canToggleListener"
          @click="handleCommand('toggle-listener')"
        >
          <el-icon class="cmd-icon">
            <VideoPause v-if="isRunning" />
            <VideoPlay v-else />
          </el-icon>
          <span class="cmd-text">{{ listenToggleText }}</span>
        </button>
      </el-tooltip>

      <span class="command-divider" />

      <!-- 全局命令-命令地址映射 -->
      <el-tooltip :content="mappingTooltip" placement="bottom">
        <button
          class="command-btn mapping-cmd"
          :class="{ active: canOpenGlobalMapping, disabled: !canOpenGlobalMapping }"
          :disabled="!canOpenGlobalMapping"
          @click="handleCommand('global-mapping')"
        >
          <el-icon class="cmd-icon"><Link /></el-icon>
          <span class="cmd-text">{{ t('slave.dashboard.mapping.command') }}</span>
        </button>
      </el-tooltip>

      <!-- 自动模拟 -->
      <el-tooltip :content="simulationTooltip" placement="bottom">
        <button
          class="command-btn simulation-cmd"
          :class="{
            active: canToggleSimulation,
            'is-enabled': globalSimulationEnabled,
            disabled: !canToggleSimulation,
          }"
          :disabled="!canToggleSimulation"
          @click="handleCommand('toggle-simulation')"
        >
          <el-icon class="cmd-icon">
            <VideoPause v-if="globalSimulationEnabled" />
            <VideoPlay v-else />
          </el-icon>
          <span class="cmd-text">{{ simulationToggleText }}</span>
        </button>
      </el-tooltip>

      <!-- 雪崩测试 -->
      <el-tooltip :content="avalancheTooltip" placement="bottom">
        <button
          class="command-btn gi-cmd"
          :class="{ active: canToggleSimulation, disabled: !canToggleSimulation }"
          :disabled="!canToggleSimulation"
          @click="handleCommand('avalanche-test')"
        >
          <el-icon class="cmd-icon"><Lightning /></el-icon>
          <span class="cmd-text">{{ t('slave.dashboard.avalanche.title') }}</span>
        </button>
      </el-tooltip>

      <!-- 强制上送 -->
      <el-tooltip :content="forceSendTooltip" placement="bottom">
        <button
          class="command-btn cs-cmd"
          :class="{ active: canToggleSimulation, disabled: !canToggleSimulation }"
          :disabled="!canToggleSimulation"
          @click="handleCommand('force-send')"
        >
          <el-icon class="cmd-icon"><Upload /></el-icon>
          <span class="cmd-text">{{ t('slave.dashboard.forceSend.title') }}</span>
        </button>
      </el-tooltip>
    </template>
  </AppStatusBar>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Lightning, Upload, VideoPause, VideoPlay, Connection, Link } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import AppStatusBar from '@shared/components/AppStatusBar.vue'
import { t } from '@shared/i18n'
import type { DeviceNode } from '@/types/slave'
import { deriveUnifiedConnStatus } from '@shared/ui/connectionStatus'

interface Props {
  selectedSlave: DeviceNode | null
  stats?: {
    connectingCount?: number
    errorCount?: number
    singlePointCount?: number
    measuredCount?: number
    pendingQueue?: number
    txUnconfirmed?: number | null
    rxPending?: number | null
  }
  globalSimulationEnabled?: boolean
  syncRevision?: number
}

const props = withDefaults(defineProps<Props>(), {
  stats: () => ({
    connectingCount: 0,
    errorCount: 0,
    singlePointCount: 0,
    measuredCount: 0,
    pendingQueue: 0,
  }),
  globalSimulationEnabled: false,
})

const emit = defineEmits<{
  slaveCommand: [command: string]
}>()

const unifiedStatus = computed(() => {
  const nodeStatus = String(props.selectedSlave?.status ?? '')
    .trim()
    .toLowerCase()
  if (!nodeStatus) {
    return deriveUnifiedConnStatus('Disconnected', 'Stopped', null, 'server')
  }

  if (nodeStatus === 'error' || nodeStatus === 'status-error') {
    return deriveUnifiedConnStatus('Error', 'Error', null, 'server')
  }
  if (nodeStatus === 'stopping') {
    return deriveUnifiedConnStatus('Connected', 'Stopping', null, 'server')
  }
  if (nodeStatus === 'starting' || nodeStatus === 'status-processing') {
    return deriveUnifiedConnStatus('Connecting', 'Stopped', null, 'server')
  }
  if (nodeStatus === 'connecting') {
    return deriveUnifiedConnStatus('Connected', 'Starting', null, 'server')
  }
  if (nodeStatus === 'connected' || nodeStatus === 'status-unready') {
    return deriveUnifiedConnStatus('Connected', 'Stopped', null, 'server')
  }
  if (nodeStatus === 'running' || nodeStatus === 'normal' || nodeStatus === 'status-normal') {
    return deriveUnifiedConnStatus('Connected', 'Started', null, 'server')
  }
  // 监听中：TCP Server 已启动但无客户端接入
  if (nodeStatus === 'listening' || nodeStatus === 'status-listening') {
    return deriveUnifiedConnStatus('Listening', 'Stopped', null, 'server')
  }
  return deriveUnifiedConnStatus('Disconnected', 'Stopped', null, 'server')
})

// 是否运行中
const isRunning = computed(() => {
  const severity = unifiedStatus.value.severity
  return (
    severity === 'status-listening' ||
    severity === 'status-processing' ||
    severity === 'status-unready' ||
    severity === 'status-normal'
  )
})

// 连接状态样式
const connectionStatus = computed(() => unifiedStatus.value.severity)

// 状态文本
const statusText = computed(() => {
  if (!props.selectedSlave) return t('slave.dashboard.noSelection')
  switch (connectionStatus.value) {
    case 'status-listening':
      return t('slave.dashboard.status.listening')
    case 'status-processing':
      if (
        String(props.selectedSlave?.status || '')
          .trim()
          .toLowerCase() === 'stopping'
      ) {
        return t('slave.dashboard.status.stoppingLink')
      }
      if (
        String(props.selectedSlave?.status || '')
          .trim()
          .toLowerCase() === 'starting' ||
        String(props.selectedSlave?.status || '')
          .trim()
          .toLowerCase() === 'connecting'
      ) {
        return t('slave.dashboard.status.startingLink')
      }
      return t('slave.dashboard.status.starting')
    case 'status-unready':
      return t('slave.dashboard.status.waitingLink')
    case 'status-normal':
      return t('slave.dashboard.status.running')
    case 'status-error':
      return t('slave.dashboard.status.error')
    default:
      return t('slave.dashboard.status.stopped')
  }
})

// 监听地址
const listenAddress = computed(() => {
  if (!props.selectedSlave) return '--'
  const host =
    String(props.selectedSlave.host || props.selectedSlave.address || '').trim() || '0.0.0.0'
  const port = props.selectedSlave.port || 2404
  return `${host}:${port}`
})

// 统计数据
const isSlaveSelected = computed(() => props.selectedSlave?.type === 'slave')
const canToggleListener = computed(
  () => props.selectedSlave?.type === 'listener' || props.selectedSlave?.type === 'slave',
)
const listenToggleText = computed(() =>
  isRunning.value ? t('slave.dashboard.listen.stop') : t('slave.dashboard.listen.start'),
)
const listenToggleTooltip = computed(() => {
  if (!canToggleListener.value) return t('slave.dashboard.listen.selectConnectionOrSlave')
  return isRunning.value
    ? t('slave.dashboard.listen.stopCurrent')
    : t('slave.dashboard.listen.startCurrent')
})
const mappingTooltip = computed(() =>
  canOpenGlobalMapping.value
    ? t('slave.dashboard.mapping.tooltip')
    : t('slave.dashboard.mapping.selectSlaveFirst'),
)
const simulationToggleText = computed(() =>
  globalSimulationEnabled.value
    ? t('slave.dashboard.simulation.stop')
    : t('slave.dashboard.simulation.start'),
)
const simulationTooltip = computed(() => {
  if (!canToggleSimulation.value) return t('slave.dashboard.simulation.selectRunningSlave')
  return globalSimulationEnabled.value
    ? t('slave.dashboard.simulation.stopTooltip')
    : t('slave.dashboard.simulation.startTooltip')
})
const avalancheTooltip = computed(() =>
  canToggleSimulation.value
    ? t('slave.dashboard.avalanche.tooltip')
    : t('slave.dashboard.simulation.selectRunningSlave'),
)
const forceSendTooltip = computed(() =>
  canToggleSimulation.value
    ? t('slave.dashboard.forceSend.tooltip')
    : t('slave.dashboard.simulation.selectRunningSlave'),
)
const canCopyListenAddress = computed(() => canToggleListener.value && listenAddress.value !== '--')
const globalSimulationEnabled = computed(() => !!props.globalSimulationEnabled)
const canOpenGlobalMapping = computed(() => isSlaveSelected.value)
const canToggleSimulation = computed(() => isSlaveSelected.value && isRunning.value)
const diagnosticScopeKey = computed(() => {
  const stationId = String(
    props.selectedSlave?.stationId || props.selectedSlave?.parentStationId || '',
  ).trim()
  if (stationId) return stationId
  return String(props.selectedSlave?.id || '').trim()
})

const normalizeCounter = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  return Math.max(0, Math.trunc(numeric))
}

const txUnconfirmedValue = computed(() => normalizeCounter(props.stats?.txUnconfirmed))
const rxPendingValue = computed(() => normalizeCounter(props.stats?.rxPending))
const hasPendingAnomaly = computed(
  () => (txUnconfirmedValue.value ?? 0) > 0 || (rxPendingValue.value ?? 0) > 0,
)
const pendingAnomalyStreak = ref(0)
const pendingDiagnosticsVisible = ref(false)

watch(
  [() => props.syncRevision ?? 0, hasPendingAnomaly, diagnosticScopeKey],
  ([revision, hasAnomaly, scopeKey], [previousRevision, _previousHasAnomaly, previousScopeKey]) => {
    if (!scopeKey || scopeKey !== previousScopeKey) {
      pendingAnomalyStreak.value = 0
      pendingDiagnosticsVisible.value = false
    }

    if (!scopeKey || revision === previousRevision) {
      return
    }

    if (hasAnomaly) {
      pendingAnomalyStreak.value += 1
      if (pendingAnomalyStreak.value >= 2) {
        pendingDiagnosticsVisible.value = true
      }
      return
    }

    pendingAnomalyStreak.value = 0
    pendingDiagnosticsVisible.value = false
  },
  { immediate: true },
)

const showPendingSplit = computed(() => pendingDiagnosticsVisible.value)
const txUnconfirmedDisplay = computed(() => {
  if (!props.selectedSlave) return '--'
  return txUnconfirmedValue.value ?? 0
})
const rxPendingDisplay = computed(() => {
  if (!props.selectedSlave) return '--'
  return rxPendingValue.value ?? 0
})
const txUnconfirmedWarning = computed(() => (txUnconfirmedValue.value ?? 0) > 10)
const rxPendingWarning = computed(() => (rxPendingValue.value ?? 0) > 10)

const handleCopyListenAddress = async () => {
  if (!canCopyListenAddress.value) return
  try {
    await navigator.clipboard.writeText(listenAddress.value)
    ElMessage.success({
      message: t('slave.dashboard.messages.copiedListenAddress'),
      duration: 1200,
      grouping: true,
    })
  } catch {
    ElMessage.warning(t('slave.dashboard.messages.copyFailed'))
  }
}

// 命令处理
const handleCommand = (command: string) => {
  if (command === 'toggle-listener') {
    if (!canToggleListener.value) return
    emit('slaveCommand', command)
    return
  }
  if (command === 'global-mapping') {
    if (!canOpenGlobalMapping.value) return
    emit('slaveCommand', command)
    return
  }
  if (command === 'toggle-simulation' || command === 'avalanche-test' || command === 'force-send') {
    if (!canToggleSimulation.value) return
    emit('slaveCommand', command)
    return
  }
  if (!isRunning.value) return
  emit('slaveCommand', command)
}
</script>

<style scoped>
/* 扩展样式由 AppStatusBar 提供 */
:deep(.status-link-icon.listening),
:deep(.status-link-icon.status-listening) {
  color: #3b82f6;
  filter: drop-shadow(0 0 6px #93c5fd);
  animation: link-breathe 1.8s ease-in-out infinite;
}

:deep(.status-link-icon.status-normal) {
  color: #10b981;
  filter: drop-shadow(0 0 6px rgba(16, 185, 129, 0.6));
}

:deep(.status-link-icon.status-unready) {
  color: #f59e0b;
  filter: drop-shadow(0 0 6px rgba(245, 158, 11, 0.4));
}

:deep(.status-link-icon.status-processing) {
  color: #f59e0b;
  filter: drop-shadow(0 0 6px rgba(245, 158, 11, 0.6));
}

:deep(.status-link-icon.error),
:deep(.status-link-icon.status-error) {
  color: #ef4444;
  filter: drop-shadow(0 0 6px #fca5a5);
}

:deep(.status-link-icon.disconnected),
:deep(.status-link-icon.status-idle) {
  color: #9ca3af;
  filter: none;
}

.copyable-address {
  cursor: pointer;
  user-select: all;
  transition: color 0.2s ease;
}

.copyable-address:hover {
  color: #2563eb;
}

.copyable-address.disabled {
  cursor: not-allowed;
  opacity: 0.65;
}

:deep(.command-btn.listen-cmd.active .cmd-icon) {
  color: #2563eb;
}

:deep(.command-btn.listen-cmd.is-running .cmd-icon) {
  color: #ef4444;
}

:deep(.command-btn.listen-cmd.disabled .cmd-icon),
:deep(.command-btn.listen-cmd.disabled .cmd-text) {
  color: #9ca3af;
}
</style>
