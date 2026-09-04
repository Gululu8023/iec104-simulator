import { nextTick, type Ref } from 'vue'

import type { LinkProfileResponse } from '@shared/api/types'
import { normalizeIecObjectDisplayName } from '@shared/api/iec104'
import { MASTER_CONTROL_GROUPS, resolveMasterControlGroupLabel } from '@shared/api/masterControl'
import { deriveUnifiedConnStatus } from '@shared/ui/connectionStatus'
import { replayExpandedTreeKeys, syncExpandedTreeKeys } from '@shared/ui/treeExpansion'
import { t } from '@shared/i18n'

import type { SlaveConnection } from '../types/master'
import type { DataTypeCountMap, PointTypeSummary } from '../types/pointTypeSummary'
import type { MasterPointData } from './useMasterPointData'

type DataTypeId = string

interface MasterTreeModelSource {
  profiles: SlaveConnection[]
  pointData: MasterPointData
  profilePointTypeSummary: Record<string, PointTypeSummary>
  links: LinkProfileResponse[]
}

interface UseMasterTreeModelOptions {
  treeRef: Ref<any>
  slaveTreeData: Ref<any[]>
  selectedSlaveId: Ref<string>
  selectedTreeNodeKey: Ref<string>
  expandedSlaveKeys: Ref<string[]>
  source: MasterTreeModelSource
}

const MASTER_MONITOR_TYPES: DataTypeId[] = [
  'M_SP_NA_1',
  'M_DP_NA_1',
  'M_ST_NA_1',
  'M_BO_NA_1',
  'M_ME_NC_1',
  'M_IT_NA_1',
]

const buildDatatypeNode = (
  slave: SlaveConnection,
  dataType: DataTypeId,
  label: string,
  count: number,
) => ({
  id: `${slave.id}-${dataType}`,
  label,
  type: 'datatype',
  dataType,
  count,
  parentSlaveId: slave.id,
  profileId: slave.profileId,
  slaveId: slave.slaveId,
  linkProfileId: slave.linkProfileId,
  connectionId: slave.connectionId ?? null,
  slaveCommonAddress: slave.commonAddress,
})

const buildSectionNode = (
  slave: SlaveConnection,
  sectionKind: 'monitor' | 'control',
  label: string,
  children: any[],
) => ({
  id: `${slave.id}-${sectionKind}`,
  label,
  type: 'section',
  sectionKind,
  parentSlaveId: slave.id,
  children,
})

const buildDatatypeGroupNode = (
  slave: SlaveConnection,
  dataType: DataTypeId,
  label: string,
  count: number,
) => ({
  id: `${slave.id}-${dataType}`,
  label,
  type: 'datatype-group',
  dataType,
  count,
  parentSlaveId: slave.id,
  profileId: slave.profileId,
  slaveId: slave.slaveId,
  linkProfileId: slave.linkProfileId,
  connectionId: slave.connectionId ?? null,
  slaveCommonAddress: slave.commonAddress,
})

const hasAnyDataTypeCount = (countByType: DataTypeCountMap): boolean =>
  Object.values(countByType).some((count) => Number(count) > 0)

const resolveSlaveTreeStatus = (slave: SlaveConnection) =>
  deriveUnifiedConnStatus(slave.transportState, slave.dataTransferState).severity

const resolveLinkTreeStatus = (group: SlaveConnection[]) => {
  if (
    group.some(
      (item) =>
        deriveUnifiedConnStatus(item.transportState, item.dataTransferState).severity ===
        'status-normal',
    )
  ) {
    return 'status-normal'
  }
  if (
    group.some(
      (item) =>
        deriveUnifiedConnStatus(item.transportState, item.dataTransferState).severity ===
        'status-unready',
    )
  ) {
    return 'status-unready'
  }
  if (
    group.some(
      (item) =>
        deriveUnifiedConnStatus(item.transportState, item.dataTransferState).severity ===
        'status-processing',
    )
  ) {
    return 'status-processing'
  }
  if (
    group.some(
      (item) =>
        deriveUnifiedConnStatus(item.transportState, item.dataTransferState).severity ===
        'status-error',
    )
  ) {
    return 'status-error'
  }
  return 'status-idle'
}

function mergeNodeListInPlace(existing: any[], incoming: any[]) {
  const existingById = new Map<string, { node: any; index: number }>()
  for (let index = 0; index < existing.length; index += 1) {
    existingById.set(String(existing[index].id), { node: existing[index], index })
  }

  const incomingIds = new Set(incoming.map((node) => String(node.id)))
  for (let index = existing.length - 1; index >= 0; index -= 1) {
    if (!incomingIds.has(String(existing[index].id))) {
      existing.splice(index, 1)
    }
  }

  for (let index = 0; index < incoming.length; index += 1) {
    const next = incoming[index]
    const id = String(next.id)
    const current = existingById.get(id)

    if (current) {
      const target = current.node
      for (const key of Object.keys(next)) {
        if (key === 'children') continue
        target[key] = next[key]
      }
      if (next.children) {
        if (!target.children) target.children = []
        mergeNodeListInPlace(target.children, next.children)
      }
      const currentIndex = existing.indexOf(target)
      if (currentIndex !== index && currentIndex >= 0) {
        existing.splice(currentIndex, 1)
        existing.splice(index, 0, target)
      }
      continue
    }

    existing.splice(index, 0, next)
  }
}

export function useMasterTreeModel(options: UseMasterTreeModelOptions) {
  const resolveDataTypeLabelForSlave = (slave: SlaveConnection, dataType: DataTypeId): string => {
    const alias = normalizeIecObjectDisplayName(
      options.source.profilePointTypeSummary[slave.id]?.displayNameByType?.[dataType],
    )
    return alias
  }

  const countRuntimeDataTypeForSlave = (slave: SlaveConnection): DataTypeCountMap => {
    const connectionId = slave.connectionId
    const slaveId = Number(slave.slaveId)
    const hasSlaveId = Number.isFinite(slaveId) && slaveId > 0
    if (!connectionId && !hasSlaveId) return {}

    return options.source.pointData.getTypeCounts(hasSlaveId ? slaveId : null, connectionId)
  }

  const resolveDataTypeCountForSlave = (slave: SlaveConnection): DataTypeCountMap => {
    const configuredCountByType =
      options.source.profilePointTypeSummary[slave.id]?.countByType ?? {}
    if (hasAnyDataTypeCount(configuredCountByType)) {
      return configuredCountByType
    }
    return countRuntimeDataTypeForSlave(slave)
  }

  const buildDatatypeChildren = (slave: SlaveConnection) => {
    const countByType = resolveDataTypeCountForSlave(slave)
    const monitorChildren = MASTER_MONITOR_TYPES.map((dataType) =>
      buildDatatypeNode(
        slave,
        dataType,
        resolveDataTypeLabelForSlave(slave, dataType),
        Number(countByType[dataType] ?? 0),
      ),
    ).filter((item) => Number(item.count) > 0)

    const controlChildren = MASTER_CONTROL_GROUPS.map((group) =>
      buildDatatypeGroupNode(
        slave,
        group.marker,
        resolveMasterControlGroupLabel(group),
        group.dataTypes.reduce((total, type) => total + Number(countByType[type] ?? 0), 0),
      ),
    ).filter((item) => Number(item.count) > 0)

    const children: any[] = []
    if (monitorChildren.length > 0) {
      children.push(
        buildSectionNode(slave, 'monitor', t('master.tree.monitorSection'), monitorChildren),
      )
    }
    if (controlChildren.length > 0) {
      children.push(
        buildSectionNode(slave, 'control', t('master.tree.controlSection'), controlChildren),
      )
    }
    return children
  }

  const mergeSlaveTree = () => {
    const currentLinks = options.source.links
    const currentSlaves = options.source.profiles
    if (currentLinks.length === 0 && currentSlaves.length === 0) {
      options.slaveTreeData.value = []
      options.expandedSlaveKeys.value = []
      return
    }

    const groupedByLink = new Map<number, SlaveConnection[]>()
    for (const slave of currentSlaves) {
      const key = Number(slave.linkProfileId)
      const group = groupedByLink.get(key) ?? []
      group.push(slave)
      groupedByLink.set(key, group)
    }

    const linkMetaById = new Map<number, { label: string; host: string; port: number }>()
    for (const link of currentLinks) {
      const linkId = Number(link.id)
      if (!Number.isFinite(linkId) || linkId <= 0) continue
      linkMetaById.set(linkId, {
        label: link.name || t('master.tree.linkFallback', { id: linkId }),
        host: link.host || '',
        port: Number(link.port) || 0,
      })
    }

    for (const [linkProfileId, group] of groupedByLink.entries()) {
      if (linkMetaById.has(linkProfileId)) continue
      const sample = group[0]
      linkMetaById.set(linkProfileId, {
        label: sample?.linkName || t('master.tree.linkFallback', { id: linkProfileId }),
        host: sample?.host || '',
        port: sample?.port || 0,
      })
    }

    const nextTree = Array.from(linkMetaById.entries())
      .map(([linkProfileId, linkMeta]) => {
        const group = groupedByLink.get(linkProfileId) ?? []
        const sortedSlaves = [...group].sort((left, right) => {
          if (left.commonAddress !== right.commonAddress) {
            return left.commonAddress - right.commonAddress
          }
          return left.profileId - right.profileId
        })
        return {
          id: `link:${linkProfileId}`,
          label: linkMeta.label,
          type: 'link',
          linkProfileId,
          host: linkMeta.host,
          port: linkMeta.port,
          status: sortedSlaves.length > 0 ? resolveLinkTreeStatus(sortedSlaves) : 'status-idle',
          children: sortedSlaves.map((slave) => ({
            id: slave.id,
            label: slave.name,
            type: 'slave',
            status: resolveSlaveTreeStatus(slave),
            data: slave,
            children: buildDatatypeChildren(slave),
          })),
        }
      })
      .sort((left, right) => {
        const labelCompare = String(left.label).localeCompare(String(right.label), 'zh-CN')
        if (labelCompare !== 0) return labelCompare
        return Number(left.linkProfileId) - Number(right.linkProfileId)
      })

    mergeNodeListInPlace(options.slaveTreeData.value, nextTree)

    const expandableKeys = new Set<string>()
    const allTreeNodeKeys = new Set<string>()
    const defaultExpandedSectionKeys: string[] = []
    for (const link of options.slaveTreeData.value) {
      const linkKey = String(link.id)
      expandableKeys.add(linkKey)
      allTreeNodeKeys.add(linkKey)
      for (const slave of link.children ?? []) {
        const slaveKey = String(slave.id)
        expandableKeys.add(slaveKey)
        allTreeNodeKeys.add(slaveKey)
        for (const child of slave.children ?? []) {
          const childKey = String(child.id)
          allTreeNodeKeys.add(childKey)
          if (child.type === 'section') {
            expandableKeys.add(childKey)
            defaultExpandedSectionKeys.push(childKey)
            for (const leaf of child.children ?? []) {
              allTreeNodeKeys.add(String(leaf.id))
            }
          }
        }
      }
    }

    const fallbackKey = String(options.slaveTreeData.value[0]?.id ?? '')
    options.expandedSlaveKeys.value = Array.from(
      new Set([
        ...syncExpandedTreeKeys(options.expandedSlaveKeys.value, expandableKeys, {
          fallbackKey,
          applyFallbackWhenEmpty: true,
        }),
        ...defaultExpandedSectionKeys,
      ]),
    )

    if (
      options.selectedTreeNodeKey.value &&
      !allTreeNodeKeys.has(String(options.selectedTreeNodeKey.value))
    ) {
      options.selectedTreeNodeKey.value = ''
    }

    nextTick(() => {
      replayExpandedTreeKeys(options.treeRef.value, options.expandedSlaveKeys.value)
      const currentKey = options.selectedTreeNodeKey.value || options.selectedSlaveId.value
      if (
        currentKey &&
        String(options.treeRef.value?.getCurrentKey?.() ?? '') !== String(currentKey)
      ) {
        options.treeRef.value?.setCurrentKey?.(currentKey)
      }
    })
  }

  return {
    mergeSlaveTree,
  }
}
