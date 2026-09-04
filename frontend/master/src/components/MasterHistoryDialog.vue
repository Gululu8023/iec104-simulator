<template>
  <el-dialog
    :model-value="visible"
    width="720px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="app-dialog-shell master-history-dialog"
    @update:model-value="emit('update:visible', $event)"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('master.pointHistory.dialogTitle') }}</div>
          <div class="app-dialog-subtitle-badges" v-if="dialog.key">
            <span
              class="app-dialog-subtitle-badge history-badge--truncate"
              :title="dialog.connectionName"
              ><span class="history-badge__text">{{ dialog.connectionName }}</span></span
            >
            <span
              class="app-dialog-subtitle-badge history-badge--truncate"
              :title="dialog.slaveName"
              ><span class="history-badge__text">{{ dialog.slaveName }}</span></span
            >
            <span class="app-dialog-subtitle-badge font-mono">{{ caText }}</span>
            <span class="app-dialog-subtitle-badge">{{ historyTypeLabel }}</span>
            <span
              class="app-dialog-subtitle-badge history-badge--truncate"
              :title="dialog.pointName"
              ><span class="history-badge__text">{{ dialog.pointName }}</span></span
            >
            <span class="app-dialog-subtitle-badge font-mono"
              >IOA {{ formatIoaAddress(dialog.address) }}</span
            >
          </div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body app-dialog-scroll-shadow history-dialog-body">
      <div class="history-summary-bar" v-if="latestEntry">
        <div class="history-summary-bar__item history-summary-bar__primary">
          <div class="history-summary-bar__label">{{ t('master.pointHistory.currentValue') }}</div>
          <div class="history-summary-bar__value" :class="`is-${latestEntry.valueTone}`">
            {{ latestEntry.valueText }}
          </div>
        </div>
        <div class="history-summary-bar__item">
          <span class="history-summary-bar__meta-label">{{
            t('master.pointHistory.latestCause')
          }}</span>
          <span class="history-summary-bar__meta-value">{{ latestEntry.causeText }}</span>
        </div>
        <div class="history-summary-bar__item">
          <span class="history-summary-bar__meta-label">{{
            t('master.pointHistory.latestQuality')
          }}</span>
          <span class="history-summary-bar__meta-value">{{ latestEntry.qualityText }}</span>
        </div>
        <div class="history-summary-bar__item">
          <span class="history-summary-bar__meta-label">{{
            t('master.pointHistory.latestUpdate')
          }}</span>
          <span class="history-summary-bar__meta-value font-mono">{{
            latestEntry.receivedAtText
          }}</span>
        </div>
      </div>

      <div v-if="entries.length === 0" class="history-empty-state">
        <el-empty :image-size="88" :description="t('master.pointHistory.emptySessionHistory')" />
      </div>

      <div v-else class="history-entry-list app-timeline">
        <article
          v-for="(entry, index) in entries"
          :key="entry.id"
          class="history-entry app-timeline__item"
          :class="{
            'is-muted': entry.isInitialSnapshot,
            'is-no-change': !entry.hasValueChange && !entry.isInitialSnapshot,
            'is-expanded':
              dialog.expandedEntryId === entry.id ||
              (dialog.expandedEntryId === '__first__' && index === 0),
          }"
          @click="emit('toggle-entry', entry.id)"
        >
          <span class="history-entry__marker app-timeline__marker" aria-hidden="true"></span>
          <div class="history-entry__card app-timeline__card">
            <div class="history-entry__head">
              <div class="history-entry__main">
                <div class="history-entry__value-line">
                  <template v-if="entry.hasValueChange">
                    <span class="history-entry__previous">{{ entry.previousValueText }}</span>
                    <span class="history-entry__arrow">→</span>
                    <span class="history-entry__value" :class="`is-${entry.valueTone}`">{{
                      entry.valueText
                    }}</span>
                  </template>
                  <template v-else>
                    <span
                      class="history-entry__value history-entry__value--compact"
                      :class="`is-${entry.valueTone}`"
                      >{{ entry.valueText }}</span
                    >
                    <span
                      class="history-entry__state"
                      :class="{ 'history-entry__state--snapshot': entry.isInitialSnapshot }"
                    >
                      {{
                        entry.isInitialSnapshot
                          ? t('master.pointHistory.initialSnapshot')
                          : t('master.pointHistory.valueUnchanged')
                      }}
                    </span>
                  </template>
                </div>
                <div class="history-entry__meta-line">
                  <span class="history-entry__cause">{{ entry.causeText }}</span>
                  <span class="history-entry__quality" :class="`is-${entry.qualityTone}`">{{
                    entry.qualityText
                  }}</span>
                </div>
              </div>

              <div class="history-entry__time">
                <div class="history-entry__time-value">{{ entry.primaryTimeText }}</div>
              </div>

              <span class="history-entry__chevron" aria-hidden="true">›</span>
            </div>

            <div
              v-if="
                dialog.expandedEntryId === entry.id ||
                (dialog.expandedEntryId === '__first__' && index === 0)
              "
              class="history-entry__details app-timeline__details"
            >
              <span v-if="entry.cp56Text !== '—'">
                <strong>{{ t('master.pointHistory.eventTimestamp') }}</strong>
                {{ entry.cp56Text }}
              </span>
              <span v-if="entry.pointTimestampText !== '—'">
                <strong>{{ t('master.pointHistory.sourceTimestamp') }}</strong>
                {{ entry.pointTimestampText }}
              </span>
              <span>
                <strong>{{ t('master.pointHistory.localReceiveTime') }}</strong>
                {{ entry.receivedAtText }}
              </span>
              <span>
                <strong>CA / IOA</strong>
                {{ caText }} / {{ formatIoaAddress(dialog.address) }}
              </span>
              <span
                v-if="entry.previousQualityText && entry.previousQualityText !== entry.qualityText"
              >
                <strong>{{ t('master.pointHistory.qualityChange') }}</strong>
                {{ entry.previousQualityText }} → {{ entry.qualityText }}
              </span>
            </div>
          </div>
        </article>
      </div>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="emit('update:visible', false)">{{
            t('master.pointHistory.close')
          }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { formatIec104TypeCompactLabel, iec104TypeNameToId } from '@shared/api/iec104'
import { currentLocale, t } from '@shared/i18n'

type HistoryDialogState = {
  key: string
  pointName: string
  address: number
  dataType: string
  connectionName: string
  slaveName: string
  expandedEntryId: string | null
}

type HistoryEntry = {
  id: string
  valueTone: string
  valueText: string
  causeText: string
  qualityText: string
  receivedAtText: string
  isInitialSnapshot: boolean
  hasValueChange: boolean
  previousValueText: string | null
  qualityTone: string
  primaryTimeText: string
  cp56Text: string
  pointTimestampText: string
  previousQualityText: string | null
}

const props = defineProps<{
  visible: boolean
  dialog: HistoryDialogState
  latestEntry: HistoryEntry | null
  entries: HistoryEntry[]
  caText: string
  formatIoaAddress: (value: unknown) => string
}>()

const historyTypeLabel = computed(() => {
  void currentLocale.value
  return formatIec104TypeCompactLabel(iec104TypeNameToId(props.dialog.dataType))
})

const emit = defineEmits<{
  'update:visible': [value: boolean]
  'toggle-entry': [id: string]
}>()
</script>

<style scoped>
.history-dialog-body {
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
}

.history-summary-bar {
  display: grid;
  grid-template-columns: minmax(110px, 1.1fr) repeat(3, minmax(0, 1fr));
  align-items: stretch;
  gap: 0;
  padding: 7px 10px;
  background: linear-gradient(135deg, #f0f7ff 0%, #fafcff 100%);
  border: 1px solid #d6e2f2;
  border-radius: 10px;
  min-height: 64px;
}

.history-summary-bar__item {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
  min-width: 0;
  padding: 0 10px;
  border-left: 1px solid #e0e8f2;
}

.history-summary-bar__item:first-child {
  padding-left: 2px;
  border-left: 0;
}

.history-summary-bar__label {
  font-size: 12px;
  font-weight: 600;
  color: #64748b;
  white-space: nowrap;
}

.history-summary-bar__value {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 20px;
  font-weight: 800;
  line-height: 1;
  color: #0f172a;
  white-space: nowrap;
}

.history-summary-bar__value.is-on {
  color: #dc2626;
}

.history-summary-bar__value.is-off {
  color: #16a34a;
}

.history-summary-bar__value.is-transition {
  color: #d97706;
}

.history-summary-bar__value.is-neutral {
  color: #0f172a;
}

.history-summary-bar__meta-label {
  font-size: 11px;
  font-weight: 600;
  color: #94a3b8;
  white-space: nowrap;
}

.history-summary-bar__meta-value {
  font-size: 13px;
  font-weight: 600;
  color: #475569;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-empty-state {
  flex: 1;
  min-height: 280px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.history-entry-list {
  gap: 6px;
  min-height: 0;
  overflow: auto;
  padding-right: 2px;
  padding-top: 2px;
}

.history-entry {
  cursor: pointer;
  padding-left: 26px;
}

.history-entry::before,
.history-entry::after {
  left: 13px;
  width: 1px;
}

.history-entry__marker {
  left: 8px;
  top: 18px;
  width: 10px;
  height: 10px;
  border-width: 2px;
}

.history-entry__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 42px;
}

.history-entry__main {
  min-width: 0;
  flex: 1;
}

.history-entry__value-line {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: nowrap;
}

.history-entry__meta-line {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 3px;
  font-size: 11px;
}

.history-entry__cause {
  color: #64748b;
  font-weight: 500;
}

.history-entry__quality {
  font-weight: 600;
}

.history-entry__quality.is-good {
  color: #16a34a;
}

.history-entry__quality.is-warning {
  color: #d97706;
}

.history-entry__quality.is-danger {
  color: #dc2626;
}

.history-entry__value {
  font-size: 16px;
  font-weight: 800;
  line-height: 1.1;
  color: #0f172a;
}

.history-entry__value.is-on {
  color: #dc2626;
}

.history-entry__value.is-off {
  color: #16a34a;
}

.history-entry__value.is-transition {
  color: #d97706;
}

.history-entry__value.is-neutral {
  color: #0f172a;
}

.history-entry__value--compact {
  font-size: 16px;
  font-weight: 700;
}

.history-entry__previous {
  color: #94a3b8;
  font-size: 14px;
  font-weight: 600;
}

.history-entry__arrow {
  color: #94a3b8;
  font-size: 15px;
  font-weight: 500;
}

.history-entry__state {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  background: #f1f5f9;
  color: #64748b;
  font-size: 11px;
  font-weight: 600;
}

.history-entry__state--snapshot {
  background: #f8fafc;
  color: #94a3b8;
}

.history-entry__time {
  text-align: right;
  flex-shrink: 0;
}

.history-entry__time-value {
  font-size: 12px;
  color: #475569;
  font-family: var(--font-mono);
  white-space: nowrap;
}

.history-entry__details {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 16px;
  margin-top: 5px;
  padding-top: 7px;
  border-top: 1px solid #e8eef5;
  font-size: 11px;
}

.history-entry__chevron {
  flex-shrink: 0;
  font-size: 18px;
  font-weight: 300;
  color: #c1c9d4;
  transition:
    transform 0.2s ease,
    color 0.2s ease;
  line-height: 1;
  user-select: none;
}

.history-entry.is-expanded .history-entry__chevron {
  transform: rotate(90deg);
  color: #64748b;
}

.history-entry:hover .history-entry__chevron {
  color: #94a3b8;
}

.history-entry.is-no-change .app-timeline__card {
  padding: 5px 10px;
}

.history-entry .app-timeline__card {
  gap: 5px;
  padding: 6px 10px;
}

.history-badge--truncate {
  max-width: 132px;
}

.history-badge__text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-entry.is-muted {
  opacity: 0.72;
}

.history-entry.is-muted:hover {
  opacity: 0.88;
}

.history-entry.is-expanded .app-timeline__card {
  background: #fafcff;
  border-color: #d6e2f2;
}

@media (max-width: 680px) {
  .history-summary-bar {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .history-summary-bar__item:nth-child(3) {
    border-left: 0;
  }
}

@media (max-width: 720px) {
  .history-entry__head {
    gap: 6px;
  }

  .history-entry__details {
    grid-template-columns: 1fr;
  }
}
</style>
