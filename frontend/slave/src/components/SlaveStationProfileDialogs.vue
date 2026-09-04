<template>
  <!-- 从站对话框（创建/编辑） -->
  <el-dialog
    v-model="showSlaveDialog"
    width="700px"
    :before-close="handleSlaveDialogBeforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="slave-dialog app-dialog-shell"
  >
    <!-- Header -->
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">
            {{
              slaveDialogMode === 'create'
                ? t('slave.deviceTree.slaveDialog.createTitle')
                : t('slave.deviceTree.slaveDialog.editTitle')
            }}
          </div>
          <div class="app-dialog-subtitle-badges" v-if="slaveDialogSubtitleBadges.length > 0">
            <span
              v-for="(badge, index) in slaveDialogSubtitleBadges"
              :key="`slave-dialog-badge-${index}`"
              class="app-dialog-subtitle-badge"
            >
              {{ badge }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body conn-flat-body slave-dialog-body" @keyup.enter="handleSlaveConfirm">
      <div class="conn-section">
        <div class="conn-section-title">
          {{ t('slave.deviceTree.slaveDialog.slaveProperties') }}
        </div>
        <el-form
          ref="slaveBaseFormRef"
          :model="slaveForm"
          :rules="slaveFormRules"
          class="app-dialog-form conn-inline-form"
          status-icon
        >
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">{{
              t('slave.deviceTree.slaveDialog.slaveName')
            }}</label>
            <el-form-item prop="name" label-width="0" class="app-dialog-field-control">
              <el-input
                v-model="slaveForm.name"
                :placeholder="t('slave.deviceTree.slaveDialog.slaveNamePlaceholder')"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </el-form-item>
            <label class="app-dialog-field-label">{{
              t('slave.deviceTree.slaveDialog.commonAddress')
            }}</label>
            <el-form-item prop="commonAddress" label-width="0" class="app-dialog-field-control">
              <el-input-number
                v-model="slaveForm.commonAddress"
                :min="1"
                :max="65535"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </el-form-item>
          </div>
        </el-form>
      </div>

      <div class="conn-section slave-policy-section">
        <div class="conn-section-title">{{ t('slave.deviceTree.slaveDialog.behaviorPolicy') }}</div>
        <div class="app-dialog-field-grid--2 slave-policy-layout">
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.slaveDialog.sq1Upload')
          }}</label>
          <div class="app-dialog-field-control app-dialog-inline-control">
            <el-switch v-model="slaveForm.enableSq1Upload" size="small" />
            <div class="slave-policy-hint app-dialog-field-hint">
              {{ t('slave.deviceTree.slaveDialog.sq1UploadHint') }}
            </div>
          </div>
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.slaveDialog.soeUpload')
          }}</label>
          <div class="app-dialog-field-control app-dialog-inline-control">
            <el-switch v-model="slaveForm.enableSoe" size="small" />
            <div class="slave-policy-hint app-dialog-field-hint">
              {{ t('slave.deviceTree.slaveDialog.soeUploadHint') }}
            </div>
          </div>
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.slaveDialog.controlMode')
          }}</label>
          <div class="app-dialog-field-control">
            <el-select
              v-model="slaveForm.controlExecutionMode"
              size="small"
              class="app-dialog-input conn-inline-input"
              popper-class="app-select-dropdown"
              fit-input-width
            >
              <el-option :label="t('slave.deviceTree.slaveDialog.controlModeSbo')" value="sbo" />
              <el-option
                :label="t('slave.deviceTree.slaveDialog.controlModeDirect')"
                value="direct"
              />
            </el-select>
          </div>
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.slaveDialog.selectTimeout')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input-number
              v-model="slaveForm.selectTimeout"
              :min="1"
              :max="3600"
              size="small"
              controls-position="right"
              class="app-dialog-input conn-inline-input"
            />
          </div>
        </div>
      </div>

      <div class="conn-section conn-objects-section">
        <div class="conn-section-title">{{ t('slave.deviceTree.slaveDialog.objectConfig') }}</div>
        <div class="slave-obj-split">
          <!-- 左侧：对象列表 -->
          <div class="slave-obj-left">
            <div class="slave-obj-toolbar app-dialog-toolbar">
              <span class="slave-obj-toolbar-count app-dialog-toolbar-count">{{
                t('slave.deviceTree.slaveDialog.objectCount', {
                  count: editingConfigObjects.length,
                })
              }}</span>
              <div class="slave-obj-toolbar-actions app-dialog-toolbar-actions">
                <div
                  class="slave-obj-toolbar-btn-group app-dialog-btn-group"
                  role="group"
                  :aria-label="t('slave.deviceTree.slaveDialog.objectActionsAria')"
                >
                  <el-button
                    size="small"
                    :icon="Files"
                    class="slave-obj-toolbar-btn app-dialog-btn app-dialog-btn--outline"
                    @click="handleApplyDefaultTemplate"
                  >
                    {{ t('slave.deviceTree.slaveDialog.appendTemplate') }}
                  </el-button>
                  <el-button
                    size="small"
                    :icon="Plus"
                    class="slave-obj-toolbar-btn app-dialog-btn app-dialog-btn--soft-primary"
                    @click="handleAddConfigObject"
                  >
                    {{ t('slave.deviceTree.slaveDialog.add') }}
                  </el-button>
                </div>
              </div>
            </div>
            <el-scrollbar class="slave-obj-list-scroll">
              <div
                v-for="(obj, index) in editingConfigObjects"
                :key="obj.id"
                class="slave-obj-item"
                :class="{
                  'slave-obj-item--active': selectedConfigIndex === index,
                  'slave-obj-item--warn': ioaConflictMap.has(obj.id),
                }"
                @click="handleSelectConfigObject(index)"
              >
                <div class="slave-obj-item-main">
                  <el-tooltip
                    v-if="ioaConflictMap.has(obj.id)"
                    :content="ioaConflictMap.get(obj.id)"
                    placement="top"
                  >
                    <el-icon class="slave-obj-warn-icon"><WarningFilled /></el-icon>
                  </el-tooltip>
                  <span class="slave-obj-item-name">{{ obj.name }}</span>
                  <el-tag
                    size="small"
                    type="info"
                    class="slave-obj-asdu-pill"
                    disable-transitions
                    >{{ formatTypeCompact(obj.asduAddress) }}</el-tag
                  >
                </div>
                <div class="slave-obj-sub">
                  <span class="slave-obj-sub-text">
                    {{ t('slave.deviceTree.slaveDialog.ioaRangeShort') }}
                    {{
                      obj.addressMode === 'expression'
                        ? obj.ioaExpression
                        : `${obj.ioaStartAddress}~${Number(obj.ioaStartAddress) + Number(obj.ioaCount) - 1}`
                    }}
                    · {{ t('slave.deviceTree.slaveDialog.countShort') }}
                    {{ configObjectAddressCount(obj) }}</span
                  >
                  <div class="slave-obj-item-actions">
                    <el-tooltip :content="t('slave.deviceTree.slaveDialog.copy')" placement="top">
                      <el-button
                        size="small"
                        :icon="CopyDocument"
                        link
                        @click.stop="handleDuplicateByIndex(index)"
                      />
                    </el-tooltip>
                    <el-tooltip :content="t('slave.deviceTree.slaveDialog.delete')" placement="top">
                      <el-button
                        size="small"
                        :icon="Delete"
                        type="danger"
                        link
                        @click.stop="handleRemoveConfigObjectByIndex(index)"
                      />
                    </el-tooltip>
                  </div>
                </div>
              </div>
              <div v-if="editingConfigObjects.length === 0" class="slave-obj-empty">
                {{ t('slave.deviceTree.slaveDialog.emptyObjects') }}
              </div>
            </el-scrollbar>
          </div>

          <!-- 右侧：详情编辑面板 -->
          <div v-if="activeConfigObject" class="slave-obj-right">
            <div class="slave-obj-right-title">
              {{ t('slave.deviceTree.slaveDialog.objectProperties') }}
            </div>
            <el-form
              ref="configDetailFormRef"
              :model="activeConfigObject"
              :rules="configObjectRules"
              label-position="right"
              class="app-dialog-form slave-obj-form"
              status-icon
              hide-required-asterisk
            >
              <div class="app-dialog-field-grid--1">
                <label class="app-dialog-field-label">{{
                  t('slave.deviceTree.slaveDialog.name')
                }}</label>
                <el-form-item label-width="0" class="app-dialog-field-control" prop="name">
                  <el-input
                    ref="objNameInputRef"
                    v-model="activeConfigObject.name"
                    size="small"
                    class="app-dialog-input"
                  />
                </el-form-item>
                <label class="app-dialog-field-label">{{
                  t('slave.deviceTree.slaveDialog.asduType')
                }}</label>
                <el-form-item label-width="0" class="app-dialog-field-control" prop="asduAddress">
                  <el-select
                    v-model="activeConfigObject.asduAddress"
                    @change="handleActiveConfigAsduChange"
                    size="small"
                    class="app-dialog-input"
                    filterable
                    popper-class="app-select-dropdown"
                    fit-input-width
                  >
                    <el-option
                      v-for="option in asduTypeOptions"
                      :key="`asdu-${option.value}`"
                      :label="option.label"
                      :value="option.value"
                      :title="option.label"
                    />
                  </el-select>
                </el-form-item>
                <label class="app-dialog-field-label">{{
                  t('slave.deviceTree.slaveDialog.addressMode')
                }}</label>
                <el-form-item label-width="0" class="app-dialog-field-control">
                  <el-radio-group
                    class="app-form-segmented slave-address-mode-group"
                    :model-value="activeConfigObject.addressMode ?? 'continuous'"
                    @update:model-value="
                      activeConfigObject.addressMode = $event as 'continuous' | 'expression'
                    "
                  >
                    <el-radio-button value="continuous">{{
                      t('slave.deviceTree.slaveDialog.continuousAddressShort')
                    }}</el-radio-button>
                    <el-radio-button value="expression">{{
                      t('slave.deviceTree.slaveDialog.addressExpressionShort')
                    }}</el-radio-button>
                  </el-radio-group>
                </el-form-item>
                <template v-if="activeConfigObject.addressMode !== 'expression'">
                  <label class="app-dialog-field-label">{{
                    t('slave.deviceTree.slaveDialog.ioaStart')
                  }}</label>
                  <el-form-item
                    label-width="0"
                    class="app-dialog-field-control"
                    prop="ioaStartAddress"
                  >
                    <el-input-number
                      v-model="activeConfigObject.ioaStartAddress"
                      :min="1"
                      :max="16777215"
                      size="small"
                      controls-position="right"
                      class="app-dialog-input"
                    />
                  </el-form-item>
                  <label class="app-dialog-field-label">{{
                    t('slave.deviceTree.slaveDialog.ioaCount')
                  }}</label>
                  <el-form-item label-width="0" class="app-dialog-field-control" prop="ioaCount">
                    <el-input-number
                      v-model="activeConfigObject.ioaCount"
                      :min="1"
                      :max="1000"
                      size="small"
                      controls-position="right"
                      class="app-dialog-input"
                    />
                  </el-form-item>
                </template>
                <template v-else>
                  <label class="app-dialog-field-label">{{
                    t('slave.deviceTree.slaveDialog.addressExpression')
                  }}</label>
                  <el-form-item
                    label-width="0"
                    class="app-dialog-field-control"
                    prop="ioaExpression"
                  >
                    <el-input
                      v-model="activeConfigObject.ioaExpression"
                      :placeholder="t('slave.deviceTree.slaveDialog.addressExpressionPlaceholder')"
                      size="small"
                      class="app-dialog-input"
                    />
                  </el-form-item>
                </template>
                <label class="app-dialog-field-label">{{
                  t('slave.deviceTree.slaveDialog.nameTemplate')
                }}</label>
                <el-form-item label-width="0" class="app-dialog-field-control" prop="nameTemplate">
                  <div class="slave-name-template-control">
                    <el-select
                      v-model="activeConfigObject.nameTemplate"
                      size="small"
                      class="app-dialog-input"
                      popper-class="app-select-dropdown"
                      fit-input-width
                    >
                      <el-option
                        :label="t('slave.deviceTree.slaveDialog.nameTemplates.baseTypeIoa')"
                        value="{base}_{type}_{ioa}"
                      />
                      <el-option
                        :label="t('slave.deviceTree.slaveDialog.nameTemplates.baseIoa')"
                        value="{base}_{ioa}"
                      />
                      <el-option
                        :label="t('slave.deviceTree.slaveDialog.nameTemplates.typeIoa')"
                        value="{type}_{ioa}"
                      />
                    </el-select>
                    <p v-if="activeConfigNameSample" class="app-dialog-field-hint">
                      {{
                        t('slave.deviceTree.slaveDialog.nameTemplateExample', {
                          sample: activeConfigNameSample,
                        })
                      }}
                    </p>
                  </div>
                </el-form-item>
              </div>
            </el-form>
          </div>
          <div v-else class="slave-obj-right slave-obj-right--empty">
            <span>{{ t('slave.deviceTree.slaveDialog.selectObject') }}</span>
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="app-dialog-footer slave-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="requestCloseSlaveDialog">{{
            t('common.cancel')
          }}</el-button>
          <el-button type="primary" @click="handleSlaveConfirm">{{ t('common.save') }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>

  <!-- 编辑连接对话框 -->
  <el-dialog
    v-model="showEditConnectionDialog"
    width="700px"
    :before-close="handleConnectionDialogBeforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="connection-dialog app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ connectionDialogTitle }}</div>
          <div class="app-dialog-subtitle-badges" v-if="connectionDialogSubtitleBadges.length > 0">
            <span
              v-for="(badge, index) in connectionDialogSubtitleBadges"
              :key="`connection-dialog-badge-${index}`"
              class="app-dialog-subtitle-badge"
              :class="{ 'app-dialog-subtitle-badge--action': index === 0 }"
              :title="
                index === 0 ? t('slave.deviceTree.connectionDialog.copyBadgeTitle') : undefined
              "
              @click="handleConnectionBadgeClick(index, badge)"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div>
    </template>

    <div
      class="app-dialog-body app-dialog-scroll-shadow conn-flat-body"
      @keyup.enter="handleSaveConnection"
    >
      <!-- 网络参数 -->
      <div class="conn-section">
        <div class="conn-section-title">
          {{ t('slave.deviceTree.connectionDialog.networkParams') }}
        </div>
        <div class="app-dialog-field-grid--3">
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.connectionDialog.connectionName')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input
              v-model="connectionForm.name"
              :placeholder="t('slave.deviceTree.connectionDialog.connectionNamePlaceholder')"
              size="small"
              class="app-dialog-input conn-inline-input"
            />
          </div>
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.connectionDialog.listenAddress')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input
              v-model="connectionForm.listenAddress"
              placeholder="0.0.0.0"
              size="small"
              class="app-dialog-input conn-inline-input"
            />
          </div>
          <label class="app-dialog-field-label">{{
            t('slave.deviceTree.connectionDialog.listenPort')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input-number
              v-model="connectionForm.listenPort"
              :min="1"
              :max="65535"
              size="small"
              controls-position="right"
              class="app-dialog-input conn-inline-input"
            />
          </div>
        </div>
      </div>

      <!-- 连接策略 -->
      <div class="conn-section connection-strategy-section">
        <div class="conn-section-title">
          {{ t('slave.deviceTree.connectionDialog.connectionStrategy') }}
        </div>
        <div class="app-dialog-field-grid--1">
          <label class="app-dialog-field-label">
            {{ t('slave.deviceTree.connectionDialog.redundancyMode') }}
            <el-tooltip
              :content="t('slave.deviceTree.connectionDialog.redundancyModeTip')"
              placement="top"
            >
              <span class="conn-tip-icon">?</span>
            </el-tooltip>
          </label>
          <div class="app-dialog-field-control">
            <el-radio-group v-model="connectionForm.redundancyMode" class="app-dialog-radio-group">
              <el-radio label="single">{{
                t('slave.deviceTree.connectionDialog.redundancySingle')
              }}</el-radio>
              <el-radio label="connection">{{
                t('slave.deviceTree.connectionDialog.redundancyConnection')
              }}</el-radio>
              <el-radio label="multi">{{
                t('slave.deviceTree.connectionDialog.redundancyMulti')
              }}</el-radio>
            </el-radio-group>
          </div>
          <label class="app-dialog-field-label">
            ASDU Max Bytes
            <el-tooltip
              :content="t('slave.deviceTree.connectionDialog.maxAsduTip')"
              placement="top"
            >
              <span class="conn-tip-icon">?</span>
            </el-tooltip>
          </label>
          <div class="app-dialog-field-control">
            <el-input-number
              v-model="connectionForm.maxAsduBytes"
              :min="4"
              :max="249"
              controls-position="right"
              size="small"
              class="app-dialog-input conn-inline-input"
            />
          </div>
        </div>
      </div>

      <!-- IEC104 高级链路参数 -->
      <div class="conn-section connection-link-params-section">
        <div class="conn-section-title-bar--compact">
          <div class="conn-section-title">
            {{ t('slave.deviceTree.connectionDialog.advancedLinkParams') }}
          </div>
          <el-switch
            v-model="connectionForm.linkParamsCustomEnabled"
            @change="handleLinkParamsCustomEnabledChange"
            size="small"
          />
        </div>

        <!-- 折叠态摘要 -->
        <div v-if="!connectionForm.linkParamsCustomEnabled" class="conn-link-summary">
          {{ linkParamsCollapsedDesc }}
        </div>

        <div v-if="connectionForm.linkParamsCustomEnabled" class="conn-link-params-body">
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">
              K
              <el-tooltip :content="t('slave.deviceTree.connectionDialog.tips.k')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="connectionForm.kValue"
                :min="1"
                :max="32767"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">
              W
              <el-tooltip :content="t('slave.deviceTree.connectionDialog.tips.w')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="connectionForm.wValue"
                :min="1"
                :max="32767"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <!-- W >= K 实时警告 -->
            <div
              v-if="linkParamsWarnWK"
              class="app-dialog-field-control app-dialog-field--full conn-inline-warn"
            >
              ⚠ {{ linkParamsWarnWK }}
            </div>
            <label class="app-dialog-field-label">
              T0
              <el-tooltip :content="t('slave.deviceTree.connectionDialog.tips.t0')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="connectionForm.t0"
                :min="1"
                :max="3600"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">
              T1
              <el-tooltip :content="t('slave.deviceTree.connectionDialog.tips.t1')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="connectionForm.t1"
                :min="1"
                :max="3600"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">
              T2
              <el-tooltip :content="t('slave.deviceTree.connectionDialog.tips.t2')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="connectionForm.t2"
                :min="1"
                :max="3600"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">
              T3
              <el-tooltip :content="t('slave.deviceTree.connectionDialog.tips.t3')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="connectionForm.t3"
                :min="1"
                :max="3600"
                controls-position="right"
                size="small"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <!-- T2 >= T1 实时警告 -->
            <div
              v-if="linkParamsWarnT2T1"
              class="app-dialog-field-control app-dialog-field--full conn-inline-warn"
            >
              ⚠ {{ linkParamsWarnT2T1 }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="handleCloseConnectionDialog">{{
            t('common.cancel')
          }}</el-button>
          <el-button type="primary" @click="handleSaveConnection">{{
            connectionDialogConfirmLabel
          }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { toRef } from 'vue'
import { CopyDocument, Delete, Files, Plus, WarningFilled } from '@element-plus/icons-vue'

import type { DeviceNode } from '@/types/slave'
import {
  type StationProfileSubmitPayload,
  useSlaveStationProfileDialogs,
} from '../composables/useSlaveStationProfileDialogs'
import { t } from '@shared/i18n'
import { formatIec104TypeCompactLabel, iec104TypeNameToId } from '@shared/api/iec104'
import type {
  LinkParams,
  RedundancyGroupMode,
  SlaveResponse,
  StationSummary,
} from '@shared/api/types'

interface Props {
  stations: StationSummary[]
  slavesByStation: Record<string, SlaveResponse[]>
  stationId: string
  host: string
  port: number
  defaultCommonAddress: number
  linkParams: LinkParams
  redundancyMode: RedundancyGroupMode
  deviceTreeData: DeviceNode[]
}

const props = defineProps<Props>()
const selectedSlave = defineModel<DeviceNode | null>('selectedSlave', { required: true })
const emit = defineEmits<{
  stationProfileSubmit: [payload: StationProfileSubmitPayload]
}>()

const {
  activeConfigObject,
  activeConfigNameSample,
  asduTypeOptions,
  configDetailFormRef,
  configObjectRules,
  connectionDialogConfirmLabel,
  configObjectAddressCount,
  connectionDialogMode,
  connectionDialogSubtitleBadges,
  connectionDialogTitle,
  connectionForm,
  dialogPanel,
  editingConfigObjects,
  handleActiveConfigAsduChange,
  handleAddConfigObject,
  handleApplyDefaultTemplate,
  handleCloseConnectionDialog,
  handleConnectionDialogBeforeClose,
  handleConnectionBadgeClick,
  handleCreateListener,
  handleCreateSlave,
  handleDuplicateByIndex,
  handleEditConnection,
  handleEditSlave,
  handleLinkParamsCustomEnabledChange,
  handleRemoveConfigObjectByIndex,
  handleSaveConnection,
  handleSelectConfigObject,
  handleSlaveConfirm,
  handleSlaveDialogBeforeClose,
  ioaConflictMap,
  linkParamsCollapsedDesc,
  linkParamsWarnT2T1,
  linkParamsWarnWK,
  objNameInputRef,
  requestCloseSlaveDialog,
  selectedConfigIndex,
  showEditConnectionDialog,
  showSlaveDialog,
  slaveBaseFormRef,
  slaveDialogMode,
  slaveDialogSubtitleBadges,
  slaveForm,
  slaveFormRules,
} = useSlaveStationProfileDialogs({
  stations: toRef(props, 'stations'),
  slavesByStation: toRef(props, 'slavesByStation'),
  stationId: toRef(props, 'stationId'),
  host: toRef(props, 'host'),
  port: toRef(props, 'port'),
  defaultCommonAddress: toRef(props, 'defaultCommonAddress'),
  linkParams: toRef(props, 'linkParams'),
  redundancyMode: toRef(props, 'redundancyMode'),
  deviceTreeData: toRef(props, 'deviceTreeData'),
  selectedSlave,
  emitStationProfileSubmit: (payload) => emit('stationProfileSubmit', payload),
})

void [configDetailFormRef, connectionDialogMode, dialogPanel, objNameInputRef, slaveBaseFormRef]

const formatTypeCompact = (dataType: string) =>
  formatIec104TypeCompactLabel(iec104TypeNameToId(dataType))

defineExpose({
  handleCreateListener,
  handleCreateSlave,
  handleEditSlave,
  handleEditConnection,
})
</script>

<style scoped>
.connection-label-with-tip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.connection-tip-icon {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1px solid #d5dbe7;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: #8b98ac;
  background: #f8fafc;
}

.connection-switch-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 8px;
}

.connection-switch-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 48px;
  padding: 0 14px;
  border: 1px solid #e4eaf3;
  border-radius: 10px;
  background: #fafcff;
}

.connection-switch-label-with-tip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.connection-switch-label {
  font-size: 14px;
  font-weight: 600;
  color: #4a5a73;
}

.slave-policy-layout {
  gap: 10px 14px;
}

.slave-name-template-control {
  width: 100%;
  min-width: 0;
}

.slave-policy-hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.35;
  color: #8b98ac;
  white-space: normal;
  overflow-wrap: anywhere;
}

.connection-advanced-panel {
  overflow: hidden;
}

.slave-dialog-summary {
  margin-top: 6px;
  line-height: 1.3;
  color: #8391a7;
}

.slave-dialog {
  --dialog-content-bottom-gap: 10px;
  height: min(82vh, 780px);
  max-height: 82vh;
}

.slave-dialog :deep(.app-section) {
  margin: 0;
  border: none;
  background: transparent;
  box-shadow: none;
  padding: 0;
}

/* --- 左栏列表头：横排工具条 --- */
.slave-edit-list-header {
  flex-direction: row !important;
  align-items: center !important;
  justify-content: space-between !important;
  gap: 8px !important;
  padding: 8px 10px !important;
  border-bottom: 1px solid #e9eff8;
}

.slave-edit-list-actions {
  flex-direction: row !important;
  align-items: center !important;
  flex-wrap: nowrap !important;
  gap: 6px !important;
  width: auto !important;
  margin-left: auto;
  justify-content: flex-end;
}

.slave-edit-list-count {
  font-weight: 400;
  color: #94a3b8;
  font-size: 12px;
  margin-left: 4px;
}

.slave-object-list .app-object-list-actions {
  flex-direction: row;
  align-items: center;
  gap: 6px;
}

.slave-edit-merged-editor {
  height: 100%;
  min-height: 0;
}

.slave-edit-object-list {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* === 统一列表项 (All-Icons Alignment) === */
.slave-list-item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 42px;
  padding: 0 12px;
  margin: 0 8px;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
  position: relative;
  transition: background-color 0.15s ease;
  flex-shrink: 0;
}

.slave-list-item + .slave-list-item {
  margin-top: 4px;
}

.slave-list-item:hover {
  background: #f5f7fa;
}

.slave-list-item.active {
  background: var(--selection-highlight-bg, #eff6ff);
  box-shadow: inset 3px 0 0 var(--primary-color, #2563eb);
}

/* 图标 */
.slave-list-item-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.slave-list-item-icon--setting {
  color: #606266;
}

.slave-list-item-icon--object {
  color: #409eff;
}

.slave-list-item.active .slave-list-item-icon--setting {
  color: #409eff;
}

/* 名称 */
.slave-list-item-name {
  flex: 1;
  font-size: 14px;

  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1;
}

/* 徽标 */
.slave-list-item-badge {
  font-family: var(--font-mono, 'SF Mono', 'Cascadia Code', 'Consolas', monospace);
  font-size: 11px;
  line-height: 1;
  color: #909399;
  background: #f4f4f5;
  border-radius: 999px;
  padding: 3px 7px;
  flex-shrink: 0;
}

/* 删除按钮：仅 hover / active 可见 */
.slave-list-item-delete {
  opacity: 0;
  transition: opacity 0.15s ease;
  flex-shrink: 0;
}

.slave-list-item:hover .slave-list-item-delete,
.slave-list-item.active .slave-list-item-delete {
  opacity: 1;
}

/* === 分割线 === */
.slave-edit-list-divider {
  height: 1px;
  background: #ebeef5;
  margin: 8px 8px 12px;
  flex-shrink: 0;
}

.slave-edit-object-scroll {
  flex: 1 1 auto;
  height: auto;
  min-height: 0;
  overflow-y: auto !important;
  overflow-x: hidden;
  scrollbar-gutter: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--app-scrollbar-thumb) var(--app-scrollbar-track);
}

.slave-edit-object-scroll::-webkit-scrollbar {
  width: var(--app-scrollbar-size);
}

.slave-edit-object-scroll::-webkit-scrollbar-thumb {
  background: var(--app-scrollbar-thumb);
  border-radius: var(--app-scrollbar-radius);
}

.slave-edit-object-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--app-scrollbar-thumb-hover);
}

.slave-edit-object-scroll::-webkit-scrollbar-track {
  background: var(--app-scrollbar-track);
  border-radius: var(--app-scrollbar-radius);
}

.slave-edit-detail-scroll {
  padding-top: 4px;
  scrollbar-width: thin;
  scrollbar-color: var(--app-scrollbar-thumb) var(--app-scrollbar-track);
}

.slave-edit-detail-scroll::-webkit-scrollbar {
  width: var(--app-scrollbar-size);
}

.slave-edit-detail-scroll::-webkit-scrollbar-thumb {
  background: var(--app-scrollbar-thumb);
  border-radius: var(--app-scrollbar-radius);
}

.slave-edit-detail-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--app-scrollbar-thumb-hover);
}

.slave-edit-detail-scroll::-webkit-scrollbar-track {
  background: var(--app-scrollbar-track);
  border-radius: var(--app-scrollbar-radius);
}

.slave-template-quick-btn {
  justify-content: flex-start;
  padding: 0 6px !important;
  color: #64748b;
  font-size: 12px;
}

.slave-object-action-btn :deep(.el-icon) {
  margin-right: 4px;
}

.slave-object-list-scroll {
  overscroll-behavior: contain;
  scrollbar-gutter: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--app-scrollbar-thumb) var(--app-scrollbar-track);
}

.slave-object-list-scroll::-webkit-scrollbar {
  width: var(--app-scrollbar-size);
}

.slave-object-list-scroll::-webkit-scrollbar-thumb {
  background: var(--app-scrollbar-thumb);
  border-radius: var(--app-scrollbar-radius);
}

.slave-object-list-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--app-scrollbar-thumb-hover);
}

.slave-object-list-scroll::-webkit-scrollbar-track {
  background: var(--app-scrollbar-track);
  border-radius: var(--app-scrollbar-radius);
}

/* === 右侧详情表单 === */
.slave-object-detail-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px 16px;
}

.slave-object-detail-form :deep(.el-form-item) {
  margin-bottom: 0;
}

/* === 表单统一规范 === */
.slave-dialog-body :deep(.el-form-item__label) {
  white-space: nowrap;
}

.slave-dialog-body :deep(.el-form-item:last-child) {
  margin-bottom: 0;
}

.slave-dialog-body :deep(.el-input-number),
.slave-dialog-body :deep(.el-select),
.slave-dialog-body :deep(.el-input) {
  width: 100%;
}

/* === 左右分栏：gutter 代替分割线 === */
.slave-dialog :deep(.app-object-detail) {
  border-left: none;
  margin-left: 28px;
}

.app-object-empty {
  min-height: 90px;
  border-color: #d8e0ea;
  background: #f5f8fd;
}

@media (max-width: 960px) {
  .slave-object-detail-grid {
    grid-template-columns: 1fr;
  }
}

/* === Split-pane object config layout === */
.conn-objects-section {
  flex: 0 1 auto;
  min-height: 300px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin-bottom: 0;
}

.conn-objects-section .conn-section-title {
  flex-shrink: 0;
}

.slave-obj-split {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  gap: 0;
  flex: 0 1 auto;
  /* min-height: 320px; */
  height: min(42vh, 380px);
  border: 1px solid #e9eff8;
  border-radius: 10px;
  overflow: hidden;
}

.slave-obj-left {
  display: flex;
  flex-direction: column;
  border-right: 1px solid #e9eff8;
  background: #fafbfd;
  min-height: 0;
  overflow: hidden;
}

.slave-obj-toolbar {
  --dialog-toolbar-btn-padding-x: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-bottom: 1px solid #e9eff8;
  flex-shrink: 0;
}

.slave-obj-toolbar-count {
  min-width: 0;
}

.slave-obj-toolbar-actions {
  min-width: 0;
  width: auto;
  margin-left: auto;
}

.slave-obj-toolbar-btn-group {
  display: flex;
  align-items: center;
  gap: 6px;
  width: auto;
  min-width: 0;
}

.slave-obj-toolbar-btn {
  min-width: 0 !important;
  max-width: 100%;
}

.slave-obj-list-scroll {
  flex: 1 1 auto;
  height: 0;
  min-height: 0;
}

.slave-obj-list-scroll :deep(.el-scrollbar__wrap) {
  overflow-x: hidden;
  overscroll-behavior: contain;
}

.slave-obj-list-scroll :deep(.el-scrollbar__view) {
  min-height: 100%;
}

.slave-obj-item {
  padding: 8px 12px;
  cursor: pointer;
  border-left: 3px solid transparent;
  transition: all 0.15s ease;
}

.slave-obj-item:hover {
  background: #f0f4fa;
}

.slave-obj-item--active {
  background: var(--selection-highlight-bg, #eff6ff) !important;
  border-left-color: var(--primary-color, #2563eb);
}

.slave-obj-item--warn .slave-obj-item-name {
  color: #d97706;
}

.slave-obj-item-main {
  display: flex;
  align-items: center;
  gap: 6px;
}

.slave-obj-item-name {
  font-size: 13px;
  font-weight: 500;
  color: #1e293b;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slave-obj-item-actions {
  display: flex;
  visibility: hidden;
  gap: 0;
  align-items: center;
  flex-shrink: 0;
  margin-left: auto;
}

.slave-obj-item-actions .el-button {
  padding: 2px !important;
  margin: 0 !important;
  min-width: 0;
  width: 22px;
  height: 22px;
}

.slave-obj-item-actions .el-tooltip__trigger {
  display: inline-flex;
  line-height: 1;
}

.slave-obj-item-actions .el-button .el-icon {
  font-size: 13px;
}

.slave-obj-item:hover .slave-obj-item-actions {
  visibility: visible;
}

.slave-obj-warn-icon {
  color: #f59e0b;
  font-size: 14px;
  flex-shrink: 0;
}

.slave-obj-asdu-pill {
  font-size: 10px !important;
  height: 18px !important;
  line-height: 16px !important;
  padding: 0 7px !important;
  border-radius: 9px !important;
  border: 1px solid #d5dbe7 !important;
  background: #f0f4fa !important;
  color: #475569 !important;
  font-family: var(--font-mono, monospace);
  flex-shrink: 0;
}

.slave-obj-sub {
  font-size: 11px;
  color: #94a3b8;
  margin-top: 2px;
  font-family: var(--font-mono, monospace);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  min-width: 0;
}

.slave-obj-sub-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slave-obj-empty {
  padding: 32px 16px;
  text-align: center;
  color: #94a3b8;
  font-size: 13px;
}

.slave-obj-right-title {
  font-size: 14px;
  font-weight: 600;
  color: #1e293b;
  margin-bottom: 10px;
}

.slave-obj-right {
  padding: 14px 0 14px 18px;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.slave-obj-right--empty {
  padding: 14px 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #94a3b8;
  font-size: 13px;
}

.slave-obj-form .el-form-item {
  margin-bottom: 0;
}

.slave-obj-form {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 10px;
}

.slave-obj-form .el-form-item__label {
  font-size: 13px;
  color: #4a5a73;
  font-weight: 500;
  white-space: nowrap;
  justify-content: flex-end;
}

.slave-obj-form .app-dialog-field-grid--1 {
  gap: 8px 10px;
}

.slave-obj-form {
  --dialog-control-min-height: 32px;
}

.slave-address-mode-group {
  align-self: flex-start;
  --app-form-segment-min-width: 72px;
}

.slave-dialog-body {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  height: auto;
  box-sizing: border-box;
  min-height: 0;
  overflow-y: auto !important;
  overflow-x: hidden !important;
  max-height: none !important;
}

.slave-obj-form .el-form-item__label::before,
.slave-obj-form .el-form-item .el-form-item__label::before {
  display: none !important;
  content: '' !important;
}

.slave-obj-conflict-tip {
  font-size: 12px;
  color: #d97706;
  padding: 6px 0;
  line-height: 1.4;
}

.label-help-icon {
  color: #94a3b8;
  cursor: help;
  font-size: 14px;
}

.label-help-icon:hover {
  color: var(--primary-color, #2563eb);
}
</style>
