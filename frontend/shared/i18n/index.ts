import { ref } from 'vue'
import { createI18n } from 'vue-i18n'
import { dayjs } from 'element-plus'
import zhCnElementLocale from 'element-plus/es/locale/lang/zh-cn'

import { getUiLanguage, updateUiLanguage } from '../api/common'
import type { AppLocale } from '../api/types'
import enUS from './locales/en-US'
import zhCN from './locales/zh-CN'

export const DEFAULT_LOCALE: AppLocale = 'zh-CN'
export const SUPPORTED_LOCALES = ['zh-CN', 'en-US'] as const
export const LOCALE_STORAGE_KEY = 'iec104.ui.language'

export const messages = {
  'zh-CN': zhCN,
  'en-US': enUS,
}

export const currentLocale = ref<AppLocale>(DEFAULT_LOCALE)

const zhCnDatepicker = zhCnElementLocale.el.datepicker
const zhCnDayjsLocale = {
  name: zhCnElementLocale.name,
  months: [
    zhCnDatepicker.months.jan,
    zhCnDatepicker.months.feb,
    zhCnDatepicker.months.mar,
    zhCnDatepicker.months.apr,
    zhCnDatepicker.months.may,
    zhCnDatepicker.months.jun,
    zhCnDatepicker.months.jul,
    zhCnDatepicker.months.aug,
    zhCnDatepicker.months.sep,
    zhCnDatepicker.months.oct,
    zhCnDatepicker.months.nov,
    zhCnDatepicker.months.dec,
  ],
  monthsShort: [
    zhCnDatepicker.month1,
    zhCnDatepicker.month2,
    zhCnDatepicker.month3,
    zhCnDatepicker.month4,
    zhCnDatepicker.month5,
    zhCnDatepicker.month6,
    zhCnDatepicker.month7,
    zhCnDatepicker.month8,
    zhCnDatepicker.month9,
    zhCnDatepicker.month10,
    zhCnDatepicker.month11,
    zhCnDatepicker.month12,
  ],
  weekdays: [
    zhCnDatepicker.weeksFull.sun,
    zhCnDatepicker.weeksFull.mon,
    zhCnDatepicker.weeksFull.tue,
    zhCnDatepicker.weeksFull.wed,
    zhCnDatepicker.weeksFull.thu,
    zhCnDatepicker.weeksFull.fri,
    zhCnDatepicker.weeksFull.sat,
  ],
  weekdaysShort: [
    zhCnDatepicker.weeks.sun,
    zhCnDatepicker.weeks.mon,
    zhCnDatepicker.weeks.tue,
    zhCnDatepicker.weeks.wed,
    zhCnDatepicker.weeks.thu,
    zhCnDatepicker.weeks.fri,
    zhCnDatepicker.weeks.sat,
  ],
  weekdaysMin: [
    zhCnDatepicker.weeks.sun,
    zhCnDatepicker.weeks.mon,
    zhCnDatepicker.weeks.tue,
    zhCnDatepicker.weeks.wed,
    zhCnDatepicker.weeks.thu,
    zhCnDatepicker.weeks.fri,
    zhCnDatepicker.weeks.sat,
  ],
  weekStart: 0,
}

let zhCnDayjsLocaleRegistered = false

export const i18n = createI18n({
  legacy: false,
  locale: DEFAULT_LOCALE,
  fallbackLocale: DEFAULT_LOCALE,
  messages,
})

export function isAppLocale(value: unknown): value is AppLocale {
  return typeof value === 'string' && SUPPORTED_LOCALES.includes(value as AppLocale)
}

function readFallbackLocale(): AppLocale | null {
  if (typeof window === 'undefined') return null
  const value = window.localStorage.getItem(LOCALE_STORAGE_KEY)
  return isAppLocale(value) ? value : null
}

function writeFallbackLocale(locale: AppLocale): void {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(LOCALE_STORAGE_KEY, locale)
}

function applyDocumentLocale(locale: AppLocale): void {
  if (typeof document === 'undefined') return
  document.documentElement.lang = locale
}

function applyDayjsLocale(locale: AppLocale): void {
  if (locale === 'en-US') {
    dayjs.locale('en')
    return
  }

  if (!zhCnDayjsLocaleRegistered) {
    dayjs.locale(zhCnDayjsLocale as Parameters<typeof dayjs.locale>[0], undefined, true)
    zhCnDayjsLocaleRegistered = true
  }
  dayjs.locale(zhCnElementLocale.name)
}

export function applyAppLocale(locale: AppLocale): void {
  currentLocale.value = locale
  i18n.global.locale.value = locale
  applyDocumentLocale(locale)
  applyDayjsLocale(locale)
  writeFallbackLocale(locale)
}

export function getCurrentLocale(): AppLocale {
  return currentLocale.value
}

export async function loadInitialLocale(): Promise<AppLocale> {
  const fallback = readFallbackLocale() ?? DEFAULT_LOCALE

  try {
    const locale = await getUiLanguage()
    const resolved = isAppLocale(locale) ? locale : fallback
    applyAppLocale(resolved)
    return resolved
  } catch (error) {
    console.warn('[i18n] failed to load persisted locale, using fallback', error)
    applyAppLocale(fallback)
    return fallback
  }
}

export async function setAppLocale(locale: AppLocale): Promise<void> {
  if (!isAppLocale(locale)) return
  applyAppLocale(locale)

  try {
    await updateUiLanguage(locale)
  } catch (error) {
    console.warn('[i18n] failed to persist locale to backend, keeping local fallback', error)
  }
}

export function t(key: string, params?: Record<string, unknown>): string {
  return i18n.global.t(key, params ?? {})
}
