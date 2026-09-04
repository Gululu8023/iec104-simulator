<template>
  <el-dialog
    v-model="visible"
    width="700px"
    :before-close="beforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="app-dialog-shell app-master-link-dialog"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">
            {{
              mode === 'create'
                ? t('master.appDialogs.link.createTitle')
                : t('master.appDialogs.link.editTitle')
            }}
          </div>
          <div v-if="badges.length > 0" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in badges"
              :key="`link-dialog-badge-${index}`"
              class="app-dialog-subtitle-badge app-dialog-subtitle-badge--action"
              :title="t('master.appDialogs.copyBadgeTitle')"
              @click="emit('badge-click', index, badge)"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body app-dialog-scroll-shadow conn-flat-body">
      <div class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.link.networkParams') }}</div>
        <div class="app-dialog-field-grid--3">
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.link.connectionName')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input
              v-model="form.name"
              :placeholder="t('master.appDialogs.link.connectionNamePlaceholder')"
              size="small"
              class="app-dialog-input conn-inline-input"
            />
          </div>
          <label class="app-dialog-field-label">{{
            t('master.appDialogs.link.remoteAddress')
          }}</label>
          <div class="app-dialog-field-control">
            <el-input
              v-model="form.host"
              placeholder="IP / Host"
              size="small"
              class="app-dialog-input conn-inline-input"
            />
          </div>
          <label class="app-dialog-field-label">{{ t('master.appDialogs.link.remotePort') }}</label>
          <div class="app-dialog-field-control">
            <el-input-number
              v-model="form.port"
              :min="1"
              :max="65535"
              size="small"
              controls-position="right"
              class="app-dialog-input conn-inline-input"
            />
          </div>
        </div>
      </div>

      <div class="conn-section">
        <div class="conn-section-title-bar--compact">
          <div class="conn-section-title">{{ t('master.appDialogs.link.advancedParams') }}</div>
          <el-switch v-model="customEnabled" size="small" @change="emit('custom-change', $event)" />
        </div>
        <div v-if="!customEnabled" class="conn-link-summary">{{ collapsedDescription }}</div>
        <div v-if="customEnabled" class="conn-link-params-body">
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label"
              >K
              <el-tooltip :content="t('master.appDialogs.link.tips.k')" placement="top"
                ><span class="conn-tip-icon">?</span></el-tooltip
              ></label
            >
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.link_params!.k_value"
                :min="1"
                :max="32767"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label"
              >W
              <el-tooltip :content="t('master.appDialogs.link.tips.w')" placement="top"
                ><span class="conn-tip-icon">?</span></el-tooltip
              ></label
            >
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.link_params!.w_value"
                :min="1"
                :max="32767"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <div
              v-if="warnWK"
              class="app-dialog-field-control app-dialog-field--full conn-inline-warn"
            >
              ⚠ {{ warnWK }}
            </div>
            <label class="app-dialog-field-label"
              >T0
              <el-tooltip :content="t('master.appDialogs.link.tips.t0')" placement="top"
                ><span class="conn-tip-icon">?</span></el-tooltip
              ></label
            >
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.link_params!.t0_seconds"
                :min="1"
                :max="3600"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label"
              >T1
              <el-tooltip :content="t('master.appDialogs.link.tips.t1')" placement="top"
                ><span class="conn-tip-icon">?</span></el-tooltip
              ></label
            >
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.link_params!.t1_seconds"
                :min="1"
                :max="3600"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label"
              >T2
              <el-tooltip :content="t('master.appDialogs.link.tips.t2')" placement="top"
                ><span class="conn-tip-icon">?</span></el-tooltip
              ></label
            >
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.link_params!.t2_seconds"
                :min="1"
                :max="3600"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label"
              >T3
              <el-tooltip :content="t('master.appDialogs.link.tips.t3')" placement="top"
                ><span class="conn-tip-icon">?</span></el-tooltip
              ></label
            >
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.link_params!.t3_seconds"
                :min="1"
                :max="3600"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <div
              v-if="warnT2T1"
              class="app-dialog-field-control app-dialog-field--full conn-inline-warn"
            >
              ⚠ {{ warnT2T1 }}
            </div>
          </div>
        </div>
      </div>

      <div class="conn-section">
        <div class="conn-section-title-bar--compact">
          <div class="conn-section-title">{{ t('master.appDialogs.link.autoReconnect') }}</div>
          <el-switch v-model="form.auto_connect" size="small" />
        </div>
        <div v-if="form.auto_connect" class="conn-link-params-body">
          <div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.link.retryCount')
            }}</label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.retry_count"
                :min="0"
                :max="999"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.link.retryInterval')
            }}</label>
            <div class="app-dialog-field-control">
              <el-input-number
                v-model="form.retry_interval"
                :min="0"
                :max="3600"
                size="small"
                controls-position="right"
                class="app-dialog-input conn-inline-input"
              />
            </div>
            <label class="app-dialog-field-label">
              {{ t('master.appDialogs.link.autoStartDataTransfer') }}
              <el-tooltip :content="t('master.appDialogs.link.autoStartHint')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-switch
                v-model="form.auto_start_data_transfer"
                size="small"
                class="app-primary-switch"
              />
            </div>
            <label class="app-dialog-field-label">
              {{ t('master.appDialogs.link.autoGeneralInterrogation') }}
              <el-tooltip :content="t('master.appDialogs.link.autoGiHint')" placement="top">
                <span class="conn-tip-icon">?</span>
              </el-tooltip>
            </label>
            <div class="app-dialog-field-control">
              <el-switch v-model="form.auto_gi" size="small" class="app-primary-switch" />
            </div>
          </div>
        </div>
        <div v-else class="app-dialog-help">
          {{ t('master.appDialogs.link.autoReconnectHelp') }}
        </div>
      </div>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="emit('close')">{{
            t('common.cancel')
          }}</el-button>
          <el-button class="app-btn-primary" type="primary" @click="emit('save')">{{
            t('common.save')
          }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import type { UpsertLinkProfileRequest } from '@shared/api/types'
import { t } from '@shared/i18n'

defineProps<{
  mode: 'create' | 'edit'
  badges: string[]
  form: UpsertLinkProfileRequest
  collapsedDescription: string
  warnWK: string
  warnT2T1: string
  beforeClose: (done: () => void) => void
}>()
const emit = defineEmits<{
  close: []
  save: []
  'badge-click': [index: number, badge: string]
  'custom-change': [value: boolean | string | number]
}>()
const visible = defineModel<boolean>('visible', { required: true })
const customEnabled = defineModel<boolean>('customEnabled', { required: true })
</script>
