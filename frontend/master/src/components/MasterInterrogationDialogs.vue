<template>
  <el-dialog
    v-model="generalVisible"
    width="520px"
    :close-on-click-modal="false"
    :lock-scroll="false"
    class="app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">
            {{ t('master.appDialogs.generalInterrogation.title') }}
          </div>
          <div v-if="generalBadges.length" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in generalBadges"
              :key="`general-interrogation-badge-${index}`"
              class="app-dialog-subtitle-badge"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div>
    </template>
    <div class="app-dialog-body conn-flat-body">
      <section class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.control.commandParams') }}</div>
        <el-form class="app-dialog-form" @submit.prevent
          ><div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.generalInterrogation.range')
            }}</label>
            <div class="app-dialog-field-control">
              <el-select
                v-model="generalGroup"
                size="small"
                class="app-dialog-input conn-inline-input"
                popper-class="app-select-dropdown"
                fit-input-width
              >
                <el-option
                  :label="t('master.appDialogs.generalInterrogation.station', { qoi: 20 })"
                  :value="0"
                />
                <el-option
                  v-for="group in 16"
                  :key="group"
                  :label="
                    t('master.appDialogs.generalInterrogation.group', { group, qoi: group + 20 })
                  "
                  :value="group"
                />
              </el-select>
            </div></div
        ></el-form>
      </section>
    </div>
    <template #footer
      ><div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="generalVisible = false">{{
            t('common.cancel')
          }}</el-button>
          <el-button
            type="primary"
            class="app-btn-primary"
            :loading="generalLoading"
            @click="$emit('submit-general')"
            >{{ t('common.ok') }}</el-button
          >
        </div>
      </div></template
    >
  </el-dialog>

  <el-dialog
    v-model="counterVisible"
    width="520px"
    :close-on-click-modal="false"
    :lock-scroll="false"
    class="app-dialog-shell"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('master.appDialogs.counter.title') }}</div>
          <div v-if="counterBadges.length" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in counterBadges"
              :key="`counter-command-badge-${index}`"
              class="app-dialog-subtitle-badge"
              >{{ badge }}</span
            >
          </div>
        </div>
      </div>
    </template>
    <div class="app-dialog-body conn-flat-body">
      <section class="conn-section">
        <div class="conn-section-title">{{ t('master.appDialogs.control.commandParams') }}</div>
        <el-form class="app-dialog-form" @submit.prevent
          ><div class="app-dialog-field-grid--2">
            <label class="app-dialog-field-label">{{ t('master.appDialogs.counter.range') }}</label>
            <div class="app-dialog-field-control">
              <el-select
                v-model="counterRequest"
                size="small"
                class="app-dialog-input conn-inline-input"
                popper-class="app-select-dropdown"
                fit-input-width
              >
                <el-option :label="t('master.appDialogs.counter.all')" :value="1" />
                <el-option
                  v-for="group in 4"
                  :key="group"
                  :label="t('master.appDialogs.counter.group', { group })"
                  :value="group + 1"
                />
              </el-select>
            </div>
            <label class="app-dialog-field-label">{{
              t('master.appDialogs.counter.action')
            }}</label>
            <div class="app-dialog-field-control">
              <el-select
                v-model="counterAction"
                size="small"
                class="app-dialog-input conn-inline-input"
                popper-class="app-select-dropdown"
                fit-input-width
              >
                <el-option :label="t('master.appDialogs.counter.read')" value="read" />
                <el-option :label="t('master.appDialogs.counter.freeze')" value="freeze" />
                <el-option
                  :label="t('master.appDialogs.counter.freezeAndReset')"
                  value="freeze-and-reset"
                />
                <el-option :label="t('master.appDialogs.counter.reset')" value="reset" />
              </el-select>
            </div></div
        ></el-form>
      </section>
    </div>
    <template #footer
      ><div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="counterVisible = false">{{
            t('common.cancel')
          }}</el-button>
          <el-button
            type="primary"
            class="app-btn-primary"
            :loading="counterLoading"
            @click="$emit('submit-counter')"
            >{{ t('common.ok') }}</el-button
          >
        </div>
      </div></template
    >
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'

defineProps<{
  generalBadges: string[]
  generalLoading: boolean
  counterBadges: string[]
  counterLoading: boolean
}>()
defineEmits<{ 'submit-general': []; 'submit-counter': [] }>()
const generalVisible = defineModel<boolean>('generalVisible', { required: true })
const generalGroup = defineModel<number>('generalGroup', { required: true })
const counterVisible = defineModel<boolean>('counterVisible', { required: true })
const counterRequest = defineModel<number>('counterRequest', { required: true })
const counterAction = defineModel<'read' | 'freeze' | 'freeze-and-reset' | 'reset'>(
  'counterAction',
  { required: true },
)
</script>
