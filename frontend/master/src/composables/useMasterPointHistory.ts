import { computed, reactive, ref, type Ref } from 'vue'

import { ElMessage } from 'element-plus'

import {
  formatPointQualityLabelLocalized,
  formatSemanticPointValueLocalized,
  getIec104Capability,
  iec104TypeNameToId,
  resolveIec104TypeId,
  resolveIec104TypeName,
} from '@shared/api/iec104'
import type { BackendDataPoint } from '@shared/api/types'
import { resolveEffectiveQualityDetail } from '@shared/api/protocolQuality'
import { t } from '@shared/i18n'

import type {
  MasterContentTabRow as TabRow,
  MasterContentTabState as TabState,
} from '../types/masterContentPanel'
import type { SlaveConnection } from '../types/master'

type PointHistoryEntry = {
  id: string
  key: string
  profileId: string
  address: number
  name: string
  dataType: string
  valueText: string
  previousValueText: string | null
  qualityText: string
  previousQualityText: string | null
  causeText: string
  cp56Text: string
  pointTimestampText: string
  receivedAtText: string
  reportCountText: string
  primaryTimeLabel: string
  primaryTimeText: string
  semanticTitle: string
  hasValueChange: boolean
  isInitialSnapshot: boolean
  showReportCount: boolean
  qualityTone: 'good' | 'warning' | 'danger'
  valueTone: 'on' | 'off' | 'transition' | 'neutral'
}

type HistoryDialogState = {
  visible: boolean
  key: string
  pointName: string
  address: number
  dataType: string
  connectionName: string
  slaveName: string
  slaveCommonAddress: number | null
  expandedEntryId: string | null
}

interface UseMasterPointHistoryOptions {
  currentTab: Ref<TabState | null | undefined>
  selectedSlave: Ref<SlaveConnection | null | undefined>
  formatCot: (raw: unknown) => string
  formatOptionalCp56: (value: string | null | undefined) => string
  formatTimestamp: (value: string) => string
  isControlDataType: (dataType: string) => boolean
  qualityLevelFromCommon: (
    qualityCommon: BackendDataPoint['quality_common'],
    quality: BackendDataPoint['quality'],
    qualityDetail: ReturnType<typeof resolveEffectiveQualityDetail>,
  ) => 'good' | 'invalid' | 'questionable'
}

const POINT_HISTORY_LIMIT = 24

export function useMasterPointHistory(options: UseMasterPointHistoryOptions) {
  const pointHistoryMap = ref<Map<string, PointHistoryEntry[]>>(new Map())
  const pointHistorySignatures = new Map<string, string>()
  const pointHistoryLatestEntry = new Map<string, PointHistoryEntry>()

  const historyDialog = reactive<HistoryDialogState>({
    visible: false,
    key: '',
    pointName: '',
    address: 0,
    dataType: '',
    connectionName: '',
    slaveName: '',
    slaveCommonAddress: null,
    expandedEntryId: null,
  })

  const historyDialogEntries = computed(() =>
    historyDialog.key ? (pointHistoryMap.value.get(historyDialog.key) ?? []) : [],
  )
  const historyDialogLatestEntry = computed(() => historyDialogEntries.value[0] ?? null)
  const historyDialogCaText = computed(() =>
    historyDialog.slaveCommonAddress != null ? `CA ${historyDialog.slaveCommonAddress}` : 'CA —',
  )

  const buildPointHistoryKey = (
    profileId: string | number | null | undefined,
    address: number,
    dataType: string,
  ): string =>
    `${String(profileId ?? '')}:${Math.max(0, Math.trunc(Number(address) || 0))}:${String(dataType || '').trim()}`

  function openHistoryDialog(row: TabRow) {
    if (!row.historyKey) {
      ElMessage.info(t('master.pointHistory.noHistory'))
      return
    }
    historyDialog.visible = true
    historyDialog.key = row.historyKey
    historyDialog.pointName = row.name
    historyDialog.address = row.address
    historyDialog.dataType = row.dataType
    historyDialog.connectionName = options.selectedSlave.value?.linkName ?? ''
    historyDialog.slaveName =
      options.currentTab.value?.slaveName ?? t('master.pointHistory.slaveFallback')
    historyDialog.slaveCommonAddress = options.currentTab.value?.slaveCommonAddress ?? null
    historyDialog.expandedEntryId = '__first__'
  }

  const toggleHistoryEntry = (id: string) => {
    historyDialog.expandedEntryId = historyDialog.expandedEntryId === id ? null : id
  }

  const resolveHistoryPrimaryTime = (
    cp56Text: string,
    pointTimestampText: string,
    receivedAtText: string,
  ) => {
    if (cp56Text !== '—') {
      return {
        primaryTimeLabel: t('master.pointHistory.primaryTime.event'),
        primaryTimeText: cp56Text,
        sourceCp56Text: cp56Text,
        sourceTimestampText: pointTimestampText,
        receivedAtText,
      }
    }
    if (pointTimestampText !== '—') {
      return {
        primaryTimeLabel: t('master.pointHistory.primaryTime.source'),
        primaryTimeText: pointTimestampText,
        sourceCp56Text: cp56Text,
        sourceTimestampText: pointTimestampText,
        receivedAtText,
      }
    }
    return {
      primaryTimeLabel: t('master.pointHistory.primaryTime.received'),
      primaryTimeText: receivedAtText,
      sourceCp56Text: cp56Text,
      sourceTimestampText: pointTimestampText,
      receivedAtText,
    }
  }

  const resolveHistorySemanticTitle = (
    rawCause: unknown,
    isInitialSnapshot: boolean,
    hasValueChange: boolean,
  ): string => {
    if (isInitialSnapshot) return t('master.pointHistory.semantic.initialSnapshot')
    if (hasValueChange) return options.formatCot(rawCause)
    return t('master.pointHistory.semantic.metadataUpdated')
  }

  const resolveHistoryQualityTone = (
    level: 'good' | 'invalid' | 'questionable',
  ): 'good' | 'warning' | 'danger' => {
    if (level === 'good') return 'good'
    if (level === 'invalid') return 'danger'
    return 'warning'
  }

  const resolveHistoryValueTone = (
    dataType: string,
    value: unknown,
  ): 'on' | 'off' | 'transition' | 'neutral' => {
    const valueModel = getIec104Capability(iec104TypeNameToId(dataType))?.value_model
    const numericValue = Number(value)
    if (valueModel === 'boolean') {
      return numericValue === 1 ? 'on' : 'off'
    }
    if (valueModel === 'double_point') {
      if (numericValue === 1) return 'off'
      if (numericValue === 2) return 'on'
      return 'transition'
    }
    return 'neutral'
  }

  const buildPointHistoryEntry = (
    profileId: string,
    key: string,
    point: BackendDataPoint,
    dataType: string,
    observedAt: string,
    previousEntry: PointHistoryEntry | null,
  ): PointHistoryEntry => {
    const valueText = formatSemanticPointValueLocalized(dataType, point.value)
    const typeId = resolveIec104TypeId(point)
    const qualityDetail = resolveEffectiveQualityDetail(
      typeId,
      point.quality_detail,
      Number(point.quality ?? 0),
      point.value,
    )
    const qualityLevel = options.qualityLevelFromCommon(
      point.quality_common,
      point.quality,
      qualityDetail,
    )
    const qualityText = formatPointQualityLabelLocalized(
      typeId,
      point.quality_common,
      Number(point.quality ?? 0),
      qualityDetail,
    )
    const hasValueChange = previousEntry != null && previousEntry.valueText !== valueText
    const isInitialSnapshot = previousEntry == null
    const primaryTime = resolveHistoryPrimaryTime(
      options.formatOptionalCp56(point.latest_event_timestamp),
      options.formatTimestamp(String(point.timestamp ?? '')),
      options.formatTimestamp(observedAt),
    )

    return {
      id: `${key}:${observedAt}`,
      key,
      profileId,
      address: point.address,
      name: point.name,
      dataType,
      valueText,
      previousValueText: previousEntry?.valueText ?? null,
      qualityText,
      previousQualityText: previousEntry?.qualityText ?? null,
      causeText: options.formatCot(point.latest_cause),
      cp56Text: primaryTime.sourceCp56Text,
      pointTimestampText: primaryTime.sourceTimestampText,
      receivedAtText: primaryTime.receivedAtText,
      reportCountText: point.report_count == null ? '—' : String(point.report_count),
      primaryTimeLabel: primaryTime.primaryTimeLabel,
      primaryTimeText: primaryTime.primaryTimeText,
      semanticTitle: resolveHistorySemanticTitle(
        point.latest_cause,
        isInitialSnapshot,
        hasValueChange,
      ),
      hasValueChange,
      isInitialSnapshot,
      showReportCount: Number(point.report_count) > 1,
      qualityTone: resolveHistoryQualityTone(qualityLevel),
      valueTone: resolveHistoryValueTone(dataType, point.value),
    }
  }

  const recordMonitorPointHistory = (points: BackendDataPoint[]) => {
    let nextMap: Map<string, PointHistoryEntry[]> | null = null
    const observedAt = new Date().toISOString()

    for (const point of points) {
      const dataType = resolveIec104TypeName(point)
      if (!dataType || options.isControlDataType(dataType)) continue
      const ownerId = point.slave_id ?? null
      if (ownerId == null) continue

      const key = buildPointHistoryKey(ownerId, point.address, dataType)
      const signature = [
        point.timestamp,
        point.value,
        point.quality,
        point.latest_cause ?? '',
        point.report_count ?? '',
        point.latest_event_timestamp ?? '',
        point.control_status_snapshot?.command_cp56 ?? '',
      ].join('|')
      if (pointHistorySignatures.get(key) === signature) continue

      const previousEntry = pointHistoryLatestEntry.get(key) ?? null
      const entry = buildPointHistoryEntry(
        String(ownerId),
        key,
        point,
        dataType,
        observedAt,
        previousEntry,
      )

      if (!nextMap) nextMap = new Map(pointHistoryMap.value)
      nextMap.set(
        key,
        [entry, ...(nextMap.get(key) ?? pointHistoryMap.value.get(key) ?? [])].slice(
          0,
          POINT_HISTORY_LIMIT,
        ),
      )
      pointHistorySignatures.set(key, signature)
      pointHistoryLatestEntry.set(key, entry)
    }

    if (nextMap) {
      pointHistoryMap.value = nextMap
    }
  }

  return {
    buildPointHistoryKey,
    historyDialog,
    historyDialogCaText,
    historyDialogEntries,
    historyDialogLatestEntry,
    openHistoryDialog,
    pointHistoryMap,
    recordMonitorPointHistory,
    toggleHistoryEntry,
  }
}
