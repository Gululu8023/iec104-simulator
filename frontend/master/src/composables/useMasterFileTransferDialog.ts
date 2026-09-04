import { computed, inject, provide, ref, watch, type InjectionKey } from 'vue'
import { open, save } from '@tauri-apps/plugin-dialog'
import { dayjs } from 'element-plus'

import { getCurrentLocale, t } from '@shared/i18n'
import type {
  MasterFileTransferRequest,
  MasterFileTransferSession,
  MasterFileTransferState,
  MasterRemoteDirectoryEntry,
} from '@shared/api/types'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'

export interface MasterFileTransferDialogProps {
  visible: boolean
  connectionId: string | null
  connectionName?: string | null
  slaveId: number | null
  slaveName?: string | null
  session: MasterFileTransferSession | null
  singleSectionMaxFileSize?: number | null
  busy?: boolean
}

export type MasterFileTransferDialogEmit = {
  (event: 'update:visible', visible: boolean): void
  (event: 'start', request: MasterFileTransferRequest): void
  (event: 'cancel'): void
}

export function useMasterFileTransferDialog(
  props: MasterFileTransferDialogProps,
  emit: MasterFileTransferDialogEmit,
) {
  const innerVisible = ref(false)
  const dialogSnapshot = ref('')
  const activeTab = ref<'download' | 'upload'>('download')
  const directoryMode = ref<'directory' | 'log'>('directory')
  const hasFetchedDirectory = ref(false)
  const hasQueriedLog = ref(false)
  const lastDirectoryEntries = ref<MasterRemoteDirectoryEntry[]>([])
  const lastLogEntries = ref<MasterRemoteDirectoryEntry[]>([])
  const lastDownloadSession = ref<MasterFileTransferSession | null>(null)
  const lastUploadSession = ref<MasterFileTransferSession | null>(null)
  const shouldAcceptDirectoryResult = ref(false)
  const shouldAcceptLogResult = ref(false)
  const uploadNof = ref(1)
  const downloadPath = ref('')
  const uploadPath = ref('')
  const queryLogNof = ref(3)
  const queryStartDate = ref<Date | null>(null)
  const queryEndDate = ref<Date | null>(null)
  const selectedEntry = ref<MasterRemoteDirectoryEntry | null>(null)
  const dialogSubtitleBadges = computed(() =>
    [props.connectionName, props.slaveName]
      .map((item) => String(item || '').trim())
      .filter((item) => item.length > 0),
  )
  const sessionForCurrentConnection = computed<MasterFileTransferSession | null>(() => {
    if (!props.session || !props.connectionId) return null
    return props.session.connection_id === props.connectionId ? props.session : null
  })

  watch(
    () => props.visible,
    (val) => {
      innerVisible.value = val
      if (val) {
        resetQueryState()
        dialogSnapshot.value = buildDialogSnapshot()
      }
    },
    { immediate: true },
  )

  watch(innerVisible, (val) => {
    emit('update:visible', val)
  })

  watch(directoryMode, () => {
    selectedEntry.value = null
  })

  watch(
    () => [props.connectionId, props.slaveId] as const,
    () => {
      resetSessionState()
      dialogSnapshot.value = buildDialogSnapshot()
    },
  )

  watch(
    sessionForCurrentConnection,
    (session) => {
      if (!session) return
      if (session.mode === 'directory' && shouldAcceptDirectoryResult.value) {
        hasFetchedDirectory.value = true
        lastDirectoryEntries.value = session.directory_entries ?? []
      }
      if (session.mode === 'query-log' && shouldAcceptLogResult.value) {
        hasQueriedLog.value = true
        lastLogEntries.value = session.directory_entries ?? []
      }
      if (session.mode === 'download') lastDownloadSession.value = session
      if (session.mode === 'upload') lastUploadSession.value = session
    },
    { immediate: true },
  )

  const isRunning = computed(() => sessionForCurrentConnection.value?.state === 'running')
  const currentDownloadSession = computed<MasterFileTransferSession | null>(() =>
    lastDownloadSession.value?.connection_id === props.connectionId
      ? lastDownloadSession.value
      : null,
  )
  const currentUploadSession = computed<MasterFileTransferSession | null>(() =>
    lastUploadSession.value?.connection_id === props.connectionId ? lastUploadSession.value : null,
  )
  const busy = computed(() => Boolean(props.busy))
  const isRepeatedSuccessfulDownload = computed(() => {
    const session = currentDownloadSession.value
    if (session?.state !== 'success') return false
    if (!props.connectionId || session.connection_id !== props.connectionId) return false
    if (!selectedEntry.value || !downloadPath.value) return false
    return (
      session.nof === selectedEntry.value.nof && (session.local_path ?? '') === downloadPath.value
    )
  })
  const isRepeatedSuccessfulUpload = computed(() => {
    const session = currentUploadSession.value
    if (session?.state !== 'success') return false
    if (!props.connectionId || session.connection_id !== props.connectionId) return false
    if (!uploadPath.value) return false
    return session.nof === uploadNof.value && (session.local_path ?? '') === uploadPath.value
  })
  const hasValidQueryRange = computed(() => {
    if (!queryStartDate.value || !queryEndDate.value) return false
    return queryStartDate.value.getTime() <= queryEndDate.value.getTime()
  })
  const canStart = computed(
    () => Boolean(props.connectionId && props.slaveId) && !isRunning.value && !busy.value,
  )
  const canQueryDirectory = computed(() => canStart.value)
  const canQueryLog = computed(() => canStart.value && hasValidQueryRange.value)
  const canStartDownload = computed(
    () =>
      canStart.value &&
      Boolean(downloadPath.value) &&
      Boolean(selectedEntry.value) &&
      !isRepeatedSuccessfulDownload.value,
  )
  const canStartUpload = computed(
    () => canStart.value && Boolean(uploadPath.value) && !isRepeatedSuccessfulUpload.value,
  )
  const isCompletedPrimaryAction = computed(() =>
    activeTab.value === 'download'
      ? isRepeatedSuccessfulDownload.value
      : isRepeatedSuccessfulUpload.value,
  )
  const hasUnsavedChanges = computed(() => {
    if (!innerVisible.value) return false
    if (!dialogSnapshot.value) return false
    if (isRunning.value || busy.value) return false
    return buildDialogSnapshot() !== dialogSnapshot.value
  })
  const directoryEntries = computed<MasterRemoteDirectoryEntry[]>(() =>
    (directoryMode.value === 'directory'
      ? lastDirectoryEntries.value
      : lastLogEntries.value
    ).filter((entry) => entry.is_file),
  )
  const selectedEntryKey = computed(() => selectedEntry.value?.nof)
  const queryDateRange = computed<[Date, Date] | undefined>(() =>
    queryStartDate.value && queryEndDate.value
      ? [queryStartDate.value, queryEndDate.value]
      : undefined,
  )
  const dateRangeSeparator = computed(() => t('master.fileTransfer.placeholders.rangeSeparator'))

  watch(directoryEntries, (entries) => {
    if (!selectedEntry.value) return

    const nextSelected = entries.find((entry) => entry.nof === selectedEntry.value?.nof) ?? null
    selectedEntry.value = nextSelected
  })

  const primaryActionLabel = computed(() => {
    if (isCompletedPrimaryAction.value) return t('master.fileTransfer.actions.complete')
    return activeTab.value === 'download'
      ? t('master.fileTransfer.actions.download')
      : t('master.fileTransfer.actions.upload')
  })
  const cancelActionLabel = computed(() =>
    activeTab.value === 'download'
      ? t('master.fileTransfer.actions.cancel')
      : t('master.fileTransfer.actions.stop'),
  )
  const canPrimaryAction = computed(() => {
    if (busy.value) return false
    if (isCompletedPrimaryAction.value) return true
    return activeTab.value === 'download' ? canStartDownload.value : canStartUpload.value
  })
  const directoryActionLabel = computed(() =>
    hasFetchedDirectory.value
      ? t('master.fileTransfer.actions.refreshDirectory')
      : t('master.fileTransfer.actions.getDirectory'),
  )
  const directoryHelperText = computed(() =>
    hasFetchedDirectory.value
      ? t('master.fileTransfer.helper.fetched')
      : t('master.fileTransfer.helper.notFetched'),
  )
  const directoryEmptyTitle = computed(() => {
    if (directoryMode.value === 'directory')
      return hasFetchedDirectory.value
        ? t('master.fileTransfer.empty.noDownloadableFiles')
        : t('master.fileTransfer.empty.directoryNotFetched')
    return hasQueriedLog.value
      ? t('master.fileTransfer.empty.noMatchedFiles')
      : t('master.fileTransfer.empty.queryLogFirst')
  })
  const directoryEmptyHint = computed(() => {
    if (directoryMode.value === 'directory') {
      return hasFetchedDirectory.value
        ? t('master.fileTransfer.empty.fetchedNoFiles')
        : t('master.fileTransfer.empty.fetchDirectoryHint')
    }
    return hasQueriedLog.value
      ? t('master.fileTransfer.empty.refineQuery')
      : t('master.fileTransfer.empty.queryLogHint')
  })
  const selectedEntryMeta = computed(() => {
    const entry = selectedEntry.value
    if (!entry) return ''
    return [
      formatFileSize(entry.length),
      formatTimestamp(entry.timestamp) || t('master.fileTransfer.noTimestamp'),
    ].join(' · ')
  })
  const uploadRequirementHint = computed(() => {
    if (!props.connectionId || !props.slaveId)
      return t('master.fileTransfer.uploadHints.selectLinkFirst')
    if (busy.value) return t('master.fileTransfer.uploadHints.busy')
    if (!uploadPath.value) return t('master.fileTransfer.uploadHints.chooseFile')
    if (isRepeatedSuccessfulUpload.value) return t('master.fileTransfer.uploadHints.repeated')
    return t('master.fileTransfer.uploadHints.ready')
  })
  const singleSectionLimitText = computed(() =>
    props.singleSectionMaxFileSize == null
      ? ''
      : t('master.fileTransfer.uploadHints.singleSectionLimit', {
          size: formatFileSize(props.singleSectionMaxFileSize),
        }),
  )

  const handlePrimaryAction = () => {
    if (isCompletedPrimaryAction.value) {
      closeDialog()
      return
    }
    if (activeTab.value === 'download') startDownload()
    else startUpload()
  }

  // IEC 60870-5-104 / DL/T 634.5104 file name (NOF) mapping
  const FILE_TYPE_KEYS: Record<number, string> = {
    0: 'master.fileTransfer.fileTypes.default',
    1: 'master.fileTransfer.fileTypes.transparent',
    2: 'master.fileTransfer.fileTypes.disturbance',
    3: 'master.fileTransfer.fileTypes.soe',
    4: 'master.fileTransfer.fileTypes.analog',
  }

  const formatFileTitle = (nof: number): string =>
    FILE_TYPE_KEYS[nof] ? t(FILE_TYPE_KEYS[nof]) : String(nof)

  const nofSelectOptions = computed(() => {
    const values = new Set([
      ...Object.keys(FILE_TYPE_KEYS).map(Number),
      uploadNof.value,
      queryLogNof.value,
    ])
    return [...values].sort((a, b) => a - b).map((v) => ({ value: v, label: formatFileTitle(v) }))
  })

  const formatFileSize = (n: number): string => {
    if (n < 1024) return `${n} B`
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
    return `${(n / (1024 * 1024)).toFixed(2)} MB`
  }

  const STATE_LABELS: Record<MasterFileTransferState, string> = {
    idle: 'master.fileTransfer.state.idle',
    running: 'master.fileTransfer.state.running',
    success: 'master.fileTransfer.state.success',
    failed: 'master.fileTransfer.state.failed',
    cancelled: 'master.fileTransfer.state.cancelled',
  }

  const resolveTransferPanelSession = (
    mode: 'download' | 'upload',
  ): MasterFileTransferSession | null =>
    (mode === 'download' ? lastDownloadSession.value : lastUploadSession.value)?.connection_id ===
    props.connectionId
      ? mode === 'download'
        ? lastDownloadSession.value
        : lastUploadSession.value
      : null

  const resolveTransferPanelState = (mode: 'download' | 'upload'): MasterFileTransferState =>
    resolveTransferPanelSession(mode)?.state ?? 'idle'

  const resolveTransferPanelLabel = (mode: 'download' | 'upload'): string =>
    t(STATE_LABELS[resolveTransferPanelState(mode)])

  const resolveTransferPanelProgress = (mode: 'download' | 'upload'): number => {
    const session = resolveTransferPanelSession(mode)
    const total = session?.bytes_total ?? 0
    const done = session?.bytes_done ?? 0
    if (total <= 0) return 0
    return Math.min(100, Math.round((done / total) * 100))
  }

  const resolveTransferPanelProgressStatus = (
    mode: 'download' | 'upload',
  ): 'success' | 'exception' | undefined => {
    const state = resolveTransferPanelState(mode)
    if (state === 'success') return 'success'
    if (state === 'failed' || state === 'cancelled') return 'exception'
    return undefined
  }

  const resolveTransferPanelBytesLabel = (mode: 'download' | 'upload'): string => {
    const session = resolveTransferPanelSession(mode)
    const done = session?.bytes_done ?? 0
    const total = session?.bytes_total ?? 0
    return `${formatFileSize(done)} / ${formatFileSize(total)}`
  }

  const resolveTransferPanelFailureReason = (mode: 'download' | 'upload'): string | null =>
    resolveTransferPanelSession(mode)?.failure_reason ?? null

  const formatTimestamp = (ts: string | null | undefined): string => {
    if (!ts) return ''
    try {
      const d = new Date(ts)
      if (isNaN(d.getTime())) return ts
      return d.toLocaleString(getCurrentLocale(), {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
      })
    } catch {
      return ts
    }
  }

  const handleQueryDateRangeChange = (value: unknown) => {
    if (
      Array.isArray(value) &&
      value.length === 2 &&
      value[0] instanceof Date &&
      value[1] instanceof Date
    ) {
      queryStartDate.value = value[0]
      queryEndDate.value = value[1]
      return
    }
    queryStartDate.value = null

    queryEndDate.value = null
  }

  const dateToRfc3339 = (d: Date | null): string | null => (d ? d.toISOString() : null)

  const closeDialog = () => {
    innerVisible.value = false
  }

  const sanitizeFileNameSegment = (value: string) =>
    String(value || '')
      .trim()
      .replace(/[<>:"/\\|?*\x00-\x1f]/g, '_')
      .replace(/\s+/g, '_')
      .replace(/_+/g, '_')
      .replace(/^_+|_+$/g, '')

  const buildSuggestedDownloadFileName = () => {
    const segments = [
      sanitizeFileNameSegment(props.connectionName || t('master.fileTransfer.fallback.connection')),
      sanitizeFileNameSegment(props.slaveName || t('master.fileTransfer.fallback.slave')),
      sanitizeFileNameSegment(
        formatFileTitle(selectedEntry.value?.nof ?? 0) || t('master.fileTransfer.fallback.file'),
      ),
      dayjs().format('YYMMDD-HHmmss'),
    ].filter((segment) => segment.length > 0)
    return segments.join('_')
  }

  function resetSessionState() {
    resetQueryState()
    lastDownloadSession.value = null
    lastUploadSession.value = null
  }

  function resetQueryState() {
    hasFetchedDirectory.value = false
    hasQueriedLog.value = false
    lastDirectoryEntries.value = []
    lastLogEntries.value = []
    selectedEntry.value = null
    shouldAcceptDirectoryResult.value = false
    shouldAcceptLogResult.value = false
  }

  function buildDialogSnapshot() {
    return JSON.stringify({
      activeTab: activeTab.value,
      directoryMode: directoryMode.value,
      uploadNof: uploadNof.value,
      downloadPath: downloadPath.value,
      uploadPath: uploadPath.value,
      queryLogNof: queryLogNof.value,
      queryStartDate: dateToRfc3339(queryStartDate.value),
      queryEndDate: dateToRfc3339(queryEndDate.value),
      selectedEntryNof: selectedEntry.value?.nof ?? null,
    })
  }

  const { requestClose: requestCloseDialog, handleBeforeClose } = useDialogCloseGuard({
    isDirty: () => hasUnsavedChanges.value,
    onClose: closeDialog,
  })

  const chooseDownloadPath = async () => {
    const selected = await save({
      title: t('master.fileTransfer.dialogs.chooseDownloadPath'),
      defaultPath: buildSuggestedDownloadFileName(),
    })
    if (typeof selected === 'string') downloadPath.value = selected
  }

  const chooseUploadPath = async () => {
    const selected = await open({
      title: t('master.fileTransfer.dialogs.chooseUploadFile'),
      multiple: false,
      directory: false,
    })
    if (typeof selected === 'string') {
      uploadPath.value = selected
      uploadNof.value = suggestNofByPath(selected)
    }
  }

  const suggestNofByPath = (path: string) => {
    const fileName = (path.split(/[\\/]/).pop() || path).toLowerCase()
    if (
      /soe|event|sequence|\u987a\u5e8f\u4e8b\u4ef6|\u4e8b\u4ef6\u987a\u5e8f|\u9065\u4fe1\u53d8\u4f4d|\u53d8\u4f4d/.test(
        fileName,
      )
    ) {
      return 3
    }
    if (/disturb|fault|wave|record|\u5f55\u6ce2|\u6270\u52a8|\u6545\u969c/.test(fileName)) {
      return 2
    }
    if (
      /analog|measure|history|trend|curve|\u6a21\u62df\u91cf|\u66f2\u7ebf|\u5386\u53f2/.test(
        fileName,
      )
    ) {
      return 4
    }
    return 1
  }

  const queryDirectory = () => {
    if (!canQueryDirectory.value || !props.connectionId || !props.slaveId) return
    shouldAcceptDirectoryResult.value = true
    shouldAcceptLogResult.value = false
    selectedEntry.value = null
    emit('start', {
      connection_id: props.connectionId,
      slave_id: props.slaveId,
      mode: 'directory',
      ioa: 0,
    })
  }

  const startQueryLog = () => {
    if (!canQueryLog.value || !props.connectionId || !props.slaveId) return
    shouldAcceptDirectoryResult.value = false
    shouldAcceptLogResult.value = true
    selectedEntry.value = null
    emit('start', {
      connection_id: props.connectionId,
      slave_id: props.slaveId,
      mode: 'query-log',
      ioa: 0,
      nof: queryLogNof.value,
      query_start_time: dateToRfc3339(queryStartDate.value),
      query_end_time: dateToRfc3339(queryEndDate.value),
    })
  }

  const handleDirectorySelect = (entry: MasterRemoteDirectoryEntry) => {
    selectedEntry.value = entry
  }

  const startDownload = () => {
    if (
      !canStartDownload.value ||
      !props.connectionId ||
      !props.slaveId ||
      !downloadPath.value ||
      !selectedEntry.value
    )
      return
    emit('start', {
      connection_id: props.connectionId,
      slave_id: props.slaveId,
      mode: 'download',
      ioa: 0,
      nof: selectedEntry.value.nof,
      local_path: downloadPath.value,
    })
  }

  const startUpload = () => {
    if (!canStartUpload.value || !props.connectionId || !props.slaveId || !uploadPath.value) return
    emit('start', {
      connection_id: props.connectionId,
      slave_id: props.slaveId,
      mode: 'upload',
      ioa: 0,
      nof: uploadNof.value,
      local_path: uploadPath.value,
    })
  }

  const cancelTransfer = () => {
    emit('cancel')
  }

  return {
    activeTab,
    busy,
    canPrimaryAction,
    canQueryDirectory,
    canQueryLog,
    canStartUpload,
    cancelActionLabel,
    cancelTransfer,
    chooseDownloadPath,
    chooseUploadPath,
    dateRangeSeparator,
    dialogSubtitleBadges,
    directoryActionLabel,
    directoryEmptyHint,
    directoryEmptyTitle,
    directoryEntries,
    directoryHelperText,
    directoryMode,
    downloadPath,
    formatFileSize,
    formatFileTitle,
    formatTimestamp,
    handleBeforeClose,
    handleDirectorySelect,
    handlePrimaryAction,
    handleQueryDateRangeChange,
    innerVisible,
    isRunning,
    nofSelectOptions,
    primaryActionLabel,
    queryDateRange,
    queryDirectory,
    queryLogNof,
    requestCloseDialog,
    resolveTransferPanelBytesLabel,
    resolveTransferPanelFailureReason,
    resolveTransferPanelLabel,
    resolveTransferPanelProgress,
    resolveTransferPanelProgressStatus,
    resolveTransferPanelState,
    selectedEntry,
    selectedEntryKey,
    selectedEntryMeta,
    singleSectionLimitText,
    startQueryLog,
    uploadNof,
    uploadPath,
    uploadRequirementHint,
  }
}

export type MasterFileTransferDialogContext = ReturnType<typeof useMasterFileTransferDialog>

const masterFileTransferDialogContextKey: InjectionKey<MasterFileTransferDialogContext> = Symbol(
  'master-file-transfer-dialog-context',
)

export function provideMasterFileTransferDialogContext(
  context: MasterFileTransferDialogContext,
): void {
  provide(masterFileTransferDialogContextKey, context)
}

export function useMasterFileTransferDialogContext(): MasterFileTransferDialogContext {
  const context = inject(masterFileTransferDialogContextKey)
  if (!context) throw new Error('Master file transfer dialog context is unavailable')
  return context
}
