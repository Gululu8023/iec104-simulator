import { createApp } from 'vue'
import App from './App.vue'
import { i18n, loadInitialLocale } from '@shared/i18n'
import { loadIec104Capabilities } from '@shared/api/common'
import { installSelectOptionOverflowTitles } from '@shared/ui/selectOptionOverflowTitle'
import '@shared/styles.css'
import 'element-plus/es/components/message/style/css'
import 'element-plus/es/components/message-box/style/css'

const app = createApp(App)
app.use(i18n)
installSelectOptionOverflowTitles()

void Promise.all([loadInitialLocale(), loadIec104Capabilities()]).finally(() => {
  app.mount('#app')
})
