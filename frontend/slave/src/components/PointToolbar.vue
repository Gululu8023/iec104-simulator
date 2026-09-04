<template>
  <!-- 控制按钮行 -->
  <div v-if="currentTab" class="app-table-toolbar">
    <div class="app-table-toolbar__left">
      <el-input
        v-model="currentTab.searchText"
        :placeholder="t('slave.contentPanel.toolbar.searchPlaceholder')"
        clearable
        :prefix-icon="Search"
        class="app-table-toolbar__search"
      >
        <template #suffix>
          <div class="app-table-filter-suffix">
            <span
              v-if="hasActiveFilters"
              class="app-table-filter-summary"
              :title="activeFilterSummary"
            >
              {{ activeFilterSummary }}
            </span>
            <el-popover
              placement="bottom-end"
              trigger="click"
              :teleported="true"
              width="auto"
              popper-class="app-menu-dropdown app-checkbox-popover"
            >
              <template #reference>
                <button
                  class="app-table-filter-trigger"
                  :class="{ 'is-active': hasActiveFilters }"
                  type="button"
                  @click.stop
                >
                  <el-icon><Filter /></el-icon>
                </button>
              </template>
              <div class="app-checkbox-popover__panel app-checkbox-popover__search-panel">
                <div
                  class="app-checkbox-popover__item el-dropdown-menu__item"
                  :class="{ 'is-active': quickFilters.onlyMapped }"
                >
                  <el-checkbox v-model="quickFilters.onlyMapped">{{
                    t('slave.contentView.filters.onlyMapped')
                  }}</el-checkbox>
                </div>
                <div
                  class="app-checkbox-popover__item el-dropdown-menu__item"
                  :class="{ 'is-active': quickFilters.onlyAbnormalQuality }"
                >
                  <el-checkbox v-model="quickFilters.onlyAbnormalQuality">{{
                    t('slave.contentView.filters.abnormalQuality')
                  }}</el-checkbox>
                </div>
                <div
                  class="app-checkbox-popover__item app-checkbox-popover__item--split el-dropdown-menu__item"
                  :class="{ 'is-active': quickFilters.onlyRecentChange }"
                >
                  <el-checkbox v-model="quickFilters.onlyRecentChange">{{
                    t('slave.contentPanel.toolbar.recentChange')
                  }}</el-checkbox>
                  <el-select
                    v-model="quickFilters.recentWindowSec"
                    size="small"
                    class="app-checkbox-popover__recent-select"
                    :disabled="!quickFilters.onlyRecentChange"
                    popper-class="app-select-dropdown app-select-dropdown--compact"
                  >
                    <el-option
                      v-for="option in recentWindowOptions"
                      :key="option.value"
                      :label="option.label"
                      :value="option.value"
                    />
                  </el-select>
                </div>
                <div class="app-checkbox-popover__actions">
                  <el-button link type="primary" @click="clearQuickFilters">{{
                    t('slave.contentPanel.toolbar.clearFilters')
                  }}</el-button>
                </div>
              </div>
            </el-popover>
          </div>
        </template>
      </el-input>
      <div class="app-table-toolbar__action" @click="refreshData">
        <el-icon class="app-table-toolbar__action-icon"><Refresh /></el-icon>
        <span class="app-table-toolbar__action-text">{{
          t('slave.contentPanel.toolbar.refresh')
        }}</span>
      </div>
      <div
        v-if="selectedRows.length > 0"
        class="app-table-toolbar__action"
        @click="clearCurrentTabSelection"
      >
        <el-icon class="app-table-toolbar__action-icon"><Close /></el-icon>
        <span class="app-table-toolbar__action-text">{{
          t('slave.contentPanel.toolbar.clearSelection', { count: selectedRows.length })
        }}</span>
      </div>
    </div>
    <div class="app-table-toolbar__right">
      <div class="app-table-toolbar__action" @click="resetData">
        <el-icon class="app-table-toolbar__action-icon"><RefreshRight /></el-icon>
        <span class="app-table-toolbar__action-text">{{
          t('slave.contentPanel.toolbar.reset')
        }}</span>
      </div>
      <div
        v-if="!currentTabIsControl"
        class="app-table-toolbar__action"
        @click="toggleSimulationFromToolbar"
      >
        <el-icon class="app-table-toolbar__action-icon">
          <VideoPause v-if="hasActiveSimulation" />
          <VideoPlay v-else />
        </el-icon>
        <span class="app-table-toolbar__action-text">{{
          hasActiveSimulation
            ? t('slave.contentPanel.toolbar.stopSimulation')
            : t('slave.contentPanel.toolbar.simulate')
        }}</span>
      </div>

      <!-- 批量映射 -->
      <div
        v-if="isCurrentTabControlMappingType"
        class="app-table-toolbar__action"
        @click="openBatchMapping"
      >
        <el-icon class="app-table-toolbar__action-icon"><Link /></el-icon>
        <span class="app-table-toolbar__action-text">{{
          t('slave.contentPanel.toolbar.mapping')
        }}</span>
      </div>
      <div
        v-if="isCurrentTabMonitorType"
        class="app-table-toolbar__action"
        :class="{ 'is-disabled': selectedRows.length === 0 }"
        :aria-disabled="selectedRows.length === 0"
        @click="openBatchGiGroupDialog"
      >
        <el-icon class="app-table-toolbar__action-icon"><CollectionTag /></el-icon>
        <span class="app-table-toolbar__action-text">{{
          t(
            isCurrentTabIntegratedTotal
              ? 'slave.contentPanel.toolbar.interrogationGroups'
              : 'slave.contentPanel.toolbar.giGroup',
          )
        }}</span>
      </div>
      <!-- 密度切换 -->
      <el-dropdown trigger="click" @command="handleDensityChange" popper-class="app-menu-dropdown">
        <div
          class="app-table-toolbar__action app-table-toolbar__action--icon"
          :title="t('slave.contentPanel.toolbar.layout')"
          :aria-label="t('slave.contentPanel.toolbar.layout')"
        >
          <el-icon class="app-table-toolbar__action-icon"><Grid /></el-icon>
        </div>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item command="small" :class="{ active: tableSize === 'small' }">{{
              t('slave.contentPanel.toolbar.density.compact')
            }}</el-dropdown-item>
            <el-dropdown-item command="default" :class="{ active: tableSize === 'default' }">{{
              t('slave.contentPanel.toolbar.density.comfortable')
            }}</el-dropdown-item>
            <el-dropdown-item command="large" :class="{ active: tableSize === 'large' }">{{
              t('slave.contentPanel.toolbar.density.loose')
            }}</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
      <el-popover
        placement="bottom-end"
        trigger="click"
        :teleported="true"
        width="auto"
        popper-class="app-menu-dropdown app-checkbox-popover"
      >
        <template #reference>
          <div
            class="app-table-toolbar__action app-table-toolbar__action--icon"
            :title="t('slave.contentPanel.toolbar.columns.settings')"
            :aria-label="t('slave.contentPanel.toolbar.columns.settings')"
          >
            <el-icon class="app-table-toolbar__action-icon"><Setting /></el-icon>
          </div>
        </template>
        <div class="app-checkbox-popover__panel app-checkbox-popover__column-panel">
          <div
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showType }"
          >
            <el-checkbox v-model="columnVisibility.showType">{{
              t('slave.contentPanel.toolbar.columns.type')
            }}</el-checkbox>
          </div>
          <div
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showDescription }"
          >
            <el-checkbox v-model="columnVisibility.showDescription">{{
              t('slave.contentPanel.toolbar.columns.description')
            }}</el-checkbox>
          </div>
          <div
            v-if="isCurrentTabMappingFamily"
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showMappingTarget }"
          >
            <el-checkbox v-model="columnVisibility.showMappingTarget">{{
              t('slave.contentPanel.toolbar.columns.mappingTarget')
            }}</el-checkbox>
          </div>
          <div
            v-if="isCurrentTabMonitorType"
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showGiGroup }"
          >
            <el-checkbox v-model="columnVisibility.showGiGroup">{{
              t('slave.contentPanel.toolbar.columns.giGroup')
            }}</el-checkbox>
          </div>
          <div
            v-if="isCurrentTabIntegratedTotal"
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showCounterGroup }"
          >
            <el-checkbox v-model="columnVisibility.showCounterGroup">{{
              t('slave.contentPanel.toolbar.columns.counterGroup')
            }}</el-checkbox>
          </div>
          <div
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showCot }"
          >
            <el-checkbox v-model="columnVisibility.showCot">{{
              t('slave.contentPanel.toolbar.columns.cot')
            }}</el-checkbox>
          </div>
          <div
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showCp56 }"
          >
            <el-checkbox v-model="columnVisibility.showCp56">{{
              t('slave.contentPanel.toolbar.columns.cp56')
            }}</el-checkbox>
          </div>
          <div
            class="app-checkbox-popover__item el-dropdown-menu__item"
            :class="{ 'is-active': columnVisibility.showPointSource }"
          >
            <el-checkbox v-model="columnVisibility.showPointSource">{{
              t('slave.contentPanel.toolbar.columns.pointSource')
            }}</el-checkbox>
          </div>
        </div>
      </el-popover>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  Close,
  CollectionTag,
  Filter,
  Grid,
  Link,
  Refresh,
  RefreshRight,
  Search,
  Setting,
  VideoPause,
  VideoPlay,
} from '@element-plus/icons-vue'
import { t } from '@shared/i18n'
import type {
  SlaveContentDataPoint,
  SlaveContentQuickFilters,
  SlaveContentRecentWindowOption,
  SlaveContentTabData,
} from '@/types/slaveContentPanel'

defineProps<{
  currentTab: SlaveContentTabData | null | undefined
  hasActiveFilters: boolean
  activeFilterSummary: string
  quickFilters: SlaveContentQuickFilters
  recentWindowOptions: SlaveContentRecentWindowOption[]
  selectedRows: SlaveContentDataPoint[]
  currentTabIsControl: boolean
  hasActiveSimulation: boolean
  isCurrentTabControlMappingType: boolean
  isCurrentTabMonitorType: boolean
  isCurrentTabIntegratedTotal: boolean
  isCurrentTabMappingFamily: boolean
  tableSize: 'small' | 'default' | 'large'
  columnVisibility: {
    showType: boolean
    showDescription: boolean
    showMappingTarget: boolean
    showGiGroup: boolean
    showCounterGroup: boolean
    showCot: boolean
    showCp56: boolean
    showPointSource: boolean
  }
  clearQuickFilters: () => void
  refreshData: () => void
  clearCurrentTabSelection: () => void
  resetData: () => void
  toggleSimulationFromToolbar: () => void
  openBatchMapping: () => void
  openBatchGiGroupDialog: () => void
  handleDensityChange: (size: 'small' | 'default' | 'large') => void
}>()
</script>
