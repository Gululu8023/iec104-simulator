import { currentLocale, t } from './index'

const COPYRIGHT_START_YEAR = 2025
const COPYRIGHT_AUTHOR = 'Gululu8023'

export function getCopyrightText(currentYear = new Date().getFullYear()): string {
  void currentLocale.value

  if (currentYear === COPYRIGHT_START_YEAR) {
    return t('appMenu.copyrightSingleYear', {
      year: currentYear,
      author: COPYRIGHT_AUTHOR,
    })
  }

  return t('appMenu.copyrightYearRange', {
    startYear: COPYRIGHT_START_YEAR,
    endYear: currentYear,
    author: COPYRIGHT_AUTHOR,
  })
}
