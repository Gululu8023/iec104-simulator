import { t } from '@shared/i18n'

export type MasterControlGroupId = 'remote-control' | 'remote-adjust'
export type MasterControlSubtypeId =
  | 'single-command'
  | 'double-command'
  | 'regulating-step'
  | 'bit-string-command'
  | 'set-point'
export type MasterRemoteControlFilterId =
  | 'all'
  | 'single-command'
  | 'double-command'
  | 'regulating-step'
  | 'bit-string-command'

export type MasterControlGroup = {
  id: MasterControlGroupId
  marker: string
  label: string
  iconDataType: string
  dataTypes: readonly string[]
}

type MasterControlSubtype = {
  id: MasterControlSubtypeId
  label: string
  groupId: MasterControlGroupId
  dataTypes: readonly string[]
}

export const MASTER_CONTROL_GROUPS: readonly MasterControlGroup[] = [
  {
    id: 'remote-control',
    marker: 'C_GROUP_REMOTE_CONTROL',
    label: '遥控',
    iconDataType: 'C_SC_NA_1',
    dataTypes: [
      'C_SC_NA_1',
      'C_DC_NA_1',
      'C_BO_NA_1',
      'C_SC_TA_1',
      'C_DC_TA_1',
      'C_BO_TA_1',
      'C_RC_NA_1',
      'C_RC_TA_1',
    ],
  },
  {
    id: 'remote-adjust',
    marker: 'C_GROUP_REMOTE_ADJUST',
    label: '遥调',
    iconDataType: 'C_SE_NA_1',
    dataTypes: ['C_SE_NA_1', 'C_SE_NB_1', 'C_SE_NC_1', 'C_SE_TA_1', 'C_SE_TB_1', 'C_SE_TC_1'],
  },
]

export const MASTER_CONTROL_SUBTYPES: readonly MasterControlSubtype[] = [
  {
    id: 'single-command',
    label: '单点',
    groupId: 'remote-control',
    dataTypes: ['C_SC_NA_1', 'C_SC_TA_1'],
  },
  {
    id: 'double-command',
    label: '双点',
    groupId: 'remote-control',
    dataTypes: ['C_DC_NA_1', 'C_DC_TA_1'],
  },
  {
    id: 'regulating-step',
    label: '步调',
    groupId: 'remote-control',
    dataTypes: ['C_RC_NA_1', 'C_RC_TA_1'],
  },
  {
    id: 'bit-string-command',
    label: '位串',
    groupId: 'remote-control',
    dataTypes: ['C_BO_NA_1', 'C_BO_TA_1'],
  },
  {
    id: 'set-point',
    label: '设定值',
    groupId: 'remote-adjust',
    dataTypes: ['C_SE_NA_1', 'C_SE_NB_1', 'C_SE_NC_1', 'C_SE_TA_1', 'C_SE_TB_1', 'C_SE_TC_1'],
  },
]

export const MASTER_REMOTE_CONTROL_FILTERS: ReadonlyArray<{
  value: MasterRemoteControlFilterId
  label: string
}> = [
  { value: 'all', label: '全部' },
  { value: 'single-command', label: '单点' },
  { value: 'double-command', label: '双点' },
  { value: 'regulating-step', label: '步调' },
  { value: 'bit-string-command', label: '位串' },
]

export const MASTER_CONTROL_ASDU_TYPES: readonly string[] = MASTER_CONTROL_GROUPS.flatMap(
  (group) => group.dataTypes,
)

const CONTROL_GROUP_BY_MARKER = new Map<string, MasterControlGroup>(
  MASTER_CONTROL_GROUPS.map((group) => [group.marker, group]),
)

const CONTROL_GROUP_BY_DATA_TYPE = new Map<string, MasterControlGroup>()
const CONTROL_SUBTYPE_BY_DATA_TYPE = new Map<string, MasterControlSubtype>()

for (const group of MASTER_CONTROL_GROUPS) {
  for (const dataType of group.dataTypes) {
    CONTROL_GROUP_BY_DATA_TYPE.set(dataType, group)
  }
}

for (const subtype of MASTER_CONTROL_SUBTYPES) {
  for (const dataType of subtype.dataTypes) {
    CONTROL_SUBTYPE_BY_DATA_TYPE.set(dataType, subtype)
  }
}

const normalizeDataType = (dataType: string | null | undefined): string => {
  return String(dataType ?? '')
    .trim()
    .toUpperCase()
}

const masterControlGroupLabelKeys: Record<MasterControlGroupId, string> = {
  'remote-control': 'master.controlGroups.remoteControl',
  'remote-adjust': 'master.controlGroups.remoteAdjust',
}

const masterControlSubtypeLabelKeys: Record<MasterControlSubtypeId, string> = {
  'single-command': 'master.controlSubtypes.singleCommand',
  'double-command': 'master.controlSubtypes.doubleCommand',
  'regulating-step': 'master.controlSubtypes.regulatingStep',
  'bit-string-command': 'master.controlSubtypes.bitStringCommand',
  'set-point': 'master.controlSubtypes.setPoint',
}

const masterRemoteControlFilterLabelKeys: Record<MasterRemoteControlFilterId, string> = {
  all: 'master.contentToolbar.remoteControlFilters.all',
  'single-command': 'master.contentToolbar.remoteControlFilters.singleCommand',
  'double-command': 'master.contentToolbar.remoteControlFilters.doubleCommand',
  'regulating-step': 'master.contentToolbar.remoteControlFilters.regulatingStep',
  'bit-string-command': 'master.contentToolbar.remoteControlFilters.bitStringCommand',
}

export const isMasterControlGroupMarker = (dataType: string | null | undefined): boolean => {
  return CONTROL_GROUP_BY_MARKER.has(normalizeDataType(dataType))
}

export const resolveMasterControlGroupByMarker = (
  dataType: string | null | undefined,
): MasterControlGroup | null => {
  return CONTROL_GROUP_BY_MARKER.get(normalizeDataType(dataType)) ?? null
}

export const resolveMasterControlGroupLabel = (
  group: MasterControlGroup | null | undefined,
): string => {
  if (!group) return ''
  return t(masterControlGroupLabelKeys[group.id])
}

export const resolveMasterControlGroupLabelByMarker = (
  dataType: string | null | undefined,
): string | null => {
  const group = resolveMasterControlGroupByMarker(dataType)
  return group ? resolveMasterControlGroupLabel(group) : null
}

export const resolveMasterControlGroupByDataType = (
  dataType: string | null | undefined,
): MasterControlGroup | null => {
  return CONTROL_GROUP_BY_DATA_TYPE.get(normalizeDataType(dataType)) ?? null
}

export const resolveMasterControlSubtypeByDataType = (
  dataType: string | null | undefined,
): MasterControlSubtype | null => {
  return CONTROL_SUBTYPE_BY_DATA_TYPE.get(normalizeDataType(dataType)) ?? null
}

export const resolveMasterControlSubtypeLabel = (
  dataType: string | null | undefined,
): string | null => {
  return resolveMasterControlSubtypeByDataType(dataType)?.label ?? null
}

export const resolveMasterControlSubtypeLabelLocalized = (
  dataType: string | null | undefined,
): string | null => {
  const subtype = resolveMasterControlSubtypeByDataType(dataType)
  return subtype ? t(masterControlSubtypeLabelKeys[subtype.id]) : null
}

export const getMasterRemoteControlFilters = (): ReadonlyArray<{
  value: MasterRemoteControlFilterId
  label: string
}> =>
  MASTER_REMOTE_CONTROL_FILTERS.map((option) => ({
    value: option.value,
    label: t(masterRemoteControlFilterLabelKeys[option.value]),
  }))

export const isMasterControlDataType = (dataType: string | null | undefined): boolean => {
  return resolveMasterControlGroupByDataType(dataType) != null
}

export const resolveMasterTabDataTypes = (
  tabDataType: string | null | undefined,
): readonly string[] => {
  const group = resolveMasterControlGroupByMarker(tabDataType)
  if (group) return group.dataTypes

  const normalized = normalizeDataType(tabDataType)
  if (!normalized) return []
  switch (normalized) {
    case 'M_SP_NA_1':
    case 'M_SP_TA_1':
    case 'M_SP_TB_1':
      return ['M_SP_NA_1', 'M_SP_TA_1', 'M_SP_TB_1']
    case 'M_DP_NA_1':
    case 'M_DP_TA_1':
    case 'M_DP_TB_1':
      return ['M_DP_NA_1', 'M_DP_TA_1', 'M_DP_TB_1']
    case 'M_ST_NA_1':
    case 'M_ST_TA_1':
    case 'M_ST_TB_1':
      return ['M_ST_NA_1', 'M_ST_TA_1', 'M_ST_TB_1']
    case 'M_BO_NA_1':
    case 'M_BO_TA_1':
    case 'M_BO_TB_1':
      return ['M_BO_NA_1', 'M_BO_TA_1', 'M_BO_TB_1']
    case 'M_ME_NA_1':
    case 'M_ME_TA_1':
    case 'M_ME_TD_1':
      return ['M_ME_NA_1', 'M_ME_TA_1', 'M_ME_TD_1']
    case 'M_ME_NB_1':
    case 'M_ME_TB_1':
    case 'M_ME_TE_1':
      return ['M_ME_NB_1', 'M_ME_TB_1', 'M_ME_TE_1']
    case 'M_ME_NC_1':
    case 'M_ME_TC_1':
    case 'M_ME_TF_1':
      return ['M_ME_NC_1', 'M_ME_TC_1', 'M_ME_TF_1']
    case 'M_IT_NA_1':
    case 'M_IT_TA_1':
    case 'M_IT_TB_1':
      return ['M_IT_NA_1', 'M_IT_TA_1', 'M_IT_TB_1']
    default:
      return [normalized]
  }
}

export const resolveMasterControlCommandType = (
  dataType: string | null | undefined,
): MasterControlSubtypeId | null => {
  const subtype = resolveMasterControlSubtypeByDataType(dataType)
  if (subtype) return subtype.id

  const group = resolveMasterControlGroupByDataType(dataType)
  if (group?.id === 'remote-adjust') return 'set-point'
  if (group?.id === 'remote-control') return 'single-command'
  return null
}

export const resolveMasterSetpointTypeByDataType = (
  dataType: string | null | undefined,
): 'normalized' | 'scaled' | 'short-float' | null => {
  switch (normalizeDataType(dataType)) {
    case 'C_SE_NA_1':
    case 'C_SE_TA_1':
      return 'normalized'
    case 'C_SE_NB_1':
    case 'C_SE_TB_1':
      return 'scaled'
    case 'C_SE_NC_1':
    case 'C_SE_TC_1':
      return 'short-float'
    default:
      return null
  }
}
