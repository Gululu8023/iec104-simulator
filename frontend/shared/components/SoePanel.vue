<template>
  <section class="soe-panel">
    <header class="panel-header">
      <div class="header-left">
        <el-icon><Bell /></el-icon><span class="panel-title">{{ t('soePanel.title') }}</span>
        <span class="panel-badge">{{ filteredEvents.length }} / {{ events.length }}</span>
        <span v-if="events.length >= MAX_EVENTS" class="panel-subtle">{{
          t('soePanel.cacheLimitReached')
        }}</span>
      </div>
      <div class="header-center">
        <input
          v-model="searchText"
          class="filter-input"
          :placeholder="t('soePanel.searchPlaceholder')"
        />
        <el-popover
          trigger="click"
          placement="bottom-start"
          popper-class="app-menu-dropdown message-panel-dropdown"
        >
          <template #reference
            ><button class="filter-btn" :class="{ active: typeFilter !== 'all' }">
              {{ t('soePanel.typeFilter') }} ▾
            </button></template
          >
          <div class="filter-options">
            <button
              v-for="option in typeOptions"
              :key="option.value"
              class="filter-option"
              :class="{ selected: typeFilter === option.value }"
              @click="typeFilter = option.value"
            >
              {{ option.label }}
            </button>
          </div>
        </el-popover>
      </div>
      <div class="header-right">
        <el-tooltip :content="t('soePanel.clear')" placement="bottom" :enterable="false"
          ><button class="icon-btn" @click="emit('clear')">
            <el-icon><Delete /></el-icon></button
        ></el-tooltip>
        <el-tooltip :content="t('soePanel.hide')" placement="bottom" :enterable="false"
          ><button class="icon-btn close-btn" @click="emit('hide')">
            <el-icon><Close /></el-icon></button
        ></el-tooltip>
      </div>
    </header>
    <div ref="listRef" class="soe-list" @scroll="handleScroll">
      <div
        v-if="filteredEvents.length"
        class="virtual-list"
        :style="{ height: `${filteredEvents.length * ROW_HEIGHT}px` }"
      >
        <div
          class="visible-window"
          :style="{ transform: `translateY(${visibleStart * ROW_HEIGHT}px)` }"
        >
          <button
            v-for="event in visibleEvents"
            :key="event.id"
            class="soe-item"
            :class="{ 'is-active': selectedId === event.id }"
            @click="handleSelect(event)"
          >
            <span class="marker" aria-hidden="true"></span>
            <span class="item-content">
              <span class="item-line top-line">
                <span class="time" :title="formatFullTimestamp(event)">{{
                  formatTimestamp(event.event_timestamp)
                }}</span>
                <span class="type" :title="formatType(event.type_id)">{{
                  formatType(event.type_id)
                }}</span>
                <span class="value" :class="valueClass(event)" :title="valueTitle(event)">{{
                  formatValue(event)
                }}</span>
                <span
                  class="quality"
                  :class="qualityPresentation(event).tone"
                  :title="qualityPresentation(event).title"
                  >{{ qualityPresentation(event).text }}</span
                >
              </span>
              <span class="item-line bottom-line">
                <span class="name" :title="event.point_name || `IOA ${event.ioa}`">{{
                  event.point_name || `IOA ${event.ioa}`
                }}</span>
                <span class="cause">{{ formatCause(event.cause) }}</span>
                <span class="source" :title="formatSource(event)">{{ formatSource(event) }}</span>
              </span>
            </span>
          </button>
        </div>
      </div>
      <div v-else class="empty">{{ emptyText }}</div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { Bell, Close, Delete } from '@element-plus/icons-vue'
import type { BackendSoeEvent } from '@shared/api/types'
import { formatIec104TypeCompactLabel, getIec104Capability } from '@shared/api/iec104'
import { buildLocalQualityCommon, resolveEffectiveQualityDetail } from '@shared/api/protocolQuality'
import { t } from '@shared/i18n'

const props = defineProps<{
  events: BackendSoeEvent[]
  stationList?: Array<{
    id: string
    name: string
    groupName?: string
    slaveName?: string
    isConnectionLevel?: boolean
    runtimeConnectionId?: string
    commonAddress?: number | null
  }>
  stationKind: 'master' | 'slave'
}>()
const emit = defineEmits<{ select: [event: BackendSoeEvent]; clear: []; hide: [] }>()
type SoeTypeFilter =
  | 'all'
  | 'single'
  | 'double'
  | 'step'
  | 'bitstring'
  | 'measurement'
  | 'integrated'
const MAX_EVENTS = 10_000,
  ROW_HEIGHT = 62,
  OVERSCAN = 8
const selectedId = ref<number | null>(null),
  searchText = ref(''),
  typeFilter = ref<SoeTypeFilter>('all')
const listRef = ref<HTMLElement | null>(null),
  scrollTop = ref(0),
  viewportHeight = ref(0)
let resizeObserver: ResizeObserver | null = null
const typeOptions = computed(() => [
  { value: 'all' as const, label: t('soePanel.typeOptions.all') },
  { value: 'single' as const, label: t('soePanel.typeOptions.single') },
  { value: 'double' as const, label: t('soePanel.typeOptions.double') },
  { value: 'step' as const, label: t('soePanel.typeOptions.step') },
  { value: 'bitstring' as const, label: t('soePanel.typeOptions.bitstring') },
  { value: 'measurement' as const, label: t('soePanel.typeOptions.measurement') },
  { value: 'integrated' as const, label: t('soePanel.typeOptions.integrated') },
])
const parseTimestamp = (value: string) => {
  const result = Date.parse(String(value))
  return Number.isFinite(result) ? result : Number.MIN_SAFE_INTEGER
}
const sortedEvents = computed(() =>
  props.events
    .slice()
    .sort(
      (a, b) =>
        parseTimestamp(b.event_timestamp) - parseTimestamp(a.event_timestamp) || b.id - a.id,
    ),
)
const filteredEvents = computed(() => {
  const search = searchText.value.trim().toLowerCase()
  return sortedEvents.value.filter(
    (event) =>
      matchesTypeFilter(event.type_id, typeFilter.value) &&
      (!search ||
        [
          event.point_name ?? '',
          event.common_address,
          event.ioa,
          event.connection_id,
          event.slave_id ?? '',
          formatType(event.type_id),
        ]
          .join(' ')
          .toLowerCase()
          .includes(search)),
  )
})
const visibleStart = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN),
)
const visibleEnd = computed(() =>
  Math.min(
    filteredEvents.value.length,
    Math.ceil((scrollTop.value + viewportHeight.value) / ROW_HEIGHT) + OVERSCAN,
  ),
)
const visibleEvents = computed(() =>
  filteredEvents.value.slice(visibleStart.value, visibleEnd.value),
)
watch([searchText, typeFilter], () => {
  scrollTop.value = 0
  if (listRef.value) listRef.value.scrollTop = 0
})

function matchesTypeFilter(typeId: number, filter: SoeTypeFilter) {
  if (filter === 'all') return true
  const model = getIec104Capability(typeId)?.value_model
  if (filter === 'single') return model === 'boolean'
  if (filter === 'double') return model === 'double_point'
  if (filter === 'step') return model === 'step_position'
  if (filter === 'bitstring') return model === 'bitstring32'
  if (filter === 'integrated') return model === 'counter'
  return model === 'normalized' || model === 'scaled_i16' || model === 'float32'
}
const emptyText = computed(() =>
  props.events.length === 0 ? t('soePanel.empty') : t('soePanel.emptyFiltered'),
)
function formatTimestamp(value: string) {
  const timestamp = Date.parse(String(value))
  if (!Number.isFinite(timestamp)) return '—'
  const date = new Date(timestamp),
    pad = (input: number, length = 2) => String(input).padStart(length, '0')
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`
}
function formatFullTimestamp(event: BackendSoeEvent) {
  return `${event.event_timestamp}${event.timestamp_detail?.invalid ? ` · ${t('soePanel.invalidTimestamp')}` : ''}`
}
const formatType = (typeId: number) => formatIec104TypeCompactLabel(typeId)
function formatValue(event: BackendSoeEvent) {
  const model = getIec104Capability(event.type_id)?.value_model
  if (model === 'boolean')
    return Number(event.value) >= 0.5
      ? t('soePanel.values.singleOn')
      : t('soePanel.values.singleOff')
  if (model === 'double_point')
    return [
      t('soePanel.values.doubleIntermediate'),
      t('soePanel.values.doubleOff'),
      t('soePanel.values.doubleOn'),
      t('soePanel.values.doubleUnknown'),
    ][Math.max(0, Math.min(3, Math.trunc(Number(event.value) || 0)))]
  if (model === 'step_position') {
    const step = Math.max(-64, Math.min(63, Math.trunc(Number(event.value) || 0))),
      transient = event.quality_detail?.kind === 'qds' && event.quality_detail.transient === true
    return `${t('soePanel.values.step', { value: step >= 0 ? `+${step}` : step })}${transient ? ' (T)' : ''}`
  }
  if (model === 'bitstring32')
    return `0x${(Math.trunc(Number(event.value)) >>> 0).toString(16).toUpperCase().padStart(8, '0')}`
  if (model === 'counter' || model === 'scaled_i16')
    return String(Math.trunc(Number(event.value) || 0))
  const numeric = Number(event.value)
  return Number.isFinite(numeric) ? numeric.toFixed(3).replace(/\.?0+$/, '') : '—'
}
function valueTitle(event: BackendSoeEvent) {
  const model = getIec104Capability(event.type_id)?.value_model
  if (model === 'bitstring32')
    return `${formatValue(event)} · ${Math.trunc(Number(event.value)) >>> 0}`
  if (
    model === 'step_position' &&
    event.quality_detail?.kind === 'qds' &&
    event.quality_detail.transient != null
  )
    return `${formatValue(event)} · ${t(event.quality_detail.transient ? 'iec104.quality.states.transient' : 'iec104.quality.states.stable')}`
  return formatValue(event)
}
function buildQualityPresentation(event: BackendSoeEvent) {
  const raw = Math.trunc(Number(event.quality) || 0) & 0xff
  const detail = resolveEffectiveQualityDetail(
      event.type_id,
      event.quality_detail,
      raw,
      event.value,
    ),
    flags: string[] = []
  const qualityModel = getIec104Capability(event.type_id)?.quality_model
  let danger = event.business_usable === false,
    warning = false
  if (detail.kind === 'bcr') {
    const { sq, cy, ca, iv } = detail
    flags.push(`SQ=${sq}`)
    if (cy) flags.push('CY')
    if (ca) flags.push('CA')
    if (iv) flags.push('IV')
    danger ||= iv
    warning ||= cy || ca
  } else {
    const common = event.quality_common ?? buildLocalQualityCommon(event.type_id, raw),
      ov = detail.kind === 'qds' && detail.ov,
      { iv, nt, sb, bl } = common
    if (ov && qualityModel === 'qds') flags.push('OV')
    if (iv) flags.push('IV')
    if (nt) flags.push('NT')
    if (sb) flags.push('SB')
    if (bl) flags.push('BL')
    danger ||= iv
    warning ||= Boolean((ov && qualityModel === 'qds') || nt || sb || bl)
  }
  if (event.timestamp_detail?.invalid) {
    flags.push(t('soePanel.timestampInvalidShort'))
    warning = true
  }
  const unavailable = event.business_usable === false,
    text = unavailable
      ? t('soePanel.qualityUnavailable')
      : flags.length
        ? flags.join(' · ')
        : t('soePanel.qualityGood')
  return {
    text,
    title: unavailable && flags.length ? `${text} · ${flags.join(' · ')}` : text,
    tone: danger ? 'is-danger' : warning ? 'is-warning' : 'is-good',
  }
}
const qualityPresentations = computed(
  () => new Map(filteredEvents.value.map((event) => [event, buildQualityPresentation(event)])),
)
const qualityPresentation = (event: BackendSoeEvent) =>
  qualityPresentations.value.get(event) ?? buildQualityPresentation(event)
function valueClass(event: BackendSoeEvent) {
  const model = getIec104Capability(event.type_id)?.value_model
  if (model === 'boolean') return Number(event.value) >= 0.5 ? 'is-on' : 'is-off'
  if (model === 'double_point') {
    const state = Math.trunc(Number(event.value) || 0)
    return state === 2
      ? 'is-on'
      : state === 1
        ? 'is-off'
        : state === 0
          ? 'is-transition'
          : 'is-neutral'
  }
  if (model === 'step_position') {
    const value = Math.trunc(Number(event.value) || 0)
    return value > 0 ? 'is-on' : value < 0 ? 'is-off' : 'is-transition'
  }
  return 'is-neutral'
}
const formatCause = (cause: number) => `COT ${Math.trunc(Number(cause) || 0) & 0x3f}`
function formatSource(event: BackendSoeEvent) {
  const base = `CA ${event.common_address} · IOA ${event.ioa}`
  if (props.stationKind === 'slave')
    return `${base} · ${event.slave_id != null ? t('soePanel.source.slave', { id: event.slave_id }) : t('soePanel.source.localSlave')}`
  const connectionId = String(event.connection_id ?? '').trim(),
    stations = props.stationList ?? []
  const slave = stations.find(
    (item) =>
      !item.isConnectionLevel &&
      String(item.runtimeConnectionId ?? '').trim() === connectionId &&
      Number(item.commonAddress ?? 0) === event.common_address,
  )
  const connection = stations.find(
    (item) =>
      item.isConnectionLevel && String(item.runtimeConnectionId ?? '').trim() === connectionId,
  )
  const connectionText = String(slave?.groupName ?? connection?.name ?? '').trim()
  const slaveText = String(
    slave?.slaveName ??
      slave?.name ??
      (event.slave_id != null
        ? t('soePanel.source.slave', { id: event.slave_id })
        : t('soePanel.source.unknownSlave')),
  ).trim()
  return `${base} · ${connectionText ? `${connectionText} · ` : ''}${slaveText}`
}
function handleScroll(event: Event) {
  scrollTop.value = (event.currentTarget as HTMLElement).scrollTop
}
function updateViewport() {
  viewportHeight.value = listRef.value?.clientHeight ?? 0
}
function handleSelect(event: BackendSoeEvent) {
  selectedId.value = event.id
  emit('select', event)
}
onMounted(() => {
  updateViewport()
  nextTick(updateViewport)
  if (typeof ResizeObserver !== 'undefined' && listRef.value) {
    resizeObserver = new ResizeObserver(updateViewport)
    resizeObserver.observe(listRef.value)
  }
})
onUnmounted(() => resizeObserver?.disconnect())
</script>

<style scoped>
.soe-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 430px;
  background: #fff;
  border-left: 1px solid #e5e7eb;
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  height: 36px;
  padding: 0 12px;
  border-top: 1px solid #e1e4e8;
  border-bottom: 1px solid #e5e7eb;
  background: #fafafa;
}
.header-left,
.header-center,
.header-right {
  display: flex;
  align-items: center;
}
.header-left {
  gap: 6px;
  flex-shrink: 0;
}
.header-center {
  gap: 8px;
  flex: 1;
  min-width: 0;
}
.header-right {
  gap: 4px;
  flex-shrink: 0;
}
.panel-title {
  font: 600 13px var(--font-mono);
  white-space: nowrap;
}
.panel-badge {
  padding: 1px 6px;
  border-radius: 10px;
  background: #e5e7eb;
  font: 500 11px var(--font-mono);
}
.panel-subtle {
  font-size: 11px;
  color: #b45309;
  white-space: nowrap;
}
.filter-input {
  flex: 1 1 140px;
  min-width: 90px;
  max-width: 220px;
  height: 26px;
  padding: 0 8px;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  font-size: 12px;
  outline: none;
}
.filter-input:focus {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.1);
}
.filter-btn,
.icon-btn {
  border: 0;
  background: transparent;
  color: #374151;
  cursor: pointer;
  border-radius: 4px;
}
.filter-btn {
  height: 28px;
  padding: 4px 10px;
  white-space: nowrap;
}
.filter-btn:hover,
.icon-btn:hover {
  background: #e5e7eb;
}
.filter-btn.active {
  color: var(--primary-color);
  background: #eff6ff;
}
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
}
.close-btn:hover {
  color: #dc2626 !important;
  background: #fef2f2 !important;
}
.filter-options {
  display: flex;
  flex-direction: column;
  min-width: 140px;
  padding: 4px 0;
}
.filter-option {
  position: relative;
  height: 30px;
  padding: 0 14px;
  border: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.filter-option::before {
  content: '';
  position: absolute;
  left: 0;
  top: 4px;
  bottom: 4px;
  width: 3px;
}
.filter-option:hover,
.filter-option.selected {
  background: var(--dropdown-selected-bg, #eff6ff);
}
.filter-option:hover::before,
.filter-option.selected::before {
  background: var(--primary-color);
}
.filter-option.selected {
  color: var(--primary-color);
  font-weight: 500;
}
.soe-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  background: linear-gradient(180deg, #fcfcfd, #f8fafc);
}
.virtual-list {
  position: relative;
  width: 100%;
}
.visible-window {
  position: absolute;
  inset: 0 0 auto;
  padding: 0 8px;
}
.soe-item {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
  height: 62px;
  padding: 4px 8px 4px 23px;
  border: 0;
  border-bottom: 1px solid #e8edf3;
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.soe-item::before {
  content: '';
  position: absolute;
  left: 14px;
  top: 0;
  bottom: 0;
  width: 1px;
  background: #dce8f5;
}
.marker {
  position: absolute;
  left: 10px;
  top: 25px;
  width: 9px;
  height: 9px;
  border: 2px solid #bfd5f3;
  border-radius: 50%;
  background: #f8fbff;
  z-index: 1;
}
.item-content {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  width: 100%;
  min-width: 0;
  height: 54px;
  padding: 4px 7px;
  border: 1px solid transparent;
  border-radius: 6px;
}
.soe-item:hover .item-content,
.soe-item:focus-visible .item-content {
  background: #f7faff;
  border-color: #dbe7f4;
}
.soe-item.is-active .item-content {
  background: #f3f8ff;
  border-color: #c5d9fb;
}
.soe-item.is-active .marker {
  border-color: #2563eb;
  background: #dbeafe;
}
.item-line {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  white-space: nowrap;
}
.top-line {
  height: 22px;
}
.bottom-line {
  height: 18px;
  color: #708096;
  font-size: 11px;
}
.time {
  flex: 0 0 auto;
  color: #0f172a;
  font: 500 11px var(--font-mono);
}
.type,
.value,
.quality {
  flex: 0 0 auto;
  display: block;
  max-width: 34%;
  overflow: hidden;
  text-overflow: ellipsis;
  padding: 2px 6px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 600;
}
.type {
  flex: 0 1 auto;
  background: #edf3ff;
  color: #345ec9;
}
.quality {
  max-width: 30%;
}
.value.is-on {
  background: #fee7e7;
  color: #c24141;
}
.value.is-off {
  background: #e7f7ee;
  color: #1e8a56;
}
.value.is-transition {
  background: #fff4db;
  color: #a16207;
}
.value.is-neutral {
  background: #eef2f7;
  color: #64748b;
}
.quality.is-good {
  background: #edf8f1;
  color: #4f7c61;
}
.quality.is-warning {
  background: #fff7e6;
  color: #9a6a17;
}
.quality.is-danger {
  background: #fceced;
  color: #b84a52;
}
.name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #0f172a;
  font-size: 12px;
  font-weight: 600;
}
.cause {
  flex: 0 0 auto;
  color: #526176;
  font-family: var(--font-mono);
}
.source {
  flex: 0 1 52%;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: right;
}
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 24px;
  color: #94a3b8;
  font-size: 12px;
}
</style>
