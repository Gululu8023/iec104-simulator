<template>
  <el-dialog
    v-model="innerVisible"
    width="820px"
    :before-close="handleBeforeClose"
    :close-on-click-modal="false"
    :close-on-press-escape="!applying"
    :show-close="!applying"
    :lock-scroll="false"
    class="app-dialog-shell app-point-table-import-dialog"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ dialogTitle }}</div>
          <div v-if="subtitleBadges.length > 0" class="app-dialog-subtitle-badges">
            <span
              v-for="(badge, index) in subtitleBadges"
              :key="`point-table-import-badge-${index}`"
              class="app-dialog-subtitle-badge"
            >
              {{ badge }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <div ref="dialogBodyRef" class="app-dialog-body app-dialog-scroll-shadow pti-dialog-body">
      <section class="conn-section">
        <div class="conn-section-title">{{ t('pointTableImport.sourceTitle') }}</div>
        <div class="app-dialog-field-grid--2 app-filled-form pti-source-form">
          <label class="app-dialog-field-label">{{ t('pointTableImport.targetObject') }}</label>
          <div class="app-dialog-field-control">
            <el-input
              v-if="hasSingleTarget"
              :model-value="selectedTargetOption?.label || t('pointTableImport.noTarget')"
              size="small"
              readonly
              class="app-dialog-input conn-inline-input"
            />
            <el-select
              v-else
              v-model="selectedTargetKey"
              size="small"
              class="app-dialog-input conn-inline-input"
              :placeholder="t('pointTableImport.targetPlaceholder')"
              :disabled="applying || previewing || !hasTargetOptions"
              popper-class="app-select-dropdown"
              fit-input-width
            >
              <el-option
                v-for="option in targetOptions"
                :key="option.key"
                :label="option.label"
                :value="option.key"
              />
            </el-select>
          </div>

          <label class="app-dialog-field-label">{{ t('pointTableImport.file') }}</label>
          <div class="app-dialog-field-control app-dialog-action-control pti-file-field">
            <el-input
              :model-value="selectedFileName || t('pointTableImport.noFile')"
              size="small"
              readonly
              class="app-dialog-input conn-inline-input"
              :title="selectedFilePath || t('pointTableImport.noFile')"
            />
            <el-button
              size="small"
              class="app-dialog-btn app-dialog-btn--outline"
              :disabled="applying || previewing || !hasTargetOptions"
              @click="chooseFilePath"
            >
              {{ t('pointTableImport.chooseFile') }}
            </el-button>
          </div>

          <label class="app-dialog-field-label">{{ t('pointTableImport.mode') }}</label>
          <div class="app-dialog-field-control">
            <el-select
              v-model="selectedMode"
              size="small"
              class="app-dialog-input conn-inline-input"
              :disabled="applying || previewing"
              popper-class="app-select-dropdown"
              fit-input-width
            >
              <el-option :label="t('pointTableImport.modes.appendOnly')" value="append_only" />
              <el-option :label="t('pointTableImport.modes.replaceAll')" value="replace_all" />
            </el-select>
          </div>
        </div>
        <div class="app-dialog-help pti-source-hint">{{ t('pointTableImport.formatHint') }}</div>
      </section>

      <section class="conn-section">
        <div class="conn-section-title">{{ t('pointTableImport.precheckTitle') }}</div>
        <div v-if="resultState === 'idle'" class="app-dialog-help pti-result-help">
          {{ t('pointTableImport.precheckIdle') }}
        </div>

        <div v-else-if="resultState === 'previewing'" class="pti-inline-status">
          <el-icon class="is-loading"><Loading /></el-icon>
          <span>{{ t('pointTableImport.precheckLoading') }}</span>
        </div>

        <div v-else class="pti-summary-inline">
          <span class="pti-summary-inline__status" :class="resultStatusClass">
            {{ resultStatusText }}
          </span>
          <span
            v-for="item in resultSummaryItems"
            :key="item.label"
            class="pti-summary-inline__item"
          >
            <span class="pti-summary-inline__label">{{ item.label }}</span>
            <strong class="pti-summary-inline__value">{{ item.value }}</strong>
          </span>
        </div>
      </section>

      <section v-if="activeRiskBanner" class="conn-section">
        <div class="conn-section-title">{{ t('pointTableImport.riskTitle') }}</div>
        <div class="pti-banner-stack">
          <div
            class="app-dialog-note pti-banner-note"
            :class="
              activeRiskSeverity === 'error' ? 'app-dialog-note--error' : 'app-dialog-note--warning'
            "
          >
            <el-icon>
              <CircleCloseFilled v-if="activeRiskSeverity === 'error'" />
              <WarningFilled v-else />
            </el-icon>
            <span class="pti-banner-note__summary">{{ activeRiskBanner }}</span>
          </div>
        </div>
      </section>

      <section v-if="preview" class="conn-section pti-preview-section">
        <div class="pti-section-header">
          <div class="conn-section-title pti-section-title">
            {{ t('pointTableImport.previewTitle') }}
          </div>
          <div class="pti-section-meta">
            {{
              t('pointTableImport.previewMeta', {
                total: preview.summary.normalized_points,
                preview: preview.points_preview.length,
              })
            }}
          </div>
        </div>
        <div class="app-dialog-help pti-preview-hint">
          {{ t('pointTableImport.previewLimitHint') }}
        </div>

        <div ref="previewTableWrapRef" class="app-dialog-table-wrap pti-preview-table-wrap">
          <el-table
            ref="previewTableRef"
            :data="previewTableRows"
            :row-key="getPreviewRowKey"
            :row-class-name="resolvePreviewRowClassName"
            :row-style="resolvePreviewRowStyle"
            :height="previewTableHeight"
            :fit="false"
            size="small"
            stripe
            class="app-dialog-table pti-preview-table"
            @sort-change="handlePreviewSortChange"
          >
            <el-table-column
              prop="address"
              label="IOA"
              width="76"
              sortable="custom"
              show-overflow-tooltip
            >
              <template #default="{ row }">
                <span v-if="!isPreviewSpacerRow(row)">{{ row.address }}</span>
              </template>
            </el-table-column>
            <el-table-column
              prop="name"
              :label="t('pointTableImport.columns.name')"
              min-width="210"
            >
              <template #default="{ row }">
                <div v-if="!isPreviewSpacerRow(row)" class="pti-point-name-cell" :title="row.name">
                  <span class="pti-point-name-cell__title">{{ row.name }}</span>
                  <div v-if="resolvePreviewRowFlags(row).length > 0" class="pti-point-flags">
                    <span
                      v-for="flag in resolvePreviewRowFlags(row)"
                      :key="`${row.address}-${flag}`"
                      class="app-tag-mono"
                    >
                      {{ flag }}
                    </span>
                  </div>
                </div>
              </template>
            </el-table-column>
            <el-table-column
              :label="t('pointTableImport.columns.type')"
              min-width="250"
              show-overflow-tooltip
            >
              <template #default="{ row }">
                <span v-if="!isPreviewSpacerRow(row)">{{
                  formatIec104TypeLabel(row.type_id)
                }}</span>
              </template>
            </el-table-column>
            <el-table-column
              v-if="hasAsduAliasPreview"
              :label="t('pointTableImport.columns.alias')"
              min-width="120"
            >
              <template #default="{ row }">
                <span
                  v-if="!isPreviewSpacerRow(row)"
                  class="pti-table-text"
                  :title="resolvePreviewRowAlias(row) || '—'"
                >
                  {{ resolvePreviewRowAlias(row) || '—' }}
                </span>
              </template>
            </el-table-column>
            <el-table-column
              prop="description"
              :label="t('pointTableImport.columns.description')"
              min-width="240"
            >
              <template #default="{ row }">
                <span
                  v-if="!isPreviewSpacerRow(row)"
                  class="pti-table-text"
                  :title="row.description || '—'"
                >
                  {{ row.description || '—' }}
                </span>
              </template>
            </el-table-column>
            <el-table-column
              prop="control_ioa"
              :label="t('pointTableImport.columns.mapping')"
              width="96"
            >
              <template #default="{ row }">
                <span
                  v-if="!isPreviewSpacerRow(row)"
                  class="pti-table-text"
                  :title="String(row.control_ioa ?? '—')"
                >
                  {{ row.control_ioa ?? '—' }}
                </span>
              </template>
            </el-table-column>
            <el-table-column
              prop="is_enabled"
              :label="t('pointTableImport.columns.enabled')"
              width="76"
            >
              <template #default="{ row }">
                <el-tag
                  v-if="!isPreviewSpacerRow(row)"
                  :type="row.is_enabled ? 'success' : 'info'"
                  effect="light"
                  size="small"
                >
                  {{ row.is_enabled ? t('pointTableImport.yes') : t('pointTableImport.no') }}
                </el-tag>
              </template>
            </el-table-column>
            <template #empty>
              <div class="app-dialog-help">{{ t('pointTableImport.emptyPreview') }}</div>
            </template>
          </el-table>
        </div>
      </section>
    </div>

    <template #footer>
      <div class="app-dialog-footer pti-dialog-footer">
        <div class="app-dialog-actions">
          <el-button class="app-btn-secondary" :disabled="applying" @click="requestCloseDialog">
            {{ lastAppliedResult ? t('pointTableImport.close') : t('pointTableImport.cancel') }}
          </el-button>
          <el-button
            type="primary"
            :loading="applying"
            :disabled="!canPrimaryAction"
            @click="handlePrimaryAction"
          >
            {{ primaryActionLabel }}
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { CircleCloseFilled, Loading, WarningFilled } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { formatIec104TypeLabel } from '@shared/api/iec104'

import {
  applyPointTableImport,
  buildPointTableImportTargetKey,
  previewPointTableImport,
  type PointTableImportTargetOption,
} from '@shared/api/pointTableImport'
import type {
  PointDef,
  PointTableImportApplyResult,
  PointTableImportNormalizedPayload,
  PointTableImportMode,
  PointTableImportPreview,
} from '@shared/api/types'
import { t } from '@shared/i18n'
import { translateApiError } from '@shared/i18n/errors'
import { confirmDangerousAction } from '@shared/ui/dialogConfirm'
import {
  useTableVirtualScroll,
  type VirtualSpacerRow,
} from '@shared/composables/useTableVirtualScroll'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'

const props = withDefaults(
  defineProps<{
    visible: boolean
    title?: string
    targets?: PointTableImportTargetOption[]
    initialTargetKey?: string | null
    normalizePayload?: (
      payload: PointTableImportNormalizedPayload,
      preview: PointTableImportPreview,
    ) => PointTableImportNormalizedPayload
  }>(),
  {
    targets: () => [],
    initialTargetKey: null,
  },
)

const emit = defineEmits<{
  'update:visible': [value: boolean]
  applied: [result: PointTableImportApplyResult]
}>()

const innerVisible = computed({
  get: () => props.visible,
  set: (value: boolean) => emit('update:visible', value),
})

const selectedTargetKey = ref('')
const selectedMode = ref<PointTableImportMode>('append_only')
const selectedFilePath = ref('')
const previewing = ref(false)
const applying = ref(false)
const previewFailureMessage = ref('')
const preview = ref<Awaited<ReturnType<typeof previewPointTableImport>> | null>(null)
const dialogBodyRef = ref<HTMLElement | null>(null)
const previewTableWrapRef = ref<HTMLElement | null>(null)
const previewTableRef = ref<any>(null)
const previewSortOrder = ref<'ascending' | 'descending' | null>(null)
const lastAppliedResult = ref<PointTableImportApplyResult | null>(null)
const previewRequestId = ref(0)
const PREVIEW_TABLE_MAX_HEIGHT = 280
const previewTableHeight = ref(PREVIEW_TABLE_MAX_HEIGHT)
let previewLayoutObserver: ResizeObserver | null = null

const dialogTitle = computed(() => props.title || t('pointTableImport.defaultTitle'))
const targetOptions = computed(() => props.targets ?? [])
const hasTargetOptions = computed(() => targetOptions.value.length > 0)
const hasSingleTarget = computed(() => targetOptions.value.length <= 1)
const selectedTargetOption = computed(
  () => targetOptions.value.find((option) => option.key === selectedTargetKey.value) ?? null,
)
const canApply = computed(() =>
  Boolean(
    preview.value?.can_apply && selectedTargetOption.value && !previewing.value && !applying.value,
  ),
)
const canPrimaryAction = computed(() => {
  if (lastAppliedResult.value) return !applying.value
  return canApply.value
})
const primaryActionLabel = computed(() =>
  lastAppliedResult.value
    ? t('pointTableImport.finishImport')
    : selectedMode.value === 'replace_all'
      ? t('pointTableImport.overwriteImport')
      : t('pointTableImport.appendImport'),
)
const hasAsduAliasPreview = computed(
  () => Object.keys(preview.value?.asdu_aliases_preview ?? {}).length > 0,
)
const previewSourceRows = computed<PointDef[]>(() => {
  const rows = preview.value?.points_preview ?? []
  if (!previewSortOrder.value) return rows
  const direction = previewSortOrder.value === 'descending' ? -1 : 1
  return [...rows].sort((left, right) => (left.address - right.address) * direction)
})
const previewRowHeight = computed(() => 40)
const previewVirtualScroll = useTableVirtualScroll<PointDef>({
  tableRef: previewTableRef,
  sourceRows: previewSourceRows,
  rowHeight: previewRowHeight,
  threshold: 50,
  overscan: 6,
})
const previewTableRows = computed(() => previewVirtualScroll.renderRows.value)
const isPreviewSpacerRow = (row: unknown): row is VirtualSpacerRow =>
  previewVirtualScroll.isVirtualSpacerRow(row)
const getPreviewRowKey = (row: PointDef | VirtualSpacerRow) =>
  isPreviewSpacerRow(row) ? row.__virtualKey : `${row.address}:${row.type_id}:${row.name}`
const resolvePreviewRowClassName = ({ row }: { row: PointDef | VirtualSpacerRow }) =>
  previewVirtualScroll.resolveRowClassName(row)
const resolvePreviewRowStyle = ({ row }: { row: PointDef | VirtualSpacerRow }) =>
  previewVirtualScroll.resolveRowStyle(row)
const handlePreviewSortChange = (payload: { order?: 'ascending' | 'descending' | null }) => {
  previewSortOrder.value = payload.order ?? null
}
const selectedFileName = computed(() => {
  const normalizedPath = selectedFilePath.value.replace(/\\/g, '/').trim()
  if (!normalizedPath) return ''
  return normalizedPath.split('/').pop() ?? normalizedPath
})
const selectedFileFormat = computed(() => {
  if (preview.value?.format) return preview.value.format.toUpperCase()
  const ext = selectedFileName.value.split('.').pop()?.trim().toUpperCase()
  return ext && ['XML', 'CSV', 'JSON'].includes(ext) ? ext : ''
})
const selectedFileMeta = computed(() => {
  const parts: string[] = []
  if (selectedFileFormat.value) parts.push(selectedFileFormat.value)
  if (preview.value?.encoding) parts.push(preview.value.encoding)
  if (preview.value?.file_size) parts.push(formatFileSize(preview.value.file_size))
  return parts.join(' · ')
})
const subtitleBadges = computed(() => {
  const explicitBadges = (selectedTargetOption.value?.subtitleBadges ?? [])
    .map((item) => String(item || '').trim())
    .filter((item) => item.length > 0)
  if (explicitBadges.length > 0) return explicitBadges

  const targetLabel = String(selectedTargetOption.value?.label || '').trim()
  return targetLabel ? [targetLabel] : []
})
const resultState = computed<'idle' | 'previewing' | 'ready' | 'error' | 'done'>(() => {
  if (lastAppliedResult.value) return 'done'
  if (previewing.value) return 'previewing'
  if (previewFailureMessage.value) return 'error'
  if (!preview.value) return 'idle'
  return preview.value.can_apply ? 'ready' : 'error'
})
const resultStatusText = computed(() => {
  switch (resultState.value) {
    case 'previewing':
      return t('pointTableImport.statuses.previewing')
    case 'ready':
      return t('pointTableImport.statuses.ready')
    case 'done':
      return t('pointTableImport.statuses.done')
    case 'error':
      return t('pointTableImport.statuses.error')
    default:
      return t('pointTableImport.statuses.idle')
  }
})
const resultPointCountText = computed(() => {
  if (lastAppliedResult.value) return String(lastAppliedResult.value.imported_points)
  if (preview.value) return String(preview.value.summary.normalized_points)
  return '—'
})
const resultStatusClass = computed(() => ({
  'is-ready': resultState.value === 'ready',
  'is-error': resultState.value === 'error',
  'is-done': resultState.value === 'done',
}))
const resultSummaryItems = computed(() => {
  const items: Array<{ label: string; value: string }> = []

  if (resultPointCountText.value !== '—') {
    items.push({
      label: t('pointTableImport.summary.importedPoints'),
      value: resultPointCountText.value,
    })
  }

  if (preview.value) {
    items.push({
      label: t('pointTableImport.summary.existingPoints'),
      value: String(preview.value.summary.existing_points_count),
    })
    items.push({
      label: t('pointTableImport.summary.warnings'),
      value: String(preview.value.summary.warning_count),
    })
    items.push({
      label: t('pointTableImport.summary.file'),
      value: selectedFileMeta.value || preview.value.format.toUpperCase(),
    })
  }

  return items
})
const errorBanner = computed<string | null>(() => {
  if (previewFailureMessage.value) {
    return t('pointTableImport.errors.fileNotImportable', { message: previewFailureMessage.value })
  }

  const issues = preview.value?.errors ?? []
  if (issues.length === 0) return null

  return t('pointTableImport.errors.blockingIssues', {
    count: issues.length,
    message: issues[0]?.message ?? t('pointTableImport.errors.fixBlockingIssues'),
  })
})
const warningBanner = computed<string | null>(() => {
  const issues = preview.value?.warnings ?? []
  if (issues.length === 0) return null

  const hasAliasWarning =
    issues.some((item) => item.code.includes('ASDU_ALIAS')) &&
    (preview.value?.summary.asdu_alias_count ?? 0) > 0

  return hasAliasWarning
    ? t('pointTableImport.errors.aliasWarning', {
        count: issues.length,
        aliasCount: preview.value?.summary.asdu_alias_count ?? 0,
      })
    : t('pointTableImport.errors.compatibilityWarning', {
        count: issues.length,
        message: issues[0]?.message ?? t('pointTableImport.errors.warningFallback'),
      })
})
const activeRiskSeverity = computed<'error' | 'warning' | null>(() => {
  if (errorBanner.value) return 'error'
  if (warningBanner.value) return 'warning'
  return null
})
const activeRiskBanner = computed(() => errorBanner.value || warningBanner.value || '')

const readPixelValue = (value: string): number => {
  const parsed = Number.parseFloat(value)
  return Number.isFinite(parsed) ? parsed : 0
}

const updatePreviewTableHeight = () => {
  const body = dialogBodyRef.value
  const tableWrap = previewTableWrapRef.value
  if (!body || !tableWrap || !preview.value) return

  const bodyStyle = window.getComputedStyle(body)
  const tableWrapChromeHeight = Math.max(0, tableWrap.offsetHeight - tableWrap.clientHeight)
  const reservedHeight = readPixelValue(bodyStyle.paddingBottom) + tableWrapChromeHeight
  const availableHeight = Math.floor(
    body.getBoundingClientRect().bottom - tableWrap.getBoundingClientRect().top - reservedHeight,
  )

  previewTableHeight.value = Math.max(1, Math.min(PREVIEW_TABLE_MAX_HEIGHT, availableHeight))
}

const syncPreviewLayout = async () => {
  await nextTick()
  previewLayoutObserver?.disconnect()

  const body = dialogBodyRef.value
  if (!innerVisible.value || !preview.value || !body) {
    previewTableHeight.value = PREVIEW_TABLE_MAX_HEIGHT
    return
  }

  updatePreviewTableHeight()
  if (typeof ResizeObserver !== 'undefined') {
    previewLayoutObserver = new ResizeObserver(updatePreviewTableHeight)
    previewLayoutObserver.observe(body)
  }
}

watch([innerVisible, preview, activeRiskBanner], () => void syncPreviewLayout(), { flush: 'post' })

onBeforeUnmount(() => {
  previewLayoutObserver?.disconnect()
})

const formatFileSize = (value: number): string => {
  if (!Number.isFinite(value) || value <= 0) return '0 B'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}

const toDisplayErrorMessage = (error: unknown): string => {
  const translated = translateApiError(error, '')
  if (translated) return translated
  if (error instanceof Error) {
    return error.message || String(error)
  }
  return typeof error === 'string' ? error : String(error)
}

const resolvePreviewRowFlags = (row: PointDef): string[] => {
  const flags: string[] = []
  if (row.control_ioa != null) flags.push(t('pointTableImport.flags.mapped'))
  if (!row.is_enabled) flags.push(t('pointTableImport.flags.disabled'))
  if (!String(row.description ?? '').trim())
    flags.push(t('pointTableImport.flags.missingDescription'))
  return flags
}

const resolvePreviewRowAlias = (row: PointDef): string => {
  const dataType = String(row.data_type ?? '').trim()
  if (!dataType) return ''
  return String(preview.value?.asdu_aliases_preview?.[dataType] ?? '').trim()
}

const collapseDetailPanels = () => {}

const invalidatePreviewRequests = () => {
  previewRequestId.value += 1
}

const resetDialogState = () => {
  selectedFilePath.value = ''
  selectedMode.value = 'append_only'
  preview.value = null
  previewSortOrder.value = null
  previewFailureMessage.value = ''
  previewing.value = false
  applying.value = false
  lastAppliedResult.value = null
  collapseDetailPanels()
}

const syncSelectedTarget = () => {
  const preferredKey = props.initialTargetKey
  if (preferredKey && targetOptions.value.some((option) => option.key === preferredKey)) {
    selectedTargetKey.value = preferredKey
    return
  }
  if (targetOptions.value.some((option) => option.key === selectedTargetKey.value)) {
    return
  }
  selectedTargetKey.value = targetOptions.value[0]?.key ?? ''
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      syncSelectedTarget()
      return
    }
    invalidatePreviewRequests()
    resetDialogState()
    syncSelectedTarget()
  },
  { immediate: true },
)

watch(
  () => [props.targets, props.initialTargetKey] as const,
  () => {
    syncSelectedTarget()
  },
  { deep: true },
)

watch(selectedTargetKey, (nextKey, previousKey) => {
  if (!props.visible || !selectedFilePath.value || !nextKey || nextKey === previousKey) return
  lastAppliedResult.value = null
  collapseDetailPanels()
  void runPreviewForCurrentFile()
})

watch(selectedMode, (nextMode, previousMode) => {
  if (!props.visible || !selectedFilePath.value || nextMode === previousMode) return
  lastAppliedResult.value = null
  void runPreviewForCurrentFile()
})

const chooseFilePath = async () => {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [
      {
        name: t('pointTableImport.fileFilterName'),
        extensions: ['xml', 'csv', 'json'],
      },
    ],
  })

  if (Array.isArray(selected) || !selected) return

  selectedFilePath.value = String(selected)
  lastAppliedResult.value = null
  collapseDetailPanels()
  await runPreviewForCurrentFile()
}

const runPreviewForCurrentFile = async () => {
  const targetOption = selectedTargetOption.value
  if (!targetOption) {
    previewFailureMessage.value = t('pointTableImport.errors.chooseTargetFirst')
    preview.value = null
    return
  }
  if (!selectedFilePath.value) {
    previewFailureMessage.value = t('pointTableImport.errors.chooseFileFirst')
    preview.value = null
    return
  }

  const requestId = previewRequestId.value + 1
  previewRequestId.value = requestId
  previewing.value = true
  previewFailureMessage.value = ''
  preview.value = null
  collapseDetailPanels()

  try {
    const result = await previewPointTableImport(
      selectedFilePath.value,
      targetOption.target,
      selectedMode.value,
    )
    if (previewRequestId.value !== requestId) return
    preview.value = result
  } catch (error) {
    if (previewRequestId.value !== requestId) return
    previewFailureMessage.value = t('pointTableImport.errors.precheckFailed', {
      error: toDisplayErrorMessage(error),
    })
  } finally {
    if (previewRequestId.value === requestId) {
      previewing.value = false
    }
  }
}

const requestApply = async () => {
  const targetOption = selectedTargetOption.value
  const previewResult = preview.value
  if (!targetOption || !previewResult || !previewResult.can_apply) return

  if (selectedMode.value === 'replace_all') {
    const confirmed = await confirmDangerousAction(
      t('pointTableImport.confirm.message', {
        existing: previewResult.summary.existing_points_count,
        incoming: previewResult.summary.normalized_points,
      }),
      t('pointTableImport.confirm.title'),
      t('pointTableImport.confirm.confirm'),
    )
    if (!confirmed) return
  }

  applying.value = true
  try {
    const normalizedPayload = props.normalizePayload
      ? props.normalizePayload(previewResult.normalized_payload, previewResult)
      : previewResult.normalized_payload
    const result = await applyPointTableImport(
      targetOption.target,
      normalizedPayload,
      selectedMode.value,
    )
    lastAppliedResult.value = result
    emit('applied', result)
  } catch (error) {
    ElMessage.error(
      t('pointTableImport.errors.importFailed', { error: toDisplayErrorMessage(error) }),
    )
  } finally {
    applying.value = false
  }
}

const handlePrimaryAction = async () => {
  if (lastAppliedResult.value) {
    requestCloseDialog()
    return
  }
  await requestApply()
}

const closeDialog = () => {
  invalidatePreviewRequests()
  innerVisible.value = false
}

const { requestClose: requestCloseDialog, handleBeforeClose } = useDialogCloseGuard({
  isBlocked: () => applying.value,
  onClose: closeDialog,
})

defineExpose({
  reset: resetDialogState,
  buildTargetKey: buildPointTableImportTargetKey,
})
</script>

<style scoped>
.pti-preview-table :deep(.virtual-spacer-row > td.el-table__cell) {
  padding: 0 !important;
  border-bottom: 0 !important;
  background: transparent !important;
}

.pti-preview-table :deep(.virtual-spacer-row .cell) {
  padding: 0 !important;
  min-height: 0 !important;
  height: 100% !important;
  visibility: hidden;
}
</style>
