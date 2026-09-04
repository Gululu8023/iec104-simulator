<template>
  <el-dialog
    v-model="dialogVisible"
    width="640px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :lock-scroll="false"
    class="app-dialog-shell about-dialog"
  >
    <template #header>
      <div class="app-dialog-header">
        <div>
          <div class="app-dialog-title">{{ t(titleKey) }}</div>
          <div class="app-dialog-help">{{ t('about.descriptionText') }}</div>
        </div>
      </div>
    </template>

    <div class="app-dialog-body about-dialog__body">
      <div v-if="loading" class="app-dialog-help">{{ t('about.loading') }}</div>
      <template v-else-if="appInfo">
        <dl class="about-dialog__metadata">
          <div>
            <dt>{{ t('about.version') }}</dt>
            <dd>{{ appInfo.version }}</dd>
          </div>
          <div>
            <dt>{{ t('about.license') }}</dt>
            <dd>{{ appInfo.license }}</dd>
          </div>
          <div>
            <dt>{{ t('about.developer') }}</dt>
            <dd>Gululu8023</dd>
          </div>
          <div>
            <dt>{{ t('about.copyright') }}</dt>
            <dd>{{ copyrightText }}</dd>
          </div>
        </dl>

        <section class="conn-section">
          <div class="conn-section-title">{{ t('about.resources') }}</div>
          <div class="about-dialog__resources">
            <div v-for="item in resources" :key="item.label" class="about-dialog__resource">
              <div>
                <strong>{{ item.label }}</strong>
                <span>{{ item.value }}</span>
              </div>
              <el-button
                size="small"
                class="app-dialog-btn app-dialog-btn--outline"
                @click="copy(item.value)"
              >
                {{ t('about.copy') }}
              </el-button>
            </div>
          </div>
        </section>

        <div class="app-dialog-note app-dialog-note--warning">{{ t('about.scopeNotice') }}</div>
        <div class="app-dialog-help">{{ t('about.supportNotice') }}</div>
      </template>
      <div v-else class="app-dialog-note app-dialog-note--error">{{ loadError }}</div>
    </div>

    <template #footer>
      <div class="app-dialog-footer">
        <el-button type="primary" @click="dialogVisible = false">{{ t('common.close') }}</el-button>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'

import { getAppInfo } from '@shared/api/common'
import type { AppInfo } from '@shared/api/types'
import { t } from '@shared/i18n'
import { getCopyrightText } from '@shared/i18n/copyright'

const SUPPORT_EMAIL = 'vergilsparda0905@gmail.com'

const props = defineProps<{
  visible: boolean
  titleKey: 'about.masterTitle' | 'about.slaveTitle'
}>()

const emit = defineEmits<{ 'update:visible': [value: boolean] }>()

const dialogVisible = computed({
  get: () => props.visible,
  set: (value: boolean) => emit('update:visible', value),
})
const appInfo = ref<AppInfo | null>(null)
const loading = ref(false)
const loadError = ref('')
const copyrightText = computed(() => getCopyrightText())
const resources = computed(() => {
  if (!appInfo.value) return []
  return [
    { label: t('about.repository'), value: appInfo.value.repository_url },
    { label: t('about.issues'), value: appInfo.value.issues_url },
    {
      label: t('about.documentation'),
      value: appInfo.value.docs_url ?? appInfo.value.repository_url,
    },
    { label: t('about.support'), value: SUPPORT_EMAIL },
  ]
})

const load = async () => {
  loading.value = true
  loadError.value = ''
  try {
    appInfo.value = await getAppInfo()
  } catch (error) {
    appInfo.value = null
    loadError.value = t('settingsMessages.readAppInfoFailed', { error })
  } finally {
    loading.value = false
  }
}

const copy = async (value: string) => {
  try {
    await navigator.clipboard.writeText(value)
    ElMessage.success(t('about.copied'))
  } catch {
    ElMessage.error(t('shell.copyFailed'))
  }
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) void load()
  },
)
</script>

<style scoped>
.about-dialog__body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.about-dialog__metadata {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 20px;
  margin: 0;
}

.about-dialog__metadata div {
  display: grid;
  grid-template-columns: 88px 1fr;
  gap: 8px;
}

.about-dialog__metadata dt {
  color: var(--text-secondary);
}

.about-dialog__metadata dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.about-dialog__resources {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.about-dialog__resource {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.about-dialog__resource > div {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.about-dialog__resource span {
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}
</style>
