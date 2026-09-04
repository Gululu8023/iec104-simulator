import { computed, ref, watch, type Ref } from 'vue'

import { t } from '@shared/i18n'

import {
  buildMessageTypeFilterGroups,
  getMessageTypeFilterOptionLabel,
  type Message,
  type MessageFocusFilter,
  type StationInfo,
} from './types'
import { extractMessageFrameMeta } from './parser'

interface UseMessagePanelFiltersOptions {
  stationList: Ref<StationInfo[] | undefined>
  messages: Ref<Message[]>
}

export function useMessagePanelFilters(options: UseMessagePanelFiltersOptions) {
  const frameMetaCache = new WeakMap<Message, ReturnType<typeof extractMessageFrameMeta>>()
  const getFrameMeta = (message: Message) => {
    const cached = frameMetaCache.get(message)
    if (cached) return cached
    const resolved = extractMessageFrameMeta(message)
    frameMetaCache.set(message, resolved)
    return resolved
  }
  const connectionFilter = ref('')
  const connectionOptions = computed(() => {
    const seen = new Set<string>()
    return (options.stationList.value ?? []).filter((station) => {
      if (!station.isConnectionLevel) return false
      const key = String(station.uiConnectionKey || station.id).trim()
      if (!key || seen.has(key)) return false
      seen.add(key)
      return true
    })
  })

  const selectableStations = computed(() =>
    (options.stationList.value ?? []).filter(
      (station) =>
        !station.isConnectionLevel &&
        (!connectionFilter.value ||
          String(station.uiConnectionKey || '').trim() === connectionFilter.value),
    ),
  )

  const showStationFilter = computed(() => selectableStations.value.length > 0)
  const searchText = ref('')
  const directionFilter = ref<Array<Message['type']>>([])
  const stationFilter = ref<string[]>([])
  const typeFilter = ref<string[]>([])
  const cotFilter = ref<string[]>([])
  const externalFocusFilter = ref<MessageFocusFilter | null>(null)

  const findStationById = (stationId: string) =>
    (options.stationList.value ?? []).find((station) => station.id === stationId) ?? null

  const buildStationIdentity = (station: StationInfo): string => {
    return [
      station.isConnectionLevel ? 'connection' : 'slave',
      String(station.stationConfigId || '').trim(),
      String(station.groupName || '').trim(),
      String(station.slaveName || station.name || '').trim(),
      String(station.host || '').trim(),
      String(station.port ?? ''),
      station.commonAddress == null ? '' : String(Math.trunc(station.commonAddress)),
    ].join('|')
  }

  const remapStationId = (
    stationId: string,
    previousStations: StationInfo[],
    nextStations: StationInfo[],
  ): string => {
    const target = String(stationId || '').trim()
    if (!target) return ''
    if (nextStations.some((station) => station.id === target)) return target

    const previousStation = previousStations.find((station) => station.id === target)
    if (!previousStation) return ''
    const identity = buildStationIdentity(previousStation)
    return nextStations.find((station) => buildStationIdentity(station) === identity)?.id ?? ''
  }

  const remapConnectionId = (
    connectionId: string,
    previousStations: StationInfo[],
    nextStations: StationInfo[],
  ): string => {
    const target = String(connectionId || '').trim()
    if (!target) return ''
    const nextConnectionStations = nextStations.filter((station) => station.isConnectionLevel)
    if (
      nextConnectionStations.some(
        (station) => String(station.runtimeConnectionId || '').trim() === target,
      )
    ) {
      return target
    }

    const previousConnectionStation = previousStations.find(
      (station) =>
        station.isConnectionLevel && String(station.runtimeConnectionId || '').trim() === target,
    )
    if (!previousConnectionStation) return ''
    const identity = buildStationIdentity(previousConnectionStation)
    return (
      nextConnectionStations.find((station) => buildStationIdentity(station) === identity)
        ?.runtimeConnectionId ?? ''
    )
  }

  watch(
    () => options.stationList.value ?? [],
    (nextStations, previousStations) => {
      let connectionRemoved = false
      if (
        connectionFilter.value &&
        !nextStations.some(
          (station) =>
            station.isConnectionLevel &&
            String(station.uiConnectionKey || station.id).trim() === connectionFilter.value,
        )
      ) {
        connectionFilter.value = ''
        connectionRemoved = true
      }
      if (connectionRemoved) stationFilter.value = []
      if (previousStations.length === 0) return

      const nextStationFilter = Array.from(
        new Set(
          stationFilter.value
            .map((stationId) => remapStationId(stationId, previousStations, nextStations))
            .filter((stationId) => stationId.length > 0),
        ),
      )
      if (
        nextStationFilter.length !== stationFilter.value.length ||
        nextStationFilter.some((stationId, index) => stationId !== stationFilter.value[index])
      ) {
        stationFilter.value = nextStationFilter
      }

      if (connectionFilter.value) {
        stationFilter.value = stationFilter.value.filter((stationId) => {
          const station = nextStations.find((item) => item.id === stationId)
          return String(station?.uiConnectionKey || '').trim() === connectionFilter.value
        })
      }

      const focus = externalFocusFilter.value
      if (!focus?.connectionId) return
      const nextConnectionId = remapConnectionId(focus.connectionId, previousStations, nextStations)
      if (nextConnectionId === focus.connectionId) return
      externalFocusFilter.value = nextConnectionId
        ? {
            ...focus,
            connectionId: nextConnectionId,
          }
        : null
    },
  )

  const matchesExternalFocus = (msg: Message, focus: MessageFocusFilter): boolean => {
    if (
      focus.connectionId &&
      String(msg.runtimeConnectionId || '').trim() !== String(focus.connectionId || '').trim()
    ) {
      return false
    }

    const meta = getFrameMeta(msg)
    if (focus.commonAddress != null && meta.commonAddress !== Math.trunc(focus.commonAddress)) {
      return false
    }
    if (focus.ioa != null && meta.ioa !== Math.trunc(focus.ioa)) {
      return false
    }
    if (focus.typeId != null && meta.typeId !== Math.trunc(focus.typeId)) {
      return false
    }
    return true
  }

  const isMessageMatchedByStation = (msg: Message, stationId: string): boolean => {
    const target = String(stationId || '').trim()
    if (!target) return false
    const station = findStationById(target)
    if (!station) return false
    if (station.isConnectionLevel) {
      return (
        String(msg.uiConnectionKey || '').trim() ===
        String(station.uiConnectionKey || station.id).trim()
      )
    }
    return String(msg.slaveUiKey || '').trim() === String(station.slaveUiKey || station.id).trim()
  }

  const resolveConnectionIdFromStationId = (stationId: string): string => {
    return String(findStationById(stationId)?.runtimeConnectionId || '').trim()
  }

  const resolveConnectionIdFromUiKey = (uiConnectionKey: string): string => {
    const target = String(uiConnectionKey || '').trim()
    if (!target) return ''
    const station = (options.stationList.value ?? []).find(
      (item) => item.isConnectionLevel && String(item.uiConnectionKey || item.id).trim() === target,
    )
    return String(station?.runtimeConnectionId || '').trim()
  }

  const matchesTypeFilter = (msg: Message): boolean => {
    const frameMeta = getFrameMeta(msg)
    if (frameMeta.typeId == null) return false
    return typeFilter.value.some((type) => Number(type) === frameMeta.typeId)
  }

  const filteredMessages = computed(() => {
    let filtered = options.messages.value

    if (directionFilter.value.length > 0) {
      filtered = filtered.filter((msg) => directionFilter.value.includes(msg.type))
    }

    if (connectionFilter.value) {
      filtered = filtered.filter(
        (msg) => String(msg.uiConnectionKey || '').trim() === connectionFilter.value,
      )
    }

    if (stationFilter.value.length > 0) {
      filtered = filtered.filter((msg) =>
        stationFilter.value.some((stationId) => isMessageMatchedByStation(msg, stationId)),
      )
    }

    if (typeFilter.value.length > 0) {
      filtered = filtered.filter(matchesTypeFilter)
    }

    if (cotFilter.value.length > 0) {
      filtered = filtered.filter((msg) => {
        const content = (msg.content || '').toUpperCase()
        return cotFilter.value.some((cot) => content.includes(cot))
      })
    }

    if (searchText.value.trim()) {
      const searchLower = searchText.value.toLowerCase()
      filtered = filtered.filter(
        (msg) =>
          msg.content.toLowerCase().includes(searchLower) ||
          Boolean(msg.hexData && msg.hexData.toLowerCase().includes(searchLower)),
      )
    }

    if (externalFocusFilter.value) {
      filtered = filtered.filter((msg) => matchesExternalFocus(msg, externalFocusFilter.value!))
    }

    return filtered
  })

  const getDirectionFilterLabel = () =>
    directionFilter.value.length > 0
      ? t('messagePanel.filters.directionWithCount', { count: directionFilter.value.length })
      : t('messagePanel.filters.direction')

  const getStationFilterLabel = () =>
    stationFilter.value.length > 0
      ? t('messagePanel.filters.stationWithCount', { count: stationFilter.value.length })
      : t('messagePanel.filters.station')

  const getConnectionOptionLabel = (station: StationInfo) => {
    const endpoint =
      station.remoteAddress?.trim() ||
      (station.host && station.port != null ? `${station.host}:${station.port}` : '')
    return endpoint ? `${station.name} · ${endpoint}` : station.name
  }

  const getConnectionFilterLabel = () => {
    if (!connectionFilter.value) return t('messagePanel.filters.allConnections')
    const station = connectionOptions.value.find(
      (item) => String(item.uiConnectionKey || item.id).trim() === connectionFilter.value,
    )
    return station ? getConnectionOptionLabel(station) : t('messagePanel.filters.allConnections')
  }

  const selectConnectionFilter = (uiConnectionKey: string) => {
    connectionFilter.value = String(uiConnectionKey || '').trim()
    if (!connectionFilter.value) return
    stationFilter.value = stationFilter.value.filter((stationId) => {
      const station = findStationById(stationId)
      return String(station?.uiConnectionKey || '').trim() === connectionFilter.value
    })
  }

  const getStationName = (msg?: Message) => {
    if (!options.stationList.value) return t('messagePanel.unknownStation')

    const slaveStation = msg?.slaveUiKey
      ? options.stationList.value.find(
          (station) => String(station.slaveUiKey || station.id).trim() === msg.slaveUiKey,
        )
      : null
    if (slaveStation) {
      return slaveStation.name
    }

    const connectionStation = msg?.uiConnectionKey
      ? options.stationList.value.find(
          (station) =>
            station.isConnectionLevel &&
            String(station.uiConnectionKey || station.id).trim() === msg.uiConnectionKey,
        )
      : null
    if (connectionStation) {
      return connectionStation.name
    }

    const runtimeConnectionId = String(msg?.runtimeConnectionId || '').trim()
    if (!runtimeConnectionId) return t('messagePanel.unknownStation')

    const mappedStation = options.stationList.value.find(
      (station) =>
        String(station.runtimeConnectionId || '').trim() === runtimeConnectionId &&
        station.isConnectionLevel,
    )
    return mappedStation?.name ?? t('messagePanel.unknownStation')
  }

  const typeFilterGroups = computed(() =>
    buildMessageTypeFilterGroups().map((group) => ({
      ...group,
      label: group.labelKey ? t(group.labelKey) : group.label,
    })),
  )

  const resetStandardFilters = () => {
    searchText.value = ''
    directionFilter.value = []
    stationFilter.value = []
    typeFilter.value = []
    cotFilter.value = []
    connectionFilter.value = ''
  }

  return {
    cotFilter,
    connectionFilter,
    connectionOptions,
    directionFilter,
    externalFocusFilter,
    filteredMessages,
    getConnectionFilterLabel,
    getConnectionOptionLabel,
    getDirectionFilterLabel,
    getStationFilterLabel,
    getStationName,
    getTypeFilterOptionLabel: getMessageTypeFilterOptionLabel,
    selectableStations,
    matchesExternalFocus,
    resolveConnectionIdFromStationId,
    resolveConnectionIdFromUiKey,
    resetStandardFilters,
    searchText,
    selectConnectionFilter,
    showStationFilter,
    stationFilter,
    typeFilter,
    typeFilterGroups,
  }
}
