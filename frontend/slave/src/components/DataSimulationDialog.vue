<template>
  <el-dialog
    v-model="visible"
    width="560px"
    :before-close="handleBeforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="closeEnabled"
    :show-close="closeEnabled"
    :lock-scroll="false"
    class="simulation-dialog app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('slave.dataSimulation.title') }}</div>
          <div class="app-dialog-subtitle-badges" v-if="subtitleBadges && subtitleBadges.length">
            <span
              v-for="(badge, index) in subtitleBadges"
              :key="index"
              class="app-dialog-subtitle-badge"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div>
    </template>

    <div
      class="app-dialog-body app-dialog-scroll-shadow conn-flat-body"
      @keyup.enter="handleDialogEnter"
    >
      <el-alert
        v-if="simulationAvailabilityError"
        :title="simulationAvailabilityError"
        type="warning"
        :closable="false"
        show-icon
        class="simulation-availability-alert"
      />

      <section class="preview-section conn-section">
        <div class="conn-section-title-bar">
          <div class="conn-section-title-heading">
            <div class="conn-section-title">{{ t('slave.dataSimulation.previewTitle') }}</div>
            <span class="conn-section-meta-pill">
              {{
                t('slave.dataSimulation.selectedInfo', {
                  count: selectedPoints.length,
                  category: pointCategoryLabel,
                })
              }}
            </span>
            <span class="conn-section-meta-pill conn-section-meta-pill--success">
              {{ t('slave.dataSimulation.unifiedTick', { seconds: effectiveGlobalTickSeconds }) }}
            </span>
          </div>
        </div>
        <div class="app-dialog-field-grid--1 simulation-mode-picker">
          <label class="app-dialog-field-label">{{ t('slave.dataSimulation.mode') }}</label>
          <div class="app-dialog-field-control simulation-mode-content">
            <el-select
              v-model="config.waveformType"
              class="app-dialog-input simulation-mode-select"
              popper-class="app-select-dropdown"
              fit-input-width
              style="width: 100%"
            >
              <el-option
                v-for="option in waveformTypeOptions"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </div>
        </div>

        <div class="mode-preview-panel">
          <div class="mode-preview-heading">
            <div class="mode-preview-name">{{ activeDefinition.label }}</div>
            <div class="mode-preview-description">{{ activeDefinition.description }}</div>
          </div>
          <svg class="mode-preview-svg" viewBox="0 0 180 60" preserveAspectRatio="none">
            <line x1="0" y1="30" x2="180" y2="30" class="preview-midline" />
            <path :d="previewPath" class="preview-path" />
          </svg>
        </div>
      </section>

      <section class="config-section conn-section">
        <div class="conn-section-title">{{ t('slave.dataSimulation.parameters') }}</div>
        <div class="app-dialog-field-grid--2 simulation-config-form">
          <template v-for="parameter in parameterDefinitions" :key="parameter.key">
            <label class="app-dialog-field-label">{{ parameter.label }}</label>
            <div class="app-dialog-field-control">
              <el-select
                v-if="parameter.options"
                :model-value="Number(config[parameter.key])"
                size="small"
                class="app-dialog-input"
                popper-class="app-select-dropdown"
                fit-input-width
                @update:model-value="updateConfigField(parameter.key, $event)"
              >
                <el-option
                  v-for="option in parameter.options"
                  :key="option.value"
                  :label="option.label"
                  :value="option.value"
                />
              </el-select>
              <el-input-number
                v-else
                :model-value="Number(config[parameter.key])"
                :min="parameter.min"
                :max="parameter.max"
                :step="parameter.step ?? 0.1"
                :precision="parameter.precision ?? 2"
                controls-position="right"
                size="small"
                @update:model-value="updateConfigField(parameter.key, $event)"
              />
            </div>
          </template>
        </div>

        <div class="simulation-help-card">
          <div class="simulation-help-title">{{ t('slave.dataSimulation.helpTitle') }}</div>
          <div class="simulation-help-summary">{{ simulationSummary }}</div>
        </div>
      </section>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" :disabled="!closeEnabled" @click="requestClose">{{
            t('common.cancel')
          }}</el-button>
          <el-button
            @click="handlePrimaryAction"
            :type="isSimulating ? 'warning' : 'primary'"
            :disabled="!isSimulating && !simulationProfile"
          >
            {{ isSimulating ? t('slave.dataSimulation.stop') : t('slave.dataSimulation.start') }}
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'
import { t } from '@shared/i18n'
import {
  WAVEFORM_DEFINITIONS,
  buildWaveformPreviewPath,
  describeWaveformPointCategory,
  formatWaveformSimulationSummary,
  getWaveformDefinition,
  getWaveformParameterDefinitions,
  normalizeWaveformSimulationConfig,
  resolveSimulationValueProfile,
  suggestWaveformSimulationType,
  suggestWaveformSimulationConfig,
  type WaveformSimulationConfig,
  type WaveformSimulationPointLike,
} from '@/utils/waveformSimulation'

interface DataPoint extends WaveformSimulationPointLike {
  commonAddress?: number
  slaveId?: number | null
  name: string
  dataType: string
}

const props = defineProps<{
  modelValue: boolean
  selectedPoints: DataPoint[]
  subtitleBadges?: string[]
  simulationActive?: boolean
  globalSimulationIntervalMs?: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  start: [mode: string, config: WaveformSimulationConfig, points: DataPoint[]]
  stop: []
}>()

const visible = ref(props.modelValue)
const dialogSnapshot = ref('')
const actionLocked = ref(false)
const profileResolution = computed(() => resolveSimulationValueProfile(props.selectedPoints))
const simulationProfile = computed(() => profileResolution.value.profile)
const waveformTypeOptions = computed(() =>
  WAVEFORM_DEFINITIONS.filter((definition) =>
    simulationProfile.value?.allowedWaveforms.includes(definition.value),
  ).map((definition) => ({ value: definition.value, label: definition.label })),
)
const defaultWaveformType = computed(() => suggestWaveformSimulationType(props.selectedPoints))

const config = reactive<WaveformSimulationConfig>(
  suggestWaveformSimulationConfig(props.selectedPoints, defaultWaveformType.value),
)

const isSimulating = computed(() => Boolean(props.simulationActive))
const activeDefinition = computed(() => getWaveformDefinition(config.waveformType))
const parameterDefinitions = computed(() =>
  getWaveformParameterDefinitions(config.waveformType, simulationProfile.value),
)
const previewPath = computed(() => buildWaveformPreviewPath(config.waveformType))
const pointCategoryLabel = computed(() => describeWaveformPointCategory(props.selectedPoints))
const simulationSummary = computed(() =>
  formatWaveformSimulationSummary(config, effectiveGlobalTickMs.value, simulationProfile.value),
)
const simulationAvailabilityError = computed(() => {
  if (profileResolution.value.error === 'mixed') {
    return t('slave.dataSimulation.mixedValueModels')
  }
  if (profileResolution.value.error === 'unsupported') {
    return t('slave.dataSimulation.unsupportedPointType')
  }
  return ''
})
const effectiveGlobalTickMs = computed(() =>
  Math.max(100, Math.trunc(Number(props.globalSimulationIntervalMs) || 1000)),
)
const effectiveGlobalTickSeconds = computed(() =>
  Number((effectiveGlobalTickMs.value / 1000).toFixed(effectiveGlobalTickMs.value >= 1000 ? 1 : 2)),
)
const hasUnsavedChanges = computed(() => {
  if (!visible.value) return false
  if (!dialogSnapshot.value) return false
  return buildDialogSnapshot() !== dialogSnapshot.value
})

const applySuggestedConfig = (
  preferredType: WaveformSimulationConfig['waveformType'] = defaultWaveformType.value,
) => {
  Object.assign(config, suggestWaveformSimulationConfig(props.selectedPoints, preferredType))
}

watch(
  () => props.modelValue,
  (val) => {
    visible.value = val
    if (val) {
      actionLocked.value = false
      applySuggestedConfig(defaultWaveformType.value)
      dialogSnapshot.value = buildDialogSnapshot()
      return
    }
    dialogSnapshot.value = ''
  },
  { immediate: true },
)

watch(
  () => config.waveformType,
  (next, prev) => {
    if (next === prev) return
    if (next === 'fixed' && prev !== 'fixed') {
      config.fixedValue = config.waveformBase
      return
    }
    if (prev === 'fixed' && next !== 'fixed') {
      config.waveformBase = config.fixedValue
    }
  },
)

watch(visible, (val) => {
  emit('update:modelValue', val)
  if (!val) {
    dialogSnapshot.value = ''
  }
})

const buildDialogSnapshot = () => {
  const normalizedConfig = normalizeWaveformSimulationConfig(config, simulationProfile.value)
  return JSON.stringify({
    ...normalizedConfig,
    selectedPoints: props.selectedPoints
      .map((point) => `${Number(point.commonAddress ?? -1)}:${Number(point.address)}`)
      .sort(),
  })
}

type NumericWaveformConfigKey = Exclude<keyof WaveformSimulationConfig, 'waveformType'>

const updateConfigField = (key: NumericWaveformConfigKey, value: number | undefined) => {
  config[key] = Number(value ?? 0)
}

const performClose = () => {
  visible.value = false
}

const { closeEnabled, handleBeforeClose, requestClose } = useDialogCloseGuard({
  isDirty: () => hasUnsavedChanges.value,
  isBlocked: () => actionLocked.value,
  onClose: performClose,
})

const handlePrimaryAction = () => {
  if (actionLocked.value) return
  if (isSimulating.value) {
    stopSimulation()
    return
  }
  startSimulation()
}

const handleDialogEnter = () => {
  handlePrimaryAction()
}

const startSimulation = () => {
  if (actionLocked.value) return
  if (props.selectedPoints.length === 0) {
    ElMessage.warning(t('slave.dataSimulation.selectPointsFirst'))
    return
  }
  if (!simulationProfile.value) {
    ElMessage.warning(simulationAvailabilityError.value)
    return
  }

  actionLocked.value = true
  const normalizedConfig = normalizeWaveformSimulationConfig(config, simulationProfile.value)
  Object.assign(config, normalizedConfig)
  visible.value = false
  emit(
    'start',
    'waveform',
    normalizedConfig,
    props.selectedPoints.map((point) => ({ ...point })),
  )
}

const stopSimulation = () => {
  if (actionLocked.value) return
  actionLocked.value = true
  visible.value = false
  emit('stop')
}
</script>

<style scoped>
.simulation-availability-alert {
  margin-bottom: 16px;
}

.simulation-mode-picker {
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.simulation-mode-content {
  flex: 1;
  min-width: 0;
  align-items: center;
}

.simulation-mode-content > * {
  min-width: 0;
}

.simulation-mode-select {
  width: 100%;
}

.mode-preview-panel {
  margin-top: 12px;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  padding: 12px;
}

.mode-preview-heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}

.mode-preview-name {
  font-size: 14px;
  font-weight: 600;
  color: #0f172a;
}

.mode-preview-description {
  font-size: 12px;
  line-height: 1.5;
  color: #64748b;
  text-align: right;
  white-space: nowrap;
  flex-shrink: 0;
}

.mode-preview-svg {
  width: 100%;
  height: 76px;
  border-radius: 10px;
  border: 1px solid #e5e7eb;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
}

.preview-midline {
  stroke: #cbd5e1;
  stroke-width: 1;
  stroke-dasharray: 3 3;
}

.preview-path {
  fill: none;
  stroke: #2563eb;
  stroke-width: 2;
}

.simulation-help-card {
  margin-top: 12px;
  padding: 12px;
  border-radius: 10px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
}

.simulation-help-title {
  font-size: 12px;
  font-weight: 600;
  color: #334155;
}

.simulation-help-summary {
  margin-top: 6px;
  color: #0f172a;
  line-height: 1.6;
}

.simulation-config-form :deep(.el-select__selected-item) {
  text-align: center;
}

@media (max-width: 720px) {
  .simulation-mode-picker,
  .mode-preview-heading {
    flex-direction: column;
    align-items: stretch;
  }

  .simulation-mode-content {
    grid-template-columns: 1fr;
  }

  .mode-preview-description {
    white-space: normal;
    text-align: left;
  }
}
</style>
