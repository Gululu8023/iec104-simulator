<template>
  <el-dialog
    v-model="visible"
    width="600px"
    :before-close="beforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="point-edit-dialog app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('slave.contentPanel.editDialog.title') }}</div>
          <div class="app-dialog-subtitle-badges" v-if="editingDataPoint">
            <span class="app-dialog-subtitle-badge" v-if="currentTab?.connectionName">{{
              currentTab.connectionName
            }}</span>
            <span class="app-dialog-subtitle-badge" v-if="currentTab?.slaveName">{{
              currentTab.slaveName
            }}</span>
            <span class="app-dialog-subtitle-badge" v-if="currentTab">{{
              formatTypeCompact(currentTab.dataType)
            }}</span>
          </div>
        </div>
      </div>
    </template>

    <div
      class="app-dialog-body app-dialog-scroll-shadow conn-flat-body"
      @keyup.enter="emit('enter', $event)"
    >
      <el-form
        v-if="editingDataPoint"
        :model="editingDataPoint"
        class="app-dialog-form point-edit-form"
      >
        <section class="conn-section">
          <div class="conn-section-title">{{ t('slave.contentPanel.editDialog.basic') }}</div>
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label" for="slave-point-edit-address">{{
              t('slave.contentPanel.editDialog.address')
            }}</label>
            <el-form-item label-width="0" class="app-dialog-field-control">
              <el-input v-model="editingDataPoint.address" disabled class="app-dialog-input" />
            </el-form-item>

            <template v-if="!isEditingControlPoint">
              <label class="app-dialog-field-label">{{
                t('slave.contentPanel.editDialog.currentValue')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control">
                <el-select
                  v-if="isEditingSinglePoint"
                  v-model="editingDataPoint.value"
                  class="app-dialog-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                >
                  <el-option
                    :label="t('slave.contentPanel.editDialog.values.singleOff')"
                    :value="0"
                  />
                  <el-option
                    :label="t('slave.contentPanel.editDialog.values.singleOn')"
                    :value="1"
                  />
                </el-select>
                <el-select
                  v-else-if="isEditingDoublePoint"
                  v-model="editingDataPoint.value"
                  class="app-dialog-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                >
                  <el-option
                    :label="t('slave.contentPanel.editDialog.values.doubleIntermediate')"
                    :value="0"
                  />
                  <el-option
                    :label="t('slave.contentPanel.editDialog.values.doubleOff')"
                    :value="1"
                  />
                  <el-option
                    :label="t('slave.contentPanel.editDialog.values.doubleOn')"
                    :value="2"
                  />
                  <el-option
                    :label="t('slave.contentPanel.editDialog.values.doubleInvalid')"
                    :value="3"
                  />
                </el-select>
                <el-input-number
                  v-else-if="editingValueProfile"
                  v-model="editingDataPoint.value"
                  :min="editingValueProfile.min"
                  :max="editingValueProfile.max"
                  :step="editingValueProfile.step"
                  :precision="editingValueProfile.precision"
                  controls-position="right"
                  class="app-dialog-input"
                />
              </el-form-item>
            </template>
            <label class="app-dialog-field-label">{{
              t('slave.contentPanel.editDialog.name')
            }}</label>
            <el-form-item label-width="0" class="app-dialog-field-control">
              <el-input v-model="editingDataPoint.name" class="app-dialog-input" />
            </el-form-item>
            <template v-if="!isEditingControlPoint">
              <label class="app-dialog-field-label">{{
                t('slave.contentPanel.editDialog.giGroup')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control">
                <el-select
                  v-model="editingGiGroupValue"
                  class="app-dialog-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                >
                  <el-option :label="t('slave.contentPanel.giGroup.stationOnly')" :value="0" />
                  <el-option
                    v-for="group in 16"
                    :key="group"
                    :label="t('slave.contentPanel.giGroup.group', { group })"
                    :value="group"
                  />
                </el-select>
              </el-form-item>
            </template>
            <template v-if="isEditingIntegratedTotal">
              <label class="app-dialog-field-label">{{
                t('slave.contentPanel.editDialog.counterGroup')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control">
                <el-select
                  v-model="editingCounterGroupValue"
                  class="app-dialog-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                >
                  <el-option :label="t('slave.contentPanel.counterGroup.allOnly')" :value="0" />
                  <el-option
                    v-for="group in 4"
                    :key="group"
                    :label="t('slave.contentPanel.counterGroup.group', { group })"
                    :value="group"
                  />
                </el-select>
              </el-form-item>
            </template>
            <label class="app-dialog-field-label app-dialog-field--full">{{
              t('slave.contentPanel.editDialog.description')
            }}</label>
            <el-form-item label-width="0" class="app-dialog-field-control app-dialog-field--full">
              <el-input v-model="editingDataPoint.description" class="app-dialog-input" />
            </el-form-item>
          </div>
        </section>

        <section v-if="protocolQualityKind !== 'none'" class="conn-section">
          <div class="conn-section-title-bar--compact">
            <div class="conn-section-title">{{ protocolQualityGroupLabel }}</div>
            <el-tag :type="editingQualitySummaryTagType" effect="dark" size="small" round>{{
              editingQualitySummaryText
            }}</el-tag>
          </div>
          <div class="app-dialog-field-grid--2">
            <template v-for="flag in protocolQualityFieldDefs" :key="flag.key">
              <label class="app-dialog-field-label">{{ flag.label }}</label>
              <div class="app-dialog-field-control">
                <el-switch
                  class="app-dialog-switch-control"
                  :model-value="isProtocolFlagActive(flag)"
                  size="small"
                  @update:model-value="toggleProtocolFlag(flag, $event)"
                />
              </div>
            </template>
          </div>
        </section>
      </el-form>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="emit('close')">{{
            t('common.cancel')
          }}</el-button>
          <el-button type="primary" @click="emit('save')">{{ t('common.save') }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import type { ProtocolFlagDef, ProtocolQualityKind } from '@shared/api/protocolQuality'
import { t } from '@shared/i18n'
import type { SlaveContentDataPoint, SlaveContentTabData } from '@/types/slaveContentPanel'

defineProps<{
  beforeClose: (done: () => void) => void
  currentTab: SlaveContentTabData | null | undefined
  editingDataPoint: SlaveContentDataPoint | null
  formatTypeCompact: (dataType: string) => string
  isEditingControlPoint: boolean
  isEditingSinglePoint: boolean
  isEditingDoublePoint: boolean
  isEditingIntegratedTotal: boolean
  editingValueProfile: { min: number; max: number; step: number; precision: number } | null
  protocolQualityKind: ProtocolQualityKind
  protocolQualityGroupLabel: string
  editingQualitySummaryTagType: 'success' | 'warning' | 'info' | 'primary' | 'danger'
  editingQualitySummaryText: string
  protocolQualityFieldDefs: ProtocolFlagDef[]
  isProtocolFlagActive: (flag: ProtocolFlagDef) => boolean
  toggleProtocolFlag: (flag: ProtocolFlagDef, active: string | number | boolean) => void
}>()

const emit = defineEmits<{
  close: []
  save: []
  enter: [event: KeyboardEvent]
}>()
const visible = defineModel<boolean>('visible', { required: true })
const editingGiGroupValue = defineModel<number>('giGroup', { required: true })
const editingCounterGroupValue = defineModel<number>('counterGroup', { required: true })
</script>
