import { computed, onMounted, onUnmounted, ref, type Ref } from 'vue'

import type { DeviceNode } from '../types/slave'

import {
  handleNodeCollapseByTypes,
  handleNodeExpandByTypes,
  handleTreeNodeClickExpansion,
} from '@shared/ui/treeExpansion'

type TreeNodeLike = {
  parent?: {
    data?: DeviceNode | null
  } | null
}

type TreeInstanceLike =
  | {
      getNode?: (key: string) =>
        | {
            expanded?: boolean
            expand?: () => void
          }
        | null
        | undefined
    }
  | null
  | undefined

interface UseSlaveTreeInteractionOptions {
  treeRef: Ref<unknown>
  selectedSlave: Ref<DeviceNode | null>
  deviceTreeData: Ref<DeviceNode[]>
  expandedNodeKeys: Ref<string[]>
  emitNodeClick: (node: DeviceNode) => void
  emitTypeIdClick: (node: DeviceNode) => void
  emitSlaveOperation: (operation: string, slave: DeviceNode | null) => void
}

const SLAVE_EXPANDABLE_TYPES = ['listener', 'slave']

export function useSlaveTreeInteraction(options: UseSlaveTreeInteractionOptions) {
  const nodeContextMenu = ref({
    visible: false,
    x: 0,
    y: 0,
    node: null as DeviceNode | null,
  })

  const isNodeRunning = (node: DeviceNode | null) => {
    if (!node) return false
    return [
      'status-listening',
      'status-processing',
      'status-unready',
      'status-normal',
      'running',
      'listening',
      'connected',
      'connecting',
      'starting',
      'stopping',
    ].includes(String(node.status ?? ''))
  }

  const isContextNodeRunning = computed(() => isNodeRunning(nodeContextMenu.value.node))
  const isContextListener = computed(() => nodeContextMenu.value.node?.type === 'listener')
  const isContextSlave = computed(() => nodeContextMenu.value.node?.type === 'slave')

  const closeNodeContextMenu = () => {
    nodeContextMenu.value.visible = false
    nodeContextMenu.value.node = null
  }

  const isContextMenuTarget = (target: EventTarget | null) =>
    target instanceof HTMLElement && Boolean(target.closest('.tree-context-menu'))

  const handleGlobalClickForContextMenu = (event: MouseEvent) => {
    if (isContextMenuTarget(event.target)) return
    closeNodeContextMenu()
  }

  const handleGlobalContextMenuForContextMenu = (event: MouseEvent) => {
    if (isContextMenuTarget(event.target)) return
    closeNodeContextMenu()
  }

  const handleNodeClick = (data: DeviceNode, node?: TreeNodeLike) => {
    options.emitNodeClick(data)

    if (data.type === 'listener' || data.type === 'slave') {
      options.selectedSlave.value = data
      // 节点选择与展开状态相互独立；点击任意可折叠行都应立即切换展开状态。
      options.expandedNodeKeys.value = handleTreeNodeClickExpansion(
        options.treeRef.value as TreeInstanceLike,
        data.id,
        false,
        options.expandedNodeKeys.value,
      )
      return
    }

    if (data.type === 'type-id') {
      const parentSlaveNode =
        node?.parent?.data?.type === 'slave' ? (node.parent.data as DeviceNode) : null
      if (parentSlaveNode) {
        options.selectedSlave.value = parentSlaveNode
      }

      const parentSlaveIdFromTree = Number(
        node?.parent?.data?.slaveId ?? node?.parent?.data?.id ?? 0,
      )
      const parentSlaveId =
        Number.isFinite(parentSlaveIdFromTree) && parentSlaveIdFromTree > 0
          ? String(parentSlaveIdFromTree)
          : String(data.parentSlaveId || '').trim()
      const parentStationId = String(
        node?.parent?.data?.parentStationId ||
          node?.parent?.data?.stationId ||
          data.parentStationId ||
          '',
      ).trim()
      const parentSlaveNameFromTree = String(
        node?.parent?.data?.name || node?.parent?.data?.label || '',
      ).trim()
      const parentSlaveName = parentSlaveNameFromTree || String(data.parentSlaveName || '').trim()

      options.emitTypeIdClick({
        ...data,
        parentStationId,
        parentSlaveId,
        slaveId: Number(parentSlaveId) || undefined,
        parentSlaveName,
      })
    }
  }

  const handleNodeContextMenu = (event: MouseEvent, data: DeviceNode) => {
    event.preventDefault()
    event.stopPropagation()

    if (data.type !== 'listener' && data.type !== 'slave') {
      closeNodeContextMenu()
      return
    }

    options.selectedSlave.value = data
    options.emitNodeClick(data)
    nodeContextMenu.value.visible = true
    nodeContextMenu.value.x = event.clientX
    nodeContextMenu.value.y = event.clientY
    nodeContextMenu.value.node = data
  }

  const handleContextMenuAction = (operation: string) => {
    const node = nodeContextMenu.value.node
    if (!node || (node.type !== 'listener' && node.type !== 'slave')) {
      closeNodeContextMenu()
      return
    }
    if (operation === 'open-connection' && node.type === 'listener' && isNodeRunning(node)) {
      closeNodeContextMenu()
      return
    }
    if (operation === 'close-connection' && node.type === 'listener' && !isNodeRunning(node)) {
      closeNodeContextMenu()
      return
    }

    options.emitSlaveOperation(operation, node)
    closeNodeContextMenu()
  }

  const handleNodeExpand = (data: DeviceNode) => {
    options.expandedNodeKeys.value = handleNodeExpandByTypes(
      data,
      options.expandedNodeKeys.value,
      SLAVE_EXPANDABLE_TYPES,
    )
  }

  const handleNodeCollapse = (data: DeviceNode) => {
    options.expandedNodeKeys.value = handleNodeCollapseByTypes(
      data,
      options.expandedNodeKeys.value,
      SLAVE_EXPANDABLE_TYPES,
    )
  }

  onMounted(() => {
    document.addEventListener('click', handleGlobalClickForContextMenu, true)
    document.addEventListener('contextmenu', handleGlobalContextMenuForContextMenu, true)
  })

  onUnmounted(() => {
    document.removeEventListener('click', handleGlobalClickForContextMenu, true)
    document.removeEventListener('contextmenu', handleGlobalContextMenuForContextMenu, true)
  })

  return {
    nodeContextMenu,
    isContextNodeRunning,
    isContextListener,
    isContextSlave,
    handleNodeClick,
    handleNodeContextMenu,
    handleContextMenuAction,
    handleNodeExpand,
    handleNodeCollapse,
  }
}
