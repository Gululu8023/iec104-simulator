<template>
  <div class="protocol-tree">
    <div v-if="props.nodes.length === 0" class="protocol-tree__empty">{{ resolvedEmptyText }}</div>
    <ParsedFrameTreeNode
      v-for="node in props.nodes"
      v-else
      :key="node.id"
      :node="node"
      :depth="0"
      :expand-all="props.expandAll"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import type { MessageParserTreeNode } from '@shared/api/types'
import { t } from '@shared/i18n'

import ParsedFrameTreeNode from './ParsedFrameTreeNode.vue'

const props = defineProps<{
  nodes: MessageParserTreeNode[]
  emptyText?: string
  expandAll?: boolean | null
}>()

const resolvedEmptyText = computed(() => props.emptyText ?? t('messageParser.tree.empty'))
</script>
