<template>
  <div class="app-menu-bar">
    <slot />
    <span class="app-menu-bar__copyright">{{ copyrightText }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getCopyrightText } from '@shared/i18n/copyright'

// AppMenuBar - 公共菜单栏布局容器
// 通过 slot 接收具体菜单内容

const copyrightText = computed(() => getCopyrightText())
</script>

<style scoped>
.app-menu-bar {
  display: flex;
  align-items: center;
  height: 28px;
  padding: 0 8px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border-color);
  gap: 4px;
}

/* 菜单项样式 */
:deep(.menu-item) {
  display: flex;
  align-items: center;
  padding: 4px 10px;
  font-size: 13px;
  font-weight: 500;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

:deep(.menu-item:hover) {
  background: #f3f4f6;
  color: var(--text-primary);
}

:deep(.menu-item.active) {
  background: var(--primary-light);
  color: var(--primary-color);
}

:deep(.el-tooltip__trigger:focus-visible),
:deep(.menu-item:focus-visible) {
  outline: 1px solid #93c5fd;
  outline-offset: 1px;
  border-radius: 4px;
}

:deep(.el-tooltip__trigger:focus),
:deep(.menu-item:focus) {
  outline: none;
}

/* 下拉箭头 */
:deep(.menu-item .arrow) {
  margin-left: 4px;
  font-size: 10px;
  opacity: 0.6;
}

/* 版权信息 */
.app-menu-bar__copyright {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-disabled);
  white-space: nowrap;
  user-select: none;
  padding-right: 4px;
}
</style>
