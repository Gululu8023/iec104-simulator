import { computed, nextTick, ref, type Ref } from 'vue'

import { ElMessage } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'

import { getStationSlavePointDefs } from '@/api/slave'
import type { DeviceNode } from '@/types/slave'

import { t } from '@shared/i18n'
import { getLoadedIec104Capabilities } from '@shared/api/common'
import { confirmDangerousAction, showAppConfirmDialog } from '@shared/ui/dialogConfirm'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'
import {
  extractIecObjectNameFromPointName,
  formatIec104TypeLabel,
  iec104DefaultStartAddress,
  iec104ProtocolDefaultValue,
  iec104TypeNameToId,
  normalizeIecObjectDisplayName,
} from '@shared/api/iec104'
import type {
  LinkParams,
  PointDef,
  RedundancyGroupMode,
  SlaveResponse,
  SlaveStationPolicy,
  SlaveStationPolicyOverride,
  StationSummary,
} from '@shared/api/types'

export interface ConfigObject {
  id: string
  name: string
  asduAddress: string
  addressMode?: 'continuous' | 'expression'
  ioaStartAddress: number
  ioaCount: number
  ioaExpression?: string
  nameTemplate?: string
}

export interface SlaveForm {
  name: string
  description: string
  commonAddress: number
  selectTimeout: number
  enableSq1Upload: boolean
  enableSoe: boolean
  controlExecutionMode: 'direct' | 'sbo'
}

export interface ConnectionForm {
  name: string
  listenAddress: string
  listenPort: number
  redundancyMode: RedundancyGroupMode
  linkParamsCustomEnabled: boolean
  kValue: number
  wValue: number
  t0: number
  t1: number
  t2: number
  t3: number
  maxAsduBytes: number
  selectTimeout: number
  enableSq1Upload: boolean
  enableSoe: boolean
  unknownTypeidNegativeAck: boolean
}

export interface StationProfileSubmitPayload {
  scope?: 'listener' | 'slave'
  mode: 'create' | 'edit'
  name: string
  commonAddress: number
  host?: string
  port?: number
  stationId?: string
  slaveId?: number
  linkParams?: LinkParams
  stationPolicyOverride?: SlaveStationPolicyOverride
  redundancyMode?: RedundancyGroupMode
  useDefaultTemplate?: boolean
  pointDefs?: PointDef[]
  configObjects?: Array<{
    name: string
    asduAddress: string
    ioaStartAddress: number
    ioaCount: number
  }>
}

const IOA_MAX = 16777215
const BATCH_POINT_MAX = 1000
const DEFAULT_POINT_NAME_TEMPLATE = '{base}_{type}_{ioa}'
const POINT_NAME_TEMPLATE_PLACEHOLDERS = new Set(['{base}', '{type}', '{typeId}', '{ioa}'])

const parseIoaExpression = (expression: string): number[] => {
  const source = String(expression || '').trim()
  if (!source) throw new Error(t('slave.profileDialogs.validation.enterIoaExpression'))

  const addresses: number[] = []
  const seen = new Set<number>()
  for (const rawPart of source.split(',')) {
    const part = rawPart.trim()
    const match = part.match(/^(\d+)(?:\s*-\s*(\d+))?$/)
    if (!match) {
      throw new Error(t('slave.profileDialogs.validation.invalidIoaExpressionPart', { part }))
    }
    const start = Number(match[1])
    const end = match[2] == null ? start : Number(match[2])
    if (start < 1 || end > IOA_MAX) {
      throw new Error(t('slave.profileDialogs.validation.ioaExpressionRange'))
    }
    if (end < start) {
      throw new Error(t('slave.profileDialogs.validation.ioaExpressionDescending', { part }))
    }
    for (let address = start; address <= end; address += 1) {
      if (seen.has(address)) {
        throw new Error(
          t('slave.profileDialogs.validation.ioaExpressionDuplicate', { ioa: address }),
        )
      }
      seen.add(address)
      addresses.push(address)
      if (addresses.length > BATCH_POINT_MAX) {
        throw new Error(
          t('slave.profileDialogs.validation.ioaExpressionLimit', { max: BATCH_POINT_MAX }),
        )
      }
    }
  }
  return addresses.sort((left, right) => left - right)
}

const resolveConfigObjectAddresses = (object: ConfigObject): number[] => {
  if (object.addressMode === 'expression') return parseIoaExpression(object.ioaExpression ?? '')
  const start = Math.trunc(Number(object.ioaStartAddress))
  const count = Math.trunc(Number(object.ioaCount))
  if (start < 1 || start > IOA_MAX || count < 1 || count > BATCH_POINT_MAX) return []
  if (start + count - 1 > IOA_MAX) return []
  return Array.from({ length: count }, (_, index) => start + index)
}

const renderPointNameTemplate = (
  template: string,
  base: string,
  typeName: string,
  typeId: number,
  ioa: number,
): string =>
  template
    .split('{base}')
    .join(base)
    .split('{type}')
    .join(typeName)
    .split('{typeId}')
    .join(String(typeId))
    .split('{ioa}')
    .join(String(ioa))

interface UseSlaveStationProfileDialogsOptions {
  stations: Ref<StationSummary[] | undefined>
  slavesByStation: Ref<Record<string, SlaveResponse[]> | undefined>
  stationId: Ref<string | undefined>
  host: Ref<string | undefined>
  port: Ref<number | undefined>
  defaultCommonAddress: Ref<number | undefined>
  linkParams: Ref<LinkParams | undefined>
  redundancyMode: Ref<RedundancyGroupMode | undefined>
  deviceTreeData: Ref<DeviceNode[]>
  selectedSlave: Ref<DeviceNode | null>
  emitStationProfileSubmit: (payload: StationProfileSubmitPayload) => void
}

const sanitizeLinkParams = (source?: Partial<LinkParams> | null): LinkParams => ({
  k_value: Math.max(1, Math.min(32767, Math.trunc(Number(source?.k_value) || 12))),
  w_value: Math.max(1, Math.min(32767, Math.trunc(Number(source?.w_value) || 8))),
  t0_seconds: Math.max(1, Math.min(3600, Math.trunc(Number(source?.t0_seconds) || 30))),
  t1_seconds: Math.max(1, Math.min(3600, Math.trunc(Number(source?.t1_seconds) || 15))),
  t2_seconds: Math.max(1, Math.min(3600, Math.trunc(Number(source?.t2_seconds) || 10))),
  t3_seconds: Math.max(1, Math.min(3600, Math.trunc(Number(source?.t3_seconds) || 20))),
  max_asdu_bytes: Math.max(4, Math.min(249, Math.trunc(Number(source?.max_asdu_bytes) || 249))),
  select_timeout_seconds: Math.max(
    1,
    Math.min(3600, Math.trunc(Number(source?.select_timeout_seconds) || 15)),
  ),
  enable_sq1_upload: Boolean(source?.enable_sq1_upload),
  enable_soe: Boolean(source?.enable_soe),
  unknown_typeid_negative_ack: source?.unknown_typeid_negative_ack ?? true,
})

const sanitizeStationPolicy = (
  source?: Partial<SlaveStationPolicy> | null,
): SlaveStationPolicy => ({
  select_timeout_seconds: Math.max(
    1,
    Math.min(3600, Math.trunc(Number(source?.select_timeout_seconds) || 15)),
  ),
  enable_sq1_upload: Boolean(source?.enable_sq1_upload),
  enable_soe: Boolean(source?.enable_soe),
  control_execution_mode: source?.control_execution_mode === 'direct' ? 'direct' : 'sbo',
})

const sanitizeStationPolicyOverride = (
  source?: SlaveStationPolicyOverride | null,
): SlaveStationPolicyOverride => {
  const selectTimeoutRaw = source?.select_timeout_seconds
  return {
    select_timeout_seconds:
      selectTimeoutRaw == null
        ? undefined
        : Math.max(1, Math.min(3600, Math.trunc(Number(selectTimeoutRaw) || 15))),
    enable_sq1_upload:
      source?.enable_sq1_upload == null ? undefined : Boolean(source.enable_sq1_upload),
    enable_soe: source?.enable_soe == null ? undefined : Boolean(source.enable_soe),
    control_execution_mode:
      source?.control_execution_mode == null ? undefined : source.control_execution_mode,
  }
}

const isSameLinkParams = (left: LinkParams, right: LinkParams) =>
  left.k_value === right.k_value &&
  left.w_value === right.w_value &&
  left.t0_seconds === right.t0_seconds &&
  left.t1_seconds === right.t1_seconds &&
  left.t2_seconds === right.t2_seconds &&
  left.t3_seconds === right.t3_seconds

const DEFAULT_POINT_NAME_KEY_BY_ASDU: Record<string, string> = {
  M_SP_NA_1: 'singlePoint',
  M_DP_NA_1: 'doublePoint',
  M_ST_NA_1: 'stepPosition',
  M_BO_NA_1: 'bitstring',
  M_ME_NA_1: 'normalizedMeasurement',
  M_ME_NB_1: 'scaledMeasurement',
  M_ME_NC_1: 'shortFloatMeasurement',
  M_IT_NA_1: 'integratedTotal',
  M_PS_NA_1: 'groupedSinglePoint',
  M_ME_ND_1: 'normalizedMeasurement',
  M_SP_TA_1: 'singlePoint',
  M_DP_TA_1: 'doublePoint',
  M_ST_TA_1: 'stepPosition',
  M_BO_TA_1: 'bitstring',
  M_ME_TA_1: 'normalizedMeasurement',
  M_ME_TB_1: 'scaledMeasurement',
  M_ME_TC_1: 'shortFloatMeasurement',
  M_IT_TA_1: 'integratedTotal',
  M_SP_TB_1: 'singlePoint',
  M_DP_TB_1: 'doublePoint',
  M_ST_TB_1: 'stepPosition',
  M_BO_TB_1: 'bitstring',
  M_ME_TD_1: 'normalizedMeasurement',
  M_ME_TE_1: 'scaledMeasurement',
  M_ME_TF_1: 'shortFloatMeasurement',
  M_IT_TB_1: 'integratedTotal',
  M_EP_TA_1: 'singlePoint',
  M_EP_TB_1: 'singlePoint',
  M_EP_TC_1: 'singlePoint',
  M_EP_TD_1: 'singlePoint',
  M_EP_TE_1: 'singlePoint',
  M_EP_TF_1: 'singlePoint',
  C_SC_NA_1: 'singleCommand',
  C_DC_NA_1: 'doubleCommand',
  C_RC_NA_1: 'stepCommand',
  C_SE_NA_1: 'normalizedSetpoint',
  C_SE_NB_1: 'scaledSetpoint',
  C_SE_NC_1: 'shortFloatSetpoint',
  C_BO_NA_1: 'bitstringCommand',
  C_SC_TA_1: 'singleCommand',
  C_DC_TA_1: 'doubleCommand',
  C_RC_TA_1: 'stepCommand',
  C_SE_TA_1: 'normalizedSetpoint',
  C_SE_TB_1: 'scaledSetpoint',
  C_SE_TC_1: 'shortFloatSetpoint',
  C_BO_TA_1: 'bitstringCommand',
  P_ME_NA_1: 'normalizedMeasurement',
  P_ME_NB_1: 'scaledMeasurement',
  P_ME_NC_1: 'shortFloatMeasurement',
  P_AC_NA_1: 'parameterActivation',
}

const defaultConfigNameByAsdu = (asduAddress: string): string => {
  const normalized = String(asduAddress || '')
    .trim()
    .toUpperCase()
  if (!normalized) return t('slave.profileDialogs.defaultPointNameFallback')
  const key = DEFAULT_POINT_NAME_KEY_BY_ASDU[normalized]
  if (!key) return t('slave.profileDialogs.defaultPointNameFallback')
  const translated = t(`slave.profileDialogs.defaultPointNames.${key}`)
  return translated === `slave.profileDialogs.defaultPointNames.${key}`
    ? t('slave.profileDialogs.defaultPointNameFallback')
    : translated
}

export function useSlaveStationProfileDialogs(options: UseSlaveStationProfileDialogsOptions) {
  const defaultLinkParams = sanitizeLinkParams()

  const showSlaveDialog = ref(false)
  const showEditConnectionDialog = ref(false)
  const connectionDialogMode = ref<'create' | 'edit'>('edit')
  const slaveDialogMode = ref<'create' | 'edit'>('create')
  const dialogPanel = ref<'base' | 'objects'>('base')
  const selectedConfigIndex = ref(-1)
  const slaveDialogSnapshot = ref('')
  const connectionDialogSnapshot = ref('')
  const slaveBaseFormRef = ref<FormInstance>()
  const configDetailFormRef = ref<FormInstance>()
  const objNameInputRef = ref<InstanceType<(typeof import('element-plus'))['ElInput']>>()

  const slaveForm = ref<SlaveForm>({
    name: '',
    description: '',
    commonAddress: 1,
    selectTimeout: 15,
    enableSq1Upload: false,
    enableSoe: false,
    controlExecutionMode: 'sbo',
  })

  const cachedCustomLinkParams = ref<LinkParams | null>(null)
  const connectionForm = ref<ConnectionForm>({
    name: '',
    listenAddress: '0.0.0.0',
    listenPort: 2404,
    redundancyMode: 'single',
    linkParamsCustomEnabled: false,
    kValue: defaultLinkParams.k_value,
    wValue: defaultLinkParams.w_value,
    t0: defaultLinkParams.t0_seconds,
    t1: defaultLinkParams.t1_seconds,
    t2: defaultLinkParams.t2_seconds,
    t3: defaultLinkParams.t3_seconds,
    maxAsduBytes: defaultLinkParams.max_asdu_bytes,
    selectTimeout: defaultLinkParams.select_timeout_seconds,
    enableSq1Upload: defaultLinkParams.enable_sq1_upload,
    enableSoe: defaultLinkParams.enable_soe,
    unknownTypeidNegativeAck: defaultLinkParams.unknown_typeid_negative_ack,
  })

  const editingConfigObjects = ref<ConfigObject[]>([])
  const activeConfigObject = computed<ConfigObject | null>(() => {
    const index = selectedConfigIndex.value
    if (index < 0 || index >= editingConfigObjects.value.length) return null
    return editingConfigObjects.value[index] ?? null
  })
  const activeConfigNameSample = computed(() => {
    const object = activeConfigObject.value
    if (!object) return ''
    const typeName = String(object.asduAddress || '')
      .trim()
      .toUpperCase()
    const typeId = iec104TypeNameToId(typeName)
    if (typeId == null) return ''
    let address: number
    try {
      const addresses = resolveConfigObjectAddresses(object)
      if (addresses.length === 0) return ''
      address = addresses[0]
    } catch {
      return ''
    }
    const base = String(object.name || '').trim() || defaultConfigNameByAsdu(typeName)
    const template = String(object.nameTemplate || DEFAULT_POINT_NAME_TEMPLATE)
    return renderPointNameTemplate(template, base, typeName, typeId, address)
  })
  const configObjectAddressCount = (object: ConfigObject): number => {
    try {
      return resolveConfigObjectAddresses(object).length
    } catch {
      return 0
    }
  }

  const buildCustomLinkParamsFromForm = () =>
    sanitizeLinkParams({
      k_value: connectionForm.value.kValue,
      w_value: connectionForm.value.wValue,
      t0_seconds: connectionForm.value.t0,
      t1_seconds: connectionForm.value.t1,
      t2_seconds: connectionForm.value.t2,
      t3_seconds: connectionForm.value.t3,
      max_asdu_bytes: connectionForm.value.maxAsduBytes,
      select_timeout_seconds: connectionForm.value.selectTimeout,
      enable_sq1_upload: connectionForm.value.enableSq1Upload,
      enable_soe: connectionForm.value.enableSoe,
      unknown_typeid_negative_ack: connectionForm.value.unknownTypeidNegativeAck,
    })

  const applyLinkParamsToForm = (params: LinkParams) => {
    connectionForm.value.kValue = params.k_value
    connectionForm.value.wValue = params.w_value
    connectionForm.value.t0 = params.t0_seconds
    connectionForm.value.t1 = params.t1_seconds
    connectionForm.value.t2 = params.t2_seconds
    connectionForm.value.t3 = params.t3_seconds
    connectionForm.value.selectTimeout = params.select_timeout_seconds
    connectionForm.value.enableSq1Upload = params.enable_sq1_upload
    connectionForm.value.enableSoe = params.enable_soe
    connectionForm.value.unknownTypeidNegativeAck = params.unknown_typeid_negative_ack
  }

  const handleLinkParamsCustomEnabledChange = (value: string | number | boolean) => {
    const enabled = value === true
    if (enabled) {
      const restore = cachedCustomLinkParams.value ?? { ...defaultLinkParams }
      cachedCustomLinkParams.value = { ...restore }
      applyLinkParamsToForm(restore)
      return
    }

    cachedCustomLinkParams.value = buildCustomLinkParamsFromForm()
    applyLinkParamsToForm(defaultLinkParams)
  }

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      ElMessage.success({
        message: t('slave.profileDialogs.messages.copied', { text }),
        duration: 1500,
        grouping: true,
      })
    } catch {
      ElMessage.warning(t('slave.profileDialogs.messages.copyFailed'))
    }
  }

  const handleConnectionBadgeClick = (index: number, badge: string) => {
    if (index !== 0) return
    const normalized = String(badge || '').trim()
    if (!normalized) return
    void copyToClipboard(normalized)
  }

  const getSlavesForSelectedStation = (stationId?: string) => {
    const sid = String(stationId || options.stationId.value || '').trim()
    if (!sid) return []
    return options.slavesByStation.value?.[sid] ?? []
  }

  const resolveStationPolicySeed = (stationId?: string): SlaveStationPolicy => {
    const rows = getSlavesForSelectedStation(stationId)
    for (const row of rows) {
      if (row.effective_station_policy) {
        return sanitizeStationPolicy(row.effective_station_policy)
      }
    }
    return sanitizeStationPolicy()
  }

  const findSlaveRowById = (stationId: string, slaveId: number): SlaveResponse | null =>
    getSlavesForSelectedStation(stationId).find((item) => Number(item.id) === Number(slaveId)) ??
    null

  const buildStationPolicyOverrideFromForm = (): SlaveStationPolicyOverride => ({
    select_timeout_seconds: Math.max(
      1,
      Math.min(3600, Math.trunc(Number(slaveForm.value.selectTimeout) || 15)),
    ),
    enable_sq1_upload: Boolean(slaveForm.value.enableSq1Upload),
    enable_soe: Boolean(slaveForm.value.enableSoe),
    control_execution_mode: slaveForm.value.controlExecutionMode,
  })

  const suggestNewSlaveName = () => {
    const used = new Set(getSlavesForSelectedStation().map((item) => item.name))
    for (let idx = 1; idx < 1000; idx += 1) {
      const candidate = t('slave.profileDialogs.defaultNames.slave', { index: idx })
      if (!used.has(candidate)) return candidate
    }
    return t('slave.profileDialogs.defaultNames.slaveTimestamp', { timestamp: Date.now() })
  }

  const ensureUniqueSlaveName = (desired: string) => {
    const used = new Set(getSlavesForSelectedStation().map((item) => item.name))
    if (!used.has(desired)) return desired

    for (let idx = 2; idx < 1000; idx += 1) {
      const candidate = `${desired}-${idx}`
      if (!used.has(candidate)) return candidate
    }
    return `${desired}-${Date.now()}`
  }

  const isSlaveNameUsedByOther = (desired: string, currentStationId?: string) => {
    const trimmed = desired.trim()
    if (!trimmed) return false

    return getSlavesForSelectedStation().some((item) => {
      if (currentStationId && String(item.id) === currentStationId) return false
      return item.name.trim() === trimmed
    })
  }

  const suggestNextCommonAddress = () => {
    const max = Math.max(
      0,
      ...getSlavesForSelectedStation().map((item) => item.common_address ?? 0),
    )
    return Math.min(65535, Math.max(1, max + 1))
  }

  const suggestNewListenerName = () => {
    const used = new Set(
      (options.stations.value ?? []).map((item) => String(item.name || '').trim()),
    )
    for (let idx = 1; idx < 1000; idx += 1) {
      const candidate = t('slave.profileDialogs.defaultNames.listener', { index: idx })
      if (!used.has(candidate)) return candidate
    }
    return t('slave.profileDialogs.defaultNames.listenerTimestamp', {
      timestamp: Date.now(),
    })
  }

  const initializeConnectionForm = (listenerNode: DeviceNode | null, mode: 'create' | 'edit') => {
    const isCreateMode = mode === 'create'
    const normalizedHost =
      (listenerNode?.address ? String(listenerNode.address).trim() : '') ||
      String(options.host.value || '').trim() ||
      '0.0.0.0'
    const normalizedPort = Math.max(
      1,
      Math.min(65535, Math.trunc(Number(listenerNode?.port ?? options.port.value ?? 2404))),
    )

    const normalizedRedundancyMode: RedundancyGroupMode = isCreateMode
      ? 'single'
      : options.redundancyMode.value === 'multi' || options.redundancyMode.value === 'connection'
        ? options.redundancyMode.value
        : 'single'

    const savedLinkParams = isCreateMode
      ? { ...defaultLinkParams }
      : sanitizeLinkParams(options.linkParams.value)
    const shouldEnableCustomLinkParams = isCreateMode
      ? false
      : !isSameLinkParams(savedLinkParams, defaultLinkParams)
    cachedCustomLinkParams.value = shouldEnableCustomLinkParams ? { ...savedLinkParams } : null

    const linkParamsForForm = shouldEnableCustomLinkParams ? savedLinkParams : defaultLinkParams

    connectionForm.value = {
      name: isCreateMode
        ? suggestNewListenerName()
        : listenerNode?.name || listenerNode?.label.split(' (')[0] || suggestNewListenerName(),
      listenAddress: normalizedHost,
      listenPort: normalizedPort,
      redundancyMode: normalizedRedundancyMode,
      linkParamsCustomEnabled: shouldEnableCustomLinkParams,
      kValue: Number(linkParamsForForm.k_value),
      wValue: Number(linkParamsForForm.w_value),
      t0: Number(linkParamsForForm.t0_seconds),
      t1: Number(linkParamsForForm.t1_seconds),
      t2: Number(linkParamsForForm.t2_seconds),
      t3: Number(linkParamsForForm.t3_seconds),
      maxAsduBytes: Number(savedLinkParams.max_asdu_bytes),
      selectTimeout: Number(linkParamsForForm.select_timeout_seconds),
      enableSq1Upload: Boolean(linkParamsForForm.enable_sq1_upload),
      enableSoe: Boolean(linkParamsForForm.enable_soe),
      unknownTypeidNegativeAck: Boolean(linkParamsForForm.unknown_typeid_negative_ack),
    }
  }

  const createDefaultTemplateObjects = (): ConfigObject[] => {
    const base = Date.now()
    return [
      {
        id: `config-${base}-sp`,
        name: defaultConfigNameByAsdu('M_SP_NA_1'),
        asduAddress: 'M_SP_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(1),
        ioaCount: 100,
      },
      {
        id: `config-${base}-dp`,
        name: defaultConfigNameByAsdu('M_DP_NA_1'),
        asduAddress: 'M_DP_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(3),
        ioaCount: 100,
      },
      {
        id: `config-${base}-st`,
        name: defaultConfigNameByAsdu('M_ST_NA_1'),
        asduAddress: 'M_ST_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(5),
        ioaCount: 20,
      },
      {
        id: `config-${base}-bo-monitor`,
        name: defaultConfigNameByAsdu('M_BO_NA_1'),
        asduAddress: 'M_BO_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(7),
        ioaCount: 20,
      },
      {
        id: `config-${base}-me`,
        name: defaultConfigNameByAsdu('M_ME_NC_1'),
        asduAddress: 'M_ME_NC_1',
        ioaStartAddress: iec104DefaultStartAddress(13),
        ioaCount: 100,
      },
      {
        id: `config-${base}-it`,
        name: defaultConfigNameByAsdu('M_IT_NA_1'),
        asduAddress: 'M_IT_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(15),
        ioaCount: 20,
      },
      {
        id: `config-${base}-sc`,
        name: defaultConfigNameByAsdu('C_SC_NA_1'),
        asduAddress: 'C_SC_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(45),
        ioaCount: 100,
      },
      {
        id: `config-${base}-dc`,
        name: defaultConfigNameByAsdu('C_DC_NA_1'),
        asduAddress: 'C_DC_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(46) + 0x80,
        ioaCount: 100,
      },
      {
        id: `config-${base}-rc`,
        name: defaultConfigNameByAsdu('C_RC_NA_1'),
        asduAddress: 'C_RC_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(47) + 0x100,
        ioaCount: 20,
      },
      {
        id: `config-${base}-se`,
        name: defaultConfigNameByAsdu('C_SE_NC_1'),
        asduAddress: 'C_SE_NC_1',
        ioaStartAddress: iec104DefaultStartAddress(50),
        ioaCount: 100,
      },
      {
        id: `config-${base}-bo-command`,
        name: defaultConfigNameByAsdu('C_BO_NA_1'),
        asduAddress: 'C_BO_NA_1',
        ioaStartAddress: iec104DefaultStartAddress(51),
        ioaCount: 20,
      },
    ].map((object) => ({ ...object, nameTemplate: DEFAULT_POINT_NAME_TEMPLATE }))
  }

  const buildTemplateTypeKey = (object: Pick<ConfigObject, 'asduAddress'>) =>
    String(object.asduAddress || '')
      .trim()
      .toUpperCase() || 'M_ME_NC_1'

  const handleApplyDefaultTemplate = async () => {
    const defaults = createDefaultTemplateObjects()
    if (defaults.length === 0) return

    if (editingConfigObjects.value.length > 0) {
      const confirmed = await showAppConfirmDialog({
        title: t('slave.profileDialogs.confirm.appendTemplateTitle'),
        message: t('slave.profileDialogs.confirm.appendTemplateMessage'),
        type: 'info',
        confirmButtonText: t('slave.profileDialogs.confirm.append'),
        cancelButtonText: t('common.cancel'),
      })
      if (!confirmed) return
    }

    const existingTypes = new Set(editingConfigObjects.value.map(buildTemplateTypeKey))
    const seed = Date.now()
    let appended = 0
    defaults.forEach((item, index) => {
      const typeKey = buildTemplateTypeKey(item)
      if (existingTypes.has(typeKey)) return

      editingConfigObjects.value.push({
        ...item,
        id: `config-template-${seed}-${index}`,
      })
      existingTypes.add(typeKey)
      appended += 1
    })

    if (appended <= 0) {
      ElMessage.info(t('slave.profileDialogs.messages.templateExists'))
      return
    }
    if (selectedConfigIndex.value < 0) {
      selectedConfigIndex.value = 0
    }
    dialogPanel.value = 'objects'
    ElMessage.success(t('slave.profileDialogs.messages.templateAppended', { count: appended }))
  }

  const handleCreateListener = () => {
    options.selectedSlave.value = null
    connectionDialogMode.value = 'create'
    initializeConnectionForm(null, 'create')
    connectionDialogSnapshot.value = JSON.stringify(connectionForm.value)
    showEditConnectionDialog.value = true
  }

  const handleCreateSlave = (listenerNode?: DeviceNode | null) => {
    const targetStationId = String(listenerNode?.id || options.stationId.value || '').trim()
    if (!targetStationId) {
      ElMessage.warning(t('slave.profileDialogs.messages.selectConnectionBeforeCreateSlave'))
      return
    }
    const policySeed = resolveStationPolicySeed(targetStationId)
    options.selectedSlave.value = listenerNode ?? null
    slaveDialogMode.value = 'create'
    dialogPanel.value = 'base'
    slaveForm.value = {
      name: suggestNewSlaveName(),
      description: '',
      commonAddress: suggestNextCommonAddress(),
      selectTimeout: policySeed.select_timeout_seconds,
      enableSq1Upload: policySeed.enable_sq1_upload,
      enableSoe: policySeed.enable_soe,
      controlExecutionMode: policySeed.control_execution_mode,
    }
    editingConfigObjects.value = []
    selectedConfigIndex.value = -1
    captureSlaveDialogSnapshot()
    showSlaveDialog.value = true
    nextTick(() => {
      slaveBaseFormRef.value?.clearValidate()
      configDetailFormRef.value?.clearValidate()
    })
  }

  const handleEditSlave = async (slave: DeviceNode | null) => {
    if (!slave || slave.type !== 'slave') return

    slaveDialogMode.value = 'edit'
    dialogPanel.value = 'base'
    options.selectedSlave.value = slave
    const stationId = String(slave.parentStationId || options.stationId.value || '').trim()
    const slaveId = Number(slave.slaveId ?? 0)
    const slaveRow =
      stationId && Number.isFinite(slaveId) && slaveId > 0
        ? findSlaveRowById(stationId, slaveId)
        : null
    const effectivePolicy = sanitizeStationPolicy(slaveRow?.effective_station_policy)
    const stationPolicyOverride = sanitizeStationPolicyOverride(slaveRow?.station_policy_override)

    slaveForm.value = {
      name: slave.name || slave.label.split(' (')[0],
      description: slave.description || '',
      commonAddress: slave.commonAddress || 1,
      selectTimeout:
        stationPolicyOverride.select_timeout_seconds ?? effectivePolicy.select_timeout_seconds,
      enableSq1Upload: stationPolicyOverride.enable_sq1_upload ?? effectivePolicy.enable_sq1_upload,
      enableSoe: stationPolicyOverride.enable_soe ?? effectivePolicy.enable_soe,
      controlExecutionMode:
        stationPolicyOverride.control_execution_mode ?? effectivePolicy.control_execution_mode,
    }

    await loadSlaveConfigObjects(slave)

    captureSlaveDialogSnapshot()
    showSlaveDialog.value = true
    nextTick(() => {
      slaveBaseFormRef.value?.clearValidate()
      configDetailFormRef.value?.clearValidate()
    })
  }

  const handleEditConnection = (slave: DeviceNode | null) => {
    if (!slave) return
    const listenerNode =
      slave.type === 'listener'
        ? slave
        : (options.deviceTreeData.value.find(
            (item) => item.type === 'listener' && item.id === slave.parentStationId,
          ) ?? null)
    if (!listenerNode) return

    options.selectedSlave.value = listenerNode
    connectionDialogMode.value = 'edit'
    initializeConnectionForm(listenerNode, 'edit')
    connectionDialogSnapshot.value = JSON.stringify(connectionForm.value)
    showEditConnectionDialog.value = true
  }

  const serializeConfigObjectsForSubmit = () =>
    editingConfigObjects.value.map((object, index) => {
      const asduAddress = String(object.asduAddress || '').trim() || 'M_ME_NC_1'
      const fallbackName = defaultConfigNameByAsdu(asduAddress)
      return {
        name: String(object.name || '').trim() || `${fallbackName}-${index + 1}`,
        asduAddress,
        ioaStartAddress: Math.max(1, Math.trunc(Number(object.ioaStartAddress) || 1)),
        ioaCount: Math.max(1, Math.trunc(Number(object.ioaCount) || 1)),
      }
    })

  const buildPointDefsForSubmit = (): PointDef[] => {
    const defs: PointDef[] = []
    const usedAddresses = new Set<number>()
    for (const object of editingConfigObjects.value) {
      const addresses = resolveConfigObjectAddresses(object)
      if (addresses.length === 0) {
        throw new Error(t('slave.profileDialogs.validation.invalidIoaBatch'))
      }
      const typeName = String(object.asduAddress || '')
        .trim()
        .toUpperCase()
      const typeId = iec104TypeNameToId(typeName)
      if (typeId == null) {
        throw new Error(t('slave.profileDialogs.validation.selectAsduType'))
      }
      const base = String(object.name || '').trim() || defaultConfigNameByAsdu(typeName)
      const template = String(object.nameTemplate || DEFAULT_POINT_NAME_TEMPLATE).trim()
      const unknownPlaceholder = Array.from(
        template.matchAll(/\{[^{}]+\}/g),
        (match) => match[0],
      ).find((placeholder) => !POINT_NAME_TEMPLATE_PLACEHOLDERS.has(placeholder))
      if (unknownPlaceholder) {
        throw new Error(
          t('slave.profileDialogs.validation.unknownNamePlaceholder', {
            placeholder: unknownPlaceholder,
          }),
        )
      }
      for (const address of addresses) {
        if (usedAddresses.has(address)) {
          throw new Error(t('slave.profileDialogs.validation.ioaConflict', { ioa: address }))
        }
        usedAddresses.add(address)
        const name = renderPointNameTemplate(template, base, typeName, typeId, address).trim()
        if (!name) throw new Error(t('slave.profileDialogs.validation.generatedNameEmpty'))
        defs.push({
          address,
          name,
          type_id: typeId,
          data_type: typeName,
          description: null,
          control_ioa: null,
          default_value: iec104ProtocolDefaultValue(typeId),
          is_enabled: true,
        })
      }
    }
    return defs.sort((left, right) => left.address - right.address)
  }

  function buildSlaveDialogSnapshot(): string {
    const normalizedCommonAddress = Math.max(
      1,
      Math.trunc(Number(slaveForm.value.commonAddress) || 0),
    )
    const normalizedObjects = editingConfigObjects.value.map((object, index) => {
      const asduAddress =
        String(object.asduAddress || '')
          .trim()
          .toUpperCase() || 'M_ME_NC_1'
      const fallbackName = defaultConfigNameByAsdu(asduAddress)
      return {
        name: String(object.name || '').trim() || `${fallbackName}-${index + 1}`,
        asduAddress,
        ioaStartAddress: Math.max(1, Math.trunc(Number(object.ioaStartAddress) || 1)),
        ioaCount: Math.max(1, Math.trunc(Number(object.ioaCount) || 1)),
        addressMode: object.addressMode ?? 'continuous',
        ioaExpression: String(object.ioaExpression || ''),
        nameTemplate: String(object.nameTemplate || DEFAULT_POINT_NAME_TEMPLATE),
      }
    })
    return JSON.stringify({
      mode: slaveDialogMode.value,
      name: String(slaveForm.value.name || '').trim(),
      commonAddress: normalizedCommonAddress,
      stationPolicyOverride: buildStationPolicyOverrideFromForm(),
      configObjects: normalizedObjects,
    })
  }

  function captureSlaveDialogSnapshot() {
    slaveDialogSnapshot.value = buildSlaveDialogSnapshot()
  }

  const loadSlaveConfigObjectsFromTree = (slave: DeviceNode) => {
    if (slave.children && slave.children.length > 0) {
      let nextAddress = 1
      editingConfigObjects.value = slave.children
        .filter((child) => child.type === 'type-id' && Number(child.count ?? 0) > 0)
        .map((child, index) => {
          const ioaCount = Math.max(1, Math.trunc(Number(child.count) || 1))
          const normalizedChildName = normalizeIecObjectDisplayName(
            String(child.name || child.label || '').trim(),
          )
          const object = {
            id: `config-fallback-${child.id || index}`,
            name: normalizedChildName || defaultConfigNameByAsdu(child.dataType || 'M_ME_NC_1'),
            asduAddress: child.dataType || 'M_ME_NC_1',
            ioaStartAddress: nextAddress,
            ioaCount,
            nameTemplate: DEFAULT_POINT_NAME_TEMPLATE,
          }
          nextAddress += ioaCount
          return object
        })
    } else {
      editingConfigObjects.value = []
    }
    selectedConfigIndex.value = editingConfigObjects.value.length > 0 ? 0 : -1
  }

  type RuntimeConfigObject = ConfigObject & {
    nameCandidates: Map<string, number>
  }

  const pickConfigObjectName = (asduAddress: string, candidates: Map<string, number>): string => {
    let bestName = ''
    let bestCount = -1
    for (const [name, count] of candidates.entries()) {
      if (!name) continue
      if (count > bestCount) {
        bestName = name
        bestCount = count
      }
    }
    return bestName || defaultConfigNameByAsdu(asduAddress)
  }

  const buildConfigObjectsFromPointDefs = (defs: PointDef[]): ConfigObject[] => {
    const normalizedDefs = defs
      .map((def) => {
        const address = Math.max(1, Math.trunc(Number(def.address) || 0))
        if (address <= 0) return null

        return {
          address,
          asduAddress:
            String(def.data_type || '')
              .trim()
              .toUpperCase() || 'M_ME_NC_1',
          objectName: extractIecObjectNameFromPointName(def.name, address),
        }
      })
      .filter((item): item is NonNullable<typeof item> => item != null)
      .sort((left, right) => left.address - right.address)

    const objects: ConfigObject[] = []
    let current: RuntimeConfigObject | null = null

    const flushCurrent = () => {
      if (!current) return
      objects.push({
        id: current.id,
        name: pickConfigObjectName(current.asduAddress, current.nameCandidates),
        asduAddress: current.asduAddress,
        ioaStartAddress: current.ioaStartAddress,
        ioaCount: current.ioaCount,
      })
      current = null
    }

    for (const def of normalizedDefs) {
      const canAppend =
        current != null &&
        current.asduAddress === def.asduAddress &&
        def.address === current.ioaStartAddress + current.ioaCount

      if (!canAppend) {
        flushCurrent()
        current = {
          id: `config-runtime-${def.asduAddress}-${def.address}`,
          name: '',
          asduAddress: def.asduAddress,
          ioaStartAddress: def.address,
          ioaCount: 1,
          nameCandidates: new Map<string, number>(),
        }
      } else if (current) {
        current.ioaCount += 1
      }

      if (def.objectName && current) {
        current.nameCandidates.set(
          def.objectName,
          (current.nameCandidates.get(def.objectName) ?? 0) + 1,
        )
      }
    }

    flushCurrent()
    return objects
  }

  const loadSlaveConfigObjects = async (slave: DeviceNode) => {
    const slaveId = Number(slave.slaveId ?? 0)
    if (Number.isFinite(slaveId) && slaveId > 0) {
      try {
        const defs = await getStationSlavePointDefs(slaveId)
        const objects = buildConfigObjectsFromPointDefs(defs)
        if (objects.length > 0) {
          editingConfigObjects.value = objects
          selectedConfigIndex.value = 0
          return
        }
      } catch (error) {
        ElMessage.warning(
          t('slave.profileDialogs.messages.loadPointTableFallback', {
            error: String(error),
          }),
        )
      }
    }

    loadSlaveConfigObjectsFromTree(slave)
  }

  const handleAddConfigObject = () => {
    const defaultAsduAddress = 'M_SP_NA_1'
    editingConfigObjects.value.push({
      id: `config-${Date.now()}`,
      name: defaultConfigNameByAsdu(defaultAsduAddress),
      asduAddress: defaultAsduAddress,
      addressMode: 'continuous',
      ioaStartAddress: 1,
      ioaCount: 10,
      ioaExpression: '',
      nameTemplate: DEFAULT_POINT_NAME_TEMPLATE,
    })
    selectedConfigIndex.value = editingConfigObjects.value.length - 1
    dialogPanel.value = 'objects'
    nextTick(() => {
      configDetailFormRef.value?.clearValidate()
      objNameInputRef.value?.focus()
      const activeItem = document.querySelector('.slave-obj-item--active')
      if (activeItem) {
        activeItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
      }
    })
  }

  const handleActiveConfigAsduChange = (nextAsduAddress: string | number) => {
    const target = activeConfigObject.value
    if (!target) return
    target.name = defaultConfigNameByAsdu(String(nextAsduAddress || target.asduAddress || ''))
  }

  const handleSelectConfigObject = (index: number) => {
    selectedConfigIndex.value = index
    nextTick(() => {
      configDetailFormRef.value?.clearValidate()
    })
  }

  const ioaConflictMap = computed(() => {
    const map = new Map<string, string>()
    const objects = editingConfigObjects.value
    for (let i = 0; i < objects.length; i += 1) {
      const left = objects[i]
      let leftAddresses: number[]
      try {
        leftAddresses = resolveConfigObjectAddresses(left)
      } catch (error) {
        map.set(left.id, error instanceof Error ? error.message : String(error))
        continue
      }
      for (let j = i + 1; j < objects.length; j += 1) {
        const right = objects[j]
        let rightAddresses: number[]
        try {
          rightAddresses = resolveConfigObjectAddresses(right)
        } catch (error) {
          map.set(right.id, error instanceof Error ? error.message : String(error))
          continue
        }
        const rightSet = new Set(rightAddresses)
        const overlaps = leftAddresses.filter((address) => rightSet.has(address))
        if (overlaps.length > 0) {
          const overlapStart = overlaps[0]
          const overlapEnd = overlaps[overlaps.length - 1]
          const leftDesc = t('slave.profileDialogs.warnings.ioaOverlap', {
            name: right.name,
            start: overlapStart,
            end: overlapEnd,
          })
          const rightDesc = t('slave.profileDialogs.warnings.ioaOverlap', {
            name: left.name,
            start: overlapStart,
            end: overlapEnd,
          })
          const leftExisting = map.get(left.id)
          map.set(left.id, leftExisting ? `${leftExisting}; ${leftDesc}` : leftDesc)
          const rightExisting = map.get(right.id)
          map.set(right.id, rightExisting ? `${rightExisting}; ${rightDesc}` : rightDesc)
        }
      }
    }
    return map
  })

  const handleDuplicateByIndex = (index: number) => {
    if (index < 0 || index >= editingConfigObjects.value.length) return
    const source = editingConfigObjects.value[index]
    editingConfigObjects.value.push({
      id: `config-${Date.now()}`,
      name: `${source.name} (${t('slave.profileDialogs.duplicateSuffix')})`,
      asduAddress: source.asduAddress,
      ioaStartAddress: Number(source.ioaStartAddress) + Number(source.ioaCount),
      ioaCount: source.ioaCount,
      addressMode: source.addressMode ?? 'continuous',
      ioaExpression: source.ioaExpression || '',
      nameTemplate: source.nameTemplate || DEFAULT_POINT_NAME_TEMPLATE,
    })
    selectedConfigIndex.value = editingConfigObjects.value.length - 1
    nextTick(() => {
      configDetailFormRef.value?.clearValidate()
    })
  }

  const removeConfigObjectAtIndex = (index: number) => {
    editingConfigObjects.value.splice(index, 1)
    if (editingConfigObjects.value.length === 0) {
      selectedConfigIndex.value = -1
      dialogPanel.value = 'base'
      return
    }
    const nextIndex = Math.min(index, editingConfigObjects.value.length - 1)
    selectedConfigIndex.value = Math.max(0, nextIndex)
  }

  const confirmAndRemoveConfigObject = async (index: number) => {
    if (index < 0 || index >= editingConfigObjects.value.length) return

    const target = editingConfigObjects.value[index]
    const confirmed = await confirmDangerousAction(
      t('slave.profileDialogs.confirm.deleteObjectMessage', {
        name: target?.name || t('slave.profileDialogs.defaultNames.object', { index: index + 1 }),
      }),
      t('slave.profileDialogs.confirm.deleteObjectTitle'),
      t('slave.profileDialogs.confirm.delete'),
    )
    if (!confirmed) return

    removeConfigObjectAtIndex(index)
  }

  const handleRemoveConfigObjectByIndex = (index: number) => {
    void confirmAndRemoveConfigObject(index)
  }

  const hasSlaveDialogUnsavedChanges = computed(() => {
    if (!showSlaveDialog.value) return false
    if (!slaveDialogSnapshot.value) return false
    return buildSlaveDialogSnapshot() !== slaveDialogSnapshot.value
  })

  const hasConnectionDialogUnsavedChanges = computed(() => {
    if (!showEditConnectionDialog.value || !connectionDialogSnapshot.value) return false
    return JSON.stringify(connectionForm.value) !== connectionDialogSnapshot.value
  })

  const closeConnectionDialog = () => {
    showEditConnectionDialog.value = false
    connectionDialogMode.value = 'edit'
    connectionDialogSnapshot.value = ''
  }

  const {
    handleBeforeClose: handleConnectionDialogBeforeClose,
    requestClose: handleCloseConnectionDialog,
  } = useDialogCloseGuard({
    isDirty: () => hasConnectionDialogUnsavedChanges.value,
    onClose: closeConnectionDialog,
  })

  const connectionDialogSubtitleBadges = computed(() => {
    const badges: string[] = []
    const connectionName = String(connectionForm.value.name || '').trim()
    const host = String(connectionForm.value.listenAddress || '').trim() || '0.0.0.0'
    const port = Number(connectionForm.value.listenPort || 2404)
    badges.push(connectionName || t('slave.profileDialogs.defaultNames.unnamedConnection'))
    badges.push(`${host}:${port}`)
    return badges
  })

  const slaveDialogConnectionName = computed(() => {
    const node = options.selectedSlave.value
    if (!node) return ''
    const listenerNode =
      node.type === 'listener'
        ? node
        : options.deviceTreeData.value.find(
            (item) => item.id === node.parentStationId && item.type === 'listener',
          )
    if (!listenerNode) return ''
    return (
      listenerNode.name ||
      listenerNode.label?.split(' (')[0] ||
      t('slave.profileDialogs.defaultNames.unknownConnection')
    )
  })

  const slaveDialogSubtitleBadges = computed(() => {
    const badges: string[] = []
    if (slaveDialogConnectionName.value) badges.push(slaveDialogConnectionName.value)
    const slaveName = String(slaveForm.value.name || '').trim()
    if (slaveName) {
      badges.push(slaveName)
    } else {
      badges.push(
        slaveDialogMode.value === 'create'
          ? t('slave.profileDialogs.defaultNames.slaveConfig')
          : t('slave.profileDialogs.defaultNames.slaveFallback'),
      )
    }
    return badges
  })

  const linkParamsCollapsedDesc = computed(() => {
    const form = connectionForm.value
    return `K=${form.kValue} W=${form.wValue} T0=${form.t0}s T1=${form.t1}s T2=${form.t2}s T3=${form.t3}s`
  })

  const linkParamsWarnWK = computed(() => {
    const k = Number(connectionForm.value.kValue || 0)
    const w = Number(connectionForm.value.wValue || 0)
    return w >= k ? t('slave.profileDialogs.warnings.wk', { w, k }) : ''
  })

  const linkParamsWarnT2T1 = computed(() => {
    const t1 = Number(connectionForm.value.t1 || 0)
    const t2 = Number(connectionForm.value.t2 || 0)
    return t2 >= t1 ? t('slave.profileDialogs.warnings.t2t1', { t2, t1 }) : ''
  })

  const connectionDialogTitle = computed(() =>
    connectionDialogMode.value === 'create'
      ? t('slave.profileDialogs.titles.createConnection')
      : t('slave.profileDialogs.titles.editConnection'),
  )
  const connectionDialogConfirmLabel = computed(() => t('common.save'))

  const asduTypeOptions = computed(() =>
    getLoadedIec104Capabilities()
      .filter(
        (capability) =>
          capability.point_configurable && (capability.slave_receive || capability.slave_upload),
      )
      .sort((left, right) => left.type_id - right.type_id)
      .map((capability) => ({
        label: formatIec104TypeLabel(capability.type_id),
        value: capability.name,
      })),
  )

  const slaveFormRules = computed<FormRules<SlaveForm>>(() => ({
    name: [
      {
        required: true,
        message: t('slave.profileDialogs.validation.enterSlaveName'),
        trigger: ['blur', 'change'],
      },
    ],
    commonAddress: [
      {
        required: true,
        message: t('slave.profileDialogs.validation.enterCommonAddress'),
        trigger: ['blur', 'change'],
      },
      {
        type: 'number',
        min: 1,
        max: 65535,
        message: t('slave.profileDialogs.validation.commonAddressRange'),
        trigger: ['blur', 'change'],
      },
    ],
  }))

  const validateIoaBatch = (_rule: unknown, _value: unknown, callback: (error?: Error) => void) => {
    const current = activeConfigObject.value
    if (!current) {
      callback()
      return
    }
    try {
      const addresses = resolveConfigObjectAddresses(current)
      if (addresses.length === 0) {
        callback(new Error(t('slave.profileDialogs.validation.invalidIoaBatch')))
        return
      }
    } catch (error) {
      callback(error instanceof Error ? error : new Error(String(error)))
      return
    }
    callback()
  }

  const configObjectRules = computed<FormRules<ConfigObject>>(() => ({
    name: [
      {
        required: true,
        message: t('slave.profileDialogs.validation.enterObjectName'),
        trigger: ['blur', 'change'],
      },
    ],
    asduAddress: [
      {
        required: true,
        message: t('slave.profileDialogs.validation.selectAsduType'),
        trigger: ['change', 'blur'],
      },
    ],
    ioaStartAddress: [
      {
        type: 'number',
        min: 1,
        max: 16777215,
        message: t('slave.profileDialogs.validation.ioaStartRange'),
        trigger: ['blur', 'change'],
      },
      { validator: validateIoaBatch, trigger: ['blur', 'change'] },
    ],
    ioaCount: [
      {
        type: 'number',
        min: 1,
        max: 1000,
        message: t('slave.profileDialogs.validation.ioaCountRange'),
        trigger: ['blur', 'change'],
      },
      { validator: validateIoaBatch, trigger: ['blur', 'change'] },
    ],
    ioaExpression: [{ validator: validateIoaBatch, trigger: ['blur', 'change'] }],
  }))

  const handleCloseSlaveDialog = () => {
    showSlaveDialog.value = false
    dialogPanel.value = 'base'
    slaveDialogSnapshot.value = ''
    editingConfigObjects.value = []
    selectedConfigIndex.value = -1
    slaveBaseFormRef.value?.resetFields()
    configDetailFormRef.value?.clearValidate()
  }

  const { requestClose: requestCloseSlaveDialog, handleBeforeClose: handleSlaveDialogBeforeClose } =
    useDialogCloseGuard({
      isDirty: () => hasSlaveDialogUnsavedChanges.value,
      onClose: handleCloseSlaveDialog,
    })

  const validateSlaveBaseForm = async () =>
    (await slaveBaseFormRef.value
      ?.validate()
      .then(() => true)
      .catch(() => false)) !== false

  const validateSlaveDetailForm = async () => {
    if (!activeConfigObject.value) return true
    return (
      (await configDetailFormRef.value
        ?.validate()
        .then(() => true)
        .catch(() => false)) !== false
    )
  }

  const validateSlaveDialogForms = async () => {
    const baseValid = await validateSlaveBaseForm()
    if (!baseValid) return false
    if (dialogPanel.value === 'base') return true
    return validateSlaveDetailForm()
  }

  const handleCreateSlaveConfirm = () => {
    const desiredName = String(slaveForm.value.name || '').trim()
    if (!desiredName) {
      ElMessage.error(t('slave.profileDialogs.validation.enterSlaveName'))
      return
    }

    const uniqueName = ensureUniqueSlaveName(desiredName)
    if (uniqueName !== desiredName) {
      slaveForm.value.name = uniqueName
      ElMessage.warning(
        t('slave.profileDialogs.messages.slaveNameAutoRenamed', { name: uniqueName }),
      )
    }

    const stationId = String(
      options.selectedSlave.value?.type === 'listener'
        ? options.selectedSlave.value.id
        : options.stationId.value || '',
    ).trim()
    if (!stationId) {
      ElMessage.error(t('slave.profileDialogs.messages.missingConnectionId'))
      return
    }

    let pointDefs: PointDef[]
    try {
      pointDefs = buildPointDefsForSubmit()
    } catch (error) {
      ElMessage.error(error instanceof Error ? error.message : String(error))
      return
    }

    options.emitStationProfileSubmit({
      scope: 'slave',
      mode: 'create',
      name: slaveForm.value.name.trim(),
      commonAddress: slaveForm.value.commonAddress,
      stationPolicyOverride: buildStationPolicyOverrideFromForm(),
      stationId,
      useDefaultTemplate: false,
      configObjects: serializeConfigObjectsForSubmit(),
      pointDefs,
    })
    handleCloseSlaveDialog()
  }

  const handleEditSlaveConfirm = () => {
    const desiredName = String(slaveForm.value.name || '').trim()
    if (!desiredName) {
      ElMessage.error(t('slave.profileDialogs.validation.enterSlaveName'))
      return
    }

    const slaveId = options.selectedSlave.value?.slaveId
    const currentId = slaveId != null ? String(slaveId) : undefined
    if (isSlaveNameUsedByOther(desiredName, currentId)) {
      ElMessage.error(t('slave.profileDialogs.messages.duplicateSlaveName'))
      return
    }

    const stationId = String(
      options.selectedSlave.value?.parentStationId || options.stationId.value || '',
    ).trim()
    if (!stationId || slaveId == null) {
      ElMessage.error(t('slave.profileDialogs.messages.missingSlaveId'))
      return
    }

    let pointDefs: PointDef[]
    try {
      pointDefs = buildPointDefsForSubmit()
    } catch (error) {
      ElMessage.error(error instanceof Error ? error.message : String(error))
      return
    }

    options.emitStationProfileSubmit({
      scope: 'slave',
      mode: 'edit',
      name: desiredName,
      commonAddress: slaveForm.value.commonAddress,
      stationPolicyOverride: buildStationPolicyOverrideFromForm(),
      stationId,
      slaveId,
      useDefaultTemplate: false,
      configObjects: serializeConfigObjectsForSubmit(),
      pointDefs,
    })
    handleCloseSlaveDialog()
  }

  const handleSlaveConfirm = async () => {
    const valid = await validateSlaveDialogForms()
    if (!valid) return

    if (slaveDialogMode.value === 'create') {
      handleCreateSlaveConfirm()
      return
    }
    handleEditSlaveConfirm()
  }

  const handleSaveConnection = () => {
    const host = String(connectionForm.value.listenAddress || '').trim()
    const port = Math.max(
      1,
      Math.min(65535, Math.trunc(Number(connectionForm.value.listenPort) || 0)),
    )
    if (!host) {
      ElMessage.error(t('slave.profileDialogs.messages.enterListenAddress'))
      return
    }

    const redundancyModePayload: RedundancyGroupMode = connectionForm.value.redundancyMode
    const customLinkParams = buildCustomLinkParamsFromForm()
    const linkParamsBase: LinkParams = connectionForm.value.linkParamsCustomEnabled
      ? customLinkParams
      : { ...defaultLinkParams }
    const linkParamsPayload: LinkParams = sanitizeLinkParams({
      ...linkParamsBase,
      max_asdu_bytes: connectionForm.value.maxAsduBytes,
    })

    const listenerNode = options.selectedSlave.value
    const mode = connectionDialogMode.value
    if (mode === 'edit' && !listenerNode) {
      ElMessage.error(t('slave.profileDialogs.messages.lostConnectionContext'))
      return
    }
    if (mode === 'edit' && listenerNode) {
      const existingNode = options.deviceTreeData.value.find(
        (item) => item.type === 'listener' && item.id === listenerNode.id,
      )
      if (existingNode) {
        existingNode.address = host
        existingNode.port = port
        existingNode.label = `${connectionForm.value.name.trim() || existingNode.label.split(' (')[0]} (${host}:${port})`
      }
    }

    options.emitStationProfileSubmit({
      scope: 'listener',
      mode,
      name: connectionForm.value.name.trim() || `${host}:${port}`,
      commonAddress:
        mode === 'edit'
          ? Number(listenerNode?.commonAddress ?? options.defaultCommonAddress.value ?? 1)
          : Number(options.defaultCommonAddress.value ?? 1),
      host,
      port,
      stationId: mode === 'edit' ? listenerNode?.id : undefined,
      linkParams: linkParamsPayload,
      redundancyMode: redundancyModePayload,
    })

    ElMessage.success(
      mode === 'create'
        ? t('slave.profileDialogs.messages.connectionCreated')
        : t('slave.profileDialogs.messages.connectionSaved'),
    )
    showEditConnectionDialog.value = false
    connectionDialogMode.value = 'edit'
    connectionDialogSnapshot.value = ''
    options.selectedSlave.value = null
  }

  return {
    activeConfigObject,
    activeConfigNameSample,
    asduTypeOptions,
    configDetailFormRef,
    configObjectRules,
    connectionDialogConfirmLabel,
    configObjectAddressCount,
    connectionDialogMode,
    connectionDialogSubtitleBadges,
    connectionDialogTitle,
    connectionForm,
    dialogPanel,
    editingConfigObjects,
    handleActiveConfigAsduChange,
    handleAddConfigObject,
    handleApplyDefaultTemplate,
    handleCloseConnectionDialog,
    handleConnectionDialogBeforeClose,
    handleConnectionBadgeClick,
    handleCreateListener,
    handleCreateSlave,
    handleDuplicateByIndex,
    handleEditConnection,
    handleEditSlave,
    handleLinkParamsCustomEnabledChange,
    handleRemoveConfigObjectByIndex,
    handleSaveConnection,
    handleSelectConfigObject,
    handleSlaveConfirm,
    handleSlaveDialogBeforeClose,
    ioaConflictMap,
    linkParamsCollapsedDesc,
    linkParamsWarnT2T1,
    linkParamsWarnWK,
    objNameInputRef,
    requestCloseSlaveDialog,
    selectedConfigIndex,
    showEditConnectionDialog,
    showSlaveDialog,
    slaveBaseFormRef,
    slaveDialogMode,
    slaveDialogSubtitleBadges,
    slaveForm,
    slaveFormRules,
  }
}
