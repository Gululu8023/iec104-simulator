<template>
  <el-dialog
    v-model="innerVisible"
    width="820px"
    :before-close="handleBeforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('master.fileTransfer.title') }}</div>
          <div class="app-dialog-subtitle-badges" v-if="dialogSubtitleBadges.length > 0">
            <span
              v-for="(badge, index) in dialogSubtitleBadges"
              :key="`file-transfer-dialog-badge-${index}`"
              class="app-dialog-subtitle-badge"
            >
              {{ badge }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body conn-flat-body ft-body">
      <el-tabs v-model="activeTab" class="ft-tabs">
        <MasterFileTransferDownloadView />
        <MasterFileTransferUploadView />
      </el-tabs>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="requestCloseDialog">{{
            t('master.fileTransfer.actions.close')
          }}</el-button>
          <el-button
            v-if="isRunning"
            type="danger"
            plain
            :loading="busy"
            :disabled="busy"
            @click="cancelTransfer"
          >
            {{ cancelActionLabel }}
          </el-button>
          <el-button
            v-else
            type="primary"
            :loading="busy"
            @click="handlePrimaryAction"
            :disabled="!canPrimaryAction"
          >
            {{ primaryActionLabel }}
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'
import type { MasterFileTransferRequest } from '@shared/api/types'
import MasterFileTransferDownloadView from './MasterFileTransferDownloadView.vue'
import MasterFileTransferUploadView from './MasterFileTransferUploadView.vue'
import {
  type MasterFileTransferDialogProps,
  provideMasterFileTransferDialogContext,
  useMasterFileTransferDialog,
} from '../composables/useMasterFileTransferDialog'

const props = defineProps<MasterFileTransferDialogProps>()
const emit = defineEmits<{
  'update:visible': [visible: boolean]
  start: [request: MasterFileTransferRequest]
  cancel: []
}>()

const dialogContext = useMasterFileTransferDialog(props, emit)
provideMasterFileTransferDialogContext(dialogContext)

const {
  activeTab,
  busy,
  canPrimaryAction,
  cancelActionLabel,
  cancelTransfer,
  dialogSubtitleBadges,
  handleBeforeClose,
  handlePrimaryAction,
  innerVisible,
  isRunning,
  primaryActionLabel,
  requestCloseDialog,
} = dialogContext
</script>
<style scoped src="./MasterFileTransferDialog.css"></style>

<style>
.app-daterange-dropdown.el-picker__popper.el-popper {
  padding: 0 !important;
  border: 1px solid #e5e7eb !important;
  border-radius: 10px !important;
  background: #ffffff !important;
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.08) !important;
  overflow: hidden;
}

.app-daterange-dropdown.el-popper .el-popper__arrow::before {
  border-color: #e5e7eb !important;
  background: #ffffff !important;
}

.app-daterange-dropdown .el-picker-panel,
.app-daterange-dropdown .el-date-range-picker {
  background: #ffffff;
  border-radius: 10px;
  color: #475569;
}

.app-daterange-dropdown .el-date-range-picker {
  --el-datepicker-border-color: #e5e7eb;
  --el-datepicker-inner-border-color: #eef2f7;
  --el-datepicker-hover-text-color: #2563eb;
  --el-datepicker-active-color: #dbeafe;
  --el-datepicker-inrange-bg-color: #f5f9ff;
  --el-datepicker-inrange-hover-bg-color: #edf4ff;
}

.app-daterange-dropdown .el-date-range-picker__time-header {
  padding: 10px 10px 8px;
  border-bottom: 1px solid #eef2f7;
  background: #fbfcfe;
}

.app-daterange-dropdown .el-date-range-picker__editors-wrap,
.app-daterange-dropdown .el-date-range-picker__time-picker-wrap {
  padding: 0 4px;
}

.app-daterange-dropdown .el-date-range-picker__time-header > .el-icon-arrow-right {
  color: #94a3b8;
  font-size: 16px;
}

.app-daterange-dropdown .el-date-range-picker__time-header .el-input__wrapper {
  min-height: 30px;
  border-radius: 8px;
  background: #f8fafc;
  box-shadow: 0 0 0 1px #e5e7eb inset !important;
}

.app-daterange-dropdown .el-date-range-picker__time-header .el-input__wrapper:hover {
  background: #f3f6fb;
}

.app-daterange-dropdown .el-date-range-picker__time-header .el-input__wrapper.is-focus,
.app-daterange-dropdown .el-date-range-picker__time-header .el-input__wrapper.is-focused {
  background: #ffffff;
  box-shadow:
    0 0 0 1px #4a88f0 inset,
    0 0 0 3px rgba(74, 136, 240, 0.12) !important;
}

.app-daterange-dropdown .el-date-range-picker__time-header .el-input__inner {
  height: 28px;
  font-size: 12px;
  color: #475569;
}

.app-daterange-dropdown .el-date-range-picker__time-header .el-input__inner::placeholder {
  color: #9ca3af;
}

.app-daterange-dropdown .el-date-range-picker__content {
  padding: 14px 14px 12px;
}

.app-daterange-dropdown .el-date-range-picker__content.is-left {
  border-right: 1px solid #eef2f7;
}

.app-daterange-dropdown .el-date-range-picker__header {
  height: 30px;
}

.app-daterange-dropdown .el-date-range-picker__header div,
.app-daterange-dropdown .el-date-range-picker__header-label {
  color: #475569;
  font-size: 15px;
  font-weight: 600;
}

.app-daterange-dropdown .el-picker-panel__icon-btn {
  color: #64748b;
  margin-top: 6px;
}

.app-daterange-dropdown .el-picker-panel__icon-btn:hover,
.app-daterange-dropdown .el-picker-panel__icon-btn:focus-visible {
  color: #2563eb;
}

.app-daterange-dropdown .el-date-table th {
  color: #94a3b8;
  border-bottom-color: #eef2f7;
  font-size: 12px;
}

.app-daterange-dropdown .el-date-table td.available:hover .el-date-table-cell__text,
.app-daterange-dropdown .el-month-table td .el-date-table-cell__text:hover,
.app-daterange-dropdown .el-year-table td .el-date-table-cell__text:hover {
  background: #eff6ff;
  color: #2563eb;
}

.app-daterange-dropdown .el-date-table td.today .el-date-table-cell__text,
.app-daterange-dropdown .el-month-table td.today .el-date-table-cell__text,
.app-daterange-dropdown .el-year-table td.today .el-date-table-cell__text {
  color: #2563eb;
  background: #f8fbff;
  box-shadow: 0 0 0 1px #bfdbfe inset;
  font-weight: 600;
}

.app-daterange-dropdown .el-date-table td.in-range .el-date-table-cell,
.app-daterange-dropdown .el-month-table td.in-range .el-date-table-cell,
.app-daterange-dropdown .el-year-table td.in-range .el-date-table-cell {
  background: #f5f9ff;
}

.app-daterange-dropdown .el-date-table td.current:not(.disabled) .el-date-table-cell__text,
.app-daterange-dropdown .el-date-table td.start-date .el-date-table-cell__text,
.app-daterange-dropdown .el-date-table td.end-date .el-date-table-cell__text,
.app-daterange-dropdown .el-month-table td.current:not(.disabled) .el-date-table-cell__text,
.app-daterange-dropdown .el-month-table td.start-date .el-date-table-cell__text,
.app-daterange-dropdown .el-month-table td.end-date .el-date-table-cell__text,
.app-daterange-dropdown .el-year-table td.current:not(.disabled) .el-date-table-cell__text,
.app-daterange-dropdown .el-year-table td.start-date .el-date-table-cell__text,
.app-daterange-dropdown .el-year-table td.end-date .el-date-table-cell__text {
  background: #dbeafe;
  color: #1d4ed8;
  font-weight: 600;
}

.app-daterange-dropdown .el-date-table td.start-date .el-date-table-cell,
.app-daterange-dropdown .el-date-table td.end-date .el-date-table-cell,
.app-daterange-dropdown .el-month-table td.start-date .el-date-table-cell,
.app-daterange-dropdown .el-month-table td.end-date .el-date-table-cell,
.app-daterange-dropdown .el-year-table td.start-date .el-date-table-cell,
.app-daterange-dropdown .el-year-table td.end-date .el-date-table-cell {
  color: #1d4ed8;
}

.app-daterange-dropdown .el-picker-panel__footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid #eef2f7;
  background: #fcfdff;
}

.app-daterange-dropdown .el-picker-panel__footer .el-button {
  min-width: 0;
  height: 28px;
  margin-left: 0;
  padding: 0 12px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  box-shadow: none !important;
}

.app-daterange-dropdown .el-picker-panel__footer .el-button:first-child {
  border: 1px solid transparent;
  background: transparent;
  color: #64748b;
}

.app-daterange-dropdown .el-picker-panel__footer .el-button:first-child:hover {
  background: #f3f6fb;
  color: #374151;
}

.app-daterange-dropdown .el-picker-panel__footer .el-button:last-child {
  border-color: #2563eb;
  background: #2563eb;
  color: #ffffff;
}

.app-daterange-dropdown .el-picker-panel__footer .el-button:last-child:hover {
  border-color: #1d4ed8;
  background: #1d4ed8;
  color: #ffffff;
}

.app-daterange-dropdown .el-picker-panel__footer .el-button.is-disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
</style>
