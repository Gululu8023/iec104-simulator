<template>
  <!-- ==================== 下载 ==================== -->
  <el-tab-pane :label="t('master.fileTransfer.tabs.download')" name="download">
    <section class="conn-section">
      <div
        class="conn-section-title ft-mode-tabs"
        role="tablist"
        :aria-label="t('master.fileTransfer.modes.aria')"
      >
        <button
          type="button"
          class="ft-mode-tab"
          :class="{ 'is-active': directoryMode === 'directory' }"
          :aria-pressed="directoryMode === 'directory'"
          @click="directoryMode = 'directory'"
        >
          {{ t('master.fileTransfer.modes.directory') }}
        </button>
        <button
          type="button"
          class="ft-mode-tab"
          :class="{ 'is-active': directoryMode === 'log' }"
          :aria-pressed="directoryMode === 'log'"
          @click="directoryMode = 'log'"
        >
          {{ t('master.fileTransfer.modes.log') }}
        </button>
      </div>

      <div v-if="directoryMode === 'log'" class="app-dialog-field-grid--2 ft-query-grid">
        <label class="app-dialog-field-label">{{
          t('master.fileTransfer.fields.targetFile')
        }}</label>
        <div class="app-dialog-field-control ft-query-field">
          <el-select
            v-model="queryLogNof"
            size="small"
            class="app-dialog-input conn-inline-input"
            popper-class="app-select-dropdown"
            fit-input-width
            filterable
          >
            <el-option
              v-for="o in nofSelectOptions"
              :key="o.value"
              :value="o.value"
              :label="o.label"
            />
          </el-select>
        </div>
        <label class="app-dialog-field-label">{{
          t('master.fileTransfer.fields.timeRange')
        }}</label>
        <div class="app-dialog-field-control ft-query-field ft-query-field--range">
          <el-date-picker
            :model-value="queryDateRange"
            @update:model-value="handleQueryDateRangeChange"
            type="datetimerange"
            size="small"
            :range-separator="dateRangeSeparator"
            :start-placeholder="t('master.fileTransfer.placeholders.startTime')"
            :end-placeholder="t('master.fileTransfer.placeholders.endTime')"
            format="YYYY-MM-DD HH:mm:ss"
            class="app-dialog-input conn-inline-input ft-query-date-range"
            popper-class="app-daterange-dropdown"
            clearable
          />
        </div>
        <div class="app-dialog-field-control app-dialog-action-control ft-query-action">
          <el-button
            size="small"
            class="app-dialog-btn app-dialog-btn--outline"
            @click="startQueryLog()"
            :disabled="!canQueryLog"
            >{{ t('master.fileTransfer.actions.query') }}</el-button
          >
        </div>
      </div>

      <div v-else class="ft-directory-toolbar">
        <div class="ft-directory-toolbar__hint">
          {{ directoryHelperText }}
        </div>
        <div class="ft-directory-toolbar__action">
          <el-button
            size="small"
            class="app-dialog-btn app-dialog-btn--outline"
            @click="queryDirectory()"
            :disabled="!canQueryDirectory"
            >{{ directoryActionLabel }}</el-button
          >
        </div>
      </div>

      <div class="app-dialog-table-wrap ft-table-wrap">
        <el-table
          :data="directoryEntries"
          row-key="nof"
          :current-row-key="selectedEntryKey"
          size="small"
          height="192"
          stripe
          highlight-current-row
          class="ft-directory-table"
          @row-click="handleDirectorySelect"
        >
          <el-table-column :label="t('master.fileTransfer.fields.file')" min-width="220">
            <template #default="{ row }">
              <div class="ft-file-cell">
                <div class="ft-file-title">{{ formatFileTitle(row.nof) }}</div>
              </div>
            </template>
          </el-table-column>
          <el-table-column
            prop="length"
            :label="t('master.fileTransfer.fields.size')"
            width="120"
            sortable
          >
            <template #default="{ row }">{{ formatFileSize(row.length) }}</template>
          </el-table-column>
          <el-table-column
            prop="nof"
            label="NOF"
            width="88"
            sortable
            header-align="right"
            align="right"
            header-class-name="ft-directory-table__th--nof"
            class-name="font-mono"
          >
            <template #default="{ row }">
              {{ row.nof }}
            </template>
          </el-table-column>
          <el-table-column
            prop="timestamp"
            :label="t('master.fileTransfer.fields.time')"
            min-width="180"
            sortable
          >
            <template #default="{ row }">{{
              formatTimestamp(row.timestamp) || t('master.fileTransfer.noTimestamp')
            }}</template>
          </el-table-column>
          <template #empty>
            <div class="ft-empty-state">
              <div class="ft-empty-state__title">{{ directoryEmptyTitle }}</div>
              <div class="ft-empty-state__hint">{{ directoryEmptyHint }}</div>
            </div>
          </template>
        </el-table>
      </div>
    </section>

    <section class="conn-section">
      <div class="conn-section-title">{{ t('master.fileTransfer.fields.downloadParams') }}</div>
      <div class="app-dialog-field-grid--1">
        <span class="app-dialog-field-label ft-selected-file-summary__label">{{
          t('master.fileTransfer.fields.selectedFile')
        }}</span>
        <div class="app-dialog-field-control ft-selected-file-summary__content">
          <template v-if="selectedEntry">
            <div class="ft-selected-file-summary__inline">
              <span class="ft-selected-file-summary__name">{{
                formatFileTitle(selectedEntry.nof)
              }}</span>
              <span class="ft-selected-file-summary__meta">{{ selectedEntryMeta }}</span>
            </div>
          </template>
          <span v-else class="ft-selected-file-summary__placeholder">{{
            t('master.fileTransfer.placeholders.noFileSelected')
          }}</span>
        </div>
        <label class="app-dialog-field-label">{{ t('master.fileTransfer.fields.savePath') }}</label>
        <div class="app-dialog-field-control app-dialog-action-control ft-path-field">
          <el-input
            v-model="downloadPath"
            size="small"
            :placeholder="t('master.fileTransfer.placeholders.downloadPath')"
            readonly
            class="conn-inline-input"
          />
          <el-button
            size="small"
            class="app-dialog-btn app-dialog-btn--outline"
            @click="chooseDownloadPath"
            >{{ t('master.fileTransfer.actions.browse') }}</el-button
          >
        </div>
      </div>
    </section>

    <section class="conn-section">
      <div class="conn-section-title">{{ t('master.fileTransfer.fields.transferStatus') }}</div>
      <div class="ft-status-panel">
        <div class="ft-status-head">
          <div class="ft-status-row ft-status-row--compact">
            <span class="ft-status-label">{{ t('master.fileTransfer.fields.status') }}</span>
            <span
              class="ft-status-badge"
              :class="`ft-status-badge--${resolveTransferPanelState('download')}`"
              >{{ resolveTransferPanelLabel('download') }}</span
            >
          </div>
          <span class="ft-status-bytes">{{ resolveTransferPanelBytesLabel('download') }}</span>
        </div>
        <div class="ft-status-progress">
          <el-progress
            :percentage="resolveTransferPanelProgress('download')"
            :stroke-width="8"
            :show-text="false"
            :status="resolveTransferPanelProgressStatus('download')"
          />
        </div>
        <div
          v-if="resolveTransferPanelFailureReason('download')"
          class="ft-status-row ft-status-row--error"
        >
          <span class="ft-status-label">{{ t('master.fileTransfer.fields.failureReason') }}</span>
          <span class="ft-status-mono">{{ resolveTransferPanelFailureReason('download') }}</span>
        </div>
      </div>
    </section>
  </el-tab-pane>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'
import { useMasterFileTransferDialogContext } from '../composables/useMasterFileTransferDialog'

const {
  canQueryDirectory,
  canQueryLog,
  chooseDownloadPath,
  dateRangeSeparator,
  directoryActionLabel,
  directoryEmptyHint,
  directoryEmptyTitle,
  directoryEntries,
  directoryHelperText,
  directoryMode,
  downloadPath,
  formatFileSize,
  formatFileTitle,
  formatTimestamp,
  handleDirectorySelect,
  handleQueryDateRangeChange,
  nofSelectOptions,
  queryDateRange,
  queryDirectory,
  queryLogNof,
  resolveTransferPanelBytesLabel,
  resolveTransferPanelFailureReason,
  resolveTransferPanelLabel,
  resolveTransferPanelProgress,
  resolveTransferPanelProgressStatus,
  resolveTransferPanelState,
  selectedEntry,
  selectedEntryKey,
  selectedEntryMeta,
  startQueryLog,
} = useMasterFileTransferDialogContext()
</script>

<style scoped src="./MasterFileTransferDialog.css"></style>
