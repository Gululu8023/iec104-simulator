// 主站相关类型定义
import type { Iec104CommandPayload, LinkParams, SelectExecuteMode } from '@shared/api/types'

// TCP 传输层状态
export type TransportState = 'disconnected' | 'connecting' | 'connected' | 'listening' | 'error'

// IEC104 数据传输层状态（STARTDT/STOPDT）
export type DataTransferState =
  | 'stopped'
  | 'starting'
  | 'started'
  | 'stopping'
  | 'timeout'
  | 'error'

// 从站连接信息
export interface SlaveConnection {
  /** slave id as string (tree node key) */
  id: string
  /** slave id (number) */
  profileId: number
  /** slave id (number, explicit semantic) */
  slaveId: number
  /** link profile id */
  linkProfileId: number
  /** runtime connection id when connected */
  connectionId?: string | null
  name: string
  linkName: string
  host: string
  port: number
  transportState: TransportState
  dataTransferState: DataTransferState
  reconnecting?: boolean
  reconnectAttempts?: number
  maxReconnectAttempts?: number
  lastConnected?: Date
  dataPointCount?: number
  commonAddress: number
  linkParams: LinkParams
}

type BackendCommandType = Iec104CommandPayload['type']

// 全局命令类型（直接对齐后端统一命令类型）
export type GlobalCommandType =
  | Extract<
      BackendCommandType,
      | 'general-interrogation'
      | 'counter-interrogation'
      | 'clock-synchronization'
      | 'test-command'
      | 'reset-process-command'
    >
  | 'toggle-data-transfer'
  | 'counter-freeze'
  | 'test-frame'
  | 'open-remote-control'
  | 'open-remote-adjust'
  | 'open-read-command'
  | 'open-file-transfer'

// 控制命令类型（保留前端 UI 别名，内部映射到后端命令）
export type ControlCommandType =
  | 'single-command'
  | 'double-command'
  | 'regulating-step'
  | 'set-point'
  | 'bit-string-command'
  | 'read-command'

// 命令类型（UI 层）
export type CommandType = GlobalCommandType | ControlCommandType

// 命令参数
export interface CommandParams {
  type: CommandType
  targetSlaveId?: string // 目标从站ID，全局命令时为空
  address?: number // 数据点地址（控制命令需要）
  value?: any // 命令值（控制命令需要）
  se?: SelectExecuteMode
  qu?: number
  setpointType?: 'normalized' | 'scaled' | 'short-float'
  ql?: number
}

// 数据点类型
export type DataPointType =
  | 'M_SP_NA_1' // 单点信息
  | 'M_DP_NA_1' // 双点信息
  | 'M_ME_NA_1' // 测量值，标准化值
  | 'M_ME_NB_1' // 测量值，标度化值
  | 'M_ME_NC_1' // 测量值，短浮点数
  | 'M_IT_NA_1' // 累积量
  | 'C_SC_NA_1' // 单点控制
  | 'C_DC_NA_1' // 双点控制
  | 'C_RC_NA_1' // 调节步控制
  | 'C_SE_NA_1' // 设定值，标准化值
  | 'C_SE_NB_1' // 设定值，标度化值
  | 'C_SE_NC_1' // 设定值，短浮点数

// 数据点信息
export interface DataPoint {
  address: number
  name: string
  type: DataPointType
  value: any
  quality: string
  timestamp: Date
  slaveId: string
}

// 主站配置
export interface MasterConfig {
  name: string
  protocol: 'iec104'
  commonAddress: number
  k: number // K参数
  w: number // W参数
  t0: number // t0超时
  t1: number // t1超时
  t2: number // t2超时
  t3: number // t3超时
}

// 命令历史记录
export interface CommandHistory {
  id: string
  timestamp: Date
  command: CommandType
  targetSlave?: string
  parameters?: any
  result: 'success' | 'failed' | 'timeout'
  message?: string
}

// 工具栏事件类型
export interface ToolbarEvents {
  menuAction: [action: string]
  globalCommand: [command: GlobalCommandType, params?: any]
  controlCommand: [command: ControlCommandType, params: CommandParams]
  connectionAction: [action: string, slaveId?: string]
}
