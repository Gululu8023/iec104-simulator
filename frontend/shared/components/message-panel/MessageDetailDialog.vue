<template>
  <el-dialog
    v-model="visible"
    width="780px"
    :before-close="beforeClose"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="app-dialog-shell message-detail-dialog message-inspector-dialog"
  >
    <template #header>
      <div class="message-inspector-header">
        <span class="message-inspector-title">{{ t('messagePanel.detail.title') }}</span>
        <div class="message-inspector-nav">
          <button
            class="message-inspector-nav-btn"
            :disabled="selectedMessageIndex <= 0"
            @click="navigateMessage(-1)"
          >
            <el-icon><ArrowLeft /></el-icon>
          </button>
          <span class="message-inspector-nav-info"
            >{{ selectedMessageIndex + 1 }} / {{ filteredMessageCount }}</span
          >
          <button
            class="message-inspector-nav-btn"
            :disabled="selectedMessageIndex >= filteredMessageCount - 1"
            @click="navigateMessage(1)"
          >
            <el-icon><ArrowRight /></el-icon>
          </button>
        </div>
      </div>
    </template>

    <el-scrollbar v-if="selectedMessage" class="message-inspector-scroll" max-height="60vh">
      <div class="message-inspector-content">
        <!-- 报文概览 -->
        <div class="conn-section message-overview-section">
          <div class="conn-section-title">{{ t('messagePanel.detail.basicInfo') }}</div>
          <div class="message-overview">
            <div class="message-overview__meta">
              <span class="message-overview__time font-mono">{{
                formatDetailTime(selectedMessage.timestamp)
              }}</span>
              <span class="message-overview__direction" :class="selectedMessage.type">{{
                getDirectionLabel(selectedMessage.type)
              }}</span>
              <span class="message-overview__route" :title="selectedMessageRoute">{{
                selectedMessageRoute
              }}</span>
            </div>
            <div
              class="message-overview__summary"
              :title="selectedFrameSummary.title || selectedMessage.content"
            >
              <span
                v-if="selectedFrameSummary.tag"
                class="message-overview__frame"
                :class="selectedFrameSummary.color"
                >{{ selectedFrameSummary.tag }}</span
              >
              <span class="message-overview__summary-text">{{
                selectedFrameSummary.label || selectedMessage.content
              }}</span>
            </div>
          </div>
        </div>

        <!-- 原始数据 -->
        <div v-if="selectedMessage.hexData" class="conn-section">
          <div class="conn-section-header">
            <div class="conn-section-title">{{ t('messagePanel.detail.rawData') }}</div>
            <div class="detail-hex-actions">
              <button
                class="detail-hex-action-btn"
                :title="t('messagePanel.detail.copy')"
                @click="copyHexData"
              >
                <el-icon :size="14"><CopyDocument /></el-icon>
              </button>
              <button
                class="detail-hex-action-btn"
                :title="
                  hexDataExpanded
                    ? t('messagePanel.detail.collapse')
                    : t('messagePanel.detail.expand')
                "
                @click="toggleHexDataExpanded"
              >
                <el-icon :size="14"><ArrowUp v-if="hexDataExpanded" /><ArrowDown v-else /></el-icon>
              </button>
            </div>
          </div>
          <div class="detail-hex-block" :class="{ 'is-expanded': hexDataExpanded }">
            <code class="detail-hex-code">{{ formatHexData(selectedMessage.hexData) }}</code>
          </div>
        </div>

        <!-- 解析结果 -->
        <div class="conn-section">
          <div class="conn-section-title">{{ t('messagePanel.detail.parseResult') }}</div>
          <template v-if="shouldShowDetailTree && detailParseResult">
            <div class="protocol-tree-panel protocol-tree-panel--fixed">
              <div class="protocol-tree-panel__content">
                <ParsedFrameTree :nodes="detailParseResult.tree" />
              </div>
            </div>
          </template>

          <template v-else>
            <div
              v-if="isTreeDetailMode"
              class="detail-parse-state"
              :class="{ 'is-error': Boolean(detailParseError), 'is-loading': detailParseLoading }"
            >
              {{
                detailParseLoading
                  ? t('messagePanel.detail.parsing')
                  : detailParseError
                    ? t('messagePanel.detail.parseErrorFallback', { error: detailParseError })
                    : selectedMessage?.hexData
                      ? t('messagePanel.detail.treeNoResultFallback')
                      : t('messagePanel.detail.noHexFallback')
              }}
            </div>

            <table class="detail-parsed-table">
              <thead>
                <tr>
                  <th style="width: 180px">{{ t('messagePanel.detail.field') }}</th>
                  <th>{{ t('messagePanel.detail.value') }}</th>
                  <th>{{ t('messagePanel.detail.note') }}</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(row, idx) in getSmartParsedData(selectedMessage, detailParseResult)"
                  :key="idx"
                >
                  <td class="detail-parsed-field">{{ row.field }}</td>
                  <td class="detail-parsed-value font-mono">{{ row.value }}</td>
                  <td class="detail-parsed-desc">{{ row.description }}</td>
                </tr>
              </tbody>
            </table>
          </template>
        </div>
      </div>
    </el-scrollbar>
  </el-dialog>
</template>

<script setup lang="ts">
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, CopyDocument } from '@element-plus/icons-vue'
import { t } from '@shared/i18n'
import ParsedFrameTree from '../message-parser/ParsedFrameTree.vue'
import { formatDetailTime, getDirectionLabel } from './frameFormatters'
import { formatHexData, getSmartParsedData } from './parser'
import type { FrameSummary, Message } from './types'

type AnyCallback = (...args: any[]) => any

defineProps<{
  beforeClose: (done: () => void) => void
  selectedMessageIndex: number
  filteredMessageCount: number
  selectedMessage: Message | null
  selectedMessageRoute: string
  selectedFrameSummary: FrameSummary
  hexDataExpanded: boolean
  shouldShowDetailTree: boolean
  detailParseResult: any
  detailParseError: string | null
  detailParseLoading: boolean
  isTreeDetailMode: boolean
  navigateMessage: (offset: number) => void
  copyHexData: AnyCallback
  toggleHexDataExpanded: AnyCallback
}>()

const visible = defineModel<boolean>('visible', { required: true })
</script>

<style scoped>
.message-overview {
  display: flex;
  min-height: 60px;
  box-sizing: border-box;
  flex-direction: column;
  justify-content: center;
  gap: 6px;
  padding: 8px 12px;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  background: #f8fafc;
}

.message-overview__meta,
.message-overview__summary {
  display: flex;
  align-items: center;
  min-width: 0;
  white-space: nowrap;
}

.message-overview__meta {
  gap: 10px;
  color: #64748b;
  font-size: 12px;
}

.message-overview__time,
.message-overview__direction,
.message-overview__frame {
  flex-shrink: 0;
}

.message-overview__direction {
  min-width: 34px;
  box-sizing: border-box;
  padding: 1px 6px;
  border: 1px solid transparent;
  border-radius: 4px;
  font-weight: 600;
  text-align: center;
}

.message-overview__direction.sent {
  border-color: rgba(9, 105, 218, 0.12);
  background: #ddf4ff;
  color: #0969da;
}

.message-overview__direction.received {
  border-color: rgba(26, 127, 55, 0.12);
  background: #dafbe1;
  color: #1a7f37;
}

.message-overview__direction.error {
  border-color: rgba(207, 34, 46, 0.12);
  background: #ffebe9;
  color: #cf222e;
}

.message-overview__route,
.message-overview__summary-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.message-overview__summary {
  gap: 8px;
  color: #1f2937;
  font-size: 13px;
  font-weight: 600;
}

.message-overview__frame {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
}

.message-overview__frame.frame-i {
  color: #1d4ed8;
}

.message-overview__frame.frame-s {
  color: #047857;
}

.message-overview__frame.frame-u {
  color: #b45309;
}

.message-overview__frame.frame-unknown {
  color: #cf222e;
}

/* 原始数据 - 极客化展示 */
.detail-hex-block {
  position: relative;
  background: #fcfcfd;
  border: 1px solid #f0f0f0;
  border-radius: 4px;
  padding: 12px 16px;
  overflow: hidden;
  max-height: 48px;
  transition: max-height 0.3s ease;
}

.detail-hex-block.is-expanded {
  max-height: 400px;
  overflow-y: auto;
  overflow-x: hidden;
}

.detail-hex-code {
  font-family: var(--font-mono);
  font-size: 12px;
  color: #374151;
  line-height: 1.8;
  letter-spacing: 0.5px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}

.detail-hex-block.is-expanded .detail-hex-code {
  white-space: pre-wrap;
  word-break: break-all;
  overflow: visible;
}

.conn-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.conn-section-header .conn-section-title {
  margin-bottom: 0;
}

.detail-hex-actions {
  display: flex;
  gap: 4px;
}

.detail-hex-action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: #bfbfbf;
  cursor: pointer;
  transition: all 0.15s;
}

.detail-hex-action-btn:hover {
  background: rgba(0, 0, 0, 0.06);
  color: #595959;
}

.detail-parse-state {
  margin-bottom: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid #e2e8f0;
  background: #f8fafc;
  color: #475569;
  font-size: 12px;
}

.detail-parse-state.is-loading {
  background: #eff6ff;
  border-color: #bfdbfe;
  color: #1d4ed8;
}

.detail-parse-state.is-error {
  background: #fff1f2;
  border-color: #fecdd3;
  color: #be123c;
}

/* 解析结果 - 轻量化表格 */
.detail-parsed-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.detail-parsed-table thead th {
  text-align: left;
  padding: 12px 12px 12px 0;
  font-weight: 600;
  color: #1f2225;
  font-size: 13px;
  border-bottom: 2px solid #e8e8e8;
}

.detail-parsed-table tbody td {
  padding: 12px 12px 12px 0;
  color: #374151;
  border-bottom: 1px solid #e8e8e8;
}

.detail-parsed-table tbody tr:last-child td {
  border-bottom: none;
}

.detail-parsed-table tbody tr:hover {
  background: #fafafa;
}

.detail-parsed-field {
  font-weight: 500;
  color: #4b5563;
}

.detail-parsed-value {
  font-family: var(--font-mono);
  color: #1f2937;
}

.detail-parsed-desc {
  color: #9ca3af;
}
</style>
