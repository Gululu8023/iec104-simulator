<template>
  <el-dialog
    v-model="visible"
    width="620px"
    :close-on-click-modal="false"
    :close-on-press-escape="!busy"
    :show-close="!busy"
    :lock-scroll="false"
    class="app-dialog-shell"
  >
    <template #header
      ><div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('master.appDialogs.readCommand.title') }}</div>
          <div v-if="badges.length" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in badges"
              :key="`read-command-badge-${index}`"
              class="app-dialog-subtitle-badge"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div></template
    >
    <div class="app-dialog-body conn-flat-body">
      <section class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.control.commandParams') }}</div>
        <el-form class="app-dialog-form" @submit.prevent
          ><div class="app-dialog-field-grid--1">
            <label class="app-dialog-field-label">IOA</label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="ioa"
                :min="0"
                :max="16777215"
                :disabled="busy"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
                @change="$emit('ioa-change')"
              />
            </div></div
        ></el-form>
      </section>
      <section v-if="status !== 'idle'" class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.readCommand.response') }}</div>
        <div class="command-result-banner" :class="`is-${status}`">{{ message }}</div>
        <div
          v-if="status === 'success' && pointName"
          class="app-dialog-field-grid--2 read-command-response-grid"
        >
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.readCommand.name')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input :model-value="pointName" readonly class="app-dialog-input" />
          </div>
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.readCommand.type')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input :model-value="responseType" readonly class="app-dialog-input" />
          </div>
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.readCommand.value')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input :model-value="responseValue" readonly class="app-dialog-input font-mono" />
          </div>
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.readCommand.quality')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input :model-value="responseQuality" readonly class="app-dialog-input" />
          </div>
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.readCommand.cause')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input model-value="COT=5" readonly class="app-dialog-input font-mono" />
          </div>
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.readCommand.updatedAt')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input :model-value="responseTimestamp" readonly class="app-dialog-input" />
          </div>
        </div>
      </section>
    </div>
    <template #footer
      ><div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" :disabled="busy" @click="visible = false">{{
            status === 'idle' ? t('common.cancel') : t('common.close')
          }}</el-button>
          <el-button
            type="primary"
            class="app-btn-primary"
            :loading="busy"
            @click="$emit('submit')"
            >{{ submitLabel }}</el-button
          >
        </div>
      </div></template
    >
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'

defineProps<{
  badges: string[]
  busy: boolean
  status: 'idle' | 'pending' | 'success' | 'error'
  message: string
  pointName: string
  responseType: string
  responseValue: string
  responseQuality: string
  responseTimestamp: string
  submitLabel: string
}>()
defineEmits<{ 'ioa-change': []; submit: [] }>()
const visible = defineModel<boolean>('visible', { required: true })
const ioa = defineModel<number>('ioa', { required: true })
</script>

<style scoped>
.command-result-banner {
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: 6px;
  border: 1px solid transparent;
  font-size: 13px;
}

.command-result-banner.is-success {
  color: #1f7a3f;
  background: #f0f9f3;
  border-color: #b7e4c7;
}

.command-result-banner.is-error {
  color: #a63a3a;
  background: #fff4f4;
  border-color: #f3c2c2;
}

.command-result-banner.is-pending {
  color: #315f9b;
  background: #f0f6ff;
  border-color: #bfd5f5;
}

.read-command-response-grid {
  margin-top: 12px;
}
</style>
