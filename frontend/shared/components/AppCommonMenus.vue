<template>
  <el-dropdown trigger="hover" @command="handleViewCommand" popper-class="app-menu-dropdown">
    <span class="menu-item">{{ t('appMenu.view') }} <span class="arrow">▾</span></span>
    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item command="toggle-message-panel">
          <el-icon><ChatLineRound /></el-icon> {{ t('appMenu.messagePanel') }}
        </el-dropdown-item>
        <el-dropdown-item command="toggle-soe-panel">
          <el-icon><ChatLineRound /></el-icon> {{ t('appMenu.soeEvents') }}
        </el-dropdown-item>
        <el-dropdown-item command="refresh-devices">
          <el-icon><Refresh /></el-icon> {{ t('appMenu.refreshDeviceTree') }}
        </el-dropdown-item>
        <el-dropdown-item divided command="reset-layout">
          <el-icon><RefreshLeft /></el-icon> {{ t('appMenu.resetLayout') }}
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>

  <el-dropdown trigger="hover" @command="handleSettingsCommand" popper-class="app-menu-dropdown">
    <span class="menu-item">{{ t('appMenu.settings') }} <span class="arrow">▾</span></span>
    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item command="global-settings">
          <el-icon><Setting /></el-icon> {{ t('appMenu.globalSettings') }}
        </el-dropdown-item>
        <el-dropdown-item divided disabled>
          {{ t('language.menuLabel') }}: {{ currentLocaleLabel }}
        </el-dropdown-item>
        <el-dropdown-item command="locale:zh-CN" :disabled="currentLocale === 'zh-CN'">
          <el-icon v-if="currentLocale === 'zh-CN'"><Check /></el-icon> {{ t('language.zhCN') }}
        </el-dropdown-item>
        <el-dropdown-item command="locale:en-US" :disabled="currentLocale === 'en-US'">
          <el-icon v-if="currentLocale === 'en-US'"><Check /></el-icon> {{ t('language.enUS') }}
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>

  <el-dropdown trigger="hover" @command="forwardMenuCommand" popper-class="app-menu-dropdown">
    <span class="menu-item">{{ t('appMenu.tools') }} <span class="arrow">▾</span></span>
    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item command="message-parser">
          <el-icon><Document /></el-icon> {{ t('appMenu.messageParser') }}
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>

  <el-dropdown trigger="hover" @command="forwardMenuCommand" popper-class="app-menu-dropdown">
    <span class="menu-item">{{ t('appMenu.help') }} <span class="arrow">▾</span></span>
    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item
          v-for="doc in helpDocMenuItems"
          :key="doc.menuAction"
          :command="doc.menuAction"
        >
          <el-icon><Document /></el-icon> {{ doc.title }}
        </el-dropdown-item>
        <el-dropdown-item command="download-point-table-template">
          <el-icon><Download /></el-icon> {{ t('appMenu.downloadPointTableTemplate') }}
        </el-dropdown-item>
        <el-dropdown-item command="about" divided>
          <el-icon><InfoFilled /></el-icon> {{ t('appMenu.about') }}
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  ChatLineRound,
  Check,
  Document,
  Download,
  InfoFilled,
  Refresh,
  RefreshLeft,
  Setting,
} from '@element-plus/icons-vue'
import type { AppLocale } from '@shared/api/types'
import type { HelpDocMenuItem } from '@shared/content/helpDocs'
import { currentLocale, setAppLocale, t } from '@shared/i18n'

defineProps<{
  helpDocMenuItems: HelpDocMenuItem[]
}>()

const emit = defineEmits<{
  viewCommand: [command: string]
  menuCommand: [command: string]
}>()

const currentLocaleLabel = computed(() =>
  currentLocale.value === 'en-US' ? t('language.enUS') : t('language.zhCN'),
)

const handleViewCommand = (command: string) => {
  emit('viewCommand', command)
}

const handleSettingsCommand = (command: string) => {
  if (command.startsWith('locale:')) {
    void setAppLocale(command.slice('locale:'.length) as AppLocale)
    return
  }
  emit('menuCommand', command)
}

const forwardMenuCommand = (command: string) => {
  emit('menuCommand', command)
}
</script>
