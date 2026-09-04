const common = {
  app: {
    name: 'IEC104 协议模拟器',
    masterTitle: 'IEC104 主站模拟器',
    slaveTitle: 'IEC104 从站模拟器',
  },
  language: {
    menuLabel: '语言',
    zhCN: '简体中文',
    enUS: '英语',
  },
  common: {
    cancel: '取消',
    close: '关闭',
    delete: '删除',
    exit: '退出',
    ok: '确定',
    save: '保存',
  },
  dialog: {
    unsavedTitle: '未保存更改',
    unsavedMessage: '当前修改尚未保存，关闭将丢失变更，是否继续？',
    closeAnyway: '仍要关闭',
    exitTitle: '退出',
    exitMessage: '确定退出应用？',
  },
  help: {
    contents: '本页目录',
  },
  about: {
    masterTitle: 'IEC104协议主站模拟器',
    slaveTitle: 'IEC104协议从站模拟器',
    version: '版本',
    loading: '正在读取应用信息...',
    copyright: '版权',
    license: '许可证',
    developer: '开发者',
    resources: '项目与支持',
    repository: '源码仓库',
    issues: '问题反馈',
    documentation: '在线文档',
    support: '支持',
    copy: '复制',
    copied: '已复制',
    descriptionText: '用于 IEC104 主从站联调与报文调试',
    scopeNotice:
      '本软件用于开发、联调与测试，不是生产级调度或安全控制系统。协议支持范围与限制以项目文档为准。',
    supportNotice: '报告问题时请提供应用版本、操作系统、复现步骤，以及已脱敏的日志或报文。',
  },
  shell: {
    copied: '已复制: {text}',
    copyFailed: '复制失败',
    stopStationsBeforeExitFailed: '退出前停止从站失败: {error}',
    exitFailed: '退出失败: {error}',
    clearLogs: '清空日志',
    resetLayout: '重置布局',
    unknownMenuAction: '未知菜单操作: {action}',
  },
  settingsMessages: {
    globalSettingsSaved: '全局设置已保存',
    loadMasterGlobalSettingsFailed: '读取主站全局设置失败: {error}',
    saveMasterGlobalSettingsFailed: '保存主站全局设置失败: {error}',
    loadSlaveGlobalSettingsFailed: '读取从站全局设置失败: {error}',
    saveGlobalSettingsFailed: '保存全局设置失败: {error}',
    readAppInfoFailed: '读取应用信息失败: {error}',
  },
}

export default common
