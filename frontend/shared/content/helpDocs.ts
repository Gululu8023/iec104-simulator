import type { AppLocale } from '@shared/api/types'
import { currentLocale } from '@shared/i18n'

import zhUserGuideDoc from '../../../docs/zh-CN/user-guide.md?raw'
import zhPointTableFormatDoc from '../../../docs/zh-CN/point-table-format.md?raw'
import zhProtocolSupportDoc from '../../../docs/zh-CN/protocol-support.md?raw'
import enUserGuideDoc from '../../../docs/en-US/user-guide.md?raw'
import enPointTableFormatDoc from '../../../docs/en-US/point-table-format.md?raw'
import enProtocolSupportDoc from '../../../docs/en-US/protocol-support.md?raw'

export type HelpDocKey = 'user-guide' | 'point-table-format' | 'protocol-support'

export interface HelpDocDefinition {
  key: HelpDocKey
  title: string
  menuAction: string
  content: string
}

export interface HelpDocMenuItem {
  key: HelpDocKey
  title: string
  menuAction: string
}

interface LocalizedHelpDocContent {
  title: string
  menuTitle: string
  content: string
}

interface HelpDocSource {
  key: HelpDocKey
  menuAction: string
  locales: Record<AppLocale, LocalizedHelpDocContent>
}

const HELP_DOC_SOURCES: HelpDocSource[] = [
  {
    key: 'user-guide',
    menuAction: 'doc-user-guide',
    locales: {
      'zh-CN': {
        title: 'IEC104 Simulator 用户指南',
        menuTitle: '用户指南',
        content: zhUserGuideDoc,
      },
      'en-US': {
        title: 'IEC104 Simulator User Guide',
        menuTitle: 'User Guide',
        content: enUserGuideDoc,
      },
    },
  },
  {
    key: 'point-table-format',
    menuAction: 'doc-point-table-format',
    locales: {
      'zh-CN': {
        title: '点表格式说明',
        menuTitle: '点表格式',
        content: zhPointTableFormatDoc,
      },
      'en-US': {
        title: 'Point Table Format',
        menuTitle: 'Point Table Format',
        content: enPointTableFormatDoc,
      },
    },
  },
  {
    key: 'protocol-support',
    menuAction: 'doc-protocol-support',
    locales: {
      'zh-CN': {
        title: 'IEC104 协议支持范围与已知限制',
        menuTitle: '协议支持与限制',
        content: zhProtocolSupportDoc,
      },
      'en-US': {
        title: 'IEC104 Protocol Support and Known Limitations',
        menuTitle: 'Protocol Support and Limitations',
        content: enProtocolSupportDoc,
      },
    },
  },
]

const DEFAULT_HELP_DOC_KEY: HelpDocKey = 'user-guide'
const HELP_DOC_KEYS: HelpDocKey[] = ['user-guide', 'point-table-format', 'protocol-support']

const getHelpDocSource = (key: HelpDocKey): HelpDocSource =>
  HELP_DOC_SOURCES.find((doc) => doc.key === key) ??
  HELP_DOC_SOURCES.find((doc) => doc.key === DEFAULT_HELP_DOC_KEY) ??
  HELP_DOC_SOURCES[0]

const getLocalizedContent = (source: HelpDocSource, locale: AppLocale): LocalizedHelpDocContent =>
  source.locales[locale] ?? source.locales['zh-CN']

export const getHelpDocDefinition = (
  key: HelpDocKey,
  locale: AppLocale = currentLocale.value,
): HelpDocDefinition => {
  const source = getHelpDocSource(key)
  const localized = getLocalizedContent(source, locale)

  return {
    key: source.key,
    title: localized.title,
    menuAction: source.menuAction,
    content: localized.content,
  }
}

export const getHelpDocDefinitions = (
  locale: AppLocale = currentLocale.value,
): HelpDocDefinition[] => HELP_DOC_SOURCES.map((doc) => getHelpDocDefinition(doc.key, locale))

export const getHelpDocMenuItems = (locale: AppLocale = currentLocale.value): HelpDocMenuItem[] =>
  HELP_DOC_KEYS.map((key) => {
    const source = getHelpDocSource(key)
    const localized = getLocalizedContent(source, locale)
    return {
      key: source.key,
      title: localized.menuTitle,
      menuAction: source.menuAction,
    }
  })

export const getMasterHelpDocMenuItems = getHelpDocMenuItems
export const getSlaveHelpDocMenuItems = getHelpDocMenuItems

export const HELP_DOCS = getHelpDocDefinitions('zh-CN')
export const HELP_DOC_MENU_ITEMS = getHelpDocMenuItems('zh-CN')
export const MASTER_HELP_DOC_MENU_ITEMS = getMasterHelpDocMenuItems('zh-CN')
export const SLAVE_HELP_DOC_MENU_ITEMS = getSlaveHelpDocMenuItems('zh-CN')

export const resolveHelpDocKeyFromAction = (action: string): HelpDocKey | null =>
  HELP_DOC_SOURCES.find((doc) => doc.menuAction === action)?.key ?? null

export const resolveHelpDocKeyFromHref = (href: string): HelpDocKey | null => {
  const filename = href.split('#', 1)[0]?.split('/').pop()?.replace(/\.md$/i, '')
  return HELP_DOC_SOURCES.some((doc) => doc.key === filename) ? (filename as HelpDocKey) : null
}
