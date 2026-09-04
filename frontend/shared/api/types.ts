export interface ApiError {
  code: string
  message: string
  detail?: string | null
  params?: Record<string, unknown> | null
}

export type AppLocale = 'zh-CN' | 'en-US'

export type Iec104ValueModel =
  | 'none'
  | 'boolean'
  | 'double_point'
  | 'step_position'
  | 'bitstring32'
  | 'normalized'
  | 'scaled_i16'
  | 'float32'
  | 'counter'
  | 'structured'

export type Iec104PointRole = 'monitor' | 'control' | 'non_point'

export type Iec104QualityModel = 'none' | 'siq' | 'diq' | 'qds' | 'bcr' | 'qdp'

export interface Iec104SimulationCapability {
  allowed_waveforms: string[]
  default_waveform: string
  input_kind: 'select' | 'integer' | 'float'
  min: number
  max: number
  step: number
  precision: number
}

export interface Iec104Capability {
  type_id: number
  name: string
  ioa_length: number
  family: string
  point_role: Iec104PointRole
  master_send: boolean
  master_receive: boolean
  slave_receive: boolean
  slave_upload: boolean
  point_configurable: boolean
  parser_only: boolean
  value_model: Iec104ValueModel
  quality_model: Iec104QualityModel
  timestamp_model: string
  cot_lifecycle: string
  simulation: Iec104SimulationCapability | null
}

export interface ExportCaptureResponse {
  file_name: string
  mime_type: string
  data_base64: string
}

export interface PointTableTemplateExportResponse {
  file_name: string
  mime_type: string
  data_base64: string
}

export type BackendTransportState = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'
export type BackendDataTransferState =
  | 'Stopped'
  | 'Starting'
  | 'Started'
  | 'Stopping'
  | 'Timeout'
  | 'Error'

export interface BackendConnectionInfo {
  id: string
  station_id: string
  /** DB profile id (master-side), when connected via a profile */
  profile_id?: number | null
  remote_addr: string
  transport_state: BackendTransportState
  data_transfer_state: BackendDataTransferState
  reconnecting?: boolean
  reconnect_attempts?: number
  max_reconnect_attempts?: number
  connected_at?: string | null
  last_clock_sync_at?: string | null
  tx_apdu_count?: number
  rx_apdu_count?: number
  tx_bytes?: number
  rx_bytes?: number
  retransmit_count?: number
  unknown_typeid_count?: number
  dropped_frame_count?: number
  soe_queue_peak?: number
}

export interface BackendDataPoint {
  connection_id: string
  link_profile_id?: number | null
  slave_id?: number | null
  common_address?: number | null
  address: number
  name: string
  description?: string | null
  control_ioa?: number | null
  gi_group?: number | null
  counter_group?: number | null
  type_id: number
  data_type: string
  value: number
  quality: number
  quality_common?: BackendDataPointQualityCommon | null
  quality_detail?: BackendDataPointQualityDetail | null
  business_usable?: boolean
  timestamp: string
  latest_event_timestamp?: string | null
  timestamp_detail?: BackendDataPointTimestampDetail | null
  report_count?: number | null
  latest_cause?: number | null
  point_source?: PointSource | null
  control_status_snapshot?: BackendControlStatusSnapshot | null
}

export type BackendDataPointTimestampDetail =
  | { kind: 'cp24'; raw: number[]; invalid: boolean; resolved?: string | null }
  | {
      kind: 'cp56'
      raw: number[]
      invalid: boolean
      summer_time: boolean
      weekday: number
      resolved?: string | null
    }

export interface BackendDataPointQualityCommon {
  iv: boolean
  nt: boolean
  sb: boolean
  bl: boolean
}

export type BackendDataPointQualityDetail =
  | {
      kind: 'siq'
      raw: number
      spi: boolean
      iv: boolean
      nt: boolean
      sb: boolean
      bl: boolean
    }
  | {
      kind: 'diq'
      raw: number
      dpi: number
      iv: boolean
      nt: boolean
      sb: boolean
      bl: boolean
    }
  | {
      kind: 'qds'
      raw: number
      ov: boolean
      iv: boolean
      nt: boolean
      sb: boolean
      bl: boolean
      transient?: boolean | null
      scd_status?: number | null
      scd_change?: number | null
    }
  | {
      kind: 'bcr'
      raw: number
      sq: number
      cy: boolean
      ca: boolean
      iv: boolean
    }
  | {
      kind: 'none'
    }

export type BackendMessageDirection = 'sent' | 'received' | 'error'

export type BackendStationStatus = 'Stopped' | 'Starting' | 'Running' | 'Stopping' | 'Error'

export interface BackendMasterMessage {
  id: number
  timestamp: string
  connection_id: string
  direction: BackendMessageDirection
  content: string
  hex_data?: string | null
  type_id?: number | null
  cause?: number | null
  common_address?: number | null
  command_trace_id?: string | null
  command_state?: string | null
}

export type PointSource = 'built_in' | 'imported' | 'manual'

export type BackendControlStatusKind = 'Idle' | 'Selected' | 'Executing' | 'Ok' | 'Ng' | 'Timeout'

export type BackendControlTimeoutType = 'select' | 'execute'

export interface BackendControlStatusSnapshot {
  state: BackendControlStatusKind
  updated_at: string
  latest_cause?: number | null
  actcon_negative?: boolean | null
  actcon_at?: string | null
  actterm_at?: string | null
  timeout_type?: BackendControlTimeoutType | null
  select_execute?: boolean | null
  command_cp56?: string | null
}

export interface BackendSlaveMessage {
  id: number
  timestamp: string
  connection_id: string
  direction: BackendMessageDirection
  content: string
  hex_data?: string | null
  type_id?: number | null
  cause?: number | null
  common_address?: number | null
}

export interface BackendMasterRuntimeHintEvent {
  station_id: string
  connection_id?: string | null
  reason: string
  attempt?: number | null
  max_attempts?: number | null
  point_cursor?: import('./pointSync').PointSyncCursor | null
}

export interface BackendSlaveRuntimeHintEvent {
  station_id: string
  connection_id?: string | null
  reason: string
  point_cursor?: import('./pointSync').PointSyncCursor | null
}

export interface BackendSoeEvent {
  id: number
  logged_at: string
  event_timestamp: string
  connection_id: string
  slave_id?: number | null
  point_name?: string | null
  common_address: number
  ioa: number
  type_id: number
  cause: number
  value: number
  quality: number
  quality_common?: BackendDataPointQualityCommon | null
  quality_detail?: BackendDataPointQualityDetail | null
  timestamp_detail?: BackendDataPointTimestampDetail | null
  business_usable?: boolean
}

export interface CreateStationRequest {
  name: string
  protocol: 'iec104'
  station_type: string
  host: string
  port: number
  common_address: number
  enabled: boolean
  point_template?: StationPointTemplate | null
}

export interface StationPointTemplateObject {
  name: string
  asdu_type: string
  ioa_start: number
  ioa_count: number
}

export interface StationPointTemplate {
  use_default_template: boolean
  objects: StationPointTemplateObject[]
}

export interface StationConfigResponse {
  station_id: string
  name: string
  protocol: 'iec104'
  station_type: string
  host: string
  port: number
  common_address: number
  enabled: boolean
}

export interface StationSummary {
  station_id: string
  name: string
  station_type: string
  host: string
  port: number
  common_address: number
  enabled: boolean
  status: BackendStationStatus
}

export interface StationStats {
  total_masters: number
  total_slaves: number
  running_masters: number
  running_slaves: number
}

export interface AppInfo {
  name: string
  version: string
  description: string
  license: string
  repository_url: string
  issues_url: string
  docs_url?: string | null
}

export interface ConnectRequest {
  host: string
  port: number
  profile_id?: number | null
  auto_start_data_transfer?: boolean | null
}

export interface LinkParams {
  k_value: number
  w_value: number
  t0_seconds: number
  t1_seconds: number
  t2_seconds: number
  t3_seconds: number
  max_asdu_bytes: number
  select_timeout_seconds: number
  enable_sq1_upload: boolean
  enable_soe: boolean
  unknown_typeid_negative_ack: boolean
}

export type RedundancyGroupMode = 'single' | 'connection' | 'multi'
export type SlaveCommandMismatchPolicy = 'strict' | 'compatible' | 'debug'
export type SlaveIoaDisplayFormat = 'dec' | 'hex'

export interface PointDef {
  address: number
  name: string
  type_id: number
  data_type: string
  description?: string | null
  control_ioa?: number | null
  gi_group?: number | null
  counter_group?: number | null
  default_value?: unknown | null
  is_enabled: boolean
  source?: PointSource | null
}

export interface UpsertLinkProfileRequest {
  name: string
  host: string
  port: number
  link_params?: LinkParams
  auto_connect?: boolean
  auto_start_data_transfer?: boolean
  auto_gi?: boolean
  retry_count?: number
  retry_interval?: number
}

export interface LinkProfileResponse {
  id: number
  station_id: string
  name: string
  host: string
  port: number
  link_params: LinkParams
  auto_connect: boolean
  auto_start_data_transfer: boolean
  auto_gi: boolean
  retry_count: number
  retry_interval: number
}

export interface UpsertLinkSlaveRequest {
  name: string
  common_address: number
  enabled?: boolean
}

export interface UpsertLinkSlaveBundleRequest {
  slave: UpsertLinkSlaveRequest
  point_defs: PointDef[]
}

export interface UpsertLinkSlaveBundleResponse {
  slave_id: number
  point_defs_updated: number
}

export interface LinkSlaveResponse {
  id: number
  connection_id: number
  name: string
  common_address: number
  enabled: boolean
}

export interface UpsertSlaveRequest {
  name: string
  common_address: number
  enabled?: boolean
  station_policy_override?: SlaveStationPolicyOverride
}

export interface UpsertSlaveBundleRequest {
  slave: UpsertSlaveRequest
  point_defs: PointDef[]
}

export interface UpsertSlaveBundleResponse {
  slave_id: number
  point_defs_updated: number
}

export interface SlaveResponse {
  id: number
  station_id: number
  name: string
  common_address: number
  enabled: boolean
  station_policy_override: SlaveStationPolicyOverride
  effective_station_policy: SlaveStationPolicy
}

export interface SlaveStationPolicy {
  select_timeout_seconds: number
  enable_sq1_upload: boolean
  enable_soe: boolean
  control_execution_mode: 'direct' | 'sbo'
}

export interface SlaveStationPolicyOverride {
  select_timeout_seconds?: number | null
  enable_sq1_upload?: boolean | null
  enable_soe?: boolean | null
  control_execution_mode?: 'direct' | 'sbo' | null
}

export interface UpdateSlaveStationConfigBundleRequest {
  config: CreateStationRequest
}

export interface UpdateSlaveStationConfigBundleResponse {
  station: StationConfigResponse
  point_defs_updated: number
}

export type PointTableImportTarget =
  | {
      kind: 'master-link-slave'
      slave_id: number
    }
  | {
      kind: 'slave-station-slave'
      slave_id: number
    }

export type PointTableImportFormat = 'xml' | 'csv' | 'json'
export type PointTableImportMode = 'append_only' | 'replace_all'

export interface PointTableImportIssue {
  code: string
  message: string
  location?: string | null
}

export interface PointTableImportSummary {
  total_points: number
  normalized_points: number
  preview_points: number
  duplicate_address_count: number
  control_mapping_count: number
  asdu_alias_count: number
  existing_points_count: number
  error_count: number
  warning_count: number
}

export interface PointTableImportNormalizedPayload {
  point_defs: PointDef[]
  asdu_aliases: Record<string, string>
  source_common_addresses?: Array<{ common_address: number; location: string }>
}

export interface PointTableImportPreview {
  target: PointTableImportTarget
  file_path: string
  file_name: string
  file_size: number
  format: PointTableImportFormat
  encoding: string
  can_apply: boolean
  summary: PointTableImportSummary
  errors: PointTableImportIssue[]
  warnings: PointTableImportIssue[]
  points_preview: PointDef[]
  asdu_aliases_preview: Record<string, string>
  normalized_payload: PointTableImportNormalizedPayload
}

export interface PointTableImportApplyResult {
  target: PointTableImportTarget
  imported_points: number
  existing_points: number
  asdu_aliases_updated: number
  runtime_applied: boolean
  applied_point_cursor?: import('./pointSync').PointSyncCursor | null
}

export type MessageDetailParseViewMode = 'table' | 'tree'

export type MessageParserStatus = 'success' | 'partial' | 'error'

export type MessageParserInputKind =
  | 'hex-continuous'
  | 'hex-byte-tokens'
  | 'hex-prefixed-tokens'
  | 'escaped-bytes'
  | 'byte-array'

export type MessageParserNodeTone = 'neutral' | 'warning' | 'danger'

export interface MessageParserByteRange {
  start: number
  end: number
}

export interface MessageParserIssue {
  code: string
  message: string
  detail?: string | null
  byte_range?: MessageParserByteRange | null
}

export interface MessageParserTreeNode {
  id: string
  label: string
  value?: string | null
  summary?: string | null
  byte_range?: MessageParserByteRange | null
  tone?: MessageParserNodeTone | null
  default_expanded: boolean
  children: MessageParserTreeNode[]
}

export interface MessageFrameParseSummary {
  input_kind?: MessageParserInputKind | null
  input_kind_label?: string | null
  input_byte_count: number
  frame_byte_count: number
  extra_byte_count: number
  declared_frame_byte_count?: number | null
  input_hex: string
  frame_hex: string
  normalized_bytes: number[]
  frame_type?: string | null
  frame_type_label?: string | null
  apdu_length?: number | null
  send_seq?: number | null
  recv_seq?: number | null
  type_id?: number | null
  type_name?: string | null
  cause_of_transmission?: number | null
  cause_description?: string | null
  common_address?: number | null
  object_count?: number | null
}

export interface MessageFrameParseResult {
  status: MessageParserStatus
  summary: MessageFrameParseSummary
  warnings: MessageParserIssue[]
  error?: MessageParserIssue | null
  tree: MessageParserTreeNode[]
}

export type Iec104ErrorCode =
  | 'INVALID_COMMON_ADDRESS'
  | 'INVALID_OBJECT_ADDRESS'
  | 'INVALID_QUALIFIER'
  | 'INVALID_VALUE'
  | 'UNSUPPORTED_COMMAND'
  | 'NOT_CONNECTED'
  | 'PROTOCOL_STATE'
  | 'ENCODE_FAILED'
  | 'SEND_FAILED'
  | 'INTERNAL_ERROR'

export type SelectExecuteMode = 'select' | 'execute' | 'cancel'
export type CounterFreezeMode = 'read' | 'freeze' | 'freeze-and-reset' | 'reset'
export type ResetProcessMode = 'general-reset' | 'clear-event-buffer'

export type InterrogationQualifier = { kind: 'station' } | { kind: 'group'; group: number }

export interface CounterInterrogationQualifier {
  request: number
  freeze: CounterFreezeMode
}

export interface SetpointQos {
  ql: number
  se: SelectExecuteMode
}

export type Iec104CommandPayload =
  | { type: 'general-interrogation'; qoi: InterrogationQualifier }
  | { type: 'counter-interrogation'; qcc: CounterInterrogationQualifier }
  | { type: 'clock-synchronization' }
  | { type: 'test-command'; ioa?: number }
  | { type: 'reset-process-command'; ioa?: number; qrp: ResetProcessMode }
  | { type: 'single-command'; ioa: number; value: boolean; se: SelectExecuteMode; qu: number }
  | { type: 'double-command'; ioa: number; value: 1 | 2; se: SelectExecuteMode; qu: number }
  | { type: 'regulating-step-command'; ioa: number; step: 1 | 2; se: SelectExecuteMode; qu: number }
  | { type: 'set-point-normalized'; ioa: number; value: number; qos: SetpointQos }
  | { type: 'set-point-scaled'; ioa: number; value: number; qos: SetpointQos }
  | { type: 'set-point-short-float'; ioa: number; value: number; qos: SetpointQos }
  | { type: 'bit-string-command'; ioa: number; value: number }
  | {
      type: 'single-command-timed'
      ioa: number
      value: boolean
      se: SelectExecuteMode
      qu: number
      timestamp?: string
    }
  | {
      type: 'double-command-timed'
      ioa: number
      value: 1 | 2
      se: SelectExecuteMode
      qu: number
      timestamp?: string
    }
  | {
      type: 'regulating-step-command-timed'
      ioa: number
      step: 1 | 2
      se: SelectExecuteMode
      qu: number
      timestamp?: string
    }
  | {
      type: 'set-point-normalized-timed'
      ioa: number
      value: number
      qos: SetpointQos
      timestamp?: string
    }
  | {
      type: 'set-point-scaled-timed'
      ioa: number
      value: number
      qos: SetpointQos
      timestamp?: string
    }
  | {
      type: 'set-point-short-float-timed'
      ioa: number
      value: number
      qos: SetpointQos
      timestamp?: string
    }
  | { type: 'bit-string-command-timed'; ioa: number; value: number; timestamp?: string }
  | { type: 'read-command'; ioa: number }

export interface Iec104CommandRequest {
  connection_id: string
  slave_id: number
  qualifier_mode: 'standard' | 'raw_test'
  command: Iec104CommandPayload
}

export interface Iec104CommandResult {
  accepted: boolean
  trace_id?: string | null
  error_code?: Iec104ErrorCode | null
  error_message?: string | null
}

export type MasterFileTransferMode = 'directory' | 'download' | 'upload' | 'query-log'
export type MasterFileTransferState = 'idle' | 'running' | 'success' | 'failed' | 'cancelled'

export interface MasterFileTransferRequest {
  connection_id: string
  slave_id: number
  mode: MasterFileTransferMode
  ioa?: number | null
  nof?: number | null
  local_path?: string | null
  query_start_time?: string | null
  query_end_time?: string | null
}

export interface MasterRemoteDirectoryEntry {
  ioa: number
  nof: number
  length: number
  status_raw: number
  is_file: boolean
  is_last: boolean
  timestamp?: string | null
}

export interface MasterFileTransferLimits {
  segment_payload_max: number
  single_section_max_file_size: number
}

export interface MasterFileTransferSession {
  session_id: string
  station_id: string
  connection_id: string
  slave_id: number
  common_address: number
  mode: MasterFileTransferMode
  state: MasterFileTransferState
  ioa: number
  nof?: number | null
  local_path?: string | null
  bytes_total: number
  bytes_done: number
  current_section: number
  current_segment: number
  failure_reason?: string | null
  directory_entries: MasterRemoteDirectoryEntry[]
  created_at: string
  updated_at: string
}

export interface UpdateDataPointRequest {
  address: number
  value: number
  quality: number
  common_address?: number
  control_ioa?: number | null
}

export interface InitializationProfile {
  point_count: number
  start_address: number
  base_value: number
  step: number
  quality: number
}

export interface SimulationProfile {
  enabled: boolean
  interval_ms: number
  delta_ratio: number
  min_value: number
  max_value: number
  seed: number
  auto_broadcast: boolean
}

export type SlaveWaveformSimulationType =
  | 'fixed'
  | 'step'
  | 'pulse'
  | 'square'
  | 'saw-up'
  | 'saw-down'
  | 'triangle'
  | 'sine'
  | 'ramp-hold-fall'
  | 'stair'
  | 'exp-approach'
  | 'damped-oscillation'
  | 'random'
  | 'random-walk'
  | 'counter'

export interface SlaveWaveformSimulationConfig {
  fixedValue: number
  waveformType: SlaveWaveformSimulationType
  waveformBase: number
  waveformAmplitude: number
  waveformPeriod: number
  waveformDutyCycle: number
  waveformPulseWidth: number
  waveformRiseTime: number
  waveformHoldTime: number
  waveformFallTime: number
  waveformStepSize: number
  waveformStepCount: number
  waveformTargetValue: number
  waveformDamping: number
  waveformMinValue: number
  waveformMaxValue: number
  waveformResetValue: number
}

export interface SlaveSelectedSimulationPoint {
  address: number
  common_address?: number
}

export interface StartSlaveSelectedSimulationRequest {
  points: SlaveSelectedSimulationPoint[]
  config: SlaveWaveformSimulationConfig
}

export type SimulationScenario =
  | { type: 'avalanche'; count?: number; common_address?: number }
  | {
      type: 'waveform'
      address: number
      common_address?: number
      waveform_type?: 'fixed' | 'linear' | 'sine' | 'triangle' | 'step' | 'random'
      amplitude?: number
      base?: number
      period_seconds?: number
      phase?: number
      fixed_value?: number
      linear_start?: number
      linear_step?: number
    }
  | { type: 'communication-fault'; reason?: string }
