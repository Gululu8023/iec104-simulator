<template>
  <div class="app-local-clock" :title="tooltipText">
    <el-icon class="app-local-clock__icon"><Clock /></el-icon>
    <time :datetime="isoTime">{{ displayTime }}</time>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { Clock } from '@element-plus/icons-vue'
import { t } from '@shared/i18n'

const props = withDefaults(
  defineProps<{
    offsetMs?: number
    label?: string
  }>(),
  {
    offsetMs: 0,
    label: '',
  },
)

const now = ref(new Date())
let timer: ReturnType<typeof setInterval> | undefined

const pad = (value: number) => String(value).padStart(2, '0')
const clockTime = computed(() => new Date(now.value.getTime() + props.offsetMs))

const displayTime = computed(() => {
  const value = clockTime.value
  return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())} ${pad(value.getHours())}:${pad(value.getMinutes())}:${pad(value.getSeconds())}`
})

const isoTime = computed(() => clockTime.value.toISOString())

const utcOffset = computed(() => {
  const totalMinutes = -clockTime.value.getTimezoneOffset()
  const sign = totalMinutes >= 0 ? '+' : '-'
  const absoluteMinutes = Math.abs(totalMinutes)
  return `UTC${sign}${pad(Math.floor(absoluteMinutes / 60))}:${pad(absoluteMinutes % 60)}`
})

const tooltipText = computed(() => {
  const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone
  return `${props.label || t('appMenu.localTime')} · ${timeZone} · ${utcOffset.value}`
})

onMounted(() => {
  timer = setInterval(() => {
    now.value = new Date()
  }, 1000)
})

onUnmounted(() => {
  if (timer !== undefined) clearInterval(timer)
})
</script>

<style scoped>
.app-local-clock {
  display: inline-flex;
  align-items: center;
  box-sizing: border-box;
  width: 174px;
  height: 18px;
  margin-left: 4px;
  padding-left: 12px;
  border-left: 1px solid var(--border-color);
  color: var(--text-secondary);
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  line-height: 18px;
  white-space: nowrap;
  user-select: none;
}

.app-local-clock__icon {
  flex: none;
  margin-right: 6px;
  font-size: 13px;
  color: var(--text-tertiary, var(--text-secondary));
}
</style>
