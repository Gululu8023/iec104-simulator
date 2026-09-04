<template>
  <AppMenuBar>
    <!-- 文件菜单 -->
    <el-dropdown trigger="hover" @command="handleFileCommand" popper-class="app-menu-dropdown">
      <span class="menu-item">{{ t('appMenu.file') }} <span class="arrow">▾</span></span>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item command="new-connection">
            <el-icon><Plus /></el-icon> {{ t('appMenu.newConnection') }}
          </el-dropdown-item>
          <el-dropdown-item command="new-station" :disabled="!canOperateLink">
            <el-icon><Plus /></el-icon> {{ t('appMenu.newSlave') }}
          </el-dropdown-item>
          <el-dropdown-item
            divided
            command="import-point-defs"
            :disabled="!canOperateStation"
            :title="stationContextHint"
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
            command="edit-link"
            :disabled="!canOperateLink"
            :title="linkContextHint"
          >
            <el-icon><Edit /></el-icon> {{ t('appMenu.editConnection') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="delete-link"
            :disabled="!canOperateLink"
            :title="linkContextHint"
            class="menu-item-danger"
          >
            <el-icon><Delete /></el-icon> {{ t('appMenu.deleteConnection') }}
          </el-dropdown-item>
          <el-dropdown-item
            divided
            command="edit-slave"
            :disabled="!canOperateStation"
            :title="stationContextHint"
          >
            <el-icon><Edit /></el-icon> {{ t('appMenu.editSlave') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="delete-slave"
            :disabled="!canOperateStation"
            :title="stationContextHint"
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
          <el-dropdown-item command="connect" :disabled="!canConnect">
            <el-icon><Connection /></el-icon> {{ t('appMenu.connect') }}
          </el-dropdown-item>
          <el-dropdown-item command="disconnect" :disabled="!canDisconnect">
            <el-icon><CloseBold /></el-icon> {{ t('appMenu.disconnect') }}
          </el-dropdown-item>
          <el-dropdown-item
            command="reconnect"
            :disabled="!canOperateLink"
            :title="linkContextHint"
          >
            <el-icon><Refresh /></el-icon> {{ t('appMenu.reconnect') }}
          </el-dropdown-item>
          <el-dropdown-item divided command="clear-logs">
            <el-icon><DeleteFilled /></el-icon> {{ t('appMenu.clearLogs') }}
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>

    <AppCommonMenus
      :help-doc-menu-items="masterHelpDocMenuItems"
      @view-command="handleViewCommand"
      @menu-command="emit('menuAction', $event)"
    />
    <AppLocalClock />
  </AppMenuBar>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Plus,
  Edit,
  Delete,
  DeleteFilled,
  Close,
  Document,
  Connection,
  CloseBold,
  Refresh,
} from '@element-plus/icons-vue'
import AppCommonMenus from '@shared/components/AppCommonMenus.vue'
import AppMenuBar from '@shared/components/AppMenuBar.vue'
import AppLocalClock from '@shared/components/AppLocalClock.vue'
import { getMasterHelpDocMenuItems } from '@shared/content/helpDocs'
import { deriveUnifiedConnStatus } from '@shared/ui/connectionStatus'
import { t } from '@shared/i18n'

// Props
const props = defineProps<{
  selectedSlave?: any
}>()

// Emits
const emit = defineEmits<{
  menuAction: [action: string]
  connectionAction: [action: string]
}>()

// 派生连接状态
const isSlaveConnectedOrConnecting = computed(() => {
  if (!props.selectedSlave) return false
  const status = deriveUnifiedConnStatus(
    props.selectedSlave.transportState,
    props.selectedSlave.dataTransferState,
  )
  return status.severity !== 'status-idle'
})

const canConnect = computed(
  () => Boolean(props.selectedSlave) && !isSlaveConnectedOrConnecting.value,
)
const canDisconnect = computed(
  () => Boolean(props.selectedSlave) && isSlaveConnectedOrConnecting.value,
)
const canOperateLink = computed(() => Boolean(props.selectedSlave))
const canOperateStation = computed(() => Boolean(props.selectedSlave))

const linkContextHint = computed(() =>
  canOperateLink.value ? '' : t('appMenu.chooseConnectionOrSlave'),
)
const stationContextHint = computed(() => (canOperateStation.value ? '' : t('appMenu.chooseSlave')))
const masterHelpDocMenuItems = computed(() => getMasterHelpDocMenuItems())

// === 菜单命令处理 ===

const handleFileCommand = (command: string) => {
  switch (command) {
    case 'new-connection':
      emit('connectionAction', 'add-connection')
      break
    case 'new-station':
      emit('connectionAction', 'add-slave')
      break
    case 'import-point-defs':
      emit('connectionAction', 'import-point-defs')
      break
    case 'exit':
      emit('menuAction', 'exit')
      break
  }
}

const handleEditCommand = (command: string) => {
  switch (command) {
    case 'edit-link':
      emit('connectionAction', 'edit-link')
      break
    case 'delete-link':
      emit('connectionAction', 'delete-link')
      break
    case 'edit-slave':
      emit('connectionAction', 'edit')
      break
    case 'delete-slave':
      emit('connectionAction', 'delete')
      break
  }
}

const handleRunCommand = (command: string) => {
  switch (command) {
    case 'connect':
    case 'disconnect':
    case 'reconnect':
      emit('connectionAction', command)
      break
    case 'clear-logs':
      emit('menuAction', 'clear-logs')
      break
  }
}

const handleViewCommand = (command: string) => {
  switch (command) {
    case 'toggle-message-panel':
    case 'toggle-soe-panel':
    case 'refresh-devices':
      emit('connectionAction', command)
      break
    default:
      emit('menuAction', command)
  }
}
</script>

<style scoped>
/* 样式由 AppMenuBar 提供 */
</style>
