<template>
  <el-dialog
    :model-value="visible"
    @update:model-value="handleVisibleUpdate"
    width="520px"
    :close-on-click-modal="false"
    :close-on-press-escape="closeEnabled"
    :show-close="closeEnabled"
    :lock-scroll="false"
    :before-close="handleBeforeClose"
    class="app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div class="app-dialog-title">{{ t('globalSettings.slaveTitle') }}</div>
      </div>
    </template>

    <div class="app-dialog-body app-dialog-scroll-shadow conn-flat-body">
      <section class="conn-section">
        <div class="conn-section-title">{{ t('globalSettings.commandMismatchPolicy') }}</div>
        <el-form label-width="0" class="app-dialog-form">
          <div class="app-dialog-field-grid--1 app-dialog-radio-field">
            <div class="app-dialog-field-control app-dialog-field--full">
              <el-radio-group v-model="localPolicy" class="app-dialog-radio-group">
                <el-radio
                  v-for="option in POLICY_OPTIONS"
                  :key="option.value"
                  :label="option.value"
                >
                  {{ t(option.titleKey) }}
                </el-radio>
              </el-radio-group>
              <div class="app-dialog-field-hint">
                {{ activePolicyHint }}
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

      <section class="conn-section">
        <div class="conn-section-title">{{ t('globalSettings.dataChangeFrequency') }}</div>
        <el-form label-width="0" class="app-dialog-form">
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">
              {{ t('globalSettings.unifiedFrequencySeconds') }}
              <el-tooltip
                :content="t('globalSettings.simulationIntervalDescription')"
                placement="top"
              >
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="localSimulationIntervalSeconds"
                class="app-dialog-input conn-inline-input"
                controls-position="right"
                size="small"
                :min="1"
                :max="60"
                :step="1"
                :precision="0"
              />
            </div>
            <label class="app-dialog-field-label">
              {{ t('globalSettings.autoBroadcast') }}
              <el-tooltip :content="t('globalSettings.autoBroadcastDescription')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-switch v-model="localSimulationAutoBroadcast" />
            </div>
          </div>
        </el-form>
      </section>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" :disabled="saving" @click="requestClose">{{
            t('common.cancel')
          }}</el-button>
          <el-button type="primary" :loading="saving" @click="handleSave">{{
            t('common.save')
          }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type {
  MessageDetailParseViewMode,
  SlaveCommandMismatchPolicy,
  SlaveIoaDisplayFormat,
} from '@shared/api/types'
import { t } from '@shared/i18n'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'

const POLICY_OPTIONS: Array<{
  value: SlaveCommandMismatchPolicy
  titleKey: string
  hintTagKey: string
  descriptionKey: string
}> = [
  {
    value: 'strict',
    titleKey: 'globalSettings.strictMode',
    hintTagKey: 'globalSettings.recommended',
    descriptionKey: 'globalSettings.strictDescription',
  },
  {
    value: 'compatible',
    titleKey: 'globalSettings.compatibleMode',
    hintTagKey: 'globalSettings.integration',
    descriptionKey: 'globalSettings.compatibleDescription',
  },
  {
    value: 'debug',
    titleKey: 'globalSettings.debugMode',
    hintTagKey: 'globalSettings.test',
    descriptionKey: 'globalSettings.debugDescription',
  },
]

const props = defineProps<{
  visible: boolean
  policy: SlaveCommandMismatchPolicy
  format: SlaveIoaDisplayFormat
  messageDetailViewMode: MessageDetailParseViewMode
  simulationIntervalMs: number
  simulationAutoBroadcast: boolean
  saving?: boolean
}>()

const emit = defineEmits<{
  'update:visible': [visible: boolean]
  save: [
    payload: {
      policy: SlaveCommandMismatchPolicy
      format: SlaveIoaDisplayFormat
      messageDetailViewMode: MessageDetailParseViewMode
      simulationIntervalMs: number
      simulationAutoBroadcast: boolean
    },
  ]
}>()

const handleVisibleUpdate = (value: boolean) => emit('update:visible', value)

const localPolicy = ref<SlaveCommandMismatchPolicy>(props.policy)
const localFormat = ref<SlaveIoaDisplayFormat>(props.format)
const localMessageDetailViewMode = ref<MessageDetailParseViewMode>(props.messageDetailViewMode)
const localSimulationIntervalSeconds = ref<number>(
  Math.max(1, Math.round(props.simulationIntervalMs / 1000)),
)
const localSimulationAutoBroadcast = ref(props.simulationAutoBroadcast)
const hasUnsavedChanges = computed(
  () =>
    localPolicy.value !== props.policy ||
    localFormat.value !== props.format ||
    localMessageDetailViewMode.value !== props.messageDetailViewMode ||
    localSimulationIntervalSeconds.value * 1000 !== props.simulationIntervalMs ||
    localSimulationAutoBroadcast.value !== props.simulationAutoBroadcast,
)
const activePolicyOption = computed(
  () => POLICY_OPTIONS.find((option) => option.value === localPolicy.value) ?? POLICY_OPTIONS[0],
)
const activePolicyHint = computed(() =>
  t('globalSettings.taggedHint', {
    tag: t(activePolicyOption.value.hintTagKey),
    description: t(activePolicyOption.value.descriptionKey),
  }),
)
watch(
  () => props.policy,
  (value) => {
    localPolicy.value = value
  },
)

watch(
  () => props.format,
  (value) => {
    localFormat.value = value
  },
)

watch(
  () => props.visible,
  (value) => {
    if (value) {
      localPolicy.value = props.policy
      localFormat.value = props.format
      localMessageDetailViewMode.value = props.messageDetailViewMode
      localSimulationIntervalSeconds.value = Math.max(
        1,
        Math.round(props.simulationIntervalMs / 1000),
      )
      localSimulationAutoBroadcast.value = props.simulationAutoBroadcast
    }
  },
)

watch(
  () => props.simulationIntervalMs,
  (value) => {
    localSimulationIntervalSeconds.value = Math.max(1, Math.round(value / 1000))
  },
)

watch(
  () => props.simulationAutoBroadcast,
  (value) => {
    localSimulationAutoBroadcast.value = value
  },
)

watch(
  () => props.messageDetailViewMode,
  (value) => {
    localMessageDetailViewMode.value = value
  },
)

const { closeEnabled, requestClose, handleBeforeClose } = useDialogCloseGuard({
  isDirty: () => hasUnsavedChanges.value,
  isBlocked: () => Boolean(props.saving),
  onClose: () => emit('update:visible', false),
})

const handleSave = () => {
  emit('save', {
    policy: localPolicy.value,
    format: localFormat.value,
    messageDetailViewMode: localMessageDetailViewMode.value,
    simulationIntervalMs: Math.max(
      1000,
      Math.min(60000, Math.round(Number(localSimulationIntervalSeconds.value || 1)) * 1000),
    ),
    simulationAutoBroadcast: localSimulationAutoBroadcast.value,
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
