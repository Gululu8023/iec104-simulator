import { getIec104Capability, resolveIec104TypeId } from '@shared/api/iec104'
import type { Iec104SimulationCapability, Iec104ValueModel } from '@shared/api/types'
import { t } from '@shared/i18n'

export type WaveformSimulationType =
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

export interface WaveformSimulationPointLike {
  address: number
  value?: unknown
  typeId?: number | null
  dataType?: string | null
}

export interface WaveformSimulationConfig {
  fixedValue: number
  waveformType: WaveformSimulationType
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

export interface WaveformParameterDefinition {
  key: Exclude<keyof WaveformSimulationConfig, 'waveformType'>
  label: string
  min?: number
  max?: number
  step?: number
  precision?: number
  options?: Array<{ label: string; value: number }>
}

export interface SimulationValueProfile {
  valueModel: Iec104ValueModel
  allowedWaveforms: WaveformSimulationType[]
  defaultWaveform: WaveformSimulationType
  inputKind: Iec104SimulationCapability['input_kind']
  min: number
  max: number
  step: number
  precision: number
}

export type SimulationProfileResolution =
  | { profile: SimulationValueProfile; error: null }
  | { profile: null; error: 'unsupported' | 'mixed' }

export type WaveformPointCategory =
  | 'single-point'
  | 'double-point'
  | 'measurement'
  | 'counter'
  | 'step-position'

export interface WaveformDefinition {
  value: WaveformSimulationType
  label: string
  description: string
  summaryBuilder: (config: WaveformSimulationConfig, tickMs: number) => string
  parameterDefs: WaveformParameterDefinition[]
  previewPathBuilder: (config: WaveformSimulationConfig) => string
}

export interface WaveformSimulationRuntimeState {
  lastBucket?: number
  currentValue?: number
}

export interface WaveformSimulationComputeResult {
  value: number
  runtimeState: WaveformSimulationRuntimeState
}

const WAVEFORM_TYPE_SET = new Set<WaveformSimulationType>([
  'fixed',
  'step',
  'pulse',
  'square',
  'saw-up',
  'saw-down',
  'triangle',
  'sine',
  'ramp-hold-fall',
  'stair',
  'exp-approach',
  'damped-oscillation',
  'random',
  'random-walk',
  'counter',
])

const FALLBACK_VALUE_PROFILE: SimulationValueProfile = {
  valueModel: 'float32',
  allowedWaveforms: ['fixed'],
  defaultWaveform: 'fixed',
  inputKind: 'float',
  min: -3.4028235e38,
  max: 3.4028235e38,
  step: 0.1,
  precision: 4,
}

const toFiniteNumber = (value: unknown, fallback: number): number => {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : fallback
}

const clamp = (value: number, min: number, max: number): number => {
  if (!Number.isFinite(value)) return min
  return Math.min(max, Math.max(min, value))
}

const normalizeInteger = (value: unknown, fallback: number, min: number, max: number): number => {
  return Math.trunc(clamp(toFiniteNumber(value, fallback), min, max))
}

const roundNumber = (value: number, precision = 4): number => {
  if (!Number.isFinite(value)) return 0
  return Number(value.toFixed(precision))
}

const formatNumber = (value: number): string => {
  if (!Number.isFinite(value)) return '0'
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/\.?0+$/, '')
}

const resolvePointTypeId = (point: WaveformSimulationPointLike | null | undefined): number => {
  return resolveIec104TypeId({
    type_id: Number(point?.typeId ?? 0),
    data_type: String(point?.dataType ?? ''),
  })
}

const toSimulationValueProfile = (
  valueModel: Iec104ValueModel,
  capability: Iec104SimulationCapability,
): SimulationValueProfile | null => {
  const allowedWaveforms = capability.allowed_waveforms.filter(
    (waveform): waveform is WaveformSimulationType =>
      WAVEFORM_TYPE_SET.has(waveform as WaveformSimulationType),
  )
  const defaultWaveform = capability.default_waveform as WaveformSimulationType
  if (!allowedWaveforms.includes(defaultWaveform)) return null
  return {
    valueModel,
    allowedWaveforms,
    defaultWaveform,
    inputKind: capability.input_kind,
    min: capability.min,
    max: capability.max,
    step: capability.step,
    precision: capability.precision,
  }
}

const resolvePointValueProfile = (
  point: WaveformSimulationPointLike | null | undefined,
): SimulationValueProfile | null => {
  const capability = getIec104Capability(resolvePointTypeId(point))
  if (!capability?.simulation) return null
  return toSimulationValueProfile(capability.value_model, capability.simulation)
}

export const resolveSimulationValueProfile = (
  points: readonly WaveformSimulationPointLike[],
): SimulationProfileResolution => {
  const profiles = points.map(resolvePointValueProfile)
  if (profiles.length === 0 || profiles.some((profile) => profile == null)) {
    return { profile: null, error: 'unsupported' }
  }
  const profile = profiles[0]!
  if (profiles.some((candidate) => candidate!.valueModel !== profile.valueModel)) {
    return { profile: null, error: 'mixed' }
  }
  return { profile, error: null }
}

const resolveWaveformPointCategory = (
  point: WaveformSimulationPointLike | null | undefined,
): WaveformPointCategory => {
  switch (resolvePointValueProfile(point)?.valueModel) {
    case 'boolean':
      return 'single-point'
    case 'double_point':
      return 'double-point'
    case 'counter':
      return 'counter'
    case 'step_position':
      return 'step-position'
    default:
      return 'measurement'
  }
}

const normalizeValueByPoint = (point: WaveformSimulationPointLike, value: number): number => {
  const profile = resolvePointValueProfile(point)
  if (!profile) return value
  if (profile.valueModel === 'boolean') {
    return value >= 0.5 ? 1 : 0
  }
  const clamped = clamp(value, profile.min, profile.max)
  return profile.inputKind === 'integer' || profile.inputKind === 'select'
    ? Math.round(clamped)
    : roundNumber(clamped, profile.precision)
}

const deterministicNoise = (seed: number, address: number, bucket: number): number => {
  const hashed = Math.sin(seed * 12.9898 + address * 78.233 + bucket * 37.719) * 43758.5453123
  const unit = hashed - Math.floor(hashed)
  return unit * 2 - 1
}

const resolveCycleSeconds = (config: WaveformSimulationConfig): number => {
  switch (config.waveformType) {
    case 'ramp-hold-fall':
      return Math.max(
        0.3,
        config.waveformRiseTime + config.waveformHoldTime + config.waveformFallTime,
      )
    case 'stair':
      return Math.max(0.2, config.waveformStepCount * config.waveformPeriod)
    default:
      return Math.max(0.2, config.waveformPeriod)
  }
}

const normalizeWaveformCycle = (elapsedSeconds: number, cycleSeconds: number): number => {
  const safeCycle = cycleSeconds > 0 ? cycleSeconds : 1
  return (elapsedSeconds / safeCycle) % 1
}

const previewConfig = (raw: Partial<WaveformSimulationConfig>): WaveformSimulationConfig =>
  normalizeWaveformSimulationConfig({
    waveformType: 'sine',
    fixedValue: 0,
    waveformBase: 0,
    waveformAmplitude: 1,
    waveformPeriod: 4,
    waveformDutyCycle: 50,
    waveformPulseWidth: 0.8,
    waveformRiseTime: 1.5,
    waveformHoldTime: 1,
    waveformFallTime: 1.5,
    waveformStepSize: 1,
    waveformStepCount: 5,
    waveformTargetValue: 1,
    waveformDamping: 0.8,
    waveformMinValue: -2,
    waveformMaxValue: 2,
    waveformResetValue: 0,
    ...raw,
  })

const formatHigh = (config: WaveformSimulationConfig): string =>
  formatNumber(config.waveformBase + config.waveformAmplitude)

const formatLow = (config: WaveformSimulationConfig): string =>
  formatNumber(config.waveformBase - config.waveformAmplitude)

const WAVEFORM_I18N_KEYS: Record<WaveformSimulationType, string> = {
  fixed: 'fixed',
  step: 'step',
  pulse: 'pulse',
  square: 'square',
  'saw-up': 'sawUp',
  'saw-down': 'sawDown',
  triangle: 'triangle',
  sine: 'sine',
  'ramp-hold-fall': 'rampHoldFall',
  stair: 'stair',
  'exp-approach': 'expApproach',
  'damped-oscillation': 'dampedOscillation',
  random: 'random',
  'random-walk': 'randomWalk',
  counter: 'counter',
}

const defineWaveform = (
  definition: Omit<WaveformDefinition, 'label' | 'description'>,
): WaveformDefinition => ({
  ...definition,
  get label() {
    return t(`slave.waveform.types.${WAVEFORM_I18N_KEYS[definition.value]}.label`)
  },
  get description() {
    return t(`slave.waveform.types.${WAVEFORM_I18N_KEYS[definition.value]}.description`)
  },
})

const defineParameter = (
  definition: Omit<WaveformParameterDefinition, 'label'> & { labelKey: string },
): WaveformParameterDefinition => {
  const { labelKey, ...rest } = definition
  return {
    ...rest,
    get label() {
      return t(`slave.waveform.params.${labelKey}`)
    },
  }
}

const formatSeconds = (seconds: number): string => {
  const normalized = Math.max(0, Number(seconds) || 0)
  const value =
    normalized >= 10
      ? normalized.toFixed(1).replace(/\.0$/, '')
      : normalized >= 1
        ? normalized.toFixed(2).replace(/\.?0+$/, '')
        : normalized.toFixed(3).replace(/\.?0+$/, '')
  return t('slave.waveform.units.second', { value })
}

const formatTickDuration = (ticks: number, tickMs: number): string =>
  t('slave.waveform.units.tickDuration', {
    ticks: formatNumber(ticks),
    duration: formatSeconds((ticks * tickMs) / 1000),
  })

const summaryFixed = (config: WaveformSimulationConfig, _tickMs: number) =>
  t('slave.waveform.summary.fixed', { fixedValue: formatNumber(config.fixedValue) })

const summaryStep = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.step', {
    base: formatNumber(config.waveformBase),
    high: formatHigh(config),
    period: formatTickDuration(config.waveformPeriod, tickMs),
  })

const summaryPulse = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.pulse', {
    period: formatTickDuration(config.waveformPeriod, tickMs),
    width: formatTickDuration(config.waveformPulseWidth, tickMs),
  })

const summarySquare = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.square', {
    base: formatNumber(config.waveformBase),
    high: formatHigh(config),
    period: formatTickDuration(config.waveformPeriod, tickMs),
    duty: formatNumber(config.waveformDutyCycle),
  })

const summarySawUp = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.sawUp', {
    period: formatTickDuration(config.waveformPeriod, tickMs),
    base: formatNumber(config.waveformBase),
    high: formatHigh(config),
  })

const summarySawDown = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.sawDown', {
    period: formatTickDuration(config.waveformPeriod, tickMs),
    base: formatNumber(config.waveformBase),
    low: formatLow(config),
  })

const summaryTriangle = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.triangle', {
    base: formatNumber(config.waveformBase),
    low: formatLow(config),
    high: formatHigh(config),
    period: formatTickDuration(config.waveformPeriod, tickMs),
  })

const summarySine = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.sine', {
    base: formatNumber(config.waveformBase),
    amplitude: formatNumber(config.waveformAmplitude),
    period: formatTickDuration(config.waveformPeriod, tickMs),
  })

const summaryRampHoldFall = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.rampHoldFall', {
    rise: formatTickDuration(config.waveformRiseTime, tickMs),
    high: formatHigh(config),
    hold: formatTickDuration(config.waveformHoldTime, tickMs),
    fall: formatTickDuration(config.waveformFallTime, tickMs),
  })

const summaryStair = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.stair', {
    base: formatNumber(config.waveformBase),
    period: formatTickDuration(config.waveformPeriod, tickMs),
    stepSize: formatNumber(config.waveformStepSize),
    stepCount: config.waveformStepCount,
  })

const summaryExpApproach = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.expApproach', {
    base: formatNumber(config.waveformBase),
    target: formatNumber(config.waveformTargetValue),
    period: formatTickDuration(config.waveformPeriod, tickMs),
  })

const summaryDamped = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.dampedOscillation', {
    base: formatNumber(config.waveformBase),
    period: formatTickDuration(config.waveformPeriod, tickMs),
    amplitude: formatNumber(config.waveformAmplitude),
    damping: formatNumber(config.waveformDamping),
  })

const summaryRandom = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.random', {
    period: formatTickDuration(config.waveformPeriod, tickMs),
    low: formatLow(config),
    high: formatHigh(config),
  })

const summaryRandomWalk = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.randomWalk', {
    base: formatNumber(config.waveformBase),
    period: formatTickDuration(config.waveformPeriod, tickMs),
    stepSize: formatNumber(config.waveformStepSize),
    min: formatNumber(config.waveformMinValue),
    max: formatNumber(config.waveformMaxValue),
  })

const summaryCounter = (config: WaveformSimulationConfig, tickMs: number) =>
  t('slave.waveform.summary.counter', {
    base: formatNumber(config.waveformBase),
    period: formatTickDuration(config.waveformPeriod, tickMs),
    stepSize: formatNumber(config.waveformStepSize),
    max: formatNumber(config.waveformMaxValue),
    reset: formatNumber(config.waveformResetValue),
  })

export const WAVEFORM_DEFINITIONS: WaveformDefinition[] = [
  defineWaveform({
    value: 'fixed',
    summaryBuilder: summaryFixed,
    parameterDefs: [
      defineParameter({ key: 'fixedValue', labelKey: 'fixedValue', step: 0.1, precision: 4 }),
    ],
    previewPathBuilder: () => 'M 8 30 L 172 30',
  }),
  defineWaveform({
    value: 'step',
    summaryBuilder: summaryStep,
    parameterDefs: [
      defineParameter({ key: 'waveformBase', labelKey: 'waveformBase', step: 0.1, precision: 4 }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeStep',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriod',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 44 L 84 44 L 84 16 L 152 16 L 152 44 L 172 44',
  }),
  defineWaveform({
    value: 'pulse',
    summaryBuilder: summaryPulse,
    parameterDefs: [
      defineParameter({ key: 'waveformBase', labelKey: 'waveformBase', step: 0.1, precision: 4 }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudePulse',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPulseWidth',
        labelKey: 'waveformPulseWidth',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodRepeat',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 44 L 36 44 L 36 16 L 64 16 L 64 44 L 116 44 L 116 16 L 144 16 L 144 44 L 172 44',
  }),
  defineWaveform({
    value: 'square',
    summaryBuilder: summarySquare,
    parameterDefs: [
      defineParameter({ key: 'waveformBase', labelKey: 'waveformBase', step: 0.1, precision: 4 }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeHigh',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformDutyCycle',
        labelKey: 'waveformDutyCycle',
        min: 1,
        max: 99,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriod',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 44 L 36 44 L 36 16 L 96 16 L 96 44 L 132 44 L 132 16 L 172 16',
  }),
  defineWaveform({
    value: 'saw-up',
    summaryBuilder: summarySawUp,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseStart',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeRise',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodShort',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 52 L 172 8 M 172 8 L 172 52',
  }),
  defineWaveform({
    value: 'saw-down',
    summaryBuilder: summarySawDown,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseStart',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeFall',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodShort',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 8 L 172 52 M 172 52 L 172 8',
  }),
  defineWaveform({
    value: 'triangle',
    summaryBuilder: summaryTriangle,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseCenter',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeRange',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriod',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 44 L 52 12 L 96 44 L 140 12 L 172 44',
  }),
  defineWaveform({
    value: 'sine',
    summaryBuilder: summarySine,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseCenter',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeRange',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriod',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 30 C 24 10, 44 10, 60 30 C 76 50, 96 50, 112 30 C 128 10, 148 10, 172 30',
  }),
  defineWaveform({
    value: 'ramp-hold-fall',
    summaryBuilder: summaryRampHoldFall,
    parameterDefs: [
      defineParameter({ key: 'waveformBase', labelKey: 'waveformBase', step: 0.1, precision: 4 }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeHeight',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformRiseTime',
        labelKey: 'waveformRiseTime',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformHoldTime',
        labelKey: 'waveformHoldTime',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformFallTime',
        labelKey: 'waveformFallTime',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 44 L 64 14 L 118 14 L 172 44',
  }),
  defineWaveform({
    value: 'stair',
    summaryBuilder: summaryStair,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseStart',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformStepSize',
        labelKey: 'waveformStepSize',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformStepCount',
        labelKey: 'waveformStepCount',
        min: 2,
        max: 64,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodPerStep',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 46 L 32 46 L 32 38 L 60 38 L 60 30 L 88 30 L 88 22 L 116 22 L 116 14 L 144 14 L 144 46 L 172 46',
  }),
  defineWaveform({
    value: 'exp-approach',
    summaryBuilder: summaryExpApproach,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseStart',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformTargetValue',
        labelKey: 'waveformTargetValue',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodApproach',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () => 'M 8 46 C 28 40, 48 26, 72 18 C 94 12, 124 10, 172 10',
  }),
  defineWaveform({
    value: 'damped-oscillation',
    summaryBuilder: summaryDamped,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseCenter',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeInitial',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodOscillation',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformDamping',
        labelKey: 'waveformDamping',
        min: 0.05,
        max: 10,
        step: 0.05,
        precision: 2,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 30 C 22 10, 34 8, 46 24 C 58 40, 70 42, 82 30 C 94 18, 106 18, 118 28 C 130 36, 142 36, 172 30',
  }),
  defineWaveform({
    value: 'random',
    summaryBuilder: summaryRandom,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseCenter',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformAmplitude',
        labelKey: 'waveformAmplitudeRandom',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodRefresh',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 35 L 28 18 L 46 40 L 64 14 L 82 36 L 100 20 L 118 42 L 136 16 L 154 34 L 172 22',
  }),
  defineWaveform({
    value: 'random-walk',
    summaryBuilder: summaryRandomWalk,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseStart',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformStepSize',
        labelKey: 'waveformStepSizeShort',
        min: 0,
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformMinValue',
        labelKey: 'waveformMinValue',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformMaxValue',
        labelKey: 'waveformMaxValue',
        step: 0.1,
        precision: 4,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodRefresh',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 34 L 28 26 L 48 30 L 68 22 L 88 28 L 108 18 L 128 26 L 148 20 L 172 24',
  }),
  defineWaveform({
    value: 'counter',
    summaryBuilder: summaryCounter,
    parameterDefs: [
      defineParameter({
        key: 'waveformBase',
        labelKey: 'waveformBaseStart',
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformStepSize',
        labelKey: 'waveformStepSizeCounter',
        min: 0,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformPeriod',
        labelKey: 'waveformPeriodRefresh',
        min: 1,
        max: 300,
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformMaxValue',
        labelKey: 'waveformMaxValueWrap',
        step: 1,
        precision: 0,
      }),
      defineParameter({
        key: 'waveformResetValue',
        labelKey: 'waveformResetValue',
        step: 1,
        precision: 0,
      }),
    ],
    previewPathBuilder: () =>
      'M 8 46 L 36 46 L 36 38 L 64 38 L 64 30 L 92 30 L 92 22 L 120 22 L 120 14 L 148 14 L 148 46 L 172 46',
  }),
]

const waveformDefinitionMap = new Map(
  WAVEFORM_DEFINITIONS.map((definition) => [definition.value, definition] as const),
)

export const normalizeWaveformSimulationType = (raw: unknown): WaveformSimulationType => {
  const normalized = String(raw ?? '')
    .trim()
    .toLowerCase()
  if (normalized === 'linear') return 'saw-up'
  if (WAVEFORM_TYPE_SET.has(normalized as WaveformSimulationType)) {
    return normalized as WaveformSimulationType
  }
  return 'sine'
}

export const getWaveformDefinition = (raw: unknown): WaveformDefinition => {
  const waveformType = normalizeWaveformSimulationType(raw)
  return waveformDefinitionMap.get(waveformType) ?? waveformDefinitionMap.get('sine')!
}

const VALUE_PARAMETER_KEYS = new Set<WaveformParameterDefinition['key']>([
  'fixedValue',
  'waveformBase',
  'waveformAmplitude',
  'waveformStepSize',
  'waveformTargetValue',
  'waveformMinValue',
  'waveformMaxValue',
  'waveformResetValue',
])

const MAGNITUDE_PARAMETER_KEYS = new Set<WaveformParameterDefinition['key']>([
  'waveformAmplitude',
  'waveformStepSize',
])

export const getWaveformParameterDefinitions = (
  raw: unknown,
  profile?: SimulationValueProfile | null,
): WaveformParameterDefinition[] => {
  const definitions = getWaveformDefinition(raw).parameterDefs
  if (!profile) return definitions
  const valueSpan = profile.max - profile.min
  return definitions.map((definition) => {
    if (!VALUE_PARAMETER_KEYS.has(definition.key)) return definition
    const isMagnitude = MAGNITUDE_PARAMETER_KEYS.has(definition.key)
    const min = isMagnitude ? 0 : profile.min
    const max = isMagnitude ? valueSpan : profile.max
    const options =
      profile.inputKind === 'select'
        ? Array.from({ length: Math.trunc(max - min) + 1 }, (_, index) => {
            const value = min + index
            return { label: String(value), value }
          })
        : undefined
    return {
      ...definition,
      min,
      max,
      step: profile.step,
      precision: profile.precision,
      options,
    }
  })
}

export const getWaveformPreviewPath = (
  rawType: unknown,
  rawConfig: Partial<WaveformSimulationConfig> | Record<string, unknown> = {},
): string => {
  const config = normalizeWaveformSimulationConfig(rawConfig)
  return getWaveformDefinition(rawType).previewPathBuilder(config)
}

export const getWaveformTypeLabel = (raw: unknown): string => {
  return getWaveformDefinition(raw).label
}

export const getWaveformShortCode = (raw: unknown): string => {
  const waveformType = normalizeWaveformSimulationType(raw)
  switch (waveformType) {
    case 'fixed':
      return 'F'
    case 'step':
      return 'Q'
    case 'pulse':
      return 'P'
    case 'square':
      return 'SQ'
    case 'saw-up':
      return 'SU'
    case 'saw-down':
      return 'SD'
    case 'triangle':
      return 'T'
    case 'sine':
      return 'S'
    case 'ramp-hold-fall':
      return 'R'
    case 'stair':
      return 'ST'
    case 'exp-approach':
      return 'EA'
    case 'damped-oscillation':
      return 'DO'
    case 'random':
      return 'RD'
    case 'random-walk':
      return 'RW'
    case 'counter':
      return 'C'
    default:
      return 'W'
  }
}

export const normalizeWaveformSimulationConfig = (
  raw: Partial<WaveformSimulationConfig> | Record<string, unknown>,
  profile?: SimulationValueProfile | null,
): WaveformSimulationConfig => {
  const waveformType = normalizeWaveformSimulationType(raw.waveformType)
  const waveformBase = roundNumber(toFiniteNumber(raw.waveformBase, 0))
  const waveformAmplitude = roundNumber(Math.abs(toFiniteNumber(raw.waveformAmplitude, 0)))
  const waveformPeriod = normalizeInteger(raw.waveformPeriod, 4, 1, 300)
  const waveformDutyCycle = roundNumber(clamp(toFiniteNumber(raw.waveformDutyCycle, 50), 1, 99), 4)
  const waveformPulseWidth = normalizeInteger(
    raw.waveformPulseWidth,
    Math.max(1, Math.trunc(waveformPeriod / 4) || 1),
    1,
    waveformPeriod,
  )
  const waveformRiseTime = normalizeInteger(raw.waveformRiseTime, 2, 1, 300)
  const waveformHoldTime = normalizeInteger(raw.waveformHoldTime, 2, 1, 300)
  const waveformFallTime = normalizeInteger(raw.waveformFallTime, 2, 1, 300)
  const waveformStepSize = roundNumber(Math.abs(toFiniteNumber(raw.waveformStepSize, 1)))
  const waveformStepCount = normalizeInteger(raw.waveformStepCount, 8, 2, 64)
  const waveformTargetValue = roundNumber(
    toFiniteNumber(raw.waveformTargetValue, waveformBase + waveformAmplitude),
  )
  const waveformDamping = roundNumber(clamp(toFiniteNumber(raw.waveformDamping, 0.8), 0.05, 10), 4)
  const rawMin = roundNumber(
    toFiniteNumber(raw.waveformMinValue, waveformBase - Math.max(waveformAmplitude * 3, 5)),
  )
  const rawMax = roundNumber(
    toFiniteNumber(raw.waveformMaxValue, waveformBase + Math.max(waveformAmplitude * 3, 5)),
  )
  const waveformMinValue = Math.min(rawMin, rawMax)
  const waveformMaxValue = Math.max(rawMin, rawMax)
  const waveformResetValue = roundNumber(
    clamp(toFiniteNumber(raw.waveformResetValue, waveformBase), waveformMinValue, waveformMaxValue),
  )
  const fixedValue = roundNumber(toFiniteNumber(raw.fixedValue, waveformBase))

  const normalized = {
    fixedValue,
    waveformType,
    waveformBase,
    waveformAmplitude,
    waveformPeriod,
    waveformDutyCycle,
    waveformPulseWidth,
    waveformRiseTime,
    waveformHoldTime,
    waveformFallTime,
    waveformStepSize,
    waveformStepCount,
    waveformTargetValue,
    waveformDamping,
    waveformMinValue,
    waveformMaxValue,
    waveformResetValue,
  }
  if (!profile) return normalized

  const normalizeProfileValue = (value: number): number => {
    const clamped = clamp(value, profile.min, profile.max)
    return profile.inputKind === 'integer' || profile.inputKind === 'select'
      ? Math.round(clamped)
      : roundNumber(clamped, profile.precision)
  }
  const valueSpan = profile.max - profile.min
  normalized.fixedValue = normalizeProfileValue(normalized.fixedValue)
  normalized.waveformBase = normalizeProfileValue(normalized.waveformBase)
  normalized.waveformTargetValue = normalizeProfileValue(normalized.waveformTargetValue)
  normalized.waveformAmplitude = Math.min(valueSpan, Math.abs(normalized.waveformAmplitude))
  normalized.waveformStepSize = Math.min(valueSpan, Math.abs(normalized.waveformStepSize))
  normalized.waveformMinValue = normalizeProfileValue(normalized.waveformMinValue)
  normalized.waveformMaxValue = normalizeProfileValue(normalized.waveformMaxValue)
  if (normalized.waveformMinValue > normalized.waveformMaxValue) {
    const minimum = normalized.waveformMaxValue
    normalized.waveformMaxValue = normalized.waveformMinValue
    normalized.waveformMinValue = minimum
  }
  normalized.waveformResetValue = normalizeProfileValue(
    clamp(normalized.waveformResetValue, normalized.waveformMinValue, normalized.waveformMaxValue),
  )
  if (!profile.allowedWaveforms.includes(normalized.waveformType)) {
    normalized.waveformType = profile.defaultWaveform
  }
  return normalized
}

export const suggestWaveformSimulationConfig = (
  points: readonly WaveformSimulationPointLike[],
  preferredType: unknown = null,
): WaveformSimulationConfig => {
  const profile = resolveSimulationValueProfile(points).profile ?? FALLBACK_VALUE_PROFILE
  const pointCategory = resolveWaveformPointCategory(points[0])
  const waveformType =
    preferredType == null ? profile.defaultWaveform : normalizeWaveformSimulationType(preferredType)
  const referencePoint = points.find((point) => Number.isFinite(Number(point?.value))) ?? points[0]
  const referenceValue = toFiniteNumber(referencePoint?.value, 0)
  const normalizedReferenceValue =
    referencePoint == null ? referenceValue : normalizeValueByPoint(referencePoint, referenceValue)

  let amplitude = Number(Math.max(Math.abs(referenceValue) * 0.2, 5).toFixed(2))
  let period = 10
  let stepSize = 1
  let stepCount = 8
  let minValue = referenceValue - Math.max(amplitude * 3, 5)
  let maxValue = referenceValue + Math.max(amplitude * 3, 5)
  let waveformBase = referenceValue
  let fixedValue = normalizedReferenceValue
  let resetValue = normalizedReferenceValue

  if (pointCategory === 'single-point') {
    amplitude = 1
    period = 4
    stepSize = 1
    stepCount = 2
    waveformBase = 0
    minValue = 0
    maxValue = 1
    fixedValue = normalizedReferenceValue >= 0.5 ? 1 : 0
    resetValue = fixedValue
  } else if (pointCategory === 'double-point') {
    amplitude = 1
    period = 4
    stepSize = 1
    stepCount = 2
    waveformBase = 1
    minValue = 1
    maxValue = 2
    fixedValue = normalizedReferenceValue === 2 ? 2 : 1
    resetValue = fixedValue
  } else if (pointCategory === 'counter') {
    amplitude = Math.max(1, Math.round(Math.abs(referenceValue) * 0.2) || 1)
    period = 1
    stepSize = 1
    stepCount = 8
    waveformBase = Math.round(referenceValue)
    minValue = Math.min(referenceValue, 0)
    maxValue = referenceValue + 100
    fixedValue = Math.round(referenceValue)
    resetValue = fixedValue
  } else if (pointCategory === 'step-position') {
    amplitude = Math.max(1, Math.round(Math.abs(referenceValue) * 0.2) || 1)
    period = 1
    stepSize = 1
    stepCount = 8
    waveformBase = clamp(Math.round(referenceValue), -64, 63)
    minValue = -64
    maxValue = 63
    fixedValue = waveformBase
    resetValue = waveformBase
  }

  const prefersAmplitude = !['fixed', 'exp-approach', 'random-walk', 'counter'].includes(
    waveformType,
  )
  const targetValue =
    pointCategory === 'single-point'
      ? 1
      : pointCategory === 'double-point'
        ? 2
        : waveformBase + (prefersAmplitude ? amplitude : Math.max(amplitude, 10))

  return normalizeWaveformSimulationConfig(
    {
      waveformType,
      fixedValue,
      waveformBase,
      waveformAmplitude: amplitude,
      waveformPeriod: period,
      waveformDutyCycle: 50,
      waveformPulseWidth: Math.max(1, Math.round(period / 4) || 1),
      waveformRiseTime: Math.max(1, Math.round(period / 3) || 1),
      waveformHoldTime: Math.max(1, Math.round(period / 3) || 1),
      waveformFallTime: Math.max(1, Math.round(period / 3) || 1),
      waveformStepSize: stepSize,
      waveformStepCount: stepCount,
      waveformTargetValue: targetValue,
      waveformDamping: 0.8,
      waveformMinValue: minValue,
      waveformMaxValue: maxValue,
      waveformResetValue: resetValue,
    },
    profile,
  )
}

export const suggestWaveformSimulationType = (
  points: readonly WaveformSimulationPointLike[],
): WaveformSimulationType => {
  return resolveSimulationValueProfile(points).profile?.defaultWaveform ?? 'fixed'
}

export const formatWaveformSimulationSummary = (
  raw: Partial<WaveformSimulationConfig> | Record<string, unknown>,
  tickMs = 1000,
  profile?: SimulationValueProfile | null,
): string => {
  const config = normalizeWaveformSimulationConfig(raw, profile)
  return getWaveformDefinition(config.waveformType).summaryBuilder(config, tickMs)
}

export const resolveWaveformTickMs = (
  raw: Partial<WaveformSimulationConfig> | Record<string, unknown>,
): number => {
  const config = normalizeWaveformSimulationConfig(raw)

  let secondsPerTick = config.waveformPeriod / 12
  switch (config.waveformType) {
    case 'fixed':
      return 1000
    case 'pulse':
      secondsPerTick = Math.min(config.waveformPulseWidth, config.waveformPeriod) / 4
      break
    case 'square':
      secondsPerTick = config.waveformPeriod / 8
      break
    case 'ramp-hold-fall':
      secondsPerTick =
        Math.min(config.waveformRiseTime, config.waveformHoldTime, config.waveformFallTime) / 4
      break
    case 'stair':
      secondsPerTick = config.waveformPeriod / 2
      break
    case 'random':
    case 'random-walk':
    case 'counter':
      secondsPerTick = config.waveformPeriod / 2
      break
    default:
      secondsPerTick = resolveCycleSeconds(config) / 16
      break
  }

  const targetMs = Math.round(Math.max(0.1, secondsPerTick) * 1000)
  return Math.min(1000, Math.max(100, targetMs))
}

export const computeWaveformSimulationValue = (
  raw: Partial<WaveformSimulationConfig> | Record<string, unknown>,
  elapsedMs: number,
  point: WaveformSimulationPointLike,
  options: {
    seed?: number
    runtimeState?: WaveformSimulationRuntimeState
  } = {},
): WaveformSimulationComputeResult => {
  const config = normalizeWaveformSimulationConfig(raw)
  const elapsedSeconds = Math.max(0, elapsedMs) / 1000
  const cycleSeconds = resolveCycleSeconds(config)
  const cyclePhaseSeconds = cycleSeconds <= 0 ? 0 : elapsedSeconds % cycleSeconds
  const t = normalizeWaveformCycle(elapsedSeconds, cycleSeconds)
  const seed = Number(options.seed ?? 1)
  const address = Math.max(0, Math.trunc(Number(point.address) || 0))
  const runtimeState: WaveformSimulationRuntimeState = {
    ...(options.runtimeState ?? {}),
  }

  let computedValue = config.fixedValue
  switch (config.waveformType) {
    case 'fixed':
      computedValue = config.fixedValue
      break
    case 'step':
      computedValue = t < 0.5 ? config.waveformBase : config.waveformBase + config.waveformAmplitude
      break
    case 'pulse':
      computedValue =
        cyclePhaseSeconds < config.waveformPulseWidth
          ? config.waveformBase + config.waveformAmplitude
          : config.waveformBase
      break
    case 'square':
      computedValue =
        t < config.waveformDutyCycle / 100
          ? config.waveformBase + config.waveformAmplitude
          : config.waveformBase
      break
    case 'saw-up':
      computedValue = config.waveformBase + config.waveformAmplitude * t
      break
    case 'saw-down':
      computedValue = config.waveformBase - config.waveformAmplitude * t
      break
    case 'triangle': {
      const triangle = t < 0.25 ? t * 4 : t < 0.75 ? 2 - t * 4 : t * 4 - 4
      computedValue = config.waveformBase + config.waveformAmplitude * triangle
      break
    }
    case 'sine':
      computedValue = config.waveformBase + config.waveformAmplitude * Math.sin(Math.PI * 2 * t)
      break
    case 'ramp-hold-fall': {
      if (cyclePhaseSeconds <= config.waveformRiseTime) {
        computedValue =
          config.waveformBase +
          config.waveformAmplitude * (cyclePhaseSeconds / config.waveformRiseTime)
      } else if (cyclePhaseSeconds <= config.waveformRiseTime + config.waveformHoldTime) {
        computedValue = config.waveformBase + config.waveformAmplitude
      } else {
        const fallPhase = cyclePhaseSeconds - config.waveformRiseTime - config.waveformHoldTime
        computedValue =
          config.waveformBase + config.waveformAmplitude * (1 - fallPhase / config.waveformFallTime)
      }
      break
    }
    case 'stair': {
      const bucket = Math.floor(elapsedSeconds / config.waveformPeriod) % config.waveformStepCount
      computedValue = config.waveformBase + config.waveformStepSize * bucket
      break
    }
    case 'exp-approach': {
      const factor = 1 - Math.exp(-5 * t)
      computedValue =
        config.waveformBase + (config.waveformTargetValue - config.waveformBase) * factor
      break
    }
    case 'damped-oscillation': {
      const envelope = Math.exp(-config.waveformDamping * t * 4)
      computedValue =
        config.waveformBase + config.waveformAmplitude * Math.sin(Math.PI * 2 * t) * envelope
      break
    }
    case 'random': {
      const bucket = Math.floor(elapsedSeconds / config.waveformPeriod)
      const noise = deterministicNoise(seed, address, bucket)
      computedValue = config.waveformBase + config.waveformAmplitude * noise
      break
    }
    case 'random-walk': {
      const bucket = Math.floor(elapsedSeconds / config.waveformPeriod)
      let currentValue =
        runtimeState.currentValue != null ? runtimeState.currentValue : config.waveformBase
      let lastBucket = runtimeState.lastBucket != null ? runtimeState.lastBucket : bucket - 1

      if (bucket > lastBucket) {
        for (let index = lastBucket + 1; index <= bucket; index += 1) {
          const stepNoise = deterministicNoise(seed, address, index)
          currentValue = clamp(
            currentValue + config.waveformStepSize * stepNoise,
            config.waveformMinValue,
            config.waveformMaxValue,
          )
        }
        lastBucket = bucket
      }

      runtimeState.currentValue = currentValue
      runtimeState.lastBucket = lastBucket
      computedValue = currentValue
      break
    }
    case 'counter': {
      const bucket = Math.floor(elapsedSeconds / config.waveformPeriod)
      let currentValue =
        runtimeState.currentValue != null ? runtimeState.currentValue : config.waveformBase
      let lastBucket = runtimeState.lastBucket != null ? runtimeState.lastBucket : bucket - 1

      if (bucket > lastBucket) {
        for (let index = lastBucket + 1; index <= bucket; index += 1) {
          currentValue += config.waveformStepSize
          if (currentValue > config.waveformMaxValue) {
            currentValue = config.waveformResetValue
          }
        }
        lastBucket = bucket
      }

      runtimeState.currentValue = currentValue
      runtimeState.lastBucket = lastBucket
      computedValue = currentValue
      break
    }
    default:
      computedValue = config.waveformBase
      break
  }

  return {
    value: normalizeValueByPoint(point, computedValue),
    runtimeState,
  }
}

export const describeWaveformPointCategory = (
  points: readonly WaveformSimulationPointLike[],
): string => {
  const category = resolveWaveformPointCategory(points[0])
  switch (category) {
    case 'single-point':
      return t('slave.waveform.categories.singlePoint')
    case 'double-point':
      return t('slave.waveform.categories.doublePoint')
    case 'counter':
      return t('slave.waveform.categories.counter')
    case 'step-position':
      return t('slave.waveform.categories.stepPosition')
    case 'measurement':
    default:
      return t('slave.waveform.categories.measurement')
  }
}

export const getWaveformPreviewConfig = (raw: unknown): WaveformSimulationConfig => {
  switch (normalizeWaveformSimulationType(raw)) {
    case 'fixed':
      return previewConfig({ waveformType: 'fixed', fixedValue: 0 })
    case 'step':
      return previewConfig({
        waveformType: 'step',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
      })
    case 'pulse':
      return previewConfig({
        waveformType: 'pulse',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
        waveformPulseWidth: 0.8,
      })
    case 'square':
      return previewConfig({
        waveformType: 'square',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
        waveformDutyCycle: 60,
      })
    case 'saw-up':
      return previewConfig({
        waveformType: 'saw-up',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
      })
    case 'saw-down':
      return previewConfig({
        waveformType: 'saw-down',
        waveformBase: 1,
        waveformAmplitude: 1,
        waveformPeriod: 4,
      })
    case 'triangle':
      return previewConfig({
        waveformType: 'triangle',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
      })
    case 'sine':
      return previewConfig({
        waveformType: 'sine',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
      })
    case 'ramp-hold-fall':
      return previewConfig({
        waveformType: 'ramp-hold-fall',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformRiseTime: 1.5,
        waveformHoldTime: 1,
        waveformFallTime: 1.5,
      })
    case 'stair':
      return previewConfig({
        waveformType: 'stair',
        waveformBase: 0,
        waveformStepSize: 0.2,
        waveformStepCount: 5,
        waveformPeriod: 0.8,
      })
    case 'exp-approach':
      return previewConfig({
        waveformType: 'exp-approach',
        waveformBase: 0,
        waveformTargetValue: 1,
        waveformPeriod: 4,
      })
    case 'damped-oscillation':
      return previewConfig({
        waveformType: 'damped-oscillation',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 4,
        waveformDamping: 0.8,
      })
    case 'random':
      return previewConfig({
        waveformType: 'random',
        waveformBase: 0,
        waveformAmplitude: 1,
        waveformPeriod: 1,
      })
    case 'random-walk':
      return previewConfig({
        waveformType: 'random-walk',
        waveformBase: 0,
        waveformStepSize: 0.3,
        waveformMinValue: -1,
        waveformMaxValue: 1,
        waveformPeriod: 0.8,
      })
    case 'counter':
      return previewConfig({
        waveformType: 'counter',
        waveformBase: 0,
        waveformStepSize: 1,
        waveformMaxValue: 5,
        waveformResetValue: 0,
        waveformPeriod: 0.8,
      })
    default:
      return previewConfig({})
  }
}

export const buildWaveformPreviewPath = (raw: unknown): string => {
  const config = getWaveformPreviewConfig(raw)
  return getWaveformPreviewPath(config.waveformType, config)
}
