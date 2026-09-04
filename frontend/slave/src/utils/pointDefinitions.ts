import type {
  PointDef,
  PointTableImportNormalizedPayload,
  PointTableImportPreview,
} from '@shared/api/types'
import {
  buildDefaultIec104PointName,
  iec104ProtocolDefaultValue,
  iec104TypeNameToId,
  resolveIec104TypeName,
} from '@shared/api/iec104'
import { t } from '@shared/i18n'

export type StationConfigObjectPayload = {
  name: string
  asduAddress: string
  ioaStartAddress: number
  ioaCount: number
}

export type StationPointTemplatePayload = {
  use_default_template: boolean
  objects: Array<{
    name: string
    asdu_type: string
    ioa_start: number
    ioa_count: number
  }>
}

function normalizePointTypeId(typeIdOrName: number | string | null | undefined): number {
  const numeric = Number(typeIdOrName)
  if (Number.isFinite(numeric) && numeric > 0) return Math.trunc(numeric)
  return (
    iec104TypeNameToId(
      String(typeIdOrName ?? '')
        .trim()
        .toUpperCase(),
    ) ?? 0
  )
}

export function resolveLocalizedDefaultPointNamePrefix(
  typeIdOrName: number | string | null | undefined,
): string {
  switch (normalizePointTypeId(typeIdOrName)) {
    case 1:
    case 2:
    case 30:
      return t('slave.profileDialogs.defaultPointNames.singlePoint')
    case 3:
    case 4:
    case 31:
      return t('slave.profileDialogs.defaultPointNames.doublePoint')
    case 5:
    case 6:
    case 32:
      return t('slave.profileDialogs.defaultPointNames.stepPosition')
    case 7:
    case 8:
    case 33:
      return t('slave.profileDialogs.defaultPointNames.bitstring')
    case 9:
    case 10:
    case 21:
    case 34:
      return t('slave.profileDialogs.defaultPointNames.normalizedMeasurement')
    case 11:
    case 12:
    case 35:
      return t('slave.profileDialogs.defaultPointNames.scaledMeasurement')
    case 13:
    case 14:
    case 36:
      return t('slave.profileDialogs.defaultPointNames.shortFloatMeasurement')
    case 15:
    case 16:
    case 37:
      return t('slave.profileDialogs.defaultPointNames.integratedTotal')
    case 17:
    case 38:
      return t('slave.profileDialogs.defaultPointNames.protectionEvent')
    case 18:
    case 39:
      return t('slave.profileDialogs.defaultPointNames.protectionStart')
    case 19:
    case 40:
      return t('slave.profileDialogs.defaultPointNames.protectionOutput')
    case 20:
      return t('slave.profileDialogs.defaultPointNames.groupedSinglePoint')
    case 45:
    case 58:
      return t('slave.profileDialogs.defaultPointNames.singleCommand')
    case 46:
    case 59:
      return t('slave.profileDialogs.defaultPointNames.doubleCommand')
    case 47:
    case 60:
      return t('slave.profileDialogs.defaultPointNames.stepCommand')
    case 48:
    case 61:
      return t('slave.profileDialogs.defaultPointNames.normalizedSetpoint')
    case 49:
    case 62:
      return t('slave.profileDialogs.defaultPointNames.scaledSetpoint')
    case 50:
    case 63:
      return t('slave.profileDialogs.defaultPointNames.shortFloatSetpoint')
    case 51:
    case 64:
      return t('slave.profileDialogs.defaultPointNames.bitstringCommand')
    case 110:
      return t('slave.profileDialogs.defaultPointNames.normalizedParameter')
    case 111:
      return t('slave.profileDialogs.defaultPointNames.scaledParameter')
    case 112:
      return t('slave.profileDialogs.defaultPointNames.shortFloatParameter')
    case 113:
      return t('slave.profileDialogs.defaultPointNames.parameterActivation')
    default:
      return t('slave.profileDialogs.defaultPointNameFallback')
  }
}

function buildLocalizedDefaultPointName(
  typeIdOrName: number | string | null | undefined,
  address: number | null | undefined,
): string {
  const normalizedAddress = Math.max(0, Math.trunc(Number(address) || 0))
  return `${resolveLocalizedDefaultPointNamePrefix(typeIdOrName)}_${normalizedAddress}`
}

export function normalizeSlavePointTableImportPayload(
  payload: PointTableImportNormalizedPayload,
  preview: PointTableImportPreview,
): PointTableImportNormalizedPayload {
  const missingNameAddresses = new Set<number>()
  for (const warning of preview.warnings ?? []) {
    if (warning.code !== 'MISSING_POINT_NAME') continue
    const match = String(warning.message ?? '').match(/IOA=(\d+)/)
    if (!match) continue
    const address = Number(match[1])
    if (Number.isFinite(address)) missingNameAddresses.add(Math.trunc(address))
  }
  if (missingNameAddresses.size === 0) return payload

  return {
    ...payload,
    point_defs: payload.point_defs.map((point) => {
      const address = Math.max(0, Math.trunc(Number(point.address) || 0))
      if (!missingNameAddresses.has(address)) return point
      const backendDefaultName = buildDefaultIec104PointName(point.type_id, address)
      if (String(point.name ?? '').trim() !== backendDefaultName) return point
      return { ...point, name: buildLocalizedDefaultPointName(point.type_id, address) }
    }),
  }
}

export function normalizeOptionalControlIoa(value: unknown): number | null {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return null
  const normalized = Math.trunc(numeric)
  return normalized >= 1 && normalized <= 0xffffff ? normalized : null
}

export function buildEditedPointDefs(
  currentDefs: PointDef[],
  dataPoint: any,
  address: number,
): PointDef[] {
  const existing = currentDefs.find((def) => Number(def.address) === Number(address))
  const candidateTypeName = String(dataPoint?.dataType ?? '')
    .trim()
    .toUpperCase()
  const candidateTypeId = iec104TypeNameToId(candidateTypeName) ?? 13
  const normalizedName =
    String(dataPoint?.name ?? '').trim() ||
    existing?.name ||
    buildDefaultIec104PointName(candidateTypeId, address)
  const normalizedGroup = (value: unknown, max: number): number | null => {
    const group = Number(value)
    return Number.isInteger(group) && group >= 1 && group <= max ? group : null
  }
  const nextPointDef: PointDef = {
    address,
    name: normalizedName,
    type_id: existing?.type_id ?? candidateTypeId,
    data_type:
      existing?.data_type ??
      resolveIec104TypeName({
        data_type: candidateTypeName,
        type_id: candidateTypeId,
        name: normalizedName,
      }),
    description: String(dataPoint?.description ?? '').trim() || null,
    control_ioa: normalizeOptionalControlIoa(dataPoint?.controlIoa),
    gi_group: normalizedGroup(dataPoint?.giGroup, 16),
    counter_group: normalizedGroup(dataPoint?.counterGroup, 4),
    default_value: existing?.default_value ?? null,
    is_enabled: existing?.is_enabled ?? true,
  }

  const mergedByAddress = new Map<number, PointDef>()
  for (const def of currentDefs) {
    mergedByAddress.set(Math.max(0, Math.trunc(Number(def.address) || 0)), def)
  }
  mergedByAddress.set(address, nextPointDef)
  return Array.from(mergedByAddress.values()).sort((left, right) => left.address - right.address)
}

export function buildPointDefsFromConfigObjects(objects: StationConfigObjectPayload[]): PointDef[] {
  const dedupByAddress = new Map<number, PointDef>()
  for (const object of objects) {
    const typeId = iec104TypeNameToId(object.asduAddress) ?? 13
    const baseName =
      String(object.name ?? '').trim() || resolveLocalizedDefaultPointNamePrefix(typeId)
    const startAddress = Math.max(1, Math.trunc(Number(object.ioaStartAddress) || 1))
    const count = Math.max(1, Math.trunc(Number(object.ioaCount) || 1))
    for (let idx = 0; idx < count; idx += 1) {
      const address = startAddress + idx
      const isFloatType = [9, 10, 11, 12, 13, 14, 21, 34, 35, 36].includes(typeId)
      dedupByAddress.set(address, {
        address,
        name: `${baseName}_${address}`,
        type_id: typeId,
        data_type: String(object.asduAddress || 'M_ME_NC_1'),
        description: null,
        control_ioa: null,
        default_value: isFloatType ? 0.0 : iec104ProtocolDefaultValue(typeId),
        is_enabled: true,
      })
    }
  }
  return Array.from(dedupByAddress.values()).sort((left, right) => left.address - right.address)
}

export function buildPointTemplatePayload(
  objects: StationConfigObjectPayload[] | undefined,
  useDefaultTemplate: boolean | undefined,
): StationPointTemplatePayload | null {
  const normalizedObjects = (objects ?? [])
    .map((object) => {
      const asduType = String(object.asduAddress ?? '')
        .trim()
        .toUpperCase()
      if (!asduType) return null
      return {
        name: String(object.name ?? '').trim() || resolveLocalizedDefaultPointNamePrefix(asduType),
        asdu_type: asduType,
        ioa_start: Math.max(1, Math.trunc(Number(object.ioaStartAddress) || 1)),
        ioa_count: Math.max(1, Math.trunc(Number(object.ioaCount) || 1)),
      }
    })
    .filter((item): item is NonNullable<typeof item> => item != null)

  if (!useDefaultTemplate && normalizedObjects.length === 0) return null
  return { use_default_template: Boolean(useDefaultTemplate), objects: normalizedObjects }
}
