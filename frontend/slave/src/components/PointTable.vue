<template>
  <!-- 数据点表格 -->
  <div class="datapoint-table" v-if="currentTab">
    <el-table
      :ref="tableRef"
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
      @selection-change="handleSelectionChange"
      @cell-click="handleSelectionCellClick"
      @row-contextmenu="handleRowContextMenu"
    >
      <el-table-column
        type="selection"
        width="36"
        :reserve-selection="true"
        :selectable="isSelectableTableRow"
        class-name="selection-column"
      />
      <el-table-column
        prop="address"
        label="IOA"
        width="104"
        sortable="custom"
        :resizable="true"
        header-align="right"
        align="right"
        class-name="font-mono"
        :formatter="formatIoaTableCell"
      />
      <el-table-column
        prop="name"
        :label="t('slave.contentPanel.table.name')"
        min-width="180"
        sortable="custom"
        :resizable="true"
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
        v-if="columnVisibility.showType"
        :label="t('slave.contentPanel.table.type')"
        width="150"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{
            formatIec104TypeCompactLabel(row.typeId)
          }}</span>
        </template>
      </el-table-column>
      <el-table-column
        v-if="columnVisibility.showDescription"
        prop="description"
        :label="t('slave.contentPanel.table.description')"
        min-width="180"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ row.description || '—' }}</span>
        </template>
      </el-table-column>
      <el-table-column
        prop="value"
        :label="
          currentTabIsControl
            ? t('slave.contentPanel.table.targetValue')
            : t('slave.contentPanel.table.value')
        "
        width="104"
        :resizable="true"
        header-align="right"
        align="right"
        class-name="value-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <div class="value-cell text-right">
              <el-tooltip
                v-if="!currentTabIsControl && isPointSimulating(row)"
                :content="getSimulationTooltip(row)"
                placement="top"
              >
                <span class="simulation-icon">{{ getSimulationIcon(row) }}</span>
              </el-tooltip>
              <el-tooltip
                :disabled="!hasValueDescriptorDetail(row)"
                :content="getValueDescriptorTooltip(row)"
                placement="top"
                :raw-content="true"
              >
                <span :class="['value-text font-mono font-bold', getSemanticValueClass(row)]">{{
                  formatValue(row)
                }}</span>
              </el-tooltip>
            </div>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        v-if="!currentTabIsControl && currentTabHasQuality"
        prop="quality"
        :label="t('slave.contentPanel.table.quality')"
        width="90"
        sortable="custom"
        :resizable="true"
        header-align="center"
        align="center"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <el-tooltip :content="getQualityTooltip(row)" placement="top" :raw-content="true">
              <el-tag
                class="state-badge"
                :type="getQualityType(row.quality)"
                size="small"
                effect="dark"
                round
              >
                {{ getQualityText(row) }}
              </el-tag>
            </el-tooltip>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        v-if="currentTabIsControl"
        :label="t('slave.contentPanel.table.status')"
        width="96"
        :resizable="true"
        header-align="center"
        align="center"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <el-tooltip :content="getExtraInfoTooltip(row)" placement="top" :raw-content="true">
              <el-tag
                class="state-badge"
                :type="getControlStatusTagType(row)"
                size="small"
                effect="dark"
                round
              >
                {{ getControlStatusText(row) }}
              </el-tag>
            </el-tooltip>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        v-if="isCurrentTabMonitorType && columnVisibility.showGiGroup"
        :label="t('slave.contentPanel.table.giGroup')"
        width="130"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ formatGiGroupValue(row.giGroup) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        v-if="isCurrentTabIntegratedTotal && columnVisibility.showCounterGroup"
        :label="t('slave.contentPanel.table.counterGroup')"
        width="130"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{
            formatCounterGroupValue(row.counterGroup)
          }}</span>
        </template>
      </el-table-column>
      <el-table-column
        v-if="columnVisibility.showCot"
        :label="t('slave.contentPanel.table.cot')"
        width="150"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ getCotColumnText(row) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        v-if="columnVisibility.showCp56"
        :label="t('slave.contentPanel.table.cp56')"
        width="176"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
        class-name="font-mono"
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ getCp56ColumnText(row) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        v-if="isCurrentTabMappingFamily && columnVisibility.showMappingTarget"
        :label="t('slave.contentPanel.table.mappingTarget')"
        min-width="210"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
        class-name="mapping-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <span v-if="getMappingTargetDisplayText(row)" class="mapping-target">
              <span class="mapping-target__type">{{ getMappingTargetTypeCellText(row) }}</span>
              <span class="mapping-target__separator">@</span>
              <span class="mapping-target__ioa">{{ getMappingTargetAddressCellText(row) }}</span>
            </span>
            <span v-else class="description-placeholder">—</span>
          </template>
        </template>
      </el-table-column>
      <el-table-column
        v-if="columnVisibility.showPointSource"
        :label="t('slave.contentPanel.table.pointSource')"
        width="112"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)">{{ formatPointSource(row.pointSource) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        prop="timestamp"
        width="184"
        sortable="custom"
        :resizable="true"
        header-align="left"
        align="left"
        show-overflow-tooltip
        class-name="timestamp-column"
      >
        <template #header>
          <span class="header-with-tooltip">
            {{ t('slave.contentPanel.table.updatedAt') }}
            <el-tooltip
              :content="t('slave.contentPanel.table.updatedAtTip')"
              placement="top"
              :enterable="false"
            >
              <el-icon class="header-tip-icon"><InfoFilled /></el-icon>
            </el-tooltip>
          </span>
        </template>
        <template #default="{ row }">
          <span v-if="!isVirtualSpacerRow(row)" class="timestamp-text">{{
            formatTimestamp(row.timestamp)
          }}</span>
        </template>
      </el-table-column>
      <el-table-column
        :label="t('slave.contentPanel.table.action')"
        width="88"
        fixed="right"
        align="center"
        :resizable="false"
        class-name="action-column"
      >
        <template #default="{ row }">
          <template v-if="!isVirtualSpacerRow(row)">
            <div class="hover-actions">
              <el-tooltip
                :content="t('slave.contentPanel.table.edit')"
                placement="top"
                :enterable="false"
              >
                <el-button link type="primary" @click="editDataPoint(row)">
                  <el-icon><Edit /></el-icon>
                </el-button>
              </el-tooltip>
              <el-tooltip
                v-if="!currentTabIsControl"
                :content="t('slave.contentPanel.table.simulation')"
                placement="top"
                :enterable="false"
              >
                <el-button link type="warning" @click="openSimulation(row)">
                  <el-icon><VideoPlay /></el-icon>
                </el-button>
              </el-tooltip>
              <el-tooltip
                v-if="isCurrentTabControlMappingType"
                :content="t('slave.contentPanel.table.commandMapping')"
                placement="top"
                :enterable="false"
              >
                <el-button link type="info" @click="openSingleMapping(row)">
                  <el-icon><Link /></el-icon>
                </el-button>
              </el-tooltip>
            </div>
          </template>
        </template>
      </el-table-column>
      <template #empty><span /></template>
    </el-table>
    <div v-if="currentTab.loading" class="tab-overlay tab-loading-state">
      <el-icon class="is-loading"><Loading /></el-icon>
      <span>{{ t('slave.contentPanel.loading') }}</span>
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
  </div>
</template>

<script setup lang="ts">
import { Edit, InfoFilled, Link, Loading, VideoPlay } from '@element-plus/icons-vue'
import { formatIec104TypeCompactLabel } from '@shared/api/iec104'
import { t } from '@shared/i18n'
import type {
  SlaveContentDataPoint,
  SlaveContentEmptyState,
  SlaveContentTabData,
} from '@/types/slaveContentPanel'

type AnyCallback = (...args: any[]) => any

defineProps<{
  currentTab: SlaveContentTabData
  currentTabEmptyState: SlaveContentEmptyState | null
  tableRef: (instance: unknown) => void
  virtualTableRows: SlaveContentDataPoint[]
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
  currentTabIsControl: boolean
  currentTabHasQuality: boolean
  isCurrentTabMonitorType: boolean
  isCurrentTabIntegratedTotal: boolean
  isCurrentTabMappingFamily: boolean
  isCurrentTabControlMappingType: boolean
  getTableRowKey: AnyCallback
  resolveTableRowClassName: AnyCallback
  resolveTableRowStyle: AnyCallback
  handleSortChange: AnyCallback
  handleSelectionChange: AnyCallback
  handleSelectionCellClick: AnyCallback
  handleRowContextMenu: AnyCallback
  isSelectableTableRow: AnyCallback
  formatIoaTableCell: AnyCallback
  isVirtualSpacerRow: AnyCallback
  isPointSimulating: AnyCallback
  getSimulationTooltip: AnyCallback
  getSimulationIcon: AnyCallback
  hasValueDescriptorDetail: AnyCallback
  getValueDescriptorTooltip: AnyCallback
  getSemanticValueClass: AnyCallback
  formatValue: AnyCallback
  getQualityTooltip: AnyCallback
  getQualityType: AnyCallback
  getQualityText: AnyCallback
  getExtraInfoTooltip: AnyCallback
  getControlStatusTagType: AnyCallback
  getControlStatusText: AnyCallback
  formatGiGroupValue: AnyCallback
  formatCounterGroupValue: AnyCallback
  getCotColumnText: AnyCallback
  getCp56ColumnText: AnyCallback
  getMappingTargetDisplayText: AnyCallback
  getMappingTargetTypeCellText: AnyCallback
  getMappingTargetAddressCellText: AnyCallback
  formatPointSource: AnyCallback
  formatTimestamp: AnyCallback
  editDataPoint: AnyCallback
  openSimulation: AnyCallback
  openSingleMapping: AnyCallback
}>()
</script>

<style scoped>
.datapoint-table {
  position: relative;
  flex: 1;
  overflow: hidden;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.tab-overlay {
  position: absolute;
  inset: 0;
  z-index: 2;
  pointer-events: auto;
}

.tab-empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: white;
  width: 100%;
}

.tab-loading-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--el-text-color-secondary);
  background-color: white;
}

:deep(.custom-data-table .el-table__cell .cell) {
  min-width: 0;
  width: 100%;
  display: flex;
  align-items: center;
  white-space: nowrap;
}

:deep(.custom-data-table th.el-table__cell .cell) {
  min-height: 32px;
}

:deep(.custom-data-table td.el-table__cell .cell) {
  min-height: 36px;
}

:deep(.custom-data-table .el-table__cell.is-right .cell) {
  justify-content: flex-end;
}

:deep(.custom-data-table .el-table__cell.is-center .cell) {
  justify-content: center;
}

:deep(.custom-data-table .el-table__cell:not(.is-right):not(.is-center) .cell) {
  justify-content: flex-start;
}

:deep(.custom-data-table th.el-table__cell),
:deep(.custom-data-table td.el-table__cell) {
  vertical-align: middle;
}

:deep(.custom-data-table th.el-table__cell .caret-wrapper) {
  margin-left: 4px;
  height: 16px;
  display: inline-flex;
  flex-direction: column;
  justify-content: center;
}

:deep(.custom-data-table .name-column .cell),
:deep(.custom-data-table .mapping-column .cell) {
  overflow: hidden;
  text-overflow: ellipsis;
}

:deep(.custom-data-table .action-column .cell) {
  overflow: visible;
}

:deep(.custom-data-table td.selection-column) {
  cursor: pointer;
}

:deep(.custom-data-table td.selection-column:hover) {
  background-color: var(--el-color-primary-light-9) !important;
}

:deep(.custom-data-table .el-table__body tr.is-located-row > td.el-table__cell) {
  background-color: var(--el-color-primary-light-9) !important;
}

:deep(.custom-data-table .virtual-spacer-row > td.el-table__cell) {
  padding: 0 !important;
  border-bottom: 0 !important;
  background: transparent !important;
}

:deep(.custom-data-table .virtual-spacer-row .cell) {
  padding: 0 !important;
  min-height: 0 !important;
  height: 100% !important;
  visibility: hidden;
}

.header-with-tooltip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.header-tip-icon {
  font-size: 13px;
  color: #94a3b8;
}

.value-cell {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.simulation-icon {
  font-size: 14px;
  animation: pulse 1.5s ease-in-out infinite;
  cursor: help;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.6;
  }
}

.description-placeholder {
  color: #9ca3af;
  font-style: italic;
}

.name-with-description {
  cursor: help;
}

.hover-actions {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

:deep(.state-badge.el-tag) {
  min-width: 56px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding-inline: 10px;
}

.el-button {
  margin: 0;
}

:deep(.el-table .el-table__row) {
  cursor: pointer;
}

:deep(.el-table__body td.el-table__cell) {
  padding: 6px 0;
}
</style>
