<template>
  <el-dialog
    v-model="visible"
    width="520px"
    :close-on-click-modal="false"
    :lock-scroll="false"
    class="app-dialog-shell"
  >
    <template #header
      ><div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('master.appDialogs.bitString.title') }}</div>
          <div v-if="badges.length" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in badges"
              :key="`bit-string-command-badge-${index}`"
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
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.bitString.value')
            }}</label>
            <div class="app-dialog-field-control">
              <el-input
                v-model="input"
                size="small"
                class="app-dialog-input conn-inline-input font-mono"
                @blur="$emit('normalize')"
              />
            </div>
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.bitString.decimal')
            }}</label>
            <div class="app-dialog-field-control">
              <el-input
                :model-value="decimal == null ? '—' : String(decimal)"
                readonly
                size="small"
                class="app-dialog-input conn-inline-input font-mono"
              />
            </div></div
        ></el-form>
      </section>
    </div>
    <template #footer
      ><div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="visible = false">{{
            t('common.cancel')
          }}</el-button>
          <el-button
            type="primary"
            class="app-btn-primary"
            :loading="busy"
            @click="$emit('submit')"
            >{{ t('common.ok') }}</el-button
          >
        </div>
      </div></template
    >
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'

defineProps<{ badges: string[]; decimal: number | null; busy: boolean }>()
defineEmits<{ normalize: []; submit: [] }>()
const visible = defineModel<boolean>('visible', { required: true })
const ioa = defineModel<number>('ioa', { required: true })
const input = defineModel<string>('input', { required: true })
</script>
