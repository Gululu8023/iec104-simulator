import { ElMessageBox, type ElMessageBoxOptions } from 'element-plus'
import { t } from '@shared/i18n'

type AppConfirmDialogOptions = {
  title: string
  message: string
  type?: ElMessageBoxOptions['type']
  confirmButtonText?: string
  cancelButtonText?: string
  customClass?: string
}

const APP_CONFIRM_BOX_CLASS = 'app-confirm-box'

const buildConfirmClassName = (customClass?: string): string => {
  const normalized = String(customClass || '').trim()
  return normalized ? `${APP_CONFIRM_BOX_CLASS} ${normalized}` : APP_CONFIRM_BOX_CLASS
}

export const showAppConfirmDialog = async ({
  title,
  message,
  type = 'warning',
  confirmButtonText = t('common.ok'),
  cancelButtonText = t('common.cancel'),
  customClass,
}: AppConfirmDialogOptions): Promise<boolean> => {
  try {
    await ElMessageBox.confirm(message, title, {
      type,
      confirmButtonText,
      cancelButtonText,
      autofocus: false,
      distinguishCancelAndClose: true,
      closeOnClickModal: false,
      closeOnPressEscape: true,
      lockScroll: false,
      showClose: true,
      customClass: buildConfirmClassName(customClass),
    })
    return true
  } catch {
    return false
  }
}

export const confirmUnsavedDialogClose = (message?: string, title?: string): Promise<boolean> =>
  showAppConfirmDialog({
    title: title ?? t('dialog.unsavedTitle'),
    message: message ?? t('dialog.unsavedMessage'),
    type: 'warning',
    confirmButtonText: t('dialog.closeAnyway'),
    cancelButtonText: t('common.cancel'),
    customClass: 'app-confirm-box--unsaved',
  })

type AppAlertDialogOptions = {
  title: string
  message: string
  confirmButtonText?: string
  customClass?: string
  showConfirmButton?: boolean
  closeOnClickModal?: boolean
}

export const showAppAlertDialog = async ({
  title,
  message,
  confirmButtonText = t('common.ok'),
  customClass,
  showConfirmButton = true,
  closeOnClickModal = false,
}: AppAlertDialogOptions): Promise<void> => {
  try {
    await ElMessageBox.alert(message, title, {
      confirmButtonText,
      autofocus: false,
      closeOnClickModal,
      closeOnPressEscape: true,
      lockScroll: false,
      showClose: true,
      customClass: buildConfirmClassName(customClass),
      dangerouslyUseHTMLString: true,
      showConfirmButton,
    })
  } catch (error) {
    const normalized = String(error ?? '')
      .trim()
      .toLowerCase()
    if (normalized === 'cancel' || normalized === 'close') {
      return
    }
    throw error
  }
}

export const confirmDangerousAction = (
  message: string,
  title: string,
  confirmButtonText = t('common.delete'),
): Promise<boolean> =>
  showAppConfirmDialog({
    title,
    message,
    type: 'warning',
    confirmButtonText,
    cancelButtonText: t('common.cancel'),
    customClass: 'app-confirm-box--danger',
  })

export const confirmExitApp = (): Promise<boolean> =>
  showAppConfirmDialog({
    title: t('dialog.exitTitle'),
    message: t('dialog.exitMessage'),
    type: 'warning',
    confirmButtonText: t('common.exit'),
    cancelButtonText: t('common.cancel'),
    customClass: 'app-confirm-box--exit',
  })
