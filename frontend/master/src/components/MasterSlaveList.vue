<template>
  <div class="sidebar-container">
    <div class="sidebar-tree">
      <el-scrollbar class="sidebar-tree-scroll">
        <div v-if="slaveTreeData.length === 0" class="sidebar-empty-state">
          <div class="sidebar-empty-title">{{ t('master.slaveList.empty.title') }}</div>
          <div class="sidebar-empty-hint">{{ t('master.slaveList.empty.hint') }}</div>
          <el-button type="primary" size="small" @click="emit('createConnection')">{{
            t('master.slaveList.empty.createConnection')
          }}</el-button>
        </div>
        <el-tree
          v-else
          ref="treeRef"
          :data="slaveTreeData"
          :props="treeProps"
          node-key="id"
          :expand-on-click-node="false"
          :highlight-current="true"
          @node-click="handleNodeClick"
          @node-contextmenu="handleNodeContextMenu"
          @node-expand="handleNodeExpand"
          @node-collapse="handleNodeCollapse"
        >
          <template #default="{ data }">
            <div
              class="sidebar-node"
              :class="{
                'sidebar-node--data-type':
                  data.type === 'datatype' || data.type === 'datatype-group',
              }"
            >
              <div class="sidebar-node-left">
                <el-icon class="sidebar-icon" :class="getNodeIconClass(data)">
                  <component :is="getNodeIcon(data)" />
                </el-icon>
                <span v-if="data.type === 'slave'" class="sidebar-label slave-label">{{
                  data.data?.name
                }}</span>
                <el-tooltip
                  v-else-if="data.type === 'link'"
                  :content="`${data.host}:${data.port}`"
                  placement="right"
                >
                  <span class="sidebar-label">{{ data.label }}</span>
                </el-tooltip>
                <span v-else class="sidebar-label">{{ getNodeDisplayLabel(data) }}</span>
              </div>
              <div class="sidebar-node-right">
                <div v-if="data.type === 'link'">
                  <span class="sidebar-badge">CONNECTOR</span>
                </div>
                <div v-if="data.type === 'slave'">
                  <span class="sidebar-badge">CA:{{ data.data?.commonAddress || 1 }}</span>
                </div>
                <div v-else-if="data.type === 'datatype'">
                  <span class="sidebar-badge">{{ formatTypeCompact(data.dataType) }}</span>
                </div>
              </div>
            </div>
          </template>
        </el-tree>
      </el-scrollbar>

      <div
        v-show="nodeContextMenu.visible"
        class="app-context-menu master-tree-context-menu"
        :style="{ left: `${nodeContextMenu.x}px`, top: `${nodeContextMenu.y}px` }"
      >
        <template v-if="nodeContextMenu.targetType === 'link'">
          <button
            type="button"
            class="app-context-menu-item"
            :disabled="isContextLinkActive"
            @click.stop="handleLinkContextMenuAction('connect-link')"
          >
            <el-icon><VideoPlay /></el-icon>
            {{ t('master.slaveList.context.connectLink') }}
          </button>
          <button
            type="button"
            class="app-context-menu-item"
            :disabled="!isContextLinkConnected"
            @click.stop="handleLinkContextMenuAction('disconnect-link')"
          >
            <el-icon><VideoPause /></el-icon>
            {{ t('master.slaveList.context.disconnectLink') }}
          </button>
          <div class="app-context-menu-divider"></div>
          <button
            type="button"
            class="app-context-menu-item"
            @click.stop="handleLinkContextMenuAction('create-station')"
          >
            <el-icon><Plus /></el-icon>
            {{ t('master.slaveList.context.createStation') }}
          </button>
          <div class="app-context-menu-divider"></div>
          <button
            type="button"
            class="app-context-menu-item"
            @click.stop="handleLinkContextMenuAction('edit-link')"
          >
            <el-icon><Edit /></el-icon>
            {{ t('master.slaveList.context.editLink') }}
          </button>
          <button
            type="button"
            class="app-context-menu-item danger"
            @click.stop="handleLinkContextMenuAction('delete-link')"
          >
            <el-icon><Delete /></el-icon>
            {{ t('master.slaveList.context.deleteLink') }}
          </button>
        </template>
        <template v-else-if="nodeContextMenu.targetType === 'slave'">
          <button
            type="button"
            class="app-context-menu-item"
            @click.stop="handleContextMenuAction('edit')"
          >
            <el-icon><Edit /></el-icon>
            {{ t('master.slaveList.context.editSlave') }}
          </button>
          <button
            type="button"
            class="app-context-menu-item danger"
            @click.stop="handleContextMenuAction('delete')"
          >
            <el-icon><Delete /></el-icon>
            {{ t('master.slaveList.context.deleteSlave') }}
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  Connection,
  Delete,
  Edit,
  Monitor,
  Platform,
  Plus,
  VideoPause,
  VideoPlay,
} from '@element-plus/icons-vue'
import { useMasterTreeModel } from '../composables/useMasterTreeModel'
import type { SlaveConnection } from '@/types/master'
import type { PointTypeSummary } from '../types/pointTypeSummary'
import type { MasterPointData } from '@/composables/useMasterPointData'
import type { LinkProfileResponse } from '@shared/api/types'
import {
  formatIec104TypeCompactLabel,
  iec104TypeNameToId,
  normalizeIecObjectDisplayName,
} from '@shared/api/iec104'
import {
  resolveMasterControlGroupByMarker,
  resolveMasterControlGroupLabelByMarker,
} from '@shared/api/masterControl'
import { getIecDataTypeNodeLabel, getIecDataTypeTreeIcon } from '@shared/ui/dataTypeIcon'
import { currentLocale, t } from '@shared/i18n'
import {
  handleTreeNodeClickExpansion,
  handleNodeExpandByTypes,
  handleNodeCollapseByTypes,
} from '@shared/ui/treeExpansion'
import { deriveUnifiedConnStatus, isTransportConnectedState } from '@shared/ui/connectionStatus'

interface Props {
  profiles: SlaveConnection[]
  pointData: MasterPointData
  profilePointTypeSummary: Record<string, PointTypeSummary>
  links: LinkProfileResponse[]
}

const props = withDefaults(defineProps<Props>(), {
  profiles: () => [],
  profilePointTypeSummary: () => ({}),
  links: () => [],
})

const emit = defineEmits<{
  slaveSelect: [slave: SlaveConnection | null]
  datatypeSelect: [datatype: unknown]
  profileOperation: [
    operation: 'connect' | 'disconnect' | 'edit' | 'delete',
    slave: SlaveConnection,
  ]
  linkOperation: [
    operation: 'connect-link' | 'disconnect-link' | 'create-station' | 'edit-link' | 'delete-link',
    linkProfileId: number,
  ]
  createConnection: []
}>()

const treeRef = ref<any>(null)
const selectedSlaveId = ref('')
const selectedLinkKey = ref('')
const selectedTreeNodeKey = ref('')
const expandedSlaveKeys = ref<string[]>([])
const treeNodeByKey = new Map<string, any>()
const ancestorKeysByNodeKey = new Map<string, string[]>()
const nodeContextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  targetType: '' as '' | 'link' | 'slave',
  slaveId: '' as string,
  linkProfileId: 0,
})

const treeProps = {
  children: 'children',
  label: 'label',
}

const slaves = computed<SlaveConnection[]>(() => props.profiles)

const slaveTreeData = ref<any[]>([])
const { mergeSlaveTree } = useMasterTreeModel({
  treeRef,
  slaveTreeData,
  selectedSlaveId,
  selectedTreeNodeKey,
  expandedSlaveKeys,
  source: props,
})

const rebuildTreeNodeIndex = () => {
  treeNodeByKey.clear()
  ancestorKeysByNodeKey.clear()
  const visit = (nodes: any[], ancestors: string[]) => {
    for (const node of nodes) {
      const key = String(node.id)
      treeNodeByKey.set(key, node)
      ancestorKeysByNodeKey.set(key, ancestors)
      visit(node.children ?? [], [...ancestors, key])
    }
  }
  visit(slaveTreeData.value, [])
}

const setCurrentTreeKey = (key: string | null) => {
  const currentKey = treeRef.value?.getCurrentKey?.()
  if (String(currentKey ?? '') === String(key ?? '')) return
  treeRef.value?.setCurrentKey?.(key)
}

const getNodeIcon = (data: any) => {
  if (data.type === 'link') return Connection
  if (data.type === 'slave') return Platform
  if (data.type === 'section') return data.sectionKind === 'control' ? Edit : Monitor
  if (data.type === 'datatype-group') {
    const group = resolveMasterControlGroupByMarker(data.dataType as string)
    if (group) return getIecDataTypeTreeIcon(group.iconDataType)
  }
  return getIecDataTypeTreeIcon(data.dataType as string)
}

const getNodeIconClass = (data: any) => {
  if (data.type === 'link' || data.type === 'slave') {
    return data.status || 'status-idle'
  }

  if (data.type === 'section' || data.type === 'datatype' || data.type === 'datatype-group') {
    const parentSlave = slaves.value.find((slave) => slave.id === data.parentSlaveId)
    return parentSlave && isTransportConnectedState(parentSlave.transportState)
      ? 'datatype'
      : 'datatype-inactive'
  }

  return 'datatype-inactive'
}

const getNodeDisplayLabel = (data: any) => {
  void currentLocale.value
  if (data.type === 'section') {
    return t(
      data.sectionKind === 'control' ? 'master.tree.controlSection' : 'master.tree.monitorSection',
    )
  }
  if (data.type === 'datatype-group') {
    return resolveMasterControlGroupLabelByMarker(data.dataType) || data.label
  }
  if (data.type !== 'datatype') return data.label
  return normalizeIecObjectDisplayName(data.label) || getIecDataTypeNodeLabel(data.dataType)
}

const formatTypeCompact = (dataType: string) =>
  formatIec104TypeCompactLabel(iec104TypeNameToId(dataType))

const contextMenuSlave = computed<SlaveConnection | null>(() => {
  const slaveId = nodeContextMenu.value.slaveId
  if (!slaveId) return null
  return slaves.value.find((slave) => slave.id === slaveId) ?? null
})

const contextMenuLinkProfileId = computed<number>(() => {
  if (nodeContextMenu.value.targetType === 'link') {
    return Number(nodeContextMenu.value.linkProfileId || 0)
  }
  const slave = contextMenuSlave.value
  if (!slave) return 0
  return Number(slave.linkProfileId || 0)
})

const contextMenuLinkSlaves = computed<SlaveConnection[]>(() => {
  const linkProfileId = contextMenuLinkProfileId.value
  if (!Number.isFinite(linkProfileId) || linkProfileId <= 0) return []
  return slaves.value.filter((slave) => Number(slave.linkProfileId) === linkProfileId)
})

const isContextLinkConnected = computed(() => {
  return contextMenuLinkSlaves.value.some((slave) => {
    const status = deriveUnifiedConnStatus(slave.transportState, slave.dataTransferState)
    return status.severity !== 'status-idle'
  })
})

const isContextLinkActive = computed(() => {
  return contextMenuLinkSlaves.value.some((slave) => {
    const status = deriveUnifiedConnStatus(slave.transportState, slave.dataTransferState)
    return (
      status.severity === 'status-processing' ||
      status.severity === 'status-unready' ||
      status.severity === 'status-normal'
    )
  })
})

const handleNodeClick = (data: any) => {
  if (data.type === 'link') {
    selectedTreeNodeKey.value = ''
    const linkKey = String(data.id)
    const isSwitching = selectedLinkKey.value !== linkKey
    selectedLinkKey.value = linkKey
    expandedSlaveKeys.value = handleTreeNodeClickExpansion(
      treeRef.value,
      linkKey,
      isSwitching,
      expandedSlaveKeys.value,
    )
    const firstSlave =
      (data.children ?? []).find((node: any) => node.type === 'slave')?.data ?? null
    if (firstSlave) {
      selectedSlaveId.value = firstSlave.id
    }
    emit('slaveSelect', firstSlave)
    return
  }

  if (data.type === 'slave') {
    selectedTreeNodeKey.value = ''
    const isSwitching = selectedSlaveId.value !== data.id
    expandedSlaveKeys.value = handleTreeNodeClickExpansion(
      treeRef.value,
      data.id,
      isSwitching,
      expandedSlaveKeys.value,
    )
    selectedSlaveId.value = data.id
    emit('slaveSelect', data.data)
    return
  }

  if (data.type === 'section') {
    selectedTreeNodeKey.value = ''
    expandedSlaveKeys.value = handleTreeNodeClickExpansion(
      treeRef.value,
      String(data.id),
      false,
      expandedSlaveKeys.value,
    )
    return
  }

  if (data.type === 'datatype' || data.type === 'datatype-group') {
    const parentSlave = slaves.value.find((slave) => slave.id === data.parentSlaveId)
    if (!parentSlave) return

    const slaveChanged = selectedSlaveId.value !== String(data.parentSlaveId)
    selectedTreeNodeKey.value = String(data.id ?? '')
    selectedSlaveId.value = String(data.parentSlaveId)
    setCurrentTreeKey(String(data.id))
    if (slaveChanged) emit('slaveSelect', parentSlave)
    emit('datatypeSelect', {
      ...data,
      label: getNodeDisplayLabel(data),
      slaveName: parentSlave.name,
    })
  }
}

const closeNodeContextMenu = () => {
  nodeContextMenu.value.visible = false
  nodeContextMenu.value.targetType = ''
  nodeContextMenu.value.slaveId = ''
  nodeContextMenu.value.linkProfileId = 0
}

const isContextMenuTarget = (target: EventTarget | null) => {
  return target instanceof HTMLElement && Boolean(target.closest('.master-tree-context-menu'))
}

const handleGlobalClickForContextMenu = (event: MouseEvent) => {
  if (isContextMenuTarget(event.target)) return
  closeNodeContextMenu()
}

const handleGlobalContextMenuForContextMenu = (event: MouseEvent) => {
  if (isContextMenuTarget(event.target)) return
  closeNodeContextMenu()
}

const handleNodeContextMenu = (event: MouseEvent, data: any) => {
  event.preventDefault()
  event.stopPropagation()

  if (data.type !== 'slave' && data.type !== 'link') {
    closeNodeContextMenu()
    return
  }

  if (data.type === 'slave') {
    selectedTreeNodeKey.value = ''
    selectedSlaveId.value = data.id
    emit('slaveSelect', data.data)
    setCurrentTreeKey(String(data.id))
  } else {
    selectedTreeNodeKey.value = ''
    setCurrentTreeKey(String(data.id))
  }

  nodeContextMenu.value.visible = true
  nodeContextMenu.value.x = event.clientX
  nodeContextMenu.value.y = event.clientY
  nodeContextMenu.value.targetType = data.type
  nodeContextMenu.value.slaveId = data.type === 'slave' ? String(data.id) : ''
  nodeContextMenu.value.linkProfileId = Number(
    data.type === 'link' ? data.linkProfileId : (data?.data?.linkProfileId ?? 0),
  )
}

const MASTER_EXPANDABLE_TYPES = ['link', 'slave', 'section']

const handleNodeExpand = (data: any) => {
  expandedSlaveKeys.value = handleNodeExpandByTypes(
    data,
    expandedSlaveKeys.value,
    MASTER_EXPANDABLE_TYPES,
  )
}

const handleNodeCollapse = (data: any) => {
  expandedSlaveKeys.value = handleNodeCollapseByTypes(
    data,
    expandedSlaveKeys.value,
    MASTER_EXPANDABLE_TYPES,
  )
}

const syncCurrentDatatypeNode = (selection: { profileId: string; dataType: string } | null) => {
  if (!selection) {
    selectedTreeNodeKey.value = ''
    setCurrentTreeKey(selectedSlaveId.value || null)
    return false
  }

  const datatypeNodeKey = `${selection.profileId}-${selection.dataType}`
  const datatypeNode = treeNodeByKey.get(datatypeNodeKey)
  if (!datatypeNode) return false
  if (
    selectedTreeNodeKey.value === datatypeNodeKey &&
    String(treeRef.value?.getCurrentKey?.() ?? '') === datatypeNodeKey
  ) {
    return true
  }

  const ancestorKeys = ancestorKeysByNodeKey.get(datatypeNodeKey) ?? []
  const expanded = new Set(expandedSlaveKeys.value)
  for (const key of ancestorKeys) {
    expanded.add(key)
    const treeNode = treeRef.value?.getNode?.(key)
    if (treeNode && !treeNode.expanded) treeNode.expand?.()
  }
  selectedLinkKey.value = ancestorKeys.find((key) => key.startsWith('link:')) ?? ''
  selectedSlaveId.value = String(selection.profileId)
  selectedTreeNodeKey.value = datatypeNodeKey
  expandedSlaveKeys.value = [...expanded]
  setCurrentTreeKey(datatypeNodeKey)
  return true
}

const handleContextMenuAction = (operation: 'edit' | 'delete') => {
  const slave = contextMenuSlave.value
  if (!slave) {
    closeNodeContextMenu()
    return
  }

  emit('profileOperation', operation, slave)
  closeNodeContextMenu()
}

const handleLinkContextMenuAction = (
  operation: 'connect-link' | 'disconnect-link' | 'create-station' | 'edit-link' | 'delete-link',
) => {
  const linkProfileId = contextMenuLinkProfileId.value
  if (!Number.isFinite(linkProfileId) || linkProfileId <= 0) {
    closeNodeContextMenu()
    return
  }

  if (operation === 'connect-link' && isContextLinkActive.value) {
    closeNodeContextMenu()
    return
  }
  if (operation === 'disconnect-link' && !isContextLinkConnected.value) {
    closeNodeContextMenu()
    return
  }

  emit('linkOperation', operation, linkProfileId)
  closeNodeContextMenu()
}

watch(slaves, () => {
  if (!nodeContextMenu.value.visible) return
  if (nodeContextMenu.value.targetType !== 'slave') return
  const slaveId = nodeContextMenu.value.slaveId
  if (!slaveId) return
  if (!slaves.value.some((slave) => slave.id === slaveId)) {
    closeNodeContextMenu()
  }
})

watch(slaveTreeData, () => {
  if (!nodeContextMenu.value.visible) return
  if (nodeContextMenu.value.targetType !== 'link') return
  const linkProfileId = Number(nodeContextMenu.value.linkProfileId || 0)
  if (!Number.isFinite(linkProfileId) || linkProfileId <= 0) {
    closeNodeContextMenu()
    return
  }
  const hasLinkNode = slaveTreeData.value.some(
    (node) => node.type === 'link' && Number(node.linkProfileId) === linkProfileId,
  )
  if (!hasLinkNode) {
    closeNodeContextMenu()
  }
})

let mergeScheduled = false
const scheduleMergeSlaveTree = () => {
  if (mergeScheduled) return
  mergeScheduled = true
  requestAnimationFrame(() => {
    mergeScheduled = false
    mergeSlaveTree()
    rebuildTreeNodeIndex()
  })
}

watch(
  () => [
    props.profiles,
    props.links,
    props.profilePointTypeSummary,
    props.pointData.summaryRevision.value,
  ],
  () => {
    scheduleMergeSlaveTree()
  },
  { deep: false, immediate: true },
)

onMounted(() => {
  document.addEventListener('click', handleGlobalClickForContextMenu)
  document.addEventListener('contextmenu', handleGlobalContextMenuForContextMenu, true)
})

onUnmounted(() => {
  document.removeEventListener('click', handleGlobalClickForContextMenu)
  document.removeEventListener('contextmenu', handleGlobalContextMenuForContextMenu, true)
})

defineExpose({ syncCurrentDatatypeNode })
</script>

<style scoped>
.sidebar-node--data-type {
  width: calc(100% + 16px);
  margin-left: -16px;
}
</style>
