<template>
  <el-dialog
    v-model="visible"
    width="960px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="app-dialog-shell help-doc-dialog"
  >
    <template #header>
      <div class="app-dialog-header">
        <div class="app-dialog-title">{{ activeDoc.title }}</div>
      </div>
    </template>

    <div class="app-dialog-body help-doc-dialog__body">
      <nav
        v-if="headingEntries.length >= 3"
        class="help-doc-dialog__toc"
        :aria-label="t('help.contents')"
      >
        <div class="help-doc-dialog__toc-title">{{ t('help.contents') }}</div>
        <button
          v-for="heading in headingEntries"
          :key="heading.id"
          type="button"
          class="help-doc-dialog__toc-link"
          :class="{ 'help-doc-dialog__toc-link--nested': heading.depth === 3 }"
          @click="scrollToHeading(heading.id)"
        >
          {{ heading.label }}
        </button>
      </nav>
      <el-scrollbar class="help-doc-dialog__scroll">
        <article
          ref="contentRef"
          class="help-doc-markdown"
          v-html="renderedHtml"
          @click="handleContentClick"
        ></article>
      </el-scrollbar>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { marked } from 'marked'

import {
  getHelpDocDefinition,
  resolveHelpDocKeyFromHref,
  type HelpDocKey,
} from '@shared/content/helpDocs'
import { t } from '@shared/i18n'

const props = defineProps<{
  visible: boolean
  docKey: HelpDocKey
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

const visible = computed({
  get: () => props.visible,
  set: (value: boolean) => emit('update:visible', value),
})

const activeDocKey = ref<HelpDocKey>(props.docKey)
const contentRef = ref<HTMLElement | null>(null)
const activeDoc = computed(() => getHelpDocDefinition(activeDocKey.value))

watch(
  () => props.docKey,
  (key) => {
    activeDocKey.value = key
  },
)

const strippedMarkdown = computed(() =>
  activeDoc.value.content.replace(/^#\s+.+?\r?\n+/, '').trim(),
)

const headingEntries = computed(() =>
  Array.from(strippedMarkdown.value.matchAll(/^(#{2,3})\s+(.+?)\s*$/gmu), (match, index) => ({
    id: `help-section-${index + 1}`,
    depth: match[1].length,
    label: match[2].replace(/[`*_~]/gu, ''),
  })),
)

const renderedHtml = computed(() => {
  let headingIndex = 0
  return (
    marked.parse(strippedMarkdown.value, {
      async: false,
      gfm: true,
      breaks: true,
    }) as string
  ).replace(/<h([23])>/gu, (headingTag) => {
    const heading = headingEntries.value[headingIndex]
    headingIndex += 1
    return heading ? headingTag.replace('>', ` id="${heading.id}">`) : headingTag
  })
})

const scrollToHeading = (id: string) => {
  contentRef.value?.querySelector<HTMLElement>(`#${id}`)?.scrollIntoView({ block: 'start' })
}

const handleContentClick = (event: MouseEvent) => {
  const anchor = (event.target as HTMLElement | null)?.closest('a') as HTMLAnchorElement | null
  if (!anchor) return
  const href = anchor.getAttribute('href') ?? ''
  if (href.startsWith('#help-section-')) {
    event.preventDefault()
    scrollToHeading(href.slice(1))
    return
  }
  const targetDoc = resolveHelpDocKeyFromHref(href)
  if (targetDoc) {
    event.preventDefault()
    activeDocKey.value = targetDoc
    return
  }
  if (/^https?:\/\//i.test(href)) {
    event.preventDefault()
    void navigator.clipboard.writeText(href).then(
      () => ElMessage.success(t('about.copied')),
      () => ElMessage.error(t('shell.copyFailed')),
    )
  }
}
</script>

<style scoped>
:deep(.help-doc-dialog.el-dialog),
:deep(.help-doc-dialog .el-dialog) {
  height: min(82vh, 760px);
  max-height: 82vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

:deep(.help-doc-dialog.el-dialog .el-dialog__body),
:deep(.help-doc-dialog .el-dialog__body) {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.help-doc-dialog__body {
  display: flex;
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
  padding: 0;
}

.help-doc-dialog__toc {
  box-sizing: border-box;
  flex: 0 0 210px;
  overflow-y: auto;
  padding: 18px 12px;
  border-right: 1px solid #e9eff8;
}

.help-doc-dialog__toc-title {
  margin: 0 8px 10px;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
}

.help-doc-dialog__toc-link {
  display: block;
  width: 100%;
  padding: 6px 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.help-doc-dialog__toc-link:hover {
  background: #f3f7fd;
  color: var(--primary-color);
}

.help-doc-dialog__toc-link--nested {
  padding-left: 20px;
  font-size: 13px;
}

.help-doc-dialog__scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

.help-doc-dialog__scroll :deep(.el-scrollbar__wrap) {
  min-height: 0;
  overflow-x: hidden;
}

.help-doc-dialog__scroll :deep(.el-scrollbar__view) {
  min-height: 100%;
}

.help-doc-markdown {
  box-sizing: border-box;
  min-height: 100%;
  padding: 18px 24px 24px;
  color: var(--text-primary);
  font-size: 14px;
  line-height: 1.75;
}

.help-doc-markdown :deep(h1),
.help-doc-markdown :deep(h2),
.help-doc-markdown :deep(h3),
.help-doc-markdown :deep(h4) {
  margin: 0 0 14px;
  color: var(--text-primary);
  line-height: 1.45;
}

.help-doc-markdown :deep(h2) {
  /* margin-top: 26px; */
  padding-top: 8px;
  border-top: 1px solid #eef2f8;
  font-size: 20px;
}

.help-doc-markdown :deep(h3) {
  margin-top: 18px;
  font-size: 17px;
}

.help-doc-markdown :deep(p),
.help-doc-markdown :deep(ul),
.help-doc-markdown :deep(ol),
.help-doc-markdown :deep(blockquote),
.help-doc-markdown :deep(pre),
.help-doc-markdown :deep(table) {
  margin: 0 0 14px;
}

.help-doc-markdown :deep(ul),
.help-doc-markdown :deep(ol) {
  padding-left: 22px;
}

.help-doc-markdown :deep(li + li) {
  margin-top: 4px;
}

.help-doc-markdown :deep(code) {
  padding: 1px 6px;
  border-radius: 6px;
  background: #f3f6fb;
  color: #1f2b42;
  font-family: 'Cascadia Mono', 'Consolas', monospace;
  font-size: 13px;
}

.help-doc-markdown :deep(pre) {
  overflow: auto;
  padding: 12px 14px;
  border: 1px solid #e4ebf5;
  border-radius: 10px;
  background: #f8fbff;
}

.help-doc-markdown :deep(pre code) {
  padding: 0;
  background: transparent;
}

.help-doc-markdown :deep(table) {
  width: 100%;
  border-collapse: collapse;
  overflow: hidden;
  border: 1px solid #e5edf8;
  border-radius: 10px;
}

.help-doc-markdown :deep(th),
.help-doc-markdown :deep(td) {
  padding: 10px 12px;
  border-bottom: 1px solid #e9eff8;
  text-align: left;
  vertical-align: top;
}

.help-doc-markdown :deep(th) {
  background: #f7faff;
  color: var(--text-secondary);
  font-weight: 600;
}

.help-doc-markdown :deep(tr:last-child td) {
  border-bottom: none;
}

.help-doc-markdown :deep(hr) {
  margin: 20px 0;
  border: none;
  border-top: 1px solid #e9eff8;
}

.help-doc-markdown :deep(a) {
  color: var(--primary-color);
  text-decoration: none;
}

.help-doc-markdown :deep(a:hover) {
  text-decoration: underline;
}
</style>
