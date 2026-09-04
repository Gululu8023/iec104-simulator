<template>
  <div class="app-status-bar">
    <!-- 左侧：状态区 -->
    <div class="status-left">
      <slot name="status" />
    </div>

    <!-- 右侧：命令区 -->
    <div class="status-right">
      <slot name="commands" />
    </div>
  </div>
</template>

<script setup lang="ts">
// AppStatusBar - 公共状态栏布局容器
// 左侧通过 status slot 接收状态信息
// 右侧通过 commands slot 接收命令按钮
</script>

<style scoped>
.app-status-bar {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 24px;
  min-height: 64px;
  padding: 6px 16px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
  min-width: 0;
}

.status-left {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 16px 24px;
  flex: 0 0 auto;
  min-width: 220px;
  max-width: 42%;
  overflow: hidden;
}

.status-right {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: clamp(4px, 0.7vw, 12px);
  flex: 1 1 0;
  min-width: 0;
}

/* 状态指示器样式 - 增大 */
:deep(.status-indicator) {
  display: flex;
  align-items: center;
  flex: 0 0 auto;
  gap: 12px;
}

:deep(.status-dot) {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--text-disabled);
  transition: all 0.3s;
}

:deep(.status-dot.connected) {
  background: var(--success-color);
  box-shadow: 0 0 8px var(--success-color);
}

:deep(.status-dot.connecting) {
  background: var(--warning-color);
  animation: pulse 1.5s infinite;
}

:deep(.status-link-icon) {
  font-size: 24px;
  min-width: 20px;
  min-height: 20px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-disabled);
  transition:
    color 0.25s ease,
    filter 0.25s ease,
    transform 0.25s ease;
}

:deep(.status-link-icon.connected) {
  color: var(--success-color);
  filter: drop-shadow(0 0 6px rgba(16, 185, 129, 0.6));
  animation: link-breathe 1.8s ease-in-out infinite;
}

:deep(.status-link-icon.listening),
:deep(.status-link-icon.status-listening) {
  color: var(--primary-color);
  filter: drop-shadow(0 0 6px rgba(59, 130, 246, 0.6));
  animation: link-breathe 1.8s ease-in-out infinite;
}

:deep(.status-link-icon.status-idle) {
  color: #9ca3af;
  filter: none;
}

:deep(.status-link-icon.status-processing) {
  color: var(--warning-color);
  filter: drop-shadow(0 0 6px rgba(245, 158, 11, 0.6));
  animation: link-breathe 1.2s ease-in-out infinite;
}

:deep(.status-link-icon.status-unready) {
  color: var(--warning-color);
  filter: drop-shadow(0 0 6px rgba(245, 158, 11, 0.35));
}

:deep(.status-link-icon.status-normal) {
  color: var(--success-color);
  filter: drop-shadow(0 0 6px rgba(16, 185, 129, 0.6));
  animation: link-breathe 1.8s ease-in-out infinite;
}

:deep(.status-link-icon.status-error) {
  color: var(--danger-color);
  filter: drop-shadow(0 0 6px rgba(239, 68, 68, 0.6));
}

:deep(.status-link-icon.connecting) {
  color: var(--warning-color);
  filter: drop-shadow(0 0 6px rgba(245, 158, 11, 0.6));
  animation: link-breathe 1.2s ease-in-out infinite;
}

:deep(.status-link-icon.error) {
  color: var(--danger-color);
  filter: drop-shadow(0 0 6px rgba(239, 68, 68, 0.6));
}

:deep(.status-link-icon.disconnected),
:deep(.status-link-icon.offline) {
  color: #9ca3af;
  filter: none;
}

:deep(.status-info) {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

:deep(.status-label) {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

:deep(.status-value) {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 命令按钮样式 - 彩色大图标 */
:deep(.command-btn) {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  flex: 1 1 72px;
  min-width: 54px;
  max-width: 98px;
  padding: 2px clamp(4px, 0.6vw, 10px);
  background: transparent;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
}

:deep(.command-btn:hover) {
  background: transparent;
}

:deep(.command-btn.disabled) {
  opacity: 0.5;
  cursor: not-allowed;
}

:deep(.command-btn .cmd-icon) {
  font-size: 28px;
  line-height: 1;
  color: #9ca3af; /* 默认灰色 */
  transition: color 0.2s;
}

:deep(.command-btn.active .cmd-icon) {
  /* 激活时显示各自颜色 - 由具体组件覆盖 */
}

:deep(.command-btn .cmd-text) {
  display: grid;
  place-items: center;
  font-size: 12px;
  line-height: 12px;
  color: #6b7280;
  font-weight: 500;
  width: 100%;
  min-height: 24px;
  text-align: center;
  white-space: pre-line;
  overflow-wrap: normal;
}

:deep(.command-btn.active) {
  background: transparent;
}

:deep(.command-divider) {
  width: 1px;
  height: 34px;
  margin: 0 2px;
  flex: 0 0 auto;
  background: #e2e8f0;
}

/* 各命令的激活颜色 */
:deep(.command-btn.gi-cmd.active .cmd-icon) {
  color: #10b981;
}
:deep(.command-btn.ci-cmd.active .cmd-icon) {
  color: #3b82f6;
}
:deep(.command-btn.cs-cmd.active .cmd-icon) {
  color: #f59e0b;
}
:deep(.command-btn.test-cmd.active .cmd-icon) {
  color: #8b5cf6;
}
:deep(.command-btn.rc-cmd.active .cmd-icon) {
  color: #2563eb;
}
:deep(.command-btn.file-cmd.active .cmd-icon) {
  color: #f59e0b;
}
:deep(.command-btn.mapping-cmd.active .cmd-icon) {
  color: #3b82f6;
}
:deep(.command-btn.simulation-cmd.active .cmd-icon) {
  color: #2563eb;
}

/* 统计信息样式 - 分隔区块 */
:deep(.stat-item) {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  flex: 0 0 auto;
  gap: 2px;
  padding: 0 16px;
  border-left: 1px solid var(--border-color);
  min-width: 0;
}

:deep(.stat-item:first-child) {
  border-left: none;
  padding-left: 0;
}

:deep(.stat-label) {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

:deep(.stat-value) {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: var(--font-mono);
  max-width: 160px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

@media (max-width: 1280px) {
  .app-status-bar {
    gap: 16px;
    padding: 8px 12px;
  }

  .status-left {
    gap: 12px 16px;
    min-width: 200px;
    max-width: 38%;
  }

  .status-right {
    gap: 4px;
  }

  :deep(.command-btn) {
    min-width: 50px;
    max-width: 86px;
    padding: 4px 3px;
  }

  :deep(.command-btn .cmd-icon) {
    font-size: 25px;
  }

  :deep(.command-btn .cmd-text) {
    font-size: 11px;
  }
}

:deep(.highlight-number) {
  font-size: 18px;
  font-weight: 700;
  color: var(--primary-color);
}

:deep(.value-suffix) {
  font-size: 12px;
  color: var(--text-secondary);
  font-weight: 400;
  margin-left: 4px;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

@keyframes link-breathe {
  0%,
  100% {
    opacity: 0.72;
    transform: scale(0.92);
  }
  50% {
    opacity: 1;
    transform: scale(1.06);
  }
}
</style>
