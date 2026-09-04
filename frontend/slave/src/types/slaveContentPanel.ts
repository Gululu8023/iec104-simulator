import type {
  BackendControlStatusSnapshot,
  BackendDataPointQualityCommon,
  BackendDataPointQualityDetail,
  PointSource,
} from '@shared/api/types'
import type { PointTableSortOrder, PointTableSortProp } from '@shared/ui/pointTableSort'

export interface SlaveContentDataPoint {
  address: number
  commonAddress?: number
  slaveId?: number | null
  name: string
  description: string
  value: any
  quality: 'good' | 'invalid' | 'questionable'
  qualityRaw: number
  qualityCommon?: BackendDataPointQualityCommon | null
  qualityDetail?: BackendDataPointQualityDetail | null
  timestamp: number
  dataType: string
  typeId: number
  controlIoa?: number | null
  giGroup?: number | null
  counterGroup?: number | null
  pointSource?: PointSource | null
  mappedTargetIoa?: number | null
  controlStatusSnapshot?: BackendControlStatusSnapshot | null
}

export interface SlaveContentTabData {
  id: string
  label: string
  dataType: string
  parentStationId?: string
  parentSlaveId: string
  slaveId?: number
  slaveName: string
  slaveCommonAddress?: number
  connectionName?: string
  searchText: string
  sortProp: PointTableSortProp
  sortOrder: PointTableSortOrder
  data: SlaveContentDataPoint[]
  loading: boolean
  dataRevision: number
}

export type SlaveContentActiveTabIdentity = Pick<
  SlaveContentTabData,
  'id' | 'parentStationId' | 'parentSlaveId' | 'slaveId' | 'dataType'
>

export interface SlaveContentMappingDialogContext {
  connectionName?: string
  slaveName?: string
  commonAddress?: number | null
}

export type SlaveContentQuickFilters = {
  onlyMapped: boolean
  onlyAbnormalQuality: boolean
  onlyRecentChange: boolean
  recentWindowSec: number
}

export type SlaveContentRecentWindowOption = {
  label: string
  value: number
}

export type SlaveContentEmptyState = {
  title: string
  hint: string
}
