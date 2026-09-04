const common = {
  app: {
    name: 'IEC104 Simulator',
    masterTitle: 'IEC104 Master Simulator',
    slaveTitle: 'IEC104 Slave Simulator',
  },
  language: {
    menuLabel: 'Language',
    zhCN: 'Simplified Chinese',
    enUS: 'English',
  },
  common: {
    cancel: 'Cancel',
    close: 'Close',
    delete: 'Delete',
    exit: 'Exit',
    ok: 'OK',
    save: 'Save',
  },
  dialog: {
    unsavedTitle: 'Unsaved Changes',
    unsavedMessage: 'Your changes have not been saved. Closing will discard them. Continue?',
    closeAnyway: 'Close Anyway',
    exitTitle: 'Exit',
    exitMessage: 'Exit the application?',
  },
  help: {
    contents: 'On this page',
  },
  about: {
    masterTitle: 'IEC104 Master Simulator',
    slaveTitle: 'IEC104 Slave Simulator',
    version: 'Version',
    loading: 'Loading application information...',
    copyright: 'Copyright',
    license: 'License',
    developer: 'Developer',
    resources: 'Project and support',
    repository: 'Source repository',
    issues: 'Issue tracker',
    documentation: 'Online documentation',
    support: 'Support',
    copy: 'Copy',
    copied: 'Copied',
    descriptionText: 'For IEC104 master/slave integration and message debugging',
    scopeNotice:
      'This software is intended for development, integration, and testing. It is not a production dispatch or safety control system. See the project documentation for the supported protocol scope and limitations.',
    supportNotice:
      'When reporting an issue, include the app version, operating system, reproduction steps, and sanitized logs or frames.',
  },
  shell: {
    copied: 'Copied: {text}',
    copyFailed: 'Copy failed',
    stopStationsBeforeExitFailed: 'Failed to stop stations before exit: {error}',
    exitFailed: 'Exit failed: {error}',
    clearLogs: 'Clear logs',
    resetLayout: 'Reset layout',
    unknownMenuAction: 'Unknown menu action: {action}',
  },
  settingsMessages: {
    globalSettingsSaved: 'Global settings saved',
    loadMasterGlobalSettingsFailed: 'Failed to read master global settings: {error}',
    saveMasterGlobalSettingsFailed: 'Failed to save master global settings: {error}',
    loadSlaveGlobalSettingsFailed: 'Failed to read slave global settings: {error}',
    saveGlobalSettingsFailed: 'Failed to save global settings: {error}',
    readAppInfoFailed: 'Failed to read application info: {error}',
  },
}

export default common
