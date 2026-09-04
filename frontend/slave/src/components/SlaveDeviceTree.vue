<template>
  <div class="slave-device-tree">
    <div class="sidebar-tree">
      <el-scrollbar class="sidebar-tree-scroll">
        <div v-if="deviceTreeData.length === 0" class="sidebar-empty-state">
          <div class="sidebar-empty-title">{{ t('slave.deviceTree.empty.title') }}</div>
          <div class="sidebar-empty-hint">{{ t('slave.deviceTree.empty.hint') }}</div>
          <el-button type="primary" size="small" @click="handleCreateListener">{{
            t('slave.deviceTree.empty.createConnection')
          }}</el-button>
        </div>

        <el-tree
          v-else
          ref="treeRef"
          :data="deviceTreeData"
          :props="treeProps"
          node-key="id"
          :default-expanded-keys="expandedNodeKeys"
          :expand-on-click-node="false"
          :highlight-current="true"
          @node-click="handleNodeClick"
          @node-contextmenu="handleNodeContextMenu"
          @node-expand="handleNodeExpand"
          @node-collapse="handleNodeCollapse"
          :indent="16"
        >
          <template #default="{ node, data }">
            <SlaveTreeNode :node="node" :data="data" :device-tree-data="deviceTreeData" />
          </template>
        </el-tree>
      </el-scrollbar>

      <div
        v-show="nodeContextMenu.visible"
        class="app-context-menu tree-context-menu"
        :style="{ left: `${nodeContextMenu.x}px`, top: `${nodeContextMenu.y}px` }"
      >
        <button
          v-if="isContextListener"
          class="app-context-menu-item"
          :disabled="isContextNodeRunning"
          @click.stop="handleContextMenuAction('open-connection')"
        >
          <el-icon><VideoPlay /></el-icon>
          {{ t('slave.deviceTree.context.startListening') }}
        </button>
        <button
          v-if="isContextListener"
          class="app-context-menu-item"
          :disabled="!isContextNodeRunning"
          @click.stop="handleContextMenuAction('close-connection')"
        >
          <el-icon><VideoPause /></el-icon>
          {{ t('slave.deviceTree.context.stopListening') }}
        </button>
        <div v-if="isContextListener" class="app-context-menu-divider"></div>
        <button
          v-if="isContextSlave"
          class="app-context-menu-item"
          @click.stop="handleContextMenuAction('edit-slave')"
        >
          <el-icon><Edit /></el-icon>
          {{ t('slave.deviceTree.context.editSlave') }}
        </button>
        <button
          v-if="isContextListener"
          class="app-context-menu-item"
          @click.stop="handleContextMenuAction('create-slave')"
        >
          <el-icon><Plus /></el-icon>
          {{ t('slave.deviceTree.context.createSlave') }}
        </button>
        <button
          v-if="isContextListener"
          class="app-context-menu-item"
          @click.stop="handleContextMenuAction('edit-connection')"
        >
          <el-icon><Setting /></el-icon>
          {{ t('slave.deviceTree.context.editConnection') }}
        </button>
        <div v-if="isContextListener || isContextSlave" class="app-context-menu-divider"></div>
        <button
          class="app-context-menu-item danger"
          @click.stop="handleContextMenuAction('delete-slave')"
        >
          <el-icon><Delete /></el-icon>
          {{
            isContextListener
              ? t('slave.deviceTree.context.deleteConnection')
              : t('slave.deviceTree.context.deleteSlave')
          }}
        </button>
      </div>
    </div>

    <SlaveStationProfileDialogs
      ref="profileDialogsRef"
      v-model:selected-slave="selectedSlave"
      :stations="stations"
      :slaves-by-station="slavesByStation"
      :station-id="stationId"
      :host="host"
      :port="port"
      :default-common-address="defaultCommonAddress"
      :link-params="linkParams"
      :redundancy-mode="redundancyMode"
      :device-tree-data="deviceTreeData"
      @station-profile-submit="emit('stationProfileSubmit', $event)"
    />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { Plus, Delete, VideoPlay, VideoPause, Edit, Setting } from '@element-plus/icons-vue'
import type { DeviceNode } from '@/types/slave'
import SlaveStationProfileDialogs from './SlaveStationProfileDialogs.vue'
import SlaveTreeNode from './SlaveTreeNode.vue'
import { useSlaveTreeModel } from '../composables/useSlaveTreeModel'
import { type StationProfileSubmitPayload } from '../composables/useSlaveStationProfileDialogs'
import { useSlaveTreeInteraction } from '../composables/useSlaveTreeInteraction'
import type { PointTypeSummary } from '../types/pointTypeSummary'
import { t } from '@shared/i18n'
import type {
  LinkParams,
  RedundancyGroupMode,
  StationSummary,
  SlaveResponse,
} from '@shared/api/types'

interface Props {
  stations?: StationSummary[]
  slavesByStation?: Record<string, SlaveResponse[]>
  slavePointTypeSummary?: Record<number, PointTypeSummary>
  stationId?: string
  selectedSlaveStatus?: DeviceNode['status'] | null
  stationRuntimeStatusMap?: Record<string, DeviceNode['status']>
  stationName?: string
  host?: string
  port?: number
  defaultCommonAddress?: number
  linkParams?: LinkParams
  redundancyMode?: RedundancyGroupMode
  isRunning?: boolean
  stationPointTypeSummary?: Record<string, PointTypeSummary>
}

const props = withDefaults(defineProps<Props>(), {
  stations: () => [],
  stationId: '',
  selectedSlaveStatus: null,
  stationRuntimeStatusMap: () => ({}),
  slavesByStation: () => ({}),
  slavePointTypeSummary: () => ({}),
  stationName: '',
  host: '0.0.0.0',
  port: 2404,
  defaultCommonAddress: 1,
  linkParams: () => ({
    k_value: 12,
    w_value: 8,
    t0_seconds: 30,
    t1_seconds: 15,
    t2_seconds: 10,
    t3_seconds: 20,
    max_asdu_bytes: 249,
    select_timeout_seconds: 15,
    enable_sq1_upload: false,
    enable_soe: false,
    unknown_typeid_negative_ack: true,
  }),
  redundancyMode: 'single',
  isRunning: false,
  stationPointTypeSummary: () => ({}),
})

// 组件引用
const treeRef = ref()

const deviceTreeData = ref<DeviceNode[]>([])
const selectedSlave = ref<DeviceNode | null>(null)
const expandedNodeKeys = ref<string[]>([])
const hasAppliedInitialAutoExpand = ref(false)
const stationRuntimeStatusCache = ref<Record<string, DeviceNode['status']>>({})
const { resolveStationNodeStatus, rebuildDeviceTreeFromBackend } = useSlaveTreeModel({
  treeRef,
  deviceTreeData,
  selectedSlave,
  expandedNodeKeys,
  hasAppliedInitialAutoExpand,
  stationRuntimeStatusCache,
  source: props,
})
const emit = defineEmits<{
  typeIdClick: [typeIdNode: DeviceNode]
  nodeClick: [node: DeviceNode]
  slaveOperation: [operation: string, slave: DeviceNode | null]
  refreshRequested: []
  stationProfileSubmit: [payload: StationProfileSubmitPayload]
}>()

const {
  nodeContextMenu,
  isContextNodeRunning,
  isContextListener,
  isContextSlave,
  handleNodeClick,
  handleNodeContextMenu,
  handleContextMenuAction,
  handleNodeExpand,
  handleNodeCollapse,
} = useSlaveTreeInteraction({
  treeRef,
  selectedSlave,
  deviceTreeData,
  expandedNodeKeys,
  emitNodeClick: (node) => emit('nodeClick', node),
  emitTypeIdClick: (node) => emit('typeIdClick', node),
  emitSlaveOperation: (operation, slave) => emit('slaveOperation', operation, slave),
})

const profileDialogsRef = ref<InstanceType<typeof SlaveStationProfileDialogs> | null>(null)
const refreshDevices = () => emit('refreshRequested')
const handleCreateListener = () => profileDialogsRef.value?.handleCreateListener()
const handleCreateSlave = (node?: DeviceNode | null) =>
  profileDialogsRef.value?.handleCreateSlave(node)
const handleEditSlave = (node: DeviceNode | null) => profileDialogsRef.value?.handleEditSlave(node)
const handleEditConnection = (node: DeviceNode | null) =>
  profileDialogsRef.value?.handleEditConnection(node)

const treeProps = {
  children: 'children',
  label: 'label',
}

// 组件挂载时初始化
onMounted(() => {
  rebuildDeviceTreeFromBackend()
})

watch(
  () => [
    props.stations,
    props.stationId,
    props.stationPointTypeSummary,
    props.slavesByStation,

    props.slavePointTypeSummary,
    props.stationRuntimeStatusMap,
  ],
  () => {
    scheduleRebuildDeviceTree()
  },
  { deep: false },
)

watch(
  () => [props.stationId, props.selectedSlaveStatus],
  () => {
    const selectedStationId = (props.stationId || '').trim()
    if (!selectedStationId) return

    const station = (props.stations ?? []).find((item) => item.station_id === selectedStationId)
    const node = deviceTreeData.value.find((item) => item.id === selectedStationId)
    if (!node) return

    node.status = resolveStationNodeStatus(selectedStationId, station?.status)
    if (selectedSlave.value?.id === selectedStationId) {
      selectedSlave.value.status = node.status
    }
  },
  { deep: false },
)

let rebuildScheduled = false
const scheduleRebuildDeviceTree = () => {
  if (rebuildScheduled) return
  rebuildScheduled = true
  requestAnimationFrame(() => {
    rebuildScheduled = false
    rebuildDeviceTreeFromBackend()
  })
}

// 更新从站节点的方法
const updateSlaveNode = (slaveId: string, updates: Partial<DeviceNode>) => {
  const slaveNode =
    deviceTreeData.value.find((node) => node.id === slaveId) ??
    deviceTreeData.value.flatMap((node) => node.children ?? []).find((node) => node.id === slaveId)
  if (slaveNode) {
    Object.assign(slaveNode, updates)
    if (updates.status) {
      const cacheKey =
        slaveNode.type === 'listener' ? slaveId : String(slaveNode.parentStationId || '')
      if (cacheKey) {
        stationRuntimeStatusCache.value[cacheKey] = updates.status
      }
    }
    return slaveNode
  }
  return null
}

const syncCurrentTypeNode = (
  selection: {
    stationId: string
    slaveId: number
    dataType: string
  } | null,
) => {
  if (!selection) {
    if (treeRef.value?.getCurrentKey?.() != null) treeRef.value?.setCurrentKey?.(null)
    return false
  }

  const listenerTreeNode = treeRef.value?.getNode?.(selection.stationId)
  const slaveTreeNode = treeRef.value?.getNode?.(
    `slave-${selection.stationId}-${selection.slaveId}`,
  )
  const typeNodeId = `${selection.stationId}-${selection.slaveId}-${selection.dataType}`
  const typeTreeNode = treeRef.value?.getNode?.(typeNodeId)
  const listenerNode = listenerTreeNode?.data as DeviceNode | undefined
  const slaveNode = slaveTreeNode?.data as DeviceNode | undefined
  const typeNode = typeTreeNode?.data as DeviceNode | undefined
  if (!listenerNode || !slaveNode || !typeNode) {
    if (treeRef.value?.getCurrentKey?.() != null) treeRef.value?.setCurrentKey?.(null)
    return false
  }

  selectedSlave.value = slaveNode
  expandedNodeKeys.value = Array.from(
    new Set([...expandedNodeKeys.value, listenerNode.id, slaveNode.id]),
  )
  if (!listenerTreeNode.expanded) listenerTreeNode.expand?.()
  if (!slaveTreeNode.expanded) slaveTreeNode.expand?.()
  if (String(treeRef.value?.getCurrentKey?.() ?? '') !== typeNodeId) {
    treeRef.value?.setCurrentKey?.(typeNodeId)
  }
  return true
}

// 暴露方法给父组件
defineExpose({
  updateSlaveNode,
  rebuildDeviceTreeFromBackend,
  deviceTreeData,
  refreshDevices,
  handleCreateListener,
  handleCreateSlave,
  handleEditSlave,
  handleEditConnection,
  syncCurrentTypeNode,
})
</script>

<style scoped>
.slave-device-tree {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: var(--bg-panel);
}
</style>
