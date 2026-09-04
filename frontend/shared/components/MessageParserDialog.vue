<template>
  <el-dialog
    v-model="visible"
    width="780px"
    :lock-scroll="false"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    class="app-dialog-shell message-parser-dialog message-inspector-dialog"
  >
    <template #header>
      <div class="message-inspector-header">
        <span class="message-inspector-title">{{ t('messageParser.title') }}</span>
      </div>
    </template>

    <el-scrollbar class="message-inspector-scroll" max-height="60vh">
      <div class="message-inspector-content">
        <section class="conn-section">
          <div class="conn-section-header">
            <div class="conn-section-title">{{ t('messageParser.inputTitle') }}</div>
            <div class="app-dialog-btn-group">
              <el-button
                size="small"
                class="app-dialog-btn app-dialog-btn--outline"
                @click="handleClear"
                >{{ t('messageParser.clear') }}</el-button
              >
              <el-button
                size="small"
                class="app-dialog-btn app-dialog-btn--soft-primary"
                :loading="loading"
                @click="handleParse"
                >{{ t('messageParser.parse') }}</el-button
              >
            </div>
          </div>
          <el-input
            v-model="inputText"
            type="textarea"
            :rows="3"
            resize="vertical"
            :placeholder="t('messageParser.placeholder')"
            class="message-parser-dialog__textarea"
          />
          <div class="app-dialog-help message-parser-dialog__hint">
            {{ t('messageParser.inputHint') }}
          </div>
        </section>

        <section class="conn-section">
          <div class="conn-section-title">{{ t('messageParser.resultTitle') }}</div>

          <div class="protocol-tree-panel protocol-tree-panel--enhanced">
            <div
              v-if="loading"
              class="protocol-tree-panel__state protocol-tree-panel__state--loading"
            >
              {{ t('messageParser.parsing') }}
            </div>

            <div
              v-else-if="activeErrorMessage"
              class="protocol-tree-panel__state protocol-tree-panel__state--error"
            >
              <div class="error-message-title">{{ activeErrorMessage }}</div>
              <div v-if="result?.error?.detail" class="error-message-detail">
                {{ result.error.detail }}
              </div>
              <div v-if="result?.error?.byte_range" class="error-message-location">
                {{
                  t('messageParser.errorLocation', {
                    start: result.error.byte_range.start,
                    end: result.error.byte_range.end,
                  })
                }}
              </div>
            </div>

            <div v-else-if="result" class="protocol-tree-panel__content">
              <ParsedFrameTree :nodes="result.tree" />
            </div>

            <div v-else class="protocol-tree-panel__state">{{ t('messageParser.waiting') }}</div>
          </div>
        </section>
      </div>
    </el-scrollbar>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'

import type { MessageFrameParseResult } from '@shared/api/types'
import {
  getMessageFrameParseErrorMessage,
  useMessageFrameParserSession,
} from '@shared/composables/useMessageFrameParser'
import { t } from '@shared/i18n'

import ParsedFrameTree from './message-parser/ParsedFrameTree.vue'

const visible = ref(false)
const parser = useMessageFrameParserSession()
const inputText = ref('')

const loading = computed(() => parser.loading.value)
const result = computed<MessageFrameParseResult | null>(() => parser.result.value)
const activeErrorMessage = computed(() => {
  if (parser.parseError.value) return parser.parseError.value
  return getMessageFrameParseErrorMessage(result.value)
})

const handleParse = async () => {
  try {
    await parser.parse(inputText.value)
  } catch {
    // parseError state is already captured by the shared parser session
  }
}

const handleClear = () => {
  inputText.value = ''
  parser.reset()
}

const open = (nextInput = '') => {
  visible.value = true
  inputText.value = nextInput
  parser.reset()
  if (nextInput.trim()) {
    void parser.parse(nextInput).catch(() => undefined)
  }
}

const close = () => {
  visible.value = false
}

defineExpose({
  open,
  close,
})
</script>

<style scoped>
.conn-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.conn-section-header .conn-section-title {
  margin-bottom: 0;
}

.conn-section-actions {
  display: flex;
  gap: 8px;
}

.message-parser-dialog__textarea :deep(textarea) {
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
  min-height: 72px;
}

.error-message-title {
  font-weight: 600;
  margin-bottom: 8px;
}

.error-message-detail {
  font-size: 12px;
  margin-bottom: 6px;
  color: #991b1b;
}

.error-message-location {
  font-size: 11px;
  font-family: var(--font-mono);
  color: #7f1d1d;
  background: #fef2f2;
  padding: 4px 8px;
  border-radius: 4px;
  display: inline-block;
  margin-top: 6px;
}
</style>
