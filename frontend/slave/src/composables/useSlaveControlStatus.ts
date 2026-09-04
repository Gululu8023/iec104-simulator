import {
  formatControlConfirmationTextLocalized,
  formatKeyValueTooltip,
  iec104TypeNameToId,
} from '@shared/api/iec104'
import type { BackendControlStatusSnapshot } from '@shared/api/types'
import { t } from '@shared/i18n'

interface ControlStatusPointLike {
  address: number
  commonAddress?: number | null
  dataType: string
  timestamp: number
  controlStatusSnapshot?: BackendControlStatusSnapshot | null
}

type ControlState = 'idle' | 'selected' | 'executing' | 'ok' | 'ng' | 'timeout'

interface ControlStatusSnapshot {
  state: ControlState
  label: string
  updatedAt: number
  cot: number | null
  oa: number | null
  cause: number | null
  negative: boolean
  selectExecute: boolean | null
  actTermAt: number | null
  actConAt?: number | null
  timeoutType?: 'select' | 'execute' | null
  commandCp56?: string | null
}

interface UseSlaveControlStatusOptions {
  isControlPointType: (dataType: string) => boolean
}

const CONTROL_STATUS_TIMEOUT_MS = 15_000

const isTimestampedCommandTypeId = (typeId: number): boolean => typeId >= 58 && typeId <= 64

const formatCotText = (cot: number, negative = false): string => {
  const cotNameMap: Record<number, string> = {
    3: t('slave.contentPanel.controlStatus.cot.labels.spontaneous'),
    6: t('slave.contentPanel.controlStatus.cot.labels.activation'),
    7: t('slave.contentPanel.controlStatus.cot.labels.activationConfirm'),
    8: t('slave.contentPanel.controlStatus.cot.labels.deactivation'),
    9: t('slave.contentPanel.controlStatus.cot.labels.deactivationConfirm'),
    10: t('slave.contentPanel.controlStatus.cot.labels.activationTermination'),
    20: t('slave.contentPanel.controlStatus.cot.labels.interrogationResponse'),
    44: t('slave.contentPanel.controlStatus.cot.labels.unknownType'),
    45: t('slave.contentPanel.controlStatus.cot.labels.unknownCot'),
    46: t('slave.contentPanel.controlStatus.cot.labels.unknownCommonAddress'),
    47: t('slave.contentPanel.controlStatus.cot.labels.unknownObjectAddress'),
  }
  const label = cotNameMap[cot] || t('slave.contentPanel.controlStatus.cot.labels.undefined')
  return t(
    negative
      ? 'slave.contentPanel.controlStatus.cot.negative'
      : 'slave.contentPanel.controlStatus.cot.normal',
    { label, cot },
  )
}

const formatClockTime = (timestamp: number): string => {
  const date = new Date(timestamp)
  const pad = (value: number, width = 2) => String(value).padStart(width, '0')
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`
}

export function useSlaveControlStatus<TPoint extends ControlStatusPointLike>(
  options: UseSlaveControlStatusOptions,
) {
  const controlStateLabel = (state: ControlState): string =>
    t(`slave.contentPanel.controlStatus.states.${state}`)

  const mapBackendControlStatusSnapshot = (
    snapshot: BackendControlStatusSnapshot | null | undefined,
    fallbackUpdatedAt: number,
  ): ControlStatusSnapshot | null => {
    if (!snapshot) return null

    const updatedAt = new Date(snapshot.updated_at).getTime()
    const actConAt = snapshot.actcon_at ? new Date(snapshot.actcon_at).getTime() : null
    const actTermAt = snapshot.actterm_at ? new Date(snapshot.actterm_at).getTime() : null
    const normalizedUpdatedAt = Number.isFinite(updatedAt) ? updatedAt : fallbackUpdatedAt
    const stateMap: Record<string, { state: ControlState; label: string }> = {
      Idle: { state: 'idle', label: controlStateLabel('idle') },
      Selected: { state: 'selected', label: controlStateLabel('selected') },
      Executing: { state: 'executing', label: controlStateLabel('executing') },
      Ok: { state: 'ok', label: controlStateLabel('ok') },
      Ng: { state: 'ng', label: controlStateLabel('ng') },
      Timeout: { state: 'timeout', label: controlStateLabel('timeout') },
    }
    const mapped = stateMap[String(snapshot.state)] ?? stateMap.Idle

    return {
      state: mapped.state,
      label: mapped.label,
      updatedAt: normalizedUpdatedAt,
      cot: snapshot.latest_cause != null ? Number(snapshot.latest_cause) & 0x3f : null,
      oa: snapshot.latest_cause != null ? (Number(snapshot.latest_cause) >> 8) & 0xff : null,
      cause: snapshot.latest_cause != null ? Number(snapshot.latest_cause) : null,
      negative: Boolean(snapshot.actcon_negative),
      selectExecute: snapshot.select_execute ?? null,
      actTermAt: actTermAt && Number.isFinite(actTermAt) ? actTermAt : null,
      actConAt: actConAt && Number.isFinite(actConAt) ? actConAt : null,
      timeoutType: snapshot.timeout_type ?? null,
      commandCp56: snapshot.command_cp56 ?? null,
    }
  }

  const resolveControlStatusSnapshot = (point: TPoint): ControlStatusSnapshot => {
    if (!options.isControlPointType(point.dataType)) {
      return {
        state: 'idle',
        label: controlStateLabel('idle'),
        updatedAt: point.timestamp,
        cot: null,
        oa: null,
        cause: null,
        negative: false,
        selectExecute: null,
        actTermAt: null,
      }
    }

    const snapshot = mapBackendControlStatusSnapshot(point.controlStatusSnapshot, point.timestamp)
    if (!snapshot) {
      return {
        state: 'idle',
        label: controlStateLabel('idle'),
        updatedAt: point.timestamp,
        cot: null,
        oa: null,
        cause: null,
        negative: false,
        selectExecute: null,
        actTermAt: null,
      }
    }

    return snapshot
  }

  const resolveTimeoutTypeText = (snapshot: ControlStatusSnapshot): string => {
    if (snapshot.timeoutType === 'select')
      return t('slave.contentPanel.controlStatus.timeoutTypes.select')
    if (snapshot.timeoutType === 'execute')
      return t('slave.contentPanel.controlStatus.timeoutTypes.execute')
    if (snapshot.selectExecute === true)
      return t('slave.contentPanel.controlStatus.timeoutTypes.select')
    if (snapshot.selectExecute === false)
      return t('slave.contentPanel.controlStatus.timeoutTypes.execute')
    return t('slave.contentPanel.controlStatus.timeoutTypes.command')
  }

  const resolveSnapshotRejectReasonText = (snapshot: ControlStatusSnapshot): string => {
    if (snapshot.cause == null)
      return t('slave.contentPanel.controlStatus.rejectReasons.commandRejected')
    const cot = snapshot.cot ?? snapshot.cause & 0x3f
    if (cot === 44) return t('slave.contentPanel.controlStatus.rejectReasons.unknownType')
    if (cot === 45) return t('slave.contentPanel.controlStatus.rejectReasons.unknownCot')
    if (cot === 46) return t('slave.contentPanel.controlStatus.rejectReasons.unknownCommonAddress')
    if (cot === 47) return t('slave.contentPanel.controlStatus.rejectReasons.unknownObjectAddress')
    if (cot === 7 && snapshot.selectExecute === true) {
      return t('slave.contentPanel.controlStatus.rejectReasons.selectRejected')
    }
    if (cot === 7 && snapshot.selectExecute === false) {
      return t('slave.contentPanel.controlStatus.rejectReasons.executeRejected')
    }
    if (cot === 9) return t('slave.contentPanel.controlStatus.rejectReasons.deactivationRejected')
    return t('slave.contentPanel.controlStatus.rejectReasons.negativeConfirmation', {
      cot: formatCotText(cot, true),
    })
  }

  const getControlStatusText = (point: TPoint): string =>
    controlStateLabel(resolveControlStatusSnapshot(point).state)

  const getControlStatusTagType = (point: TPoint): 'info' | 'warning' | 'success' | 'danger' => {
    const status = resolveControlStatusSnapshot(point).state
    if (status === 'selected' || status === 'executing') return 'warning'
    if (status === 'ok') return 'success'
    if (status === 'ng' || status === 'timeout') return 'danger'
    return 'info'
  }

  const getExtraInfoTooltip = (point: TPoint): string => {
    if (!options.isControlPointType(point.dataType)) return '--'

    const snapshot = resolveControlStatusSnapshot(point)
    const rows: Array<{ label: string; value: string }> = []
    const pushRow = (label: string, value: string | null | undefined) => {
      const normalized = String(value ?? '').trim()
      if (!normalized) return
      rows.push({ label, value: normalized })
    }

    const resolvedCotText =
      snapshot.cause != null
        ? formatCotText(snapshot.cot ?? snapshot.cause & 0x3f, snapshot.negative)
        : ''
    const snapshotActConText =
      snapshot.actConAt != null
        ? `${formatControlConfirmationTextLocalized(snapshot.negative)} ${formatClockTime(snapshot.actConAt)}`
        : ''
    const snapshotActTermText =
      snapshot.actTermAt != null ? formatClockTime(snapshot.actTermAt) : ''

    switch (snapshot.state) {
      case 'selected': {
        pushRow('ActCon', snapshotActConText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestCot'), resolvedCotText)
        const remainMs = Math.max(0, CONTROL_STATUS_TIMEOUT_MS - (Date.now() - snapshot.updatedAt))
        pushRow(
          t('slave.contentPanel.controlStatus.tooltip.selectRemaining'),
          `${Math.ceil(remainMs / 1000)} s`,
        )
        break
      }
      case 'executing':
        pushRow('ActCon', snapshotActConText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestCot'), resolvedCotText)
        break
      case 'ok':
        pushRow('ActCon', snapshotActConText)
        pushRow('ActTerm', snapshotActTermText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestCot'), resolvedCotText)
        break
      case 'ng':
        pushRow(
          t('slave.contentPanel.controlStatus.tooltip.latestConfirmation'),
          snapshotActConText,
        )
        pushRow(
          t('slave.contentPanel.controlStatus.tooltip.failureReason'),
          resolveSnapshotRejectReasonText(snapshot),
        )
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestCot'), resolvedCotText)
        break
      case 'timeout':
        pushRow(
          t('slave.contentPanel.controlStatus.tooltip.timeoutType'),
          resolveTimeoutTypeText(snapshot),
        )
        pushRow(
          t('slave.contentPanel.controlStatus.tooltip.timeoutAt'),
          formatClockTime(snapshot.updatedAt + CONTROL_STATUS_TIMEOUT_MS),
        )
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestActCon'), snapshotActConText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestActTerm'), snapshotActTermText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestCot'), resolvedCotText)
        break
      case 'idle':
      default:
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestActCon'), snapshotActConText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestActTerm'), snapshotActTermText)
        pushRow(t('slave.contentPanel.controlStatus.tooltip.latestCot'), resolvedCotText)
        break
    }

    const typeId = iec104TypeNameToId(point.dataType)
    if (typeId != null && isTimestampedCommandTypeId(typeId)) {
      pushRow(
        t('slave.contentPanel.controlStatus.tooltip.commandCp56'),
        snapshot.commandCp56 ?? t('slave.contentPanel.controlStatus.tooltip.none'),
      )
    }
    if (rows.length === 0) {
      rows.push({
        label: t('slave.contentPanel.controlStatus.tooltip.stateDescription'),
        value: t('slave.contentPanel.controlStatus.tooltip.idleNoRecentAck'),
      })
    }

    return formatKeyValueTooltip(rows)
  }

  const getCotColumnText = (point: TPoint): string => {
    const snapshot = resolveControlStatusSnapshot(point)
    if (snapshot.cause == null) return '—'
    return formatCotText(snapshot.cot ?? snapshot.cause & 0x3f, snapshot.negative)
  }

  const getCp56ColumnText = (point: TPoint): string => {
    const cp56 = String(resolveControlStatusSnapshot(point).commandCp56 ?? '').trim()
    if (!cp56) return '—'
    return cp56
  }

  return {
    getControlStatusTagType,
    getControlStatusText,
    getCotColumnText,
    getCp56ColumnText,
    getExtraInfoTooltip,
  }
}
