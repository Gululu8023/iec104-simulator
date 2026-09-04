import { computed } from 'vue'
import en from 'element-plus/es/locale/lang/en'
import zhCn from 'element-plus/es/locale/lang/zh-cn'

import { currentLocale } from './index'

export const elementPlusLocale = computed(() => (currentLocale.value === 'en-US' ? en : zhCn))
