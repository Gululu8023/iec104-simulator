<template>
  <!-- ==================== 上传 ==================== -->
  <el-tab-pane :label="t('master.fileTransfer.tabs.upload')" name="upload">
    <section class="conn-section">
      <div class="conn-section-title">{{ t('master.fileTransfer.fields.uploadParams') }}</div>
      <div class="app-dialog-stack">
        <div class="app-dialog-field-grid--2">
          <label class="app-dialog-field-label">{{
            t('master.fileTransfer.fields.fileType')
          }}</label>
          <div class="app-dialog-field-control">
            <el-select
              v-model="uploadNof"
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
            t('master.fileTransfer.fields.uploadFile')
          }}</label>
          <div class="app-dialog-field-control app-dialog-action-control ft-path-field">
            <el-input
              v-model="uploadPath"
              size="small"
              :placeholder="t('master.fileTransfer.placeholders.uploadFile')"
              readonly
              class="conn-inline-input"
            />
            <el-button
              size="small"
              class="app-dialog-btn app-dialog-btn--outline"
              @click="chooseUploadPath"
              >{{ t('master.fileTransfer.actions.browse') }}</el-button
            >
          </div>
          <div
            class="app-dialog-field-control app-dialog-field--full ft-requirement-hint"
            :class="{ 'is-ready': canStartUpload }"
          >
            {{ uploadRequirementHint }}
          </div>
          <div
            v-if="singleSectionLimitText"
            class="app-dialog-field-control app-dialog-field--full ft-requirement-hint"
          >
            {{ singleSectionLimitText }}
          </div>
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
              :class="`ft-status-badge--${resolveTransferPanelState('upload')}`"
              >{{ resolveTransferPanelLabel('upload') }}</span
            >
          </div>
          <span class="ft-status-bytes">{{ resolveTransferPanelBytesLabel('upload') }}</span>
        </div>
        <div class="ft-status-progress">
          <el-progress
            :percentage="resolveTransferPanelProgress('upload')"
            :stroke-width="8"
            :show-text="false"
            :status="resolveTransferPanelProgressStatus('upload')"
          />
        </div>
        <div
          v-if="resolveTransferPanelFailureReason('upload')"
          class="ft-status-row ft-status-row--error"
        >
          <span class="ft-status-label">{{ t('master.fileTransfer.fields.failureReason') }}</span>
          <span class="ft-status-mono">{{ resolveTransferPanelFailureReason('upload') }}</span>
        </div>
      </div>
    </section>
  </el-tab-pane>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'
import { useMasterFileTransferDialogContext } from '../composables/useMasterFileTransferDialog'

const {
  canStartUpload,
  chooseUploadPath,
  nofSelectOptions,
  resolveTransferPanelBytesLabel,
  resolveTransferPanelFailureReason,
  resolveTransferPanelLabel,
  resolveTransferPanelProgress,
  resolveTransferPanelProgressStatus,
  resolveTransferPanelState,
  singleSectionLimitText,
  uploadNof,
  uploadPath,
  uploadRequirementHint,
} = useMasterFileTransferDialogContext()
</script>

<style scoped src="./MasterFileTransferDialog.css"></style>
