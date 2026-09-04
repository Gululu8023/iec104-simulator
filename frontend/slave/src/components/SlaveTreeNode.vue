<template>
  <div class="sidebar-node" :class="{ 'sidebar-node--type-id': data.type === 'type-id' }">
    <div class="sidebar-node-left">
      <el-icon class="sidebar-icon" :class="deviceIconClass">
        <component :is="deviceIcon" />
      </el-icon>
      <el-tooltip
        v-if="data.type === 'listener' && data.address"
        :content="`${data.address}:${data.port}`"
        placement="right"
        :show-after="500"
      >
        <span class="sidebar-label slave-label">{{ data.name || node.label }}</span>
      </el-tooltip>
      <span v-else class="sidebar-label">{{ displayLabel }}</span>
    </div>
    <div class="sidebar-node-right">
      <div v-if="data.type === 'listener'">
        <span class="sidebar-badge">LISTENER</span>
      </div>
      <div v-if="data.type === 'slave'">
        <span class="sidebar-badge">CA:{{ data.commonAddress }}</span>
      </div>
      <div v-else-if="data.type === 'type-id'">
        <span class="sidebar-badge">{{ typeLabel }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Connection, Files, Monitor, Platform } from '@element-plus/icons-vue'

import type { DeviceNode } from '@/types/slave'
import { currentLocale } from '@shared/i18n'
import {
  formatIec104TypeCompactLabel,
  iec104TypeNameToId,
  normalizeIecObjectDisplayName,
} from '@shared/api/iec104'
import { getIecDataTypeNodeLabel, getIecDataTypeTreeIcon } from '@shared/ui/dataTypeIcon'

const props = defineProps<{
  node: { label: string }
  data: DeviceNode
  deviceTreeData: DeviceNode[]
}>()

const deviceIcon = computed(() => {
  switch (props.data.type) {
    case 'listener':
    case 'connection':
      return Connection
    case 'slave':
      return Platform
    case 'type-id':
      return getIecDataTypeTreeIcon(props.data.dataType)
    case 'group':
      return Files
    default:
      return Monitor
  }
})

const displayLabel = computed(() => {
  void currentLocale.value
  if (props.data.type !== 'type-id') return props.data.name || props.node.label
  return (
    normalizeIecObjectDisplayName(props.data.label) || getIecDataTypeNodeLabel(props.data.dataType)
  )
})

const typeLabel = computed(() =>
  formatIec104TypeCompactLabel(iec104TypeNameToId(props.data.dataType)),
)

function resolveStatusClass(status: DeviceNode['status']): string {
  switch (
    String(status ?? '')
      .trim()
      .toLowerCase()
  ) {
    case 'status-listening':
    case 'listening':
      return 'status-listening'
    case 'status-processing':
    case 'connecting':
    case 'starting':
    case 'stopping':
      return 'status-processing'
    case 'status-unready':
    case 'connected':
      return 'status-unready'
    case 'status-normal':
    case 'running':
    case 'normal':
      return 'status-normal'
    case 'status-error':
    case 'error':
      return 'status-error'
    default:
      return 'status-idle'
  }
}

const deviceIconClass = computed(() => {
  if (['listener', 'slave', 'connection'].includes(props.data.type)) {
    return resolveStatusClass(props.data.status)
  }
  if (props.data.type === 'type-id' || props.data.type === 'group') {
    const stationId = String(props.data.parentStationId || '').trim()
    const station = stationId
      ? props.deviceTreeData.find((node) => String(node.id) === stationId)
      : null
    const status = String(station?.status ?? '')
      .trim()
      .toLowerCase()
    return [
      'status-unready',
      'status-normal',
      'status-error',
      'connected',
      'running',
      'normal',
      'error',
    ].includes(status)
      ? 'datatype'
      : 'datatype-inactive'
  }
  return 'datatype-inactive'
})
</script>

<style scoped>
.sidebar-node--type-id {
  width: calc(100% + 16px);
  margin-left: -16px;
}
</style>
