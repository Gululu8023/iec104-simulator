<template>
  <el-dialog
    :model-value="visible"
    @update:model-value="handleVisibleUpdate"
    width="520px"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    :before-close="handleBeforeClose"
    class="app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div class="app-dialog-title">{{ t('globalSettings.masterTitle') }}</div>
      </div>
    </template>

    <div class="app-dialog-body app-dialog-scroll-shadow conn-flat-body">
      <section class="conn-section">
        <div class="conn-section-title">{{ t('globalSettings.resetProcessDefaultAction') }}</div>
        <el-form label-width="0" class="app-dialog-form">
          <div class="app-dialog-field-grid--1 app-dialog-radio-field">
            <div class="app-dialog-field-control app-dialog-field--full">
              <el-radio-group v-model="localResetProcessMode" class="app-dialog-radio-group">
                <el-radio label="general-reset">{{ t('globalSettings.generalReset') }}</el-radio>
                <el-radio label="clear-event-buffer">{{
                  t('globalSettings.clearEventBuffer')
                }}</el-radio>
              </el-radio-group>
              <div class="app-dialog-field-hint">
                {{ resetProcessHint }}
              </div>
            </div>
          </div>
        </el-form>
      </section>

      <section class="conn-section">
        <div class="conn-section-title">{{ t('globalSettings.ioaDisplayFormat') }}</div>
        <el-form label-width="0" class="app-dialog-form">
          <div class="app-dialog-field-grid--1 app-dialog-radio-field">
            <div class="app-dialog-field-control app-dialog-field--full">
              <el-radio-group v-model="localFormat" class="app-dialog-radio-group">
                <el-radio label="dec">{{ t('globalSettings.decimal') }}</el-radio>
                <el-radio label="hex">{{ t('globalSettings.hex') }}</el-radio>
              </el-radio-group>
            </div>
          </div>
        </el-form>
      </section>

      <section class="conn-section">
        <div class="conn-section-title">{{ t('globalSettings.messageParseView') }}</div>
        <el-form label-width="0" class="app-dialog-form">
          <div class="app-dialog-field-grid--1 app-dialog-radio-field">
            <div class="app-dialog-field-control app-dialog-field--full">
              <el-radio-group v-model="localMessageDetailViewMode" class="app-dialog-radio-group">
                <el-radio label="table">{{ t('globalSettings.tableCompatible') }}</el-radio>
                <el-radio label="tree">{{ t('globalSettings.treeWireshark') }}</el-radio>
              </el-radio-group>
            </div>
          </div>
        </el-form>
      </section>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="requestClose">{{
            t('common.cancel')
          }}</el-button>
          <el-button type="primary" @click="handleSave">{{ t('common.save') }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type {
  MessageDetailParseViewMode,
  ResetProcessMode,
  SlaveIoaDisplayFormat,
} from '@shared/api/types'
import { t } from '@shared/i18n'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'

const props = defineProps<{
  visible: boolean
  format: SlaveIoaDisplayFormat
  resetProcessMode: ResetProcessMode
  messageDetailViewMode: MessageDetailParseViewMode
}>()

const emit = defineEmits<{
  'update:visible': [visible: boolean]
  save: [
    payload: {
      format: SlaveIoaDisplayFormat
      resetProcessMode: ResetProcessMode
      messageDetailViewMode: MessageDetailParseViewMode
    },
  ]
}>()

const handleVisibleUpdate = (value: boolean) => emit('update:visible', value)

const localFormat = ref<SlaveIoaDisplayFormat>(props.format)
const localResetProcessMode = ref<ResetProcessMode>(props.resetProcessMode)
const localMessageDetailViewMode = ref<MessageDetailParseViewMode>(props.messageDetailViewMode)
const hasUnsavedChanges = computed(
  () =>
    localFormat.value !== props.format ||
    localResetProcessMode.value !== props.resetProcessMode ||
    localMessageDetailViewMode.value !== props.messageDetailViewMode,
)

const resetProcessHint = computed(() =>
  localResetProcessMode.value === 'clear-event-buffer'
    ? t('globalSettings.resetProcessClearHint')
    : t('globalSettings.resetProcessGeneralHint'),
)

watch(
  () => props.format,
  (value) => {
    localFormat.value = value
  },
)

watch(
  () => props.resetProcessMode,
  (value) => {
    localResetProcessMode.value = value
  },
)

watch(
  () => props.visible,
  (value) => {
    if (value) {
      localFormat.value = props.format
      localResetProcessMode.value = props.resetProcessMode
      localMessageDetailViewMode.value = props.messageDetailViewMode
    }
  },
)

watch(
  () => props.messageDetailViewMode,
  (value) => {
    localMessageDetailViewMode.value = value
  },
)

const { requestClose, handleBeforeClose } = useDialogCloseGuard({
  isDirty: () => hasUnsavedChanges.value,
  onClose: () => emit('update:visible', false),
})

const handleSave = () => {
  emit('save', {
    format: localFormat.value,
    resetProcessMode: localResetProcessMode.value,
    messageDetailViewMode: localMessageDetailViewMode.value,
  })
}
</script>

<style scoped>
.app-dialog-radio-row {
  justify-content: flex-start;
}

.app-dialog-radio-desc {
  margin-left: 0;
}
</style>
