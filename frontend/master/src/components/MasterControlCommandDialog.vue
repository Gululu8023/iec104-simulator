<template>
  <el-dialog
    v-model="visible"
    width="580px"
    :before-close="beforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="!controlFlowActive"
    :show-close="!controlFlowActive"
    :lock-scroll="false"
    class="control-confirm-dialog app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">
            {{
              t(
                isSetpointType
                  ? 'master.appDialogs.control.setpointTitle'
                  : 'master.appDialogs.control.title',
              )
            }}
          </div>
          <div class="app-dialog-subtitle-badges" v-if="controlConfirmSubtitleBadges.length > 0">
            <span
              v-for="(badge, index) in controlConfirmSubtitleBadges"
              :key="`control-confirm-badge-${index}`"
              class="app-dialog-subtitle-badge"
            >
              {{ badge }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body app-dialog-scroll-shadow conn-flat-body">
      <!-- ═══ Section 1: 命令参数 ═══ -->
      <div class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.control.commandParams') }}</div>

        <div class="app-dialog-field-grid--2">
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.control.commandType')
          }}</label>
          <div class="app-dialog-field-control">
            <el-select
              v-model="currentAsduType"
              size="small"
              class="app-dialog-input conn-inline-input"
              popper-class="app-select-dropdown"
              fit-input-width
              style="width: 100%"
            >
              <el-option
                v-for="opt in asduTypeOptions"
                :key="opt.value"
                :value="opt.value"
                :label="opt.label"
              />
            </el-select>
          </div>
          <label class="app-dialog-field-label">IOA</label>
          <div class="app-dialog-field-control">
            <el-input-number
              id="control-dialog-ioa-input"
              v-model="controlConfirmAddress"
              :min="0"
              :step="1"
              controls-position="right"
              size="small"
              class="app-dialog-input conn-inline-input"
            />
          </div>
          <template v-if="controlConfirmState.command">
            <!-- 左侧：控制值 -->
            <template v-if="isDiscreteCommand">
              <label class="app-dialog-field-label">{{
                t('master.appDialogs.control.controlValue')
              }}</label>
              <div class="app-dialog-field-control">
                <!-- 离散开关：单点/双点 -->
                <el-select
                  v-model="controlConfirmState.params.value"
                  size="small"
                  class="app-dialog-input conn-inline-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                >
                  <el-option
                    :value="isDoubleCommand ? 1 : 0"
                    :label="
                      isDoubleCommand
                        ? t('master.appDialogs.control.values.doubleOff')
                        : t('master.appDialogs.control.values.singleOff')
                    "
                  />
                  <el-option
                    :value="isDoubleCommand ? 2 : 1"
                    :label="
                      isDoubleCommand
                        ? t('master.appDialogs.control.values.doubleOn')
                        : t('master.appDialogs.control.values.singleOn')
                    "
                  />
                </el-select>
              </div>
            </template>

            <!-- 步调节：降/升 -->
            <template v-else-if="isStepCommand">
              <label class="app-dialog-field-label">{{
                t('master.appDialogs.control.stepValue')
              }}</label>
              <div class="app-dialog-field-control">
                <el-select
                  v-model="controlConfirmState.params.value"
                  size="small"
                  class="app-dialog-input conn-inline-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                >
                  <el-option :value="1" :label="t('master.appDialogs.control.values.lower')" />
                  <el-option :value="2" :label="t('master.appDialogs.control.values.raise')" />
                </el-select>
              </div>
            </template>

            <!-- 设定值 -->
            <template v-else>
              <label class="app-dialog-field-label">{{
                t('master.appDialogs.control.setpointValue')
              }}</label>
              <div class="app-dialog-field-control">
                <el-input-number
                  id="control-dialog-setpoint-input"
                  v-model="controlConfirmState.params.value"
                  :step="setpointStep"
                  :precision="setpointPrecision"
                  :min="setpointMin"
                  :max="setpointMax"
                  controls-position="right"
                  size="small"
                  class="app-dialog-input conn-inline-input"
                  style="width: 100%"
                />
              </div>
            </template>

            <label class="app-dialog-field-label">{{ qualifierFieldLabel }}</label>
            <div class="app-dialog-field-control control-qualifier-field">
              <el-select
                v-if="!isSetpointType && controlQualifierMode === 'standard'"
                id="control-dialog-qualifier-select"
                v-model="controlQualifier"
                size="small"
                class="app-dialog-input conn-inline-input"
                popper-class="app-select-dropdown"
                fit-input-width
              >
                <el-option
                  v-for="opt in qualifierOptions"
                  :key="opt.value"
                  :value="opt.value"
                  :label="opt.label"
                />
              </el-select>
              <el-input-number
                v-else
                id="control-dialog-qualifier-input"
                v-model="controlQualifier"
                :min="0"
                :max="isSetpointType ? 127 : 31"
                :step="1"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
                style="width: 100%"
              />
            </div>
            <template v-if="!isSetpointType">
              <label class="app-dialog-field-label">{{
                t('master.appDialogs.control.qualifierMode')
              }}</label>
              <div class="app-dialog-field-control control-qualifier-mode-field">
                <el-radio-group v-model="controlQualifierMode">
                  <el-radio value="standard">{{
                    t('master.appDialogs.control.qualifierStandard')
                  }}</el-radio>
                  <el-radio value="raw_test">{{
                    t('master.appDialogs.control.qualifierRawTest')
                  }}</el-radio>
                </el-radio-group>
                <p
                  v-if="controlQualifierMode === 'raw_test' && controlQualifier > 3"
                  class="app-dialog-field-hint"
                >
                  {{ t('master.appDialogs.control.reservedQuWarning', { qu: controlQualifier }) }}
                </p>
              </div>
            </template>
          </template>
        </div>
      </div>

      <!-- ═══ Section 2: 执行控制 ═══ -->
      <div class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.control.executeControl') }}</div>
        <div class="app-dialog-field-grid--1 app-dialog-radio-field">
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.control.dispatchMode')
          }}</label>
          <div class="app-dialog-field-control">
            <el-radio-group
              id="control-dialog-mode"
              v-model="controlDispatchMode"
              class="app-dialog-radio-group"
            >
              <el-radio id="control-dialog-mode-auto" value="auto-sbo">{{
                t('master.appDialogs.control.autoSbo')
              }}</el-radio>
              <el-radio id="control-dialog-mode-manual" value="manual">{{
                t('master.appDialogs.control.manualStep')
              }}</el-radio>
              <el-radio id="control-dialog-mode-direct" value="direct">{{
                t('master.appDialogs.control.directExecute')
              }}</el-radio>
            </el-radio-group>
            <p v-if="controlDispatchMode === 'direct'" class="app-dialog-field-hint">
              {{ t('master.appDialogs.control.directExecuteHint') }}
            </p>
          </div>
        </div>
        <div v-if="controlDispatchMode !== 'direct'" class="control-step-section">
          <div class="control-sbo-steps-horizontal">
            <!-- Step 1: Select -->
            <div class="sbo-h-step" :class="controlStepClass('select')">
              <div class="sbo-h-indicator">
                <span class="sbo-h-circle">{{ controlStepCircleLabel('select', '1') }}</span>
                <div
                  class="sbo-h-line"
                  :class="{ active: isControlStepLineActive('select') }"
                ></div>
              </div>
              <div class="sbo-h-content">
                <span class="sbo-h-title">{{
                  t('master.appDialogs.control.steps.selectTitle')
                }}</span>
                <span class="sbo-h-desc">{{
                  t('master.appDialogs.control.steps.selectDesc')
                }}</span>
              </div>
            </div>
            <!-- Step 2: Confirm -->
            <div class="sbo-h-step" :class="controlStepClass('confirm')">
              <div class="sbo-h-indicator">
                <span class="sbo-h-circle">{{ controlStepCircleLabel('confirm', '2') }}</span>
                <div
                  class="sbo-h-line"
                  :class="{ active: isControlStepLineActive('confirm') }"
                ></div>
              </div>
              <div class="sbo-h-content">
                <span class="sbo-h-title">{{
                  t('master.appDialogs.control.steps.confirmTitle')
                }}</span>
                <span class="sbo-h-desc">{{
                  t('master.appDialogs.control.steps.confirmDesc')
                }}</span>
              </div>
            </div>
            <!-- Step 3: Execute -->
            <div class="sbo-h-step" :class="controlStepClass('execute')">
              <div class="sbo-h-indicator">
                <span class="sbo-h-circle">{{ controlStepCircleLabel('execute', '3') }}</span>
                <div
                  class="sbo-h-line"
                  :class="{ active: isControlStepLineActive('execute') }"
                ></div>
              </div>
              <div class="sbo-h-content">
                <span class="sbo-h-title">{{
                  t('master.appDialogs.control.steps.executeTitle')
                }}</span>
                <span class="sbo-h-desc">{{
                  t('master.appDialogs.control.steps.executeDesc')
                }}</span>
              </div>
            </div>
            <!-- Step 4: Finish -->
            <div class="sbo-h-step" :class="controlStepClass('finish')">
              <div class="sbo-h-indicator">
                <span class="sbo-h-circle">{{ controlStepCircleLabel('finish', '4') }}</span>
              </div>
              <div class="sbo-h-content">
                <span class="sbo-h-title">{{
                  t('master.appDialogs.control.steps.finishTitle')
                }}</span>
                <span class="sbo-h-desc">{{
                  t('master.appDialogs.control.steps.finishDesc')
                }}</span>
              </div>
            </div>
          </div>
        </div>
        <div
          v-if="controlExecuteResult.status"
          class="command-result-banner"
          :class="controlExecuteResult.status === 'success' ? 'is-success' : 'is-error'"
        >
          {{ controlExecuteResult.message }}
        </div>
      </div>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button
            id="control-dialog-btn-cancel"
            class="app-btn-secondary"
            @click="emit('cancel')"
          >
            {{ controlCancelButtonText }}
          </el-button>
          <el-button
            id="control-dialog-btn-submit"
            type="primary"
            class="app-btn-primary"
            @click="emit('submit')"
          >
            {{ controlConfirmButtonText }}
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'
import type { ControlCommandType } from '../types/master'

type ControlProgressStep = 'select' | 'confirm' | 'execute' | 'finish'

defineProps<{
  beforeClose: (done: () => void) => void
  controlFlowActive: boolean
  isSetpointType: boolean
  controlConfirmSubtitleBadges: string[]
  asduTypeOptions: Array<{ value: string; label: string }>
  controlConfirmState: {
    step: 'idle' | ControlProgressStep
    command: ControlCommandType | null
    params: any
  }
  isDiscreteCommand: boolean
  isDoubleCommand: boolean
  isStepCommand: boolean
  setpointStep: number
  setpointPrecision: number
  setpointMin: number
  setpointMax: number
  qualifierFieldLabel: string
  qualifierOptions: Array<{ value: number; label: string }>
  controlStepClass: (step: ControlProgressStep) => Record<string, boolean>
  controlStepCircleLabel: (step: ControlProgressStep, fallback: string) => string
  isControlStepLineActive: (step: Exclude<ControlProgressStep, 'finish'>) => boolean
  controlExecuteResult: { status: 'success' | 'error' | null; message: string }
  controlCancelButtonText: string
  controlConfirmButtonText: string
}>()

const emit = defineEmits<{ cancel: []; submit: [] }>()
const visible = defineModel<boolean>('visible', { required: true })
const currentAsduType = defineModel<string>('asduType', { required: true })
const controlConfirmAddress = defineModel<number>('address', { required: true })
const controlQualifier = defineModel<number>('qualifier', { required: true })
const controlQualifierMode = defineModel<'standard' | 'raw_test'>('qualifierMode', {
  required: true,
})
const controlDispatchMode = defineModel<'auto-sbo' | 'manual' | 'direct'>('dispatchMode', {
  required: true,
})
</script>

<style scoped>
.control-step-section {
  margin-top: 16px;
}

.control-sbo-steps-horizontal {
  display: flex;
  justify-content: space-between;
  width: 100%;
  padding-top: 8px;
}

.sbo-h-step {
  display: flex;
  flex-direction: column;
  align-items: center;
  position: relative;
  flex: 1;
  opacity: 0.35;
  transition: opacity 0.4s ease;
}

.sbo-h-step.active,
.sbo-h-step.done,
.sbo-h-step.error {
  opacity: 1;
}

.sbo-h-indicator {
  display: flex;
  align-items: center;
  position: relative;
  width: 100%;
  justify-content: center;
  margin-bottom: 8px;
}

.sbo-h-circle {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  background: var(--el-fill-color-light, #f5f7fa);
  color: var(--el-text-color-secondary, #909399);
  border: 2px solid var(--el-border-color-light, #e4e7ed);
  transition: all 0.35s ease;
  flex-shrink: 0;
  z-index: 2;
  position: relative;
}

.sbo-h-step.active .sbo-h-circle {
  background: var(--primary-color, #2563eb);
  color: #fff;
  border-color: var(--primary-color, #2563eb);
  animation: sbo-pulse 2s ease-in-out infinite;
}

.sbo-h-step.done .sbo-h-circle {
  background: var(--success-color, #10b981);
  color: #fff;
  border-color: var(--success-color, #10b981);
  animation: none;
}

.sbo-h-step.error .sbo-h-circle {
  background: var(--danger-color, #ef4444);
  color: #fff;
  border-color: var(--danger-color, #ef4444);
  animation: none;
}

@keyframes sbo-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 0 rgba(37, 99, 235, 0.4);
  }
  50% {
    box-shadow: 0 0 0 6px rgba(37, 99, 235, 0);
  }
}

.sbo-h-line {
  position: absolute;
  top: 50%;
  left: calc(50% + 18px);
  right: calc(-50% + 18px);
  height: 2px;
  transform: translateY(-50%);
  background: var(--el-border-color-light, #e4e7ed);
  transition: background 0.4s ease;
  z-index: 1;
}

.sbo-h-line.active {
  background: var(--primary-color, #2563eb);
}

.sbo-h-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.sbo-h-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-secondary, #909399);
  line-height: 1.4;
  transition: color 0.3s;
}

.sbo-h-step.active .sbo-h-title,
.sbo-h-step.done .sbo-h-title,
.sbo-h-step.error .sbo-h-title {
  color: var(--el-text-color-primary, #303133);
  font-weight: 600;
}

.sbo-h-desc {
  font-size: 11px;
  color: var(--el-text-color-placeholder, #a8abb2);
  line-height: 1.4;
  margin-top: 2px;
  font-family: var(--font-mono, monospace);
}

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

.app-dialog-input.el-select,
.control-qualifier-field .app-dialog-input,
.control-qualifier-field :deep(.el-input-number),
.control-qualifier-field :deep(.el-input-number .el-input__wrapper),
.control-qualifier-field :deep(.el-select),
.control-qualifier-field :deep(.el-select__wrapper) {
  min-width: 0;
  width: 100%;
}

.control-qualifier-mode-field {
  grid-column: span 3;
}
</style>
