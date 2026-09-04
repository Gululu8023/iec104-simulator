<template>
  <AppMenuBar>
    <!-- 文件菜单 -->
    <el-dropdown trigger="hover" @command="handleFileCommand" popper-class="app-menu-dropdown">
      <span class="menu-item">{{ t('appMenu.file') }} <span class="arrow">▾</span></span>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item command="new-station">
            <el-icon><Plus /></el-icon> {{ t('appMenu.newConnection') }}
          </el-dropdown-item>
          <el-dropdown-item command="new-slave">
            <el-icon><Plus /></el-icon> {{ t('appMenu.newSlave') }}
          </el-dropdown-item>
          <el-dropdown-item
            divided
            command="import-point-defs"
            :disabled="!hasPointTableImportContext"
            :title="pointTableImportHint"
          >
            <el-icon><Document /></el-icon> {{ t('appMenu.importPointTable') }}
          </el-dropdown-item>
          <el-dropdown-item divided command="exit">
            <el-icon><Close /></el-icon> {{ t('appMenu.exit') }}
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>

    <!-- 编辑菜单 -->
    <el-dropdown trigger="hover" @command="handleEditCommand" popper-class="app-menu-dropdown">
      <span class="menu-item">{{ t('appMenu.edit') }} <span class="arrow">▾</span></span>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item
            command="edit-connection"
            :disabled="!hasListenerContext"
            :title="listenerContextHint"
          >
            <el-icon><Edit /></el-icon> {{ t('appMenu.editConnection') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="delete-connection"
            :disabled="!hasListenerContext"
            :title="listenerContextHint"
            class="menu-item-danger"
          >
            <el-icon><Delete /></el-icon> {{ t('appMenu.deleteConnection') }}
          </el-dropdown-item>
          <el-dropdown-item
            divided
            command="edit-slave"
            :disabled="!hasSlaveContext"
            :title="slaveContextHint"
          >
            <el-icon><Edit /></el-icon> {{ t('appMenu.editSlave') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="delete-slave"
            :disabled="!hasSlaveContext"
            :title="slaveContextHint"
            class="menu-item-danger"
          >
            <el-icon><Delete /></el-icon> {{ t('appMenu.deleteSlave') }}
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>

    <!-- 运行菜单 -->
    <el-dropdown trigger="hover" @command="handleRunCommand" popper-class="app-menu-dropdown">
      <span class="menu-item">{{ t('appMenu.run') }} <span class="arrow">▾</span></span>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item
            command="start-listening"
            :disabled="!hasListenerContext || isConnected"
            :title="listenerContextHint"
          >
            <el-icon><Connection /></el-icon> {{ t('appMenu.startListening') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="stop-listening"
            :disabled="!hasListenerContext || !isConnected"
            :title="listenerContextHint"
          >
            <el-icon><CloseBold /></el-icon> {{ t('appMenu.stopListening') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="restart-listening"
            :disabled="!hasListenerContext"
            :title="listenerContextHint"
          >
            <el-icon><Refresh /></el-icon> {{ t('appMenu.restartListening') }}
          </el-dropdown-item>
          <el-dropdown-item
            divided
            command="restart-listening"
            :disabled="!hasListenerContext || !isConnected"
            :title="listenerContextHint"
          >
            <el-icon><RefreshLeft /></el-icon> {{ t('appMenu.restartListening') }}
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>

    <AppCommonMenus
      :help-doc-menu-items="slaveHelpDocMenuItems"
      @view-command="handleViewCommand"
      @menu-command="emit('menuAction', $event)"
    />
    <AppLocalClock :offset-ms="stationTimeOffsetMs" :label="t('appMenu.simulatedTime')" />
  </AppMenuBar>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Plus,
  Edit,
  Delete,
  Close,
  Connection,
  CloseBold,
  Refresh,
  RefreshLeft,
  Document,
} from '@element-plus/icons-vue'
import AppCommonMenus from '@shared/components/AppCommonMenus.vue'
import AppMenuBar from '@shared/components/AppMenuBar.vue'
import AppLocalClock from '@shared/components/AppLocalClock.vue'
import { getSlaveHelpDocMenuItems } from '@shared/content/helpDocs'
import { t } from '@shared/i18n'

import type { DeviceNode } from '@/types/slave'

// Props
const props = defineProps<{
  selectedSlave?: DeviceNode | null
  stationTimeOffsetMs?: number
}>()

// Emits
const emit = defineEmits<{
  slaveOperation: [operation: string, slave: DeviceNode | null]
  menuAction: [action: string]
}>()

// 是否已连接
const isConnected = computed(() => {
  if (!props.selectedSlave) return false
  return (
    props.selectedSlave.status === 'running' ||
    props.selectedSlave.status === 'listening' ||
    props.selectedSlave.status === 'connected'
  )
})

const hasListenerContext = computed(() => props.selectedSlave?.type === 'listener')

const hasSlaveContext = computed(() => {
  const node = props.selectedSlave
  return node?.type === 'slave'
})

const hasPointTableImportContext = computed(() => {
  const node = props.selectedSlave
  return node?.type === 'listener' || node?.type === 'slave'
})

const listenerContextHint = computed(() =>
  hasListenerContext.value ? '' : t('appMenu.chooseConnection'),
)

const slaveContextHint = computed(() => (hasSlaveContext.value ? '' : t('appMenu.chooseSlave')))

const pointTableImportHint = computed(() =>
  hasPointTableImportContext.value ? '' : t('appMenu.chooseConnectionOrSlave'),
)
const slaveHelpDocMenuItems = computed(() => getSlaveHelpDocMenuItems())

// 菜单命令处理
const handleFileCommand = (command: string) => {
  switch (command) {
    case 'new-station':
      emit('slaveOperation', 'create-station', null)
      break
    case 'new-slave':
      emit('slaveOperation', 'create-slave', null)
      break
    default:
      emit('menuAction', command)
  }
}

const handleEditCommand = (command: string) => {
  switch (command) {
    case 'edit-slave':
      emit('slaveOperation', 'edit-slave', props.selectedSlave || null)
      break
    case 'edit-connection':
      emit('slaveOperation', 'edit-connection', props.selectedSlave || null)
      break
    case 'delete-connection':
      emit('slaveOperation', 'delete-connection', props.selectedSlave || null)
      break
    case 'delete-slave':
      emit('slaveOperation', 'delete-slave', props.selectedSlave || null)
      break
    default:
      emit('menuAction', command)
  }
}

const handleRunCommand = (command: string) => {
  emit('slaveOperation', command, props.selectedSlave || null)
}

const handleViewCommand = (command: string) => {
  switch (command) {
    case 'toggle-message-panel':
    case 'refresh-devices':
      emit('slaveOperation', command, null)
      break
    default:
      emit('menuAction', command)
  }
}
</script>

<style scoped>
/* 样式由 AppMenuBar 提供 */
</style>
