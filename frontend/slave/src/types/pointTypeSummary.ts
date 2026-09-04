export type DataTypeCountMap = Record<string, number>

export type DataTypeDisplayNameMap = Record<string, string>

export type PointTypeSummary = {
  countByType: DataTypeCountMap
  displayNameByType: DataTypeDisplayNameMap
}

export const createEmptyPointTypeSummary = (): PointTypeSummary => ({
  countByType: {},
  displayNameByType: {},
})
