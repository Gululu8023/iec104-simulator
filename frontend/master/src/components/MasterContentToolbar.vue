<template>
  <template v-if="currentTab">
    <div class="app-table-toolbar">
      <div class="app-table-toolbar__left">
        <el-input
          v-model="currentTab.searchText"
          :placeholder="t('master.contentToolbar.searchPlaceholder')"
          clearable
          :prefix-icon="Search"
          class="app-table-toolbar__search app-table-toolbar__search--compact"
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
                    type="button"
                    class="app-table-filter-trigger"
                    :class="{ 'is-active': hasActiveFilters }"
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
                      t('master.contentToolbar.filters.onlyMapped')
                    }}</el-checkbox>
                  </div>
                  <div
                    class="app-checkbox-popover__item el-dropdown-menu__item"
                    :class="{ 'is-active': quickFilters.onlyAbnormalQuality }"
                  >
                    <el-checkbox v-model="quickFilters.onlyAbnormalQuality">{{
                      t('master.contentToolbar.filters.abnormalQuality')
                    }}</el-checkbox>
                  </div>
                  <div
                    class="app-checkbox-popover__item app-checkbox-popover__item--split el-dropdown-menu__item"
                    :class="{ 'is-active': quickFilters.onlyRecentChange }"
                  >
                    <el-checkbox v-model="quickFilters.onlyRecentChange">{{
                      t('master.contentToolbar.filters.recentChange')
                    }}</el-checkbox>
                    <el-select
                      v-model="quickFilters.recentWindowSec"
                      size="small"
                      class="app-checkbox-popover__recent-select"
                      :disabled="!quickFilters.onlyRecentChange"
                      popper-class="app-select-dropdown app-select-dropdown--compact"
                    >
                      <el-option
                        v-for="item in recentWindowOptions"
                        :key="item.value"
                        :label="item.label"
                        :value="item.value"
                      />
                    </el-select>
                  </div>
                  <div class="app-checkbox-popover__actions">
                    <el-button link type="primary" @click="emit('clear-filters')">{{
                      t('master.contentToolbar.filters.clear')
                    }}</el-button>
                  </div>
                </div>
              </el-popover>
            </div>
          </template>
        </el-input>

        <div class="app-table-toolbar__action" @click="emit('refresh')">
          <el-icon class="app-table-toolbar__action-icon"><Refresh /></el-icon>
          <span class="app-table-toolbar__action-text">{{
            t('master.contentToolbar.refresh')
          }}</span>
        </div>
      </div>

      <div class="app-table-toolbar__right">
        <span v-if="showControlSubtypeFilter" class="app-table-toolbar__segmented-shell">
          <span
            class="app-toolbar-slider app-toolbar-slider--compact app-toolbar-slider--pill"
            :style="remoteControlFilterSliderStyle"
            role="tablist"
            :aria-label="t('master.contentToolbar.filters.remoteControl')"
          >
            <span class="app-toolbar-slider__thumb" aria-hidden="true"></span>
            <button
              v-for="option in localizedRemoteControlFilters"
              :key="option.value"
              type="button"
              class="app-toolbar-slider__option"
              :class="{ 'is-active': currentTab.controlSubtypeFilter === option.value }"
              :aria-pressed="currentTab.controlSubtypeFilter === option.value"
              @click="currentTab.controlSubtypeFilter = option.value"
            >
              {{ option.label }}
            </button>
          </span>
        </span>

        <el-dropdown
          trigger="click"
          popper-class="app-menu-dropdown"
          @command="handleDensityChange"
        >
          <div
            class="app-table-toolbar__action app-table-toolbar__action--icon"
            :title="t('master.contentToolbar.layout')"
            :aria-label="t('master.contentToolbar.layout')"
          >
            <el-icon class="app-table-toolbar__action-icon"><Grid /></el-icon>
          </div>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="small">{{
                t('master.contentToolbar.density.compact')
              }}</el-dropdown-item>
              <el-dropdown-item command="default">{{
                t('master.contentToolbar.density.comfortable')
              }}</el-dropdown-item>
              <el-dropdown-item command="large">{{
                t('master.contentToolbar.density.loose')
              }}</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>

        <el-popover
          placement="bottom-end"
          trigger="click"
          :teleported="true"
          width="220"
          popper-class="app-menu-dropdown app-checkbox-popover"
        >
          <template #reference>
            <div
              class="app-table-toolbar__action app-table-toolbar__action--icon"
              :title="t('master.contentToolbar.columns.settings')"
              :aria-label="t('master.contentToolbar.columns.settings')"
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
                t('master.contentToolbar.columns.type')
              }}</el-checkbox>
            </div>
            <div
              class="app-checkbox-popover__item el-dropdown-menu__item"
              :class="{ 'is-active': columnVisibility.showDescription }"
            >
              <el-checkbox v-model="columnVisibility.showDescription">{{
                t('master.contentToolbar.columns.description')
              }}</el-checkbox>
            </div>
            <template v-if="!currentTabIsControl">
              <div
                class="app-checkbox-popover__item el-dropdown-menu__item"
                :class="{ 'is-active': columnVisibility.showCause }"
              >
                <el-checkbox v-model="columnVisibility.showCause">{{
                  t('master.contentToolbar.columns.cause')
                }}</el-checkbox>
              </div>
              <div
                class="app-checkbox-popover__item el-dropdown-menu__item"
                :class="{ 'is-active': columnVisibility.showReportCount }"
              >
                <el-checkbox v-model="columnVisibility.showReportCount">{{
                  t('master.contentToolbar.columns.reportCount')
                }}</el-checkbox>
              </div>
            </template>
            <div
              class="app-checkbox-popover__item el-dropdown-menu__item"
              :class="{ 'is-active': columnVisibility.showOaCa }"
            >
              <el-checkbox v-model="columnVisibility.showOaCa">OA/CA</el-checkbox>
            </div>
            <div
              class="app-checkbox-popover__item el-dropdown-menu__item"
              :class="{ 'is-active': columnVisibility.showCp56 }"
            >
              <el-checkbox v-model="columnVisibility.showCp56">{{
                currentTabIsControl
                  ? t('master.contentToolbar.columns.commandCp56')
                  : t('master.contentToolbar.columns.eventCp56')
              }}</el-checkbox>
            </div>
            <div
              class="app-checkbox-popover__item el-dropdown-menu__item"
              :class="{ 'is-active': columnVisibility.showPointSource }"
            >
              <el-checkbox v-model="columnVisibility.showPointSource">{{
                t('master.contentToolbar.columns.pointSource')
              }}</el-checkbox>
            </div>
          </div>
        </el-popover>
      </div>
    </div>

    <div v-if="currentControlTabUsesBuiltIn" class="master-built-in-hint">
      {{ t('master.contentToolbar.builtInHint') }}
    </div>
  </template>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Filter, Grid, Refresh, Search, Setting } from '@element-plus/icons-vue'

import { getMasterRemoteControlFilters } from '@shared/api/masterControl'
import { t } from '@shared/i18n'

import type {
  MasterContentQuickFilters,
  MasterContentRecentWindowOption,
  MasterContentTabState,
} from '@/types/masterContentPanel'

type ColumnVisibilityState = {
  showType: boolean
  showDescription: boolean
  showCause: boolean
  showReportCount: boolean
  showOaCa: boolean
  showCp56: boolean
  showPointSource: boolean
}

const props = defineProps<{
  currentTab: MasterContentTabState | null
  currentTabIsControl: boolean
  currentControlTabUsesBuiltIn: boolean
  showControlSubtypeFilter: boolean
  hasActiveFilters: boolean
  activeFilterSummary: string
  quickFilters: MasterContentQuickFilters
  recentWindowOptions: MasterContentRecentWindowOption[]
  remoteControlFilterSliderStyle: Record<string, string>
  columnVisibility: ColumnVisibilityState
}>()

const emit = defineEmits<{
  refresh: []
  'clear-filters': []
  'density-change': [size: 'small' | 'default' | 'large']
}>()

const localizedRemoteControlFilters = computed(() => getMasterRemoteControlFilters())

const handleDensityChange = (size: string | number | object) => {
  const normalized = size === 'small' || size === 'large' ? size : 'default'
  emit('density-change', normalized)
}

void props
</script>

<style scoped>
.master-built-in-hint {
  min-height: 34px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  background: #fff9db;
  border-bottom: 1px solid #fde68a;
  color: #92400e;
  font-size: 12px;
}
</style>
