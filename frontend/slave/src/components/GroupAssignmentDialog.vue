<template>
  <el-dialog
    v-model="visible"
    width="660px"
    :close-on-click-modal="false"
    :lock-scroll="false"
    class="app-dialog-shell"
  >
    <template #header
      ><div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ title }}</div>
          <div class="app-dialog-subtitle-badges">
            <span v-for="(badge, index) in badges" :key="index" class="app-dialog-subtitle-badge">{{
              badge
            }}</span>
          </div>
        </div>
      </div></template
    >
    <div class="app-dialog-body app-dialog-scroll-shadow conn-flat-body">
      <el-form class="app-dialog-form" @submit.prevent>
        <section class="conn-section">
          <div class="conn-section-title">{{ t('slave.contentPanel.giGroup.settings') }}</div>
          <div :class="integratedTotal ? 'app-dialog-field-grid--2' : 'app-dialog-field-grid--1'">
            <label class="app-dialog-field-label">{{
              t('slave.contentPanel.editDialog.giGroup')
            }}</label>
            <div class="app-dialog-field-control">
              <el-select
                v-model="giGroup"
                :placeholder="t('slave.contentPanel.giGroup.select')"
                class="app-dialog-input conn-inline-input"
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
            </div>
            <template v-if="integratedTotal">
              <label class="app-dialog-field-label">{{
                t('slave.contentPanel.editDialog.counterGroup')
              }}</label>
              <div class="app-dialog-field-control">
                <el-select
                  v-model="counterGroup"
                  :placeholder="t('slave.contentPanel.counterGroup.select')"
                  class="app-dialog-input conn-inline-input"
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
              </div>
            </template>
          </div>
        </section>
        <section class="conn-section">
          <div class="conn-section-title-bar">
            <div class="conn-section-title-heading">
              <div class="conn-section-title">
                {{ t('slave.contentPanel.giGroup.selectedPoints') }}
              </div>
              <span class="conn-section-meta-pill">{{
                t('slave.contentPanel.giGroup.selectedSummary', { count: pointCount })
              }}</span>
            </div>
          </div>
          <div class="app-dialog-table-wrap">
            <el-table
              ref="tableRef"
              :data="rows"
              :row-key="rowKey"
              :row-class-name="rowClassName"
              :row-style="rowStyle"
              size="small"
              max-height="220"
              stripe
              class="app-dialog-table"
            >
              <el-table-column label="IOA" width="100" align="right" header-align="right"
                ><template #default="{ row }"
                  ><span v-if="!isSpacer(row)" class="font-mono">{{
                    formatIoa(row.address)
                  }}</span></template
                ></el-table-column
              >
              <el-table-column
                prop="name"
                :label="t('slave.contentPanel.editDialog.name')"
                min-width="190"
                show-overflow-tooltip
                ><template #default="{ row }"
                  ><span v-if="!isSpacer(row)">{{ row.name }}</span></template
                ></el-table-column
              >
              <el-table-column
                :label="t('slave.contentPanel.giGroup.currentGroup')"
                width="130"
                show-overflow-tooltip
                ><template #default="{ row }"
                  ><span v-if="!isSpacer(row)">{{ formatGiGroup(row.giGroup) }}</span></template
                ></el-table-column
              >
              <el-table-column
                v-if="integratedTotal"
                :label="t('slave.contentPanel.counterGroup.currentGroup')"
                width="130"
                show-overflow-tooltip
                ><template #default="{ row }"
                  ><span v-if="!isSpacer(row)">{{
                    formatCounterGroup(row.counterGroup)
                  }}</span></template
                ></el-table-column
              >
            </el-table>
          </div>
        </section>
      </el-form>
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
            :disabled="giGroup < 0 || (integratedTotal && counterGroup < 0)"
            @click="$emit('submit')"
            >{{ t('common.save') }}</el-button
          >
        </div>
      </div></template
    >
  </el-dialog>
</template>

<script setup lang="ts">
import { t } from '@shared/i18n'
import type { VirtualSpacerRow } from '@shared/composables/useTableVirtualScroll'
import type { SlaveContentDataPoint as DataPoint } from '@/types/slaveContentPanel'

defineProps<{
  title: string
  badges: string[]
  rows: Array<DataPoint | VirtualSpacerRow>
  pointCount: number
  integratedTotal: boolean
  tableRef: (value: unknown) => void
  rowKey: (row: DataPoint | VirtualSpacerRow) => string
  rowClassName: (value: { row: DataPoint | VirtualSpacerRow }) => string
  rowStyle: (value: { row: DataPoint | VirtualSpacerRow }) => Record<string, string>
  isSpacer: (row: unknown) => boolean
  formatIoa: (address: number) => string
  formatGiGroup: (group: number | null | undefined) => string
  formatCounterGroup: (group: number | null | undefined) => string
}>()
defineEmits<{ submit: [] }>()
const visible = defineModel<boolean>('visible', { required: true })
const giGroup = defineModel<number>('giGroup', { required: true })
const counterGroup = defineModel<number>('counterGroup', { required: true })
</script>
