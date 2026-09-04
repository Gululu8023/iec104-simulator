<template>
  <div class="tabs-header">
    <div class="tabs-wrapper app-capsule-tabs-shell">
      <el-tabs
        v-model="activeName"
        @tab-click="emit('tabClick', $event)"
        class="tabs-only app-capsule-tabs"
      >
        <el-tab-pane v-for="tab in tabs" :key="tab.id" :name="tab.id">
          <template #label>
            <el-dropdown
              trigger="contextmenu"
              @command="emit('tabCommand', { command: $event, tabId: tab.id })"
              popper-class="app-menu-dropdown"
            >
              <div class="custom-tab-label app-capsule-tab-label">
                <el-icon class="tab-icon app-capsule-tab-icon">
                  <component :is="getTabIcon(tab.dataType)" />
                </el-icon>
                <span class="tab-title app-capsule-tab-title">{{ getTabDisplayLabel(tab) }}</span>
                <el-tooltip
                  :content="getTabTooltip(tab)"
                  placement="bottom"
                  :enterable="false"
                  :show-after="500"
                >
                  <span class="tab-badge app-capsule-tab-badge">{{ getTabBadge(tab) }}</span>
                </el-tooltip>
                <div
                  class="tab-close-btn app-capsule-tab-close"
                  @click.stop="emit('close', tab.id)"
                >
                  <el-icon><Close /></el-icon>
                </div>
              </div>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item command="close">{{
                    t('slave.contentPanel.tabs.close')
                  }}</el-dropdown-item>
                  <el-dropdown-item command="closeOthers">
                    {{ t('slave.contentPanel.tabs.closeOthers') }}
                  </el-dropdown-item>
                  <el-dropdown-item command="closeAll" divided class="menu-item-danger">
                    {{ t('slave.contentPanel.tabs.closeAll') }}
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </template>
        </el-tab-pane>
      </el-tabs>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, type Component } from 'vue'
import { Close } from '@element-plus/icons-vue'

import { t } from '@shared/i18n'
import type { SlaveContentTabData } from '@/types/slaveContentPanel'

const props = defineProps<{
  modelValue: string
  tabs: SlaveContentTabData[]
  getTabIcon: (dataType: string) => Component
  getTabDisplayLabel: (tab: SlaveContentTabData) => string
  getTabTooltip: (tab: SlaveContentTabData) => string
  getTabBadge: (tab: SlaveContentTabData) => string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  tabClick: [tab: unknown]
  tabCommand: [payload: { command: string; tabId: string }]
  close: [tabId: string]
}>()

const activeName = computed({
  get: () => props.modelValue,
  set: (value: string) => emit('update:modelValue', value),
})
</script>
