<template>
  <div
    class="protocol-tree-node"
    :class="[
      `protocol-tree-node--depth-${Math.min(depth, 5)}`,
      { 'is-open': expanded, 'is-leaf': !hasChildren, [`is-${node.tone}`]: Boolean(node.tone) },
    ]"
  >
    <div class="protocol-tree-node__row" :class="{ 'is-clickable': hasChildren }" @click="toggle">
      <span class="protocol-tree-node__toggle">
        <span v-if="hasChildren" class="toggle-icon">{{ expanded ? '▾' : '▸' }}</span>
      </span>
      <span class="protocol-tree-node__label">{{ displayLabel }}</span>
      <span v-if="node.value" class="protocol-tree-node__value font-mono">{{ node.value }}</span>
      <span v-if="displaySummary" class="protocol-tree-node__summary">{{ displaySummary }}</span>
      <span v-if="node.byte_range" class="protocol-tree-node__range font-mono">
        [{{ node.byte_range.start }}..{{ node.byte_range.end }})
      </span>
    </div>

    <div v-if="hasChildren && expanded" class="protocol-tree-node__children">
      <ParsedFrameTreeNode
        v-for="child in node.children"
        :key="child.id"
        :node="child"
        :depth="depth + 1"
        :expand-all="expandAll"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import type { MessageParserTreeNode } from '@shared/api/types'
import { currentLocale } from '@shared/i18n'

import { getParsedFrameTreeLabel, getParsedFrameTreeSummary } from './treeDisplay'

defineOptions({
  name: 'ParsedFrameTreeNode',
})

const props = defineProps<{
  node: MessageParserTreeNode
  depth?: number
  expandAll?: boolean | null
}>()

const depth = computed(() => props.depth ?? 0)
const hasChildren = computed(() => props.node.children.length > 0)
const expanded = ref(props.node.default_expanded || depth.value === 0)
const displayLabel = computed(() => {
  void currentLocale.value
  return getParsedFrameTreeLabel(props.node)
})
const displaySummary = computed(() => {
  void currentLocale.value
  return getParsedFrameTreeSummary(props.node)
})

watch(
  () => props.node.id,
  () => {
    expanded.value = props.node.default_expanded || depth.value === 0
  },
)

watch(
  () => props.expandAll,
  (newVal) => {
    if (typeof newVal === 'boolean' && hasChildren.value) {
      expanded.value = newVal
    }
  },
)

const toggle = () => {
  if (!hasChildren.value) return
  expanded.value = !expanded.value
}
</script>
