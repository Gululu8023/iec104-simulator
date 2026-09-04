<template>
  <el-dialog
    :model-value="visible"
    @update:model-value="handleVisibleUpdate"
    width="660px"
    :before-close="handleBeforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="point-edit-dialog app-dialog-shell"
    destroy-on-close
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t('slave.addressMapping.title') }}</div>
          <div class="app-dialog-subtitle-badges" v-if="resolvedSubtitleBadges.length > 0">
            <span
              v-for="(badge, index) in resolvedSubtitleBadges"
              :key="`mapping-subtitle-${index}`"
              class="app-dialog-subtitle-badge"
            >
              {{ badge }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body app-dialog-scroll-shadow conn-flat-body">
      <!-- 统一的映射UI -->
      <div class="mapping-container batch-mode">
        <el-form size="small" class="app-dialog-form mapping-form" @submit.prevent>
          <section class="conn-section">
            <div class="conn-section-title">{{ t('slave.addressMapping.sectionTitle') }}</div>
            <div class="app-dialog-field-grid--2 mapping-grid">
              <label class="app-dialog-field-label">{{
                t('slave.addressMapping.sourceType')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control">
                <el-select
                  :model-value="sourceDataType"
                  @update:model-value="$emit('update:sourceDataType', $event)"
                  class="app-dialog-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                  :disabled="!globalMode"
                >
                  <el-option
                    v-for="type in availableSourceTypes"
                    :key="type"
                    :label="formatTypeLabel(type)"
                    :value="type"
                  />
                </el-select>
              </el-form-item>
              <label class="app-dialog-field-label">{{
                t('slave.addressMapping.targetType')
              }}</label>
              <el-form-item label-width="0" class="app-dialog-field-control mapping-config-item">
                <el-select
                  v-model="targetDataType"
                  class="app-dialog-input"
                  popper-class="app-select-dropdown"
                  fit-input-width
                  :disabled="availableTargetTypes.length <= 1"
                >
                  <el-option
                    v-for="type in availableTargetTypes"
                    :key="type.value"
                    :label="formatTypeLabel(type.value)"
                    :value="type.value"
                  />
                </el-select>
                <div
                  class="app-dialog-help mapping-config-hint"
                  v-if="availableTargetTypes.length === 0"
                >
                  {{ t('slave.addressMapping.noTargetType') }}
                </div>
              </el-form-item>
            </div>
          </section>

          <template v-if="targetDataType">
            <section class="conn-section mapping-preview-section">
              <div class="conn-section-title-bar">
                <div class="conn-section-title-heading">
                  <span class="conn-section-title">{{
                    t('slave.addressMapping.previewTitle')
                  }}</span>
                  <span
                    class="conn-section-meta-pill"
                    :class="{ 'conn-section-meta-pill--warning': hasNoSourcePoints }"
                  >
                    {{
                      hasNoSourcePoints
                        ? t('slave.addressMapping.selected.empty')
                        : t('slave.addressMapping.selected.count', { count: selectedPointCount })
                    }}
                  </span>
                </div>
                <div class="mapping-section-actions">
                  <button
                    type="button"
                    class="app-dialog-link-action"
                    :disabled="!targetDataType || !targetPointsExists"
                    @click="batchFill"
                  >
                    <el-icon class="mr-1"><CircleCheck /></el-icon>
                    {{ t('slave.addressMapping.batchFill') }}
                  </button>
                  <button
                    type="button"
                    class="app-dialog-link-action mapping-clear-action"
                    @click="clearBatchMapping"
                  >
                    <el-icon class="mr-1"><Remove /></el-icon>
                    {{ t('slave.addressMapping.clearAll') }}
                  </button>
                </div>
              </div>

              <!-- 目标类型已有配置点提示 -->
              <div v-if="targetPointsExists" class="app-dialog-note app-dialog-note--info">
                <el-icon><InfoFilled /></el-icon>
                <span>{{
                  t('slave.addressMapping.existingTargetNote', {
                    type: formatTypeLabel(targetDataType),
                  })
                }}</span>
              </div>

              <!-- 目标类型无配置点提示 -->
              <div v-else class="app-dialog-note app-dialog-note--info">
                <el-icon><InfoFilled /></el-icon>
                <span>{{
                  t('slave.addressMapping.noTargetNote', {
                    type: formatTypeLabel(targetDataType),
                  })
                }}</span>
              </div>

              <!-- 映射预览表格 -->
              <div class="preview-section-container">
                <div class="app-dialog-table-wrap">
                  <div class="preview-table-container">
                    <el-table
                      ref="previewTableRef"
                      :data="previewTableRows"
                      :row-key="getPreviewRowKey"
                      :row-class-name="resolvePreviewRowClassName"
                      :row-style="resolvePreviewRowStyle"
                      size="small"
                      max-height="180"
                      stripe
                      class="preview-table"
                    >
                      <el-table-column width="24" align="center" class-name="preview-status-col">
                        <template #default="{ row }">
                          <template v-if="!isPreviewSpacerRow(row)">
                            <el-tooltip
                              v-if="row.status === 'matched'"
                              :content="t('slave.addressMapping.matchedTooltip')"
                              placement="top"
                              :enterable="false"
                            >
                              <el-icon color="#67c23a"><CircleCheck /></el-icon>
                            </el-tooltip>
                            <el-tooltip
                              v-else-if="row.status === 'cleared'"
                              :content="t('slave.addressMapping.clearedTooltip')"
                              placement="top"
                              :enterable="false"
                            >
                              <el-icon color="#f56c6c"><Remove /></el-icon>
                            </el-tooltip>
                          </template>
                        </template>
                      </el-table-column>
                      <el-table-column
                        :label="t('slave.addressMapping.sourceIoa')"
                        width="84"
                        align="right"
                        header-align="right"
                        class-name="preview-source-ioa-col"
                      >
                        <template #default="{ row }">
                          <span v-if="!isPreviewSpacerRow(row)" class="preview-ioa">{{
                            formatIoaAddress(row.source.address)
                          }}</span>
                        </template>
                      </el-table-column>
                      <el-table-column
                        :label="t('slave.addressMapping.name')"
                        min-width="140"
                        show-overflow-tooltip
                      >
                        <template #default="{ row }">
                          <span v-if="!isPreviewSpacerRow(row)" class="preview-name">{{
                            row.source.name
                          }}</span>
                        </template>
                      </el-table-column>
                      <el-table-column width="24" align="center" class-name="preview-arrow-col">
                        <template #default="{ row }">
                          <el-icon v-if="!isPreviewSpacerRow(row)" color="#909399"
                            ><Right
                          /></el-icon>
                        </template>
                      </el-table-column>
                      <el-table-column
                        :label="targetAddressLabel"
                        min-width="100"
                        align="left"
                        header-align="left"
                      >
                        <template #default="{ row }">
                          <div v-if="!isPreviewSpacerRow(row)" class="target-ioa-cell">
                            <el-select
                              :model-value="row.targetIoa"
                              clearable
                              filterable
                              class="mapping-target-select"
                              popper-class="app-select-dropdown"
                              fit-input-width
                              :placeholder="t('slave.addressMapping.noMapping')"
                              @update:model-value="setRowTarget(row.source.address, $event)"
                              @visible-change="setActiveTargetSelect(row.source.address, $event)"
                            >
                              <el-option
                                v-for="target in targetOptionsForRow(row)"
                                :key="`${target.commonAddress ?? ''}:${target.address}`"
                                :label="formatTargetOption(target)"
                                :value="target.address"
                                :disabled="isTargetDisabled(target, row.source.address)"
                              />
                            </el-select>
                          </div>
                        </template>
                      </el-table-column>
                      <el-table-column width="44" align="center" class-name="preview-actions-col">
                        <template #default="{ row }">
                          <div
                            v-if="
                              !isPreviewSpacerRow(row) &&
                              (row.targetIoa || row.status === 'cleared')
                            "
                            class="row-actions"
                          >
                            <el-tooltip
                              v-if="row.status !== 'cleared'"
                              :content="t('slave.addressMapping.clearRow')"
                              placement="top"
                              :enterable="false"
                            >
                              <el-icon
                                class="app-hover-action app-hover-action--danger"
                                @click="excludeFromBatch(row)"
                                ><Delete
                              /></el-icon>
                            </el-tooltip>
                            <el-tooltip
                              v-else
                              :content="t('slave.addressMapping.recoverRow')"
                              placement="top"
                              :enterable="false"
                            >
                              <el-icon
                                class="app-hover-action app-hover-action--success"
                                @click="recoverInBatch(row)"
                                ><RefreshLeft
                              /></el-icon>
                            </el-tooltip>
                          </div>
                        </template>
                      </el-table-column>
                    </el-table>
                  </div>
                </div>
              </div>
            </section>
          </template>
        </el-form>
      </div>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" @click="requestClose">{{
            t('common.cancel')
          }}</el-button>
          <el-button type="primary" @click="handleApply" :disabled="!canApply">{{
            t('common.save')
          }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import {
  InfoFilled,
  CircleCheck,
  Remove,
  Right,
  Delete,
  RefreshLeft,
} from '@element-plus/icons-vue'
import type { SlaveIoaDisplayFormat } from '@shared/api/types'
import { formatIec104TypeLabel, getIec104Capability, iec104TypeNameToId } from '@shared/api/iec104'
import { t } from '@shared/i18n'
import {
  useTableVirtualScroll,
  type VirtualSpacerRow,
} from '@shared/composables/useTableVirtualScroll'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'

const props = defineProps<{
  visible: boolean
  mode: 'single' | 'batch'
  globalMode?: boolean
  ioaDisplayFormat?: SlaveIoaDisplayFormat
  sourceDataType: string
  sourceTypeOptions?: string[]
  subtitleBadges?: string[]
  batchPoints?: any[]
  existingTargetPoints?: any[] // 当前从站内对应目标类型的所有测点 (用于智能匹配)
}>()

const emit = defineEmits<{
  'update:visible': [visible: boolean]
  'update:sourceDataType': [type: string]
  'apply-batch': [
    data: {
      updates: Array<{ address: number; commonAddress?: number; controlIoa: number | null }>
    },
  ]
}>()

const handleVisibleUpdate = (value: boolean) => emit('update:visible', value)

const IOA_HEX_WIDTH = 6

const formatIoaAddress = (value: unknown): string => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return '--'
  const normalized = Math.max(0, Math.trunc(numeric))
  if (props.ioaDisplayFormat === 'hex') {
    return `0x${normalized.toString(16).toUpperCase().padStart(IOA_HEX_WIDTH, '0')}`
  }
  return String(normalized)
}

// 状态
const targetDataType = ref<string>('')
const targetBySource = ref<Map<number, number | null>>(new Map())
const initialTargetBySource = ref<Map<number, number | null>>(new Map())
const dialogSnapshot = ref('')
const activeTargetSelectSource = ref<number | null>(null)

// 计算属性
const formatTypeLabel = (type: string): string => formatIec104TypeLabel(iec104TypeNameToId(type))
const compareTypesById = (left: string, right: string): number =>
  (iec104TypeNameToId(left) ?? Number.MAX_SAFE_INTEGER) -
    (iec104TypeNameToId(right) ?? Number.MAX_SAFE_INTEGER) || left.localeCompare(right)

const sourceTypeName = computed(() => formatTypeLabel(props.sourceDataType))
const resolvedSubtitleBadges = computed(() => {
  if (props.subtitleBadges && props.subtitleBadges.length > 0) {
    return props.subtitleBadges.filter((item) => String(item || '').trim().length > 0)
  }

  const badges: string[] = []
  if (sourceTypeName.value) badges.push(sourceTypeName.value)
  badges.push(
    props.mode === 'single'
      ? t('slave.addressMapping.singleMapping')
      : t('slave.addressMapping.batchMapping'),
  )
  return badges
})
const availableSourceTypes = computed(() => {
  return [...(props.sourceTypeOptions ?? [props.sourceDataType])]
    .filter((type) => {
      const capability = getIec104Capability(iec104TypeNameToId(type))
      return capability?.point_role === 'control'
    })
    .sort(compareTypesById)
})

const availableTargetTypes = computed(() => {
  const types = new Set(
    (props.existingTargetPoints ?? []).map((point) => String(point.dataType || '')),
  )
  return [...types]
    .filter(Boolean)
    .sort(compareTypesById)
    .map((value) => ({
      value,
    }))
})
const targetAddressLabel = computed(() => t('slave.addressMapping.targetIoa'))

const targetPointsExists = computed(() => {
  return (props.existingTargetPoints?.length || 0) > 0
})

const selectedPointCount = computed(() => props.batchPoints?.length ?? 0)
const hasNoSourcePoints = computed(() => selectedPointCount.value === 0)

const canApply = computed(() => {
  return !hasNoSourcePoints.value && hasUnsavedChanges.value
})
const hasUnsavedChanges = computed(() => {
  if (!props.visible) return false
  if (!dialogSnapshot.value) return false
  return buildDialogSnapshot() !== dialogSnapshot.value
})

// 映射预览逻辑
interface PreviewRow {
  source: any
  targetIoa: number | null
  status: 'matched' | 'cleared' | 'none'
}

const normalizeMappingIoa = (value: unknown): number | null => {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  if (normalized < 1 || normalized > 16777215) return null
  return normalized
}

const sortedBatchPoints = computed(() =>
  [...(props.batchPoints ?? [])].sort((left, right) => left.address - right.address),
)

const previewList = computed<PreviewRow[]>(() => {
  return sortedBatchPoints.value.map((source) => {
    const targetIoa = targetBySource.value.get(source.address) ?? null
    const initialTarget = initialTargetBySource.value.get(source.address) ?? null
    return {
      source,
      targetIoa,
      status: targetIoa != null ? 'matched' : initialTarget != null ? 'cleared' : 'none',
    }
  })
})

const previewTableRef = ref<any>(null)
const previewRowHeight = computed(() => 36)
const previewVirtualScroll = useTableVirtualScroll<PreviewRow>({
  tableRef: previewTableRef,
  sourceRows: previewList,
  rowHeight: previewRowHeight,
  threshold: 30,
  overscan: 6,
})
const previewTableRows = computed(() => previewVirtualScroll.renderRows.value)
const isPreviewSpacerRow = (row: unknown): row is VirtualSpacerRow =>
  previewVirtualScroll.isVirtualSpacerRow(row)
const getPreviewRowKey = (row: PreviewRow | VirtualSpacerRow) =>
  isPreviewSpacerRow(row) ? row.__virtualKey : String(row.source.address)
const resolvePreviewRowClassName = ({ row }: { row: PreviewRow | VirtualSpacerRow }) =>
  previewVirtualScroll.resolveRowClassName(row)
const resolvePreviewRowStyle = ({ row }: { row: PreviewRow | VirtualSpacerRow }) =>
  previewVirtualScroll.resolveRowStyle(row)

const usedTargetIoa = computed(
  () => new Set([...targetBySource.value.values()].filter((ioa) => ioa != null)),
)
const targetPointByIoa = computed(
  () =>
    new Map(
      (props.existingTargetPoints ?? []).map((point) => [
        normalizeMappingIoa(point.address),
        point,
      ]),
    ),
)

const isTargetDisabled = (target: any, sourceAddress: number): boolean => {
  const targetIoa = normalizeMappingIoa(target.address)
  if (targetIoa == null) return true
  const persistedOwner = normalizeMappingIoa(target.controlIoa)
  if (
    persistedOwner != null &&
    persistedOwner !== sourceAddress &&
    (!targetBySource.value.has(persistedOwner) ||
      targetBySource.value.get(persistedOwner) === targetIoa)
  ) {
    return true
  }
  return usedTargetIoa.value.has(targetIoa) && targetBySource.value.get(sourceAddress) !== targetIoa
}

const formatTargetOption = (point: any): string => {
  return `${formatIoaAddress(point.address)} (${point.name})`
}

const targetOptionsForRow = (row: PreviewRow): any[] => {
  if (activeTargetSelectSource.value === row.source.address) {
    return props.existingTargetPoints ?? []
  }
  if (row.targetIoa == null) return []
  const selectedTarget = targetPointByIoa.value.get(row.targetIoa)
  return selectedTarget ? [selectedTarget] : []
}

const setActiveTargetSelect = (sourceAddress: number, visible: boolean) => {
  if (visible) {
    activeTargetSelectSource.value = sourceAddress
  } else if (activeTargetSelectSource.value === sourceAddress) {
    activeTargetSelectSource.value = null
  }
}

const setRowTarget = (sourceAddress: number, value: unknown) => {
  const next = new Map(targetBySource.value)
  next.set(sourceAddress, normalizeMappingIoa(value))
  targetBySource.value = next
}

const batchFill = () => {
  const targets = (props.existingTargetPoints ?? [])
    .filter((point) => !targetDataType.value || point.dataType === targetDataType.value)
    .sort((left, right) => left.address - right.address)
  const next = new Map(targetBySource.value)
  const used = new Set([...next.values()].filter((ioa) => ioa != null))
  for (const source of [...(props.batchPoints ?? [])].sort(
    (left, right) => left.address - right.address,
  )) {
    if (next.get(source.address) != null) continue
    const target = targets.find(
      (candidate) => !used.has(candidate.address) && !isTargetDisabled(candidate, source.address),
    )
    if (!target) break
    next.set(source.address, target.address)
    used.add(target.address)
  }
  targetBySource.value = next
}

// 监听弹窗打开和数据变化
watch(
  () => props.visible,
  (val) => {
    if (val) {
      resetState()
      dialogSnapshot.value = buildDialogSnapshot()
      return
    }
    dialogSnapshot.value = ''
  },
)

watch(
  () => props.sourceDataType,
  () => {
    if (!props.visible) return
    resetState()
  },
)

const resetState = () => {
  activeTargetSelectSource.value = null
  const initial = new Map<number, number | null>()
  for (const source of props.batchPoints ?? []) {
    initial.set(
      source.address,
      normalizeMappingIoa(source.mappedTargetIoa) ?? normalizeMappingIoa(source.controlIoa),
    )
  }
  initialTargetBySource.value = initial
  targetBySource.value = new Map(initial)
  targetDataType.value = availableTargetTypes.value[0]?.value ?? ''
}

const buildDialogSnapshot = () =>
  JSON.stringify({
    sourceDataType: props.sourceDataType,
    targetDataType: targetDataType.value,
    targets: [...targetBySource.value.entries()].sort(([left], [right]) => left - right),
  })

const clearBatchMapping = () => {
  targetBySource.value = new Map((props.batchPoints ?? []).map((point) => [point.address, null]))
}

const excludeFromBatch = (row: any) => {
  setRowTarget(row.source.address, null)
}

const recoverInBatch = (row: any) => {
  setRowTarget(row.source.address, initialTargetBySource.value.get(row.source.address) ?? null)
}

const performClose = () => {
  emit('update:visible', false)
}

const { requestClose, handleBeforeClose } = useDialogCloseGuard({
  isDirty: () => hasUnsavedChanges.value,
  onClose: performClose,
})

const handleApply = () => {
  const updates: Array<{ address: number; commonAddress?: number; controlIoa: number | null }> = []
  previewList.value.forEach((row) => {
    updates.push({
      address: row.source.address,
      commonAddress: row.source.commonAddress,
      controlIoa: row.targetIoa,
    })
  })

  emit('apply-batch', { updates })
}
</script>

<style scoped>
.mapping-container {
  padding: 0 4px;
  min-width: 0;
}

.point-info {
  display: flex;
  align-items: center;
  gap: 8px;
}
.point-name {
  font-weight: 500;
  color: var(--text-primary);
}
.point-ioa {
  color: var(--text-secondary);
  font-family: monospace;
}

.mapping-config-item {
  position: relative;
}

.mapping-config-hint {
  margin-top: 6px;
}

.mapping-grid {
  gap: 20px;
}

.mapping-form :deep(.el-form-item__content) {
  min-height: var(--dialog-control-min-height, 34px);
}

.mapping-form,
.mapping-preview-section,
.preview-section-container,
.app-dialog-table-wrap,
.preview-table-container {
  min-width: 0;
  max-width: 100%;
}

.mapping-grid--offset {
  margin-top: 16px;
}

.mapping-preview-section {
  margin-top: 16px;
}

.mapping-container .conn-section:last-of-type {
  margin-bottom: 0;
}

.mapping-section-actions {
  display: flex;
  align-items: center;
  margin-left: auto;
  gap: 16px;
}

.mapping-section-actions .app-dialog-link-action:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.mapping-clear-action {
  color: var(--danger-color, #dc2626);
}

.preview-section {
  border: 1px solid var(--border-color);
  border-radius: 4px;
  overflow: hidden;
}

.preview-table-container {
  background: white;
}

.preview-table {
  width: 100%;
  max-width: 100%;
}

/* 预览表格: 无边框线，参照首页面板表格样式 */
.preview-table :deep(.el-table__inner-wrapper::before),
.preview-table :deep(.el-table__border-left-patch),
.preview-table :deep(.el-table__border-bottom-patch) {
  display: none;
}

.preview-table :deep(td.el-table__cell),
.preview-table :deep(th.el-table__cell) {
  border-right: none !important;
}

.preview-table :deep(.el-table__inner-wrapper) {
  border: none;
}

.preview-table :deep(.el-table__header th) {
  background-color: #f9fafb !important;
  font-weight: 600;
  color: var(--text-secondary);
  font-size: var(--dialog-label-font-size, 13px);
}

.preview-table :deep(.el-table__body td) {
  padding: 6px 0;
}

.preview-table :deep(.virtual-spacer-row > td.el-table__cell) {
  padding: 0 !important;
  border-bottom: 0 !important;
  background: transparent !important;
}

.preview-table :deep(.virtual-spacer-row .cell) {
  padding: 0 !important;
  min-height: 0 !important;
  height: 100% !important;
  visibility: hidden;
}

.preview-table :deep(.preview-status-col .cell),
.preview-table :deep(.preview-arrow-col .cell),
.preview-table :deep(.preview-actions-col .cell) {
  padding-left: 0 !important;
  padding-right: 0 !important;
}

.preview-table :deep(.preview-source-ioa-col .cell) {
  padding-left: 4px !important;
  padding-right: 8px !important;
  white-space: nowrap;
}

.preview-ioa {
  color: var(--text-secondary);
  font-family: monospace;
  font-size: var(--dialog-label-font-size, 13px);
}
.preview-name {
  font-weight: 500;
}
.preview-target-ioa {
  color: var(--primary-color);
}
.preview-target-empty {
  color: var(--text-disabled);
  font-style: italic;
}

.target-ioa-cell {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  min-width: 0;
  min-height: 22px;
}

.mapping-target-select {
  width: 100%;
  max-width: 220px;
  min-width: 0;
}

.row-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.2s;
}

:deep(.el-table__row:hover) .row-actions {
  opacity: 1;
  pointer-events: auto;
}

:deep(.el-table__row:focus-within) .row-actions {
  opacity: 1;
  pointer-events: auto;
}

/* 给按钮加一点边距 */
.mr-1 {
  margin-right: 4px;
}

/* Layout uses standard conn-adaptive-grid */

/* 覆盖 el-form-item 默认边距，适应 flat-body */
.single-mode .el-form-item,
.batch-mode .el-form-item {
  margin-bottom: 0;
}

.mapping-grid .full-width-item {
  grid-column: 1 / -1;
}
</style>
