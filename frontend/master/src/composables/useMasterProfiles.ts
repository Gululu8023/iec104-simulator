import { computed, reactive, ref } from 'vue'

import { ElMessage, ElMessageBox } from 'element-plus'

import { confirmDangerousAction } from '@shared/ui/dialogConfirm'
import { useDialogCloseGuard } from '@shared/ui/useDialogCloseGuard'
import { normalizeIecObjectDisplayName, resolveIec104TypeName } from '@shared/api/iec104'
import { t } from '@shared/i18n'
import { translateApiError } from '@shared/i18n/errors'
import {
  deriveUnifiedConnStatus,
  normalizeDataTransferState,
  normalizeTransportState,
} from '@shared/ui/connectionStatus'
import type {
  BackendConnectionInfo,
  LinkParams,
  LinkProfileResponse,
  LinkSlaveResponse,
  PointDef,
  UpsertLinkProfileRequest,
  UpsertLinkSlaveBundleRequest,
} from '@shared/api/types'

import type { DataTransferState, SlaveConnection, TransportState } from '../types/master'
import type { DataTypeCountMap, PointTypeSummary } from '../types/pointTypeSummary'
import {
  createLinkProfile,
  createLinkSlave,
  deleteLinkProfile,
  deleteLinkSlave,
  getLinkSlaveAsduAliases,
  getLinkSlavePointDefs,
  listLinkProfiles,
  listLinkSlaves,
  updateLinkProfile,
  updateLinkSlave,
} from '../api/master'

type ReadonlyRef<T> = {
  readonly value: T
}

type MasterConnectForm = {
  host: string
  port: number
}

type MasterSlaveProfile = {
  id: number
  link_profile_id: number
  name: string
  common_address: number
  enabled: boolean
  link_name: string
  host: string
  port: number
  link_params: LinkParams
  auto_connect: boolean
  auto_start_data_transfer: boolean
  auto_gi: boolean
  retry_count: number
  retry_interval: number
}

type MasterMessageStationItem = {
  id: string
  name: string
  groupName: string
  slaveName: string
  host: string
  port: number
  status: string
  isConnectionLevel?: boolean
  runtimeConnectionId?: string
  uiConnectionKey?: string
  slaveUiKey?: string
  stationConfigId?: string
  commonAddress?: number | null
}

interface UseMasterProfilesOptions {
  currentStationId: ReadonlyRef<string>
  connectForm: MasterConnectForm
  connections: ReadonlyRef<BackendConnectionInfo[]>
  disconnectFromSlave: (connectionId?: string | null) => Promise<boolean>
  onProfilesChanged?: () => void
  onProfileFocused?: (profileId: number) => void
  getSelectedProfileId?: () => number | null
  clearSelectedSlave?: () => void
}

const defaultLinkParams = (): LinkParams => ({
  k_value: 12,
  w_value: 8,
  t0_seconds: 30,
  t1_seconds: 15,
  t2_seconds: 10,
  t3_seconds: 20,
  max_asdu_bytes: 249,
  select_timeout_seconds: 15,
  enable_sq1_upload: false,
  enable_soe: false,
  unknown_typeid_negative_ack: true,
})

export function useMasterProfiles(options: UseMasterProfilesOptions) {
  const profiles = ref<MasterSlaveProfile[]>([])
  const linkProfiles = ref<LinkProfileResponse[]>([])
  const slavesByLink = ref<Record<number, LinkSlaveResponse[]>>({})
  const profilesLoading = ref(false)
  const profilePointTypeSummary = ref<Record<string, PointTypeSummary>>({})
  const profilePointDefsMap = ref<Record<string, PointDef[]>>({})

  const showProfileDialog = ref(false)
  const profileDialogMode = ref<'create' | 'edit'>('create')
  const editingProfileId = ref<number | null>(null)
  const profileDialogSnapshot = ref('')

  const showLinkDialog = ref(false)
  const linkDialogMode = ref<'create' | 'edit'>('create')
  const editingStandaloneLinkId = ref<number | null>(null)
  const linkDialogSnapshot = ref('')

  const profileForm = reactive<{
    name: string
    link_profile_id: number
    common_address: number
  }>({
    name: '',
    link_profile_id: 0,
    common_address: 1,
  })

  const linkForm = reactive<UpsertLinkProfileRequest>({
    name: '',
    host: options.connectForm.host,
    port: options.connectForm.port,
    link_params: defaultLinkParams(),
    auto_connect: false,
    auto_start_data_transfer: true,
    auto_gi: false,
    retry_count: 3,
    retry_interval: 5,
  })

  const linkParamsCustomEnabled = ref(false)

  const selectedLinkProfile = computed(() => {
    const targetId = Number(profileForm.link_profile_id || 0)
    if (!Number.isFinite(targetId) || targetId <= 0) return null
    return linkProfiles.value.find((item) => item.id === targetId) ?? null
  })

  const profileBoundLinkName = computed(() => {
    const link = selectedLinkProfile.value
    if (link) return link.name || t('master.profiles.fallback.link', { id: link.id })
    const fallbackId = Number(profileForm.link_profile_id || 0)
    if (!Number.isFinite(fallbackId) || fallbackId <= 0)
      return t('master.profiles.fallback.unboundLink')
    return t('master.profiles.fallback.link', { id: fallbackId })
  })

  const profileBoundLinkEndpoint = computed(() => {
    const link = selectedLinkProfile.value
    if (!link) return '0.0.0.0:0'
    return `${link.host}:${link.port}`
  })

  const profileDialogSubtitleBadges = computed(() => {
    const badges: string[] = []
    badges.push(profileBoundLinkName.value)
    badges.push(profileBoundLinkEndpoint.value)
    const profileName = (profileForm.name || '').trim()
    badges.push(
      profileName ||
        (profileDialogMode.value === 'create'
          ? t('master.profiles.fallback.newSlave')
          : t('master.profiles.fallback.station')),
    )
    return badges
  })

  const linkDialogSubtitleBadges = computed(() => {
    const badges: string[] = []
    badges.push((linkForm.name || '').trim() || t('master.profiles.fallback.unnamedLink'))
    const host = (linkForm.host || '').trim() || '0.0.0.0'
    const port = Number(linkForm.port || 0)
    badges.push(`${host}:${port}`)
    return badges
  })

  const linkParamsCollapsedDesc = computed(() => {
    const params = linkForm.link_params
    if (!params) return ''
    return `K=${params.k_value}  W=${params.w_value}  T0=${params.t0_seconds}s  T1=${params.t1_seconds}s  T2=${params.t2_seconds}s  T3=${params.t3_seconds}s`
  })

  const linkParamsWarnWK = computed(() => {
    const k = Number(linkForm.link_params?.k_value || 0)
    const w = Number(linkForm.link_params?.w_value || 0)
    return w >= k ? t('master.profiles.warnings.wk', { w, k }) : ''
  })

  const linkParamsWarnT2T1 = computed(() => {
    const t1 = Number(linkForm.link_params?.t1_seconds || 0)
    const t2 = Number(linkForm.link_params?.t2_seconds || 0)
    return t2 >= t1 ? t('master.profiles.warnings.t2t1', { t2, t1 }) : ''
  })

  const mapRuntimeTransportState = (state: string): TransportState => normalizeTransportState(state)

  const mapRuntimeDataTransferState = (state: string): DataTransferState =>
    normalizeDataTransferState(state)

  const summarizePointTypes = (
    defs: PointDef[],
    aliasesByType: Record<string, string> = {},
  ): PointTypeSummary => {
    const countByType: DataTypeCountMap = {}
    const pointNamesByType = new Map<string, Set<string>>()

    for (const def of defs) {
      const dataType = resolveIec104TypeName({
        data_type: def.data_type,
        type_id: def.type_id,
        name: def.name,
      })
      countByType[dataType] = (countByType[dataType] ?? 0) + 1
      const normalizedName = normalizeIecObjectDisplayName(def.name)
      if (!normalizedName) continue
      const names = pointNamesByType.get(dataType) ?? new Set<string>()
      names.add(normalizedName)
      pointNamesByType.set(dataType, names)
    }

    const displayNameByType: Record<string, string> = {}
    for (const dataType of Object.keys(countByType)) {
      const alias = normalizeIecObjectDisplayName(aliasesByType[dataType])
      const isSuspiciousPointNameAlias =
        (countByType[dataType] ?? 0) > 1 && Boolean(pointNamesByType.get(dataType)?.has(alias))
      if (alias && !isSuspiciousPointNameAlias) {
        displayNameByType[dataType] = alias
      }
    }

    return {
      countByType,
      displayNameByType,
    }
  }

  const refreshProfilePointTypeSummary = async (nextProfiles: MasterSlaveProfile[]) => {
    const defsByProfile: Record<string, PointDef[]> = {}
    const summaryEntries = await Promise.all(
      nextProfiles.map(async (profile) => {
        const profileKey = String(profile.id)
        let defs: PointDef[] = []
        let aliasesByType: Record<string, string> = {}
        try {
          const defsResult = await getLinkSlavePointDefs(profile.id)
          defs = (defsResult ?? []).slice().sort((a, b) => a.address - b.address)
        } catch {
          defs = []
        }
        try {
          aliasesByType = await getLinkSlaveAsduAliases(profile.id)
        } catch {
          aliasesByType = {}
        }

        defsByProfile[profileKey] = defs
        return [profileKey, summarizePointTypes(defs, aliasesByType)] as const
      }),
    )

    profilePointTypeSummary.value = Object.fromEntries(summaryEntries)
    profilePointDefsMap.value = defsByProfile
  }

  const refreshProfiles = async () => {
    if (!options.currentStationId.value) return
    profilesLoading.value = true
    try {
      const nextLinks = await listLinkProfiles(options.currentStationId.value)
      const slaveEntries = await Promise.all(
        nextLinks.map(async (link) => {
          const slaves = await listLinkSlaves(link.id)
          return [link.id, slaves] as const
        }),
      )
      const nextSlaveMap = Object.fromEntries(slaveEntries)
      const nextProfiles: MasterSlaveProfile[] = []

      for (const link of nextLinks) {
        const slaves = nextSlaveMap[link.id] ?? []
        for (const slave of slaves) {
          nextProfiles.push({
            id: slave.id,
            link_profile_id: link.id,
            name: slave.name,
            common_address: slave.common_address,
            enabled: slave.enabled,
            link_name: link.name,
            host: link.host,
            port: link.port,
            link_params: link.link_params,
            auto_connect: link.auto_connect,
            auto_start_data_transfer: link.auto_start_data_transfer,
            auto_gi: link.auto_gi,
            retry_count: link.retry_count,
            retry_interval: link.retry_interval,
          })
        }
      }

      nextProfiles.sort((left, right) => {
        if (left.link_profile_id !== right.link_profile_id) {
          return left.link_profile_id - right.link_profile_id
        }
        if (left.common_address !== right.common_address) {
          return left.common_address - right.common_address
        }
        return left.id - right.id
      })

      await refreshProfilePointTypeSummary(nextProfiles)
      linkProfiles.value = nextLinks
      slavesByLink.value = nextSlaveMap
      profiles.value = nextProfiles
      options.onProfilesChanged?.()
    } catch (error) {
      ElMessage.error(
        t('master.profiles.messages.loadFailed', {
          error: translateApiError(error, String(error)),
        }),
      )
      profiles.value = []
      linkProfiles.value = []
      slavesByLink.value = {}
      profilePointTypeSummary.value = {}
      profilePointDefsMap.value = {}
    } finally {
      profilesLoading.value = false
    }
  }

  const suggestNewLinkName = () => {
    const used = new Set(linkProfiles.value.map((link) => String(link.name || '').trim()))
    for (let index = 1; index < 1000; index += 1) {
      const candidate = t('master.profiles.fallback.link', { id: index })
      if (!used.has(candidate)) return candidate
    }
    return t('master.profiles.fallback.link', { id: Date.now() })
  }

  const suggestNewSlaveName = (linkProfileId: number) => {
    const used = new Set(
      (slavesByLink.value[linkProfileId] ?? []).map((slave) => String(slave.name || '').trim()),
    )
    for (let index = 1; index < 1000; index += 1) {
      const candidate = t('soePanel.source.slave', { id: index }).replace(/\s+/g, '-')
      if (!used.has(candidate)) return candidate
    }
    return t('soePanel.source.slave', { id: Date.now() }).replace(/\s+/g, '-')
  }

  const suggestNextCommonAddress = (linkProfileId: number) => {
    const maxCommonAddress = Math.max(
      0,
      ...(slavesByLink.value[linkProfileId] ?? []).map(
        (slave) => Number(slave.common_address) || 0,
      ),
    )
    return Math.min(65535, Math.max(1, maxCommonAddress + 1))
  }

  const applyLinkProfileToLinkForm = (link: LinkProfileResponse) => {
    linkForm.name = link.name
    linkForm.host = link.host
    linkForm.port = link.port
    linkForm.link_params = { ...link.link_params }
    linkForm.auto_connect = link.auto_connect
    linkForm.auto_start_data_transfer = link.auto_start_data_transfer
    linkForm.auto_gi = link.auto_gi
    linkForm.retry_count = link.retry_count
    linkForm.retry_interval = link.retry_interval
    linkParamsCustomEnabled.value =
      link.link_params.k_value !== 12 ||
      link.link_params.w_value !== 8 ||
      link.link_params.t0_seconds !== 30 ||
      link.link_params.t1_seconds !== 15 ||
      link.link_params.t2_seconds !== 10 ||
      link.link_params.t3_seconds !== 20
  }

  const resetLinkForm = () => {
    linkForm.name = suggestNewLinkName()
    linkForm.host = options.connectForm.host
    linkForm.port = options.connectForm.port
    linkForm.link_params = defaultLinkParams()
    linkForm.auto_connect = false
    linkForm.auto_start_data_transfer = true
    linkForm.auto_gi = false
    linkForm.retry_count = 3
    linkForm.retry_interval = 5
    linkParamsCustomEnabled.value = false
  }

  const handleLinkParamsCustomEnabledChange = (value: boolean | string | number) => {
    if (!value) {
      linkForm.link_params = defaultLinkParams()
    }
  }

  const buildProfileDialogSnapshot = () =>
    JSON.stringify({
      mode: profileDialogMode.value,
      name: String(profileForm.name || '').trim(),
      link_profile_id: Number(profileForm.link_profile_id || 0),
      common_address: Number(profileForm.common_address || 1),
    })

  const buildLinkDialogSnapshot = () =>
    JSON.stringify({
      mode: linkDialogMode.value,
      name: String(linkForm.name || '').trim(),
      host: String(linkForm.host || '').trim(),
      port: Number(linkForm.port || 0),
      link_params: { ...(linkForm.link_params ?? defaultLinkParams()) },
      auto_connect: Boolean(linkForm.auto_connect),
      auto_start_data_transfer: Boolean(linkForm.auto_start_data_transfer),
      auto_gi: Boolean(linkForm.auto_gi),
      retry_count: Number(linkForm.retry_count || 0),
      retry_interval: Number(linkForm.retry_interval || 0),
      link_params_custom_enabled: Boolean(linkParamsCustomEnabled.value),
    })

  const hasUnsavedProfileDialogChanges = () =>
    showProfileDialog.value &&
    Boolean(profileDialogSnapshot.value) &&
    buildProfileDialogSnapshot() !== profileDialogSnapshot.value

  const hasUnsavedLinkDialogChanges = () =>
    showLinkDialog.value &&
    Boolean(linkDialogSnapshot.value) &&
    buildLinkDialogSnapshot() !== linkDialogSnapshot.value

  const finalizeProfileDialogClose = () => {
    showProfileDialog.value = false
    editingProfileId.value = null
    profileDialogSnapshot.value = ''
  }

  const finalizeLinkDialogClose = () => {
    showLinkDialog.value = false
    editingStandaloneLinkId.value = null
    linkDialogSnapshot.value = ''
  }

  const openCreateProfileDialog = (linkProfileId: number) => {
    const targetLinkId = Number(linkProfileId || 0)
    if (!Number.isFinite(targetLinkId) || targetLinkId <= 0) {
      ElMessage.warning(t('master.profiles.warnings.createSlaveFromLink'))
      return
    }

    const targetLink = linkProfiles.value.find((link) => link.id === targetLinkId)
    if (!targetLink) {
      ElMessage.warning(t('master.profiles.warnings.linkMissingRefresh'))
      return
    }

    profileDialogMode.value = 'create'
    editingProfileId.value = null
    profileForm.name = suggestNewSlaveName(targetLink.id)
    profileForm.link_profile_id = targetLink.id
    profileForm.common_address = suggestNextCommonAddress(targetLink.id)
    profileDialogSnapshot.value = buildProfileDialogSnapshot()
    showProfileDialog.value = true
  }

  const openEditProfileDialog = (profileId: number) => {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (!profile) {
      ElMessage.warning(t('master.profiles.warnings.slaveMissing'))
      return
    }
    if (ensureProfileDisconnectedForConfig(profileId, t('master.profiles.actions.editSlaveConfig')))
      return

    profileDialogMode.value = 'edit'
    editingProfileId.value = profileId
    profileForm.name = profile.name
    profileForm.link_profile_id = profile.link_profile_id
    profileForm.common_address = profile.common_address
    profileDialogSnapshot.value = buildProfileDialogSnapshot()
    showProfileDialog.value = true
  }

  const { requestClose: closeProfileDialog, handleBeforeClose: handleProfileDialogBeforeClose } =
    useDialogCloseGuard({
      isDirty: hasUnsavedProfileDialogChanges,
      onClose: finalizeProfileDialogClose,
    })

  const handleProfileDialogEnter = (event: KeyboardEvent) => {
    const target = event.target as HTMLElement | null
    if (!target) return
    if (target.tagName === 'TEXTAREA') return
    void submitProfileDialog()
  }

  const submitProfileDialog = async () => {
    if (!options.currentStationId.value) {
      ElMessage.warning(t('master.profiles.warnings.initMasterFirst'))
      return
    }

    try {
      if (profileDialogMode.value === 'create') {
        const targetLinkId = Number(profileForm.link_profile_id || 0)
        if (!Number.isFinite(targetLinkId) || targetLinkId <= 0) {
          ElMessage.error(t('master.profiles.warnings.invalidBoundLink'))
          return
        }

        const slaveBundle: UpsertLinkSlaveBundleRequest = {
          slave: {
            name: profileForm.name,
            common_address: profileForm.common_address,
            enabled: true,
          },
          point_defs: [],
        }
        const created = await createLinkSlave(targetLinkId, slaveBundle)
        ElMessage.success(t('master.profiles.messages.slaveCreated'))
        await refreshProfiles()
        options.onProfileFocused?.(created.slave_id)
        finalizeProfileDialogClose()
        return
      }

      if (editingProfileId.value == null) {
        ElMessage.error(t('master.profiles.warnings.missingConfigId'))
        return
      }
      if (
        ensureProfileDisconnectedForConfig(
          editingProfileId.value,
          t('master.profiles.actions.saveSlaveConfig'),
        )
      ) {
        return
      }

      const targetProfile = profiles.value.find((item) => item.id === editingProfileId.value)
      if (!targetProfile) {
        ElMessage.error(t('master.profiles.warnings.configMissing'))
        return
      }

      await updateLinkSlave(editingProfileId.value, {
        slave: {
          name: profileForm.name,
          common_address: profileForm.common_address,
          enabled: targetProfile.enabled,
        },
        point_defs: [],
      })
      ElMessage.success(t('master.profiles.messages.slaveSaved'))
      await refreshProfiles()
      options.onProfileFocused?.(editingProfileId.value)
      finalizeProfileDialogClose()
    } catch (error) {
      ElMessage.error(
        t('master.profiles.messages.slaveSaveFailed', {
          error: translateApiError(error, String(error)),
        }),
      )
    }
  }

  const openCreateLinkDialog = () => {
    linkDialogMode.value = 'create'
    editingStandaloneLinkId.value = null
    resetLinkForm()
    linkDialogSnapshot.value = buildLinkDialogSnapshot()
    showLinkDialog.value = true
  }

  const openEditLinkDialog = (linkProfileId: number) => {
    const link = linkProfiles.value.find((item) => item.id === linkProfileId)
    if (!link) {
      ElMessage.warning(t('master.profiles.warnings.linkMissing'))
      return
    }
    if (ensureLinkDisconnectedForConfig(linkProfileId, t('master.profiles.actions.editLinkConfig')))
      return

    linkDialogMode.value = 'edit'
    editingStandaloneLinkId.value = linkProfileId
    applyLinkProfileToLinkForm(link)
    linkDialogSnapshot.value = buildLinkDialogSnapshot()
    showLinkDialog.value = true
  }

  const { requestClose: closeLinkDialog, handleBeforeClose: handleLinkDialogBeforeClose } =
    useDialogCloseGuard({
      isDirty: hasUnsavedLinkDialogChanges,
      onClose: finalizeLinkDialogClose,
    })

  const submitLinkDialog = async () => {
    if (!options.currentStationId.value) {
      ElMessage.warning(t('master.profiles.warnings.initMasterFirst'))
      return
    }

    try {
      if (linkDialogMode.value === 'create') {
        const created = await createLinkProfile(options.currentStationId.value, { ...linkForm })
        ElMessage.success(t('master.profiles.messages.linkCreated'))
        await refreshProfiles()
        profileForm.link_profile_id = created.id
        finalizeLinkDialogClose()
        return
      }

      if (editingStandaloneLinkId.value == null) {
        ElMessage.error(t('master.profiles.warnings.missingConfigId'))
        return
      }
      if (
        ensureLinkDisconnectedForConfig(
          editingStandaloneLinkId.value,
          t('master.profiles.actions.saveLinkConfig'),
        )
      ) {
        return
      }

      await updateLinkProfile(editingStandaloneLinkId.value, { ...linkForm })
      ElMessage.success(t('master.profiles.messages.linkSaved'))
      await refreshProfiles()
      finalizeLinkDialogClose()
    } catch (error) {
      ElMessage.error(
        t('master.profiles.messages.linkSaveFailed', {
          error: translateApiError(error, String(error)),
        }),
      )
    }
  }

  const normalizeHost = (host: string): string => {
    const trimmed = host.trim().toLowerCase()
    const noBracketHost =
      trimmed.startsWith('[') && trimmed.endsWith(']') ? trimmed.slice(1, -1) : trimmed

    if (noBracketHost === 'localhost') {
      return '127.0.0.1'
    }
    if (noBracketHost.startsWith('::ffff:')) {
      return noBracketHost.slice('::ffff:'.length)
    }
    return noBracketHost
  }

  const parseRemoteAddress = (remote: string): { host: string; port: number } | null => {
    const index = remote.lastIndexOf(':')
    if (index <= 0) return null
    const host = remote.slice(0, index)
    const port = Number(remote.slice(index + 1))
    if (!Number.isFinite(port)) return null
    return { host, port }
  }

  const isRuntimeConnectionBusy = (connection: BackendConnectionInfo): boolean => {
    const unified = deriveUnifiedConnStatus(
      connection.transport_state,
      connection.data_transfer_state,
    )
    return (
      unified.severity === 'status-processing' ||
      unified.severity === 'status-unready' ||
      unified.severity === 'status-normal'
    )
  }

  const resolveRuntimeConnectionsForLink = (linkProfileId: number): BackendConnectionInfo[] => {
    const directMatches = options.connections.value.filter(
      (conn) => conn.profile_id === linkProfileId,
    )
    if (directMatches.length > 0) {
      return [...directMatches].sort(compareRuntimeConnectionPriority)
    }

    const link = linkProfiles.value.find((item) => item.id === linkProfileId)
    if (!link) return []

    return options.connections.value
      .filter((conn) => {
        const parsed = parseRemoteAddress(conn.remote_addr)
        if (!parsed) return false
        return normalizeHost(parsed.host) === normalizeHost(link.host) && parsed.port === link.port
      })
      .sort(compareRuntimeConnectionPriority)
  }

  const ensureLinkDisconnectedForConfig = (linkProfileId: number, actionLabel: string): boolean => {
    const busyConnections =
      resolveRuntimeConnectionsForLink(linkProfileId).filter(isRuntimeConnectionBusy)
    if (busyConnections.length === 0) return false
    ElMessage.warning(t('master.profiles.warnings.busySession', { action: actionLabel }))
    return true
  }

  const ensureProfileDisconnectedForConfig = (profileId: number, actionLabel: string): boolean => {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (!profile) return false
    return ensureLinkDisconnectedForConfig(profile.link_profile_id, actionLabel)
  }

  const runtimeConnectionRank = (conn: BackendConnectionInfo): number => {
    const unified = deriveUnifiedConnStatus(conn.transport_state, conn.data_transfer_state)
    if (unified.severity === 'status-normal') return 50
    if (unified.severity === 'status-unready') return 40
    if (unified.severity === 'status-processing') return 30
    if (unified.severity === 'status-error') return 20
    return 0
  }

  const compareRuntimeConnectionPriority = (
    left: BackendConnectionInfo,
    right: BackendConnectionInfo,
  ): number => {
    const rankDiff = runtimeConnectionRank(right) - runtimeConnectionRank(left)
    if (rankDiff !== 0) return rankDiff

    const leftConnectedAt = Date.parse(String(left.connected_at ?? '')) || 0
    const rightConnectedAt = Date.parse(String(right.connected_at ?? '')) || 0
    if (rightConnectedAt !== leftConnectedAt) {
      return rightConnectedAt - leftConnectedAt
    }

    return right.id.localeCompare(left.id)
  }

  const resolveRuntimeConnectionsForProfile = (
    profile: MasterSlaveProfile,
  ): BackendConnectionInfo[] => {
    const directMatches = options.connections.value.filter(
      (conn) => conn.profile_id === profile.link_profile_id,
    )
    const candidates =
      directMatches.length > 0
        ? directMatches
        : options.connections.value.filter((conn) => {
            const parsed = parseRemoteAddress(conn.remote_addr)
            if (!parsed) return false
            return (
              normalizeHost(parsed.host) === normalizeHost(profile.host) &&
              parsed.port === profile.port
            )
          })

    return [...candidates].sort(compareRuntimeConnectionPriority)
  }

  const findActiveRuntimeConnectionIdForProfile = (profile: MasterSlaveProfile): string | null => {
    const active = resolveRuntimeConnectionsForProfile(profile).find((conn) => {
      const unified = deriveUnifiedConnStatus(conn.transport_state, conn.data_transfer_state)
      return unified.severity !== 'status-idle'
    })
    return active?.id ?? null
  }

  const findBestRuntimeConnection = (profile: MasterSlaveProfile): BackendConnectionInfo | null => {
    const [best] = resolveRuntimeConnectionsForProfile(profile)
    return best ?? null
  }

  const findRuntimeConnectionIdForProfile = (profile: MasterSlaveProfile): string | null => {
    const best = findBestRuntimeConnection(profile)
    if (!best) return null
    if (runtimeConnectionRank(best) <= 0) return null
    return best.id
  }

  const findConnectionProfileById = (profileId: number): MasterSlaveProfile | null =>
    profiles.value.find((profile) => profile.id === profileId) ?? null

  const sanitizeStationDisplayName = (rawName: string | null | undefined): string => {
    const normalized = String(rawName ?? '').trim()
    if (!normalized) return t('master.profiles.fallback.slave')
    if (/conn[_-][a-z0-9-]+/i.test(normalized)) return t('master.profiles.fallback.slave')
    return normalized
  }

  const resolveRuntimeLinkStateByConnectionId = (
    connectionId: string | null | undefined,
  ): {
    transport: TransportState
    link: DataTransferState
  } => {
    if (!connectionId) {
      return { transport: 'disconnected', link: 'stopped' }
    }
    const runtime = options.connections.value.find((conn) => conn.id === connectionId)
    if (!runtime) {
      return { transport: 'disconnected', link: 'stopped' }
    }
    return {
      transport: normalizeTransportState(runtime.transport_state),
      link: normalizeDataTransferState(runtime.data_transfer_state),
    }
  }

  const resolveConnectionIdForProfileAction = (profileId: number): string | null => {
    const profile = findConnectionProfileById(profileId)
    if (!profile) return null
    return findActiveRuntimeConnectionIdForProfile(profile)
  }

  const resolveConnectTargetForProfile = (
    profileId: number,
  ): { host: string; port: number } | null => {
    const profile = findConnectionProfileById(profileId)
    if (!profile) return null
    return {
      host: profile.host,
      port: profile.port,
    }
  }

  const listRuntimeConnectionIdsForLink = (linkProfileId: number): string[] => {
    const runtimeConnectionIds = new Set<string>()

    for (const runtimeConn of options.connections.value) {
      if (runtimeConn.profile_id === linkProfileId) {
        runtimeConnectionIds.add(runtimeConn.id)
      }
    }

    for (const slave of slaveProfiles.value) {
      if (slave.linkProfileId === linkProfileId && slave.connectionId) {
        runtimeConnectionIds.add(slave.connectionId)
      }
    }

    return [...runtimeConnectionIds]
  }

  const resolveConnectionIdBySlaveId = (slaveId: string): string | null => {
    const byProfile = slaveProfiles.value.find((profile) => profile.id === slaveId)
    if (byProfile) {
      const preferred = resolveConnectionIdForProfileAction(byProfile.profileId)
      return preferred ?? byProfile.connectionId ?? null
    }

    const byConnection = options.connections.value.find((conn) => conn.id === slaveId)
    if (!byConnection) return null

    if (typeof byConnection.profile_id === 'number') {
      const sameLinkSlave = slaveProfiles.value.find(
        (slave) => slave.linkProfileId === byConnection.profile_id,
      )
      if (sameLinkSlave) {
        const preferred = resolveConnectionIdForProfileAction(sameLinkSlave.profileId)
        return preferred ?? byConnection.id
      }
    }

    return byConnection.id
  }

  const slaveProfiles = computed<SlaveConnection[]>(() =>
    profiles.value.map((profile) => {
      const connectionId = findRuntimeConnectionIdForProfile(profile)
      const runtime = connectionId
        ? (options.connections.value.find((conn) => conn.id === connectionId) ?? null)
        : null

      return {
        id: String(profile.id),
        profileId: profile.id,
        slaveId: profile.id,
        linkProfileId: profile.link_profile_id,
        connectionId,
        name: profile.name,
        linkName: profile.link_name,
        host: profile.host,
        port: profile.port,
        transportState: runtime
          ? mapRuntimeTransportState(runtime.transport_state)
          : 'disconnected',
        dataTransferState: runtime
          ? mapRuntimeDataTransferState(runtime.data_transfer_state)
          : 'stopped',
        reconnecting: runtime?.reconnecting ?? false,
        reconnectAttempts: runtime?.reconnect_attempts ?? 0,
        maxReconnectAttempts: runtime?.max_reconnect_attempts ?? 0,
        commonAddress: profile.common_address,
        linkParams: profile.link_params,
      }
    }),
  )

  const stationListForMessage = computed<MasterMessageStationItem[]>(() => {
    const list: MasterMessageStationItem[] = []

    for (const link of linkProfiles.value) {
      const connectionName = sanitizeStationDisplayName(
        link.name || t('master.profiles.fallback.slave'),
      )
      const activeConnectionId = resolveConnectionIdForProfileAction(link.id)
      const uiConnectionKey = `master-link:${link.id}`

      list.push({
        id: uiConnectionKey,
        name: connectionName,
        groupName: connectionName,
        slaveName: connectionName,
        isConnectionLevel: true,
        host: link.host,
        port: link.port,
        status: activeConnectionId ? 'Connected' : 'disconnected',
        runtimeConnectionId: activeConnectionId || '',
        uiConnectionKey,
        stationConfigId: `master-link:${link.id}`,
        commonAddress: null,
      })

      const slaves = slaveProfiles.value.filter((slave) => slave.linkProfileId === link.id)
      for (const slave of slaves) {
        const slaveName = String(slave.name || '').trim()
        const messageStationName =
          connectionName && slaveName && connectionName !== slaveName
            ? `${connectionName} - ${slaveName}`
            : slaveName || connectionName || t('master.profiles.fallback.slave')
        const slaveUiKey = `master-link:${link.id}:slave:${slave.profileId}`

        list.push({
          id: slaveUiKey,
          name: messageStationName,
          groupName: connectionName,
          slaveName: slave.name,
          host: link.host,
          port: link.port,
          status: activeConnectionId ? 'Connected' : 'disconnected',
          runtimeConnectionId: activeConnectionId || '',
          uiConnectionKey,
          slaveUiKey,
          stationConfigId: String(slave.profileId),
          commonAddress: slave.commonAddress,
        })
      }
    }

    return list
  })

  const deleteSelectedProfile = async (profileId: number) => {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (!profile) {
      ElMessage.warning(t('master.profiles.warnings.slaveMissing'))
      return
    }

    const confirmed = await confirmDangerousAction(
      t('master.profiles.confirm.deleteSlaveMessage', { name: profile.name }),
      t('master.profiles.confirm.deleteSlaveTitle'),
      t('common.delete'),
    )
    if (!confirmed) return

    try {
      const slave = slaveProfiles.value.find((item) => item.profileId === profileId)
      if (slave?.connectionId) {
        await options.disconnectFromSlave(slave.connectionId)
      }

      await deleteLinkSlave(profileId)
      ElMessage.success(t('master.profiles.messages.slaveDeleted'))
      await refreshProfiles()

      if (options.getSelectedProfileId?.() === profileId) {
        options.clearSelectedSlave?.()
      }
    } catch (error) {
      ElMessage.error(
        t('master.profiles.messages.slaveDeleteFailed', {
          error: translateApiError(error, String(error)),
        }),
      )
    }
  }

  const deleteLinkWithCascade = async (linkProfileId: number) => {
    const targetLinkId = Number(linkProfileId || 0)
    if (!Number.isFinite(targetLinkId) || targetLinkId <= 0) {
      ElMessage.warning(t('master.profiles.warnings.missingLinkId'))
      return
    }

    const link = linkProfiles.value.find((item) => item.id === targetLinkId)
    if (!link) {
      ElMessage.warning(t('master.profiles.warnings.linkMissing'))
      return
    }

    const linkedSlaves = slavesByLink.value[targetLinkId] ?? []
    try {
      await ElMessageBox.confirm(
        t('master.profiles.confirm.deleteLinkMessage', {
          name: link.name,
          count: linkedSlaves.length,
        }),
        t('master.profiles.confirm.deleteLinkTitle'),
        {
          confirmButtonText: t('common.delete'),
          cancelButtonText: t('common.cancel'),
          type: 'warning',
          center: true,
          autofocus: false,
        },
      )
    } catch {
      return
    }

    try {
      const shouldClearSelection = linkedSlaves.some(
        (slave) => slave.id === options.getSelectedProfileId?.(),
      )

      for (const connectionId of listRuntimeConnectionIdsForLink(targetLinkId)) {
        await options.disconnectFromSlave(connectionId)
      }

      for (const slave of linkedSlaves) {
        await deleteLinkSlave(slave.id)
      }

      await deleteLinkProfile(targetLinkId)
      ElMessage.success(t('master.profiles.messages.linkDeleted'))
      await refreshProfiles()

      if (shouldClearSelection) {
        options.clearSelectedSlave?.()
      }
    } catch (error) {
      ElMessage.error(
        t('master.profiles.messages.linkDeleteFailed', {
          error: translateApiError(error, String(error)),
        }),
      )
    }
  }

  return {
    profilesLoading,
    linkProfiles,
    profilePointTypeSummary,
    profilePointDefsMap,
    slaveProfiles,
    stationListForMessage,
    showProfileDialog,
    profileDialogMode,
    profileDialogSubtitleBadges,
    profileForm,
    showLinkDialog,
    linkDialogMode,
    linkDialogSubtitleBadges,
    linkForm,
    linkParamsCustomEnabled,
    linkParamsCollapsedDesc,
    linkParamsWarnWK,
    linkParamsWarnT2T1,
    refreshProfiles,
    handleLinkParamsCustomEnabledChange,
    openCreateProfileDialog,
    openEditProfileDialog,
    closeProfileDialog,
    handleProfileDialogBeforeClose,
    handleProfileDialogEnter,
    submitProfileDialog,
    openCreateLinkDialog,
    openEditLinkDialog,
    closeLinkDialog,
    handleLinkDialogBeforeClose,
    submitLinkDialog,
    deleteSelectedProfile,
    deleteLinkWithCascade,
    resolveConnectionIdForProfileAction,
    resolveConnectTargetForProfile,
    resolveConnectionIdBySlaveId,
    resolveRuntimeLinkStateByConnectionId,
    findConnectionProfileById,
    listRuntimeConnectionIdsForLink,
  }
}
