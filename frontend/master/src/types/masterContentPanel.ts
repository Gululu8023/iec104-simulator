import type {
  BackendControlStatusSnapshot,
  BackendDataPointQualityCommon,
  BackendDataPointQualityDetail,
} from '@shared/api/types'
import type { MasterRemoteControlFilterId } from '@shared/api/masterControl'
import type { PointTableSortOrder, PointTableSortProp } from '@shared/ui/pointTableSort'

export type MasterContentTabRow = {
  address: number
  name: string
  description: string
  dataType: string
  typeId: number
  historyKey?: string | null
  value: number
  qualityCode: number
  qualityLevel: 'good' | 'invalid' | 'questionable'
  qualityCommon?: BackendDataPointQualityCommon | null
  qualityDetail?: BackendDataPointQualityDetail | null
  timestamp: string
  latestCauseText: string
  reportCount: number | null
  oaCaText: string
  cp56Text: string
  pointSource?: string | null
  controlStatusSnapshot?: BackendControlStatusSnapshot | null
  controlIoa?: number | null
  isRuntime: boolean
}

export type MasterPointMetaUpdatePayload = {
  address: number
  addresses?: number[]
  dataType: string
  typeId: number
  name: string
  description: string
}

export type MasterContentTabState = {
  id: string
  label: string
  dataType: string
  profileId: string
  linkProfileId: number | null
  connectionId: string | null
  slaveCommonAddress?: number
  slaveName: string
  searchText: string
  controlSubtypeFilter: MasterRemoteControlFilterId
  sortProp: PointTableSortProp
  sortOrder: PointTableSortOrder
  data: MasterContentTabRow[]
  loading: boolean
  dataRevision: number
}

export type MasterContentQuickFilters = {
  onlyMapped: boolean
  onlyAbnormalQuality: boolean
  onlyRecentChange: boolean
  recentWindowSec: number
}

export type MasterContentRecentWindowOption = {
  label: string
  value: number
}

export type MasterContentEmptyState = {
  title: string
  hint: string
}
