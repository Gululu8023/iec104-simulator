// 从站相关类型定义

// TCP连接配置
export interface TcpConnectionConfig {
  /** 本地IP地址 */
  localAddress: string
  /** 本地端口 */
  localPort: number
  /** 连接超时时间(秒) */
  connectionTimeout: number
  /** 心跳间隔(秒) */
  heartbeatInterval: number
  /** 最大连接数 */
  maxConnections: number
}

// IEC104协议参数
export interface Iec104ProtocolConfig {
  /** ASDU地址 */
  asduAddress: number
  /** K值 - 发送方在t1超时前，最多可发送的I格式帧数 */
  kValue: number
  /** W值 - 接收方在t2超时前，最多可接收的I格式帧数 */
  wValue: number
  /** t0 - 连接建立的超时时间(秒) */
  t0: number
  /** t1 - 发送或测试APDU的超时时间(秒) */
  t1: number
  /** t2 - 无数据报文时确认的超时时间(秒) */
  t2: number
  /** t3 - 长时间空闲状态下发送测试帧的超时时间(秒) */
  t3: number
  /** 是否启用时标 */
  enableTimestamp: boolean
  /** 时标大小(字节) */
  timestampSize: number
}

// 从站配置
export interface SlaveStationConfig {
  /** 从站ID */
  id: string
  /** 从站名称 */
  name: string
  /** 从站描述 */
  description?: string
  /** TCP连接配置 */
  tcpConfig: TcpConnectionConfig
  /** IEC104协议配置 */
  protocolConfig: Iec104ProtocolConfig
  /** 是否启用 */
  enabled: boolean
  /** 创建时间 */
  createdAt: number
  /** 更新时间 */
  updatedAt: number
}

// 从站状态
export type SlaveStationStatus = 'stopped' | 'starting' | 'running' | 'stopping' | 'error'

// 从站运行时信息
export interface SlaveStationRuntime {
  /** 从站配置 */
  config: SlaveStationConfig
  /** 当前状态 */
  status: SlaveStationStatus
  /** 连接数 */
  connectionCount: number
  /** 最后活动时间 */
  lastActivity: number
  /** 错误信息 */
  errorMessage?: string
}

// 设备树节点类型
export interface DeviceNode {
  id: string
  label: string
  type: 'listener' | 'slave' | 'connection' | 'type-id' | 'group'
  status?:
    | 'status-idle'
    | 'status-listening'
    | 'status-processing'
    | 'status-unready'
    | 'status-normal'
    | 'status-error'
    | 'running'
    | 'listening'
    | 'connected'
    | 'disconnected'
    | 'normal'
    | 'stopped'
    | 'error'
    | 'connecting'
    | 'starting'
    | 'stopping'
  address?: string
  port?: number
  stationId?: string
  parentStationId?: string
  slaveId?: number
  dataType?: string
  count?: number
  children?: DeviceNode[]
  // 数据点类型节点的父从站信息
  parentSlaveId?: string
  parentSlaveName?: string
  // 允许额外的动态属性
  [key: string]: any
}

// 数据点类型
export interface DataType {
  id: string
  label: string
  dataType: string
  count: number
}

// 数据点
export interface DataPoint {
  address: number
  name: string
  description: string
  value: any
  quality: 'good' | 'invalid' | 'questionable'
  timestamp: number
  dataType: string
}

// 从站操作类型
export type SlaveOperation = 'edit-connection' | 'disconnect' | 'edit-parameters' | 'delete'

// 从站操作事件
export interface SlaveOperationEvent {
  operation: SlaveOperation
  slaveId: string
  data?: any
}
