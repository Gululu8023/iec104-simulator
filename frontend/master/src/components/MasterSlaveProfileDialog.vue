<template>
  <el-dialog
    v-model="visible"
    width="520px"
    :before-close="beforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="profile-dialog app-dialog-shell app-master-config-dialog"
  >
    <template #header
      ><div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">
            {{
              mode === 'create'
                ? t('master.appDialogs.profile.createTitle')
                : t('master.appDialogs.profile.editTitle')
            }}
          </div>
          <div v-if="badges.length" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in badges"
              :key="`profile-dialog-badge-${index}`"
              class="app-dialog-subtitle-badge"
              :class="{ 'app-dialog-subtitle-badge--action': index <= 1 }"
              :title="index <= 1 ? t('master.appDialogs.copyBadgeTitle') : undefined"
              @click="$emit('badge-click', index, badge)"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div></template
    >
    <div class="app-dialog-body conn-flat-body" @keyup.enter="$emit('enter', $event)">
      <el-form
        :model="form"
        label-position="top"
        class="profile-form app-dialog-form app-filled-form"
        ><div class="conn-section">
          <div class="conn-section-title">{{ t('master.appDialogs.profile.basicParams') }}</div>
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.profile.slaveName')
            }}</label>
            <div class="app-dialog-field-control">
              <el-input
                v-model="form.name"
                :placeholder="t('master.appDialogs.profile.slaveNamePlaceholder')"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.profile.commonAddress')
            }}</label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.common_address"
                :min="1"
                :max="65535"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
          </div></div
      ></el-form>
    </div>
    <template #footer
      ><div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="$emit('close')">{{
            t('common.cancel')
          }}</el-button>
          <el-button class="app-btn-primary" type="primary" @click="$emit('save')">{{
            t('common.save')
          }}</el-button>
        </div>
      </div></template
    >
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'

defineProps<{
  mode: 'create' | 'edit'
  badges: string[]
  form: { name: string; common_address: number }
  beforeClose: (done: () => void) => void
}>()
defineEmits<{
  close: []
  save: []
  enter: [event: KeyboardEvent]
  'badge-click': [index: number, badge: string]
}>()
const visible = defineModel<boolean>('visible', { required: true })
</script>
