<template>
  <div v-if="currentTab" class="table-container">
    <div v-if="currentTab.loading" class="tab-overlay tab-loading-state">
      <el-icon class="is-loading"><Loading /></el-icon>
      <span>{{ t('master.contentPanel.loading') }}</span>
    </div>
    <div v-else-if="currentTabEmptyState" class="tab-overlay tab-empty-state">
      <el-empty :image-size="100">
        <template #description>
          <div class="tab-empty-guide">
            <p class="tab-empty-title">{{ currentTabEmptyState.title }}</p>
            <p class="tab-empty-hint">{{ currentTabEmptyState.hint }}</p>
          </div>
        </template>
      </el-empty>
    </div>
    <el-table
      :ref="setTableRef"
      :data="virtualTableRows"
      stripe
      border
      height="100%"
      table-layout="fixed"
      fit
      :size="tableSize"
      :row-key="getTableRowKey"
      :row-class-name="resolveTableRowClassName"
      :row-style="resolveTableRowStyle"
      :default-sort="{ prop: 'address', order: 'ascending' }"
      class="custom-data-table"
      @sort-change="handleSortChange"
      @row-contextmenu="handleRowContextMenu"
    >
      <el-table-column
        key="ioa"
        prop="address"
        label="IOA"
        width="104"
        sortable="custom"
        header-align="right"
        align="right"
        class-name="font-mono"
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ formatIoaAddress(row.address) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        key="name"
        prop="name"
        :label="t('master.contentPanel.columns.name')"
        min-width="180"
        sortable="custom"
        header-align="left"
        align="left"
        show-overflow-tooltip
        class-name="name-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <el-tooltip
              v-if="row.description"
              :content="row.description"
              placement="top"
              :show-after="180"
            >
              <span class="name-with-description">{{ row.name }}</span>
            </el-tooltip>
            <span v-else>{{ row.name }}</span>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        key="type"
        v-if="showControlTypeColumn || columnVisibility.showType"
        prop="dataType"
        :label="t('master.contentPanel.columns.type')"
        width="150"
        header-align="left"
        align="left"
        class-name="control-type-column"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ getControlTypeLabel(row) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        key="description"
        v-if="columnVisibility.showDescription"
        prop="description"
        :label="t('master.contentPanel.columns.description')"
        min-width="180"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ row.description || '—' }}</span>
        </template>
      </el-table-column>
      <el-table-column
        key="value"
        prop="value"
        :label="
          currentTabIsControl
            ? t('master.contentPanel.columns.valueTarget')
            : t('master.contentPanel.columns.value')
        "
        min-width="120"
        header-align="right"
        align="right"
        class-name="value-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <div class="value-cell text-right">
              <el-tooltip
                :disabled="!hasValueDescriptorDetail(row)"
                :content="getValueDescriptorTooltip(row)"
                placement="top"
                :raw-content="true"
              >
                <span
                  v-flash-update="row.value"
                  :class="['value-text font-mono font-bold', getSemanticValueClass(row)]"
                  >{{ formatValue(row) }}</span
                >
              </el-tooltip>
            </div>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        key="quality"
        v-if="!currentTabIsControl && currentTabHasQuality"
        :label="t('master.contentPanel.columns.quality')"
        width="104"
        header-align="center"
        align="center"
        class-name="state-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <el-tooltip
              :content="
                formatPointQualityTooltipLocalized(
                  row.typeId,
                  row.qualityCommon,
                  row.qualityDetail,
                  row.qualityCode,
                )
              "
              placement="top"
              :raw-content="true"
            >
              <el-tag
                class="state-badge"
                :type="qualityTagType(row.qualityLevel)"
                effect="dark"
                size="small"
                round
                >{{
                  formatPointQualityLabelLocalized(
                    row.typeId,
                    row.qualityCommon,
                    row.qualityCode,
                    row.qualityDetail,
                  )
                }}</el-tag
              >
            </el-tooltip>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        key="control-status"
        v-if="currentTabIsControl"
        :label="t('master.contentPanel.columns.status')"
        width="108"
        header-align="center"
        align="center"
        class-name="state-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <el-tooltip :content="controlStatusTooltip(row)" placement="top" :raw-content="true">
              <el-tag :type="controlStatusTagType(row)" effect="dark" size="small" round>{{
                controlStatusText(row)
              }}</el-tag>
            </el-tooltip>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        key="cause"
        v-if="!currentTabIsControl && columnVisibility.showCause"
        prop="latestCauseText"
        :label="t('master.contentPanel.columns.cause')"
        min-width="132"
        header-align="left"
        align="left"
        show-overflow-tooltip
      />
      <el-table-column
        key="report-count"
        v-if="!currentTabIsControl && columnVisibility.showReportCount"
        prop="reportCount"
        :label="t('master.contentPanel.columns.count')"
        width="88"
        sortable="custom"
        header-align="right"
        align="right"
        class-name="font-mono"
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ row.reportCount ?? '—' }}</span>
        </template>
      </el-table-column>
      <el-table-column
        key="oa-ca"
        v-if="columnVisibility.showOaCa"
        prop="oaCaText"
        label="OA/CA"
        min-width="108"
        header-align="left"
        align="left"
        show-overflow-tooltip
      />
      <el-table-column
        key="cp56"
        v-if="columnVisibility.showCp56"
        prop="cp56Text"
        :label="
          currentTabIsControl
            ? t('master.contentPanel.columns.commandCp56')
            : t('master.contentPanel.columns.eventCp56')
        "
        min-width="172"
        header-align="left"
        align="left"
        show-overflow-tooltip
      />
      <el-table-column
        key="point-source"
        v-if="columnVisibility.showPointSource"
        :label="t('master.contentPanel.columns.pointSource')"
        width="112"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ formatPointSource(row.pointSource) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        key="updated-at"
        prop="timestamp"
        :label="t('master.contentPanel.columns.updatedAt')"
        width="184"
        sortable="custom"
        header-align="left"
        align="left"
        class-name="timestamp-column"
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)" class="timestamp-text">{{
            formatTimestamp(row.timestamp)
          }}</span>
        </template>
      </el-table-column>
      <el-table-column
        key="actions"
        :label="t('master.contentPanel.columns.actions')"
        width="88"
        fixed="right"
        align="center"
        class-name="action-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <div class="hover-actions">
              <el-tooltip
                :content="t('master.contentPanel.actions.editNameDescription')"
                placement="top"
                :enterable="false"
              >
                <el-button link type="primary" @click.stop="openEditDialog(row)">
                  <el-icon><Edit /></el-icon>
                </el-button>
              </el-tooltip>
              <el-tooltip
                v-if="!currentTabIsControl"
                :content="t('master.contentPanel.actions.viewHistory')"
                placement="top"
                :enterable="false"
              >
                <el-button link type="info" @click.stop="openHistoryDialog(row)">
                  <el-icon><Clock /></el-icon>
                </el-button>
              </el-tooltip>
              <el-tooltip
                v-else
                :content="t('master.contentPanel.actions.sendCommand')"
                placement="top"
                :enterable="false"
              >
                <el-button link type="primary" @click.stop="handleControlDataPoint(row)">
                  <el-icon><Operation /></el-icon>
                </el-button>
              </el-tooltip>
            </div>
          </template>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>
<script setup lang="ts">
import { type Directive } from 'vue'
import { Clock, Edit, Loading, Operation } from '@element-plus/icons-vue'

import {
  formatPointQualityLabelLocalized,
  formatPointQualityTooltipLocalized,
} from '@shared/api/iec104'
import { t } from '@shared/i18n'
import type { MasterContentTabRow, MasterContentTabState } from '@/types/masterContentPanel'

type EmptyState = { title: string; hint: string } | null
type TableSize = 'small' | 'default' | 'large'

const vFlashUpdate: Directive = {
  updated(el, binding) {
    if (binding.value !== binding.oldValue) {
      el.classList.add('flash-update')
      setTimeout(() => el.classList.remove('flash-update'), 800)
    }
  },
}

defineProps<{
  currentTab: MasterContentTabState
  currentTabEmptyState: EmptyState
  virtualTableRows: MasterContentTabRow[]
  tableSize: TableSize
  setTableRef: (instance: unknown) => void
  getTableRowKey: (row: any) => string
  resolveTableRowClassName: (payload: any) => string
  resolveTableRowStyle: (payload: any) => Record<string, string>
  handleSortChange: (payload: any) => void
  handleRowContextMenu: (row: MasterContentTabRow, column: unknown, event: MouseEvent) => void
  isVirtualSpacerRow: (row: unknown) => boolean
  formatIoaAddress: (value: unknown) => string
  showControlTypeColumn: boolean
  columnVisibility: Record<string, boolean>
  getControlTypeLabel: (row: MasterContentTabRow) => string
  currentTabIsControl: boolean
  currentTabHasQuality: boolean
  hasValueDescriptorDetail: (row: MasterContentTabRow) => boolean
  getValueDescriptorTooltip: (row: MasterContentTabRow) => string
  getSemanticValueClass: (row: MasterContentTabRow) => string
  formatValue: (row: MasterContentTabRow) => string
  qualityTagType: (quality: 'good' | 'questionable' | 'invalid') => any
  controlStatusTooltip: (row: MasterContentTabRow) => string
  controlStatusTagType: (row: MasterContentTabRow) => any
  controlStatusText: (row: MasterContentTabRow) => string
  formatPointSource: (source: string | null | undefined) => string
  formatTimestamp: (value: string) => string
  openEditDialog: (row: MasterContentTabRow) => void
  openHistoryDialog: (row: MasterContentTabRow) => void
  handleControlDataPoint: (row: MasterContentTabRow) => void
}>()
</script>

<style scoped>
.table-container {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.tab-overlay {
  position: absolute;
  inset: 0;
  z-index: 2;
  pointer-events: auto;
}

.tab-empty-state,
.tab-loading-state {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: white;
}

.tab-loading-state {
  gap: 8px;
  color: var(--el-text-color-secondary);
}

.name-with-description {
  cursor: help;
}

.flash-update {
  animation: flash-highlight 400ms ease-out;
}

@keyframes flash-highlight {
  0% {
    background-color: #fef3c7;
  }
  100% {
    background-color: transparent;
  }
}
</style>
