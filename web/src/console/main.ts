import '@/assets/index.css'
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { install as VueMonacoEditorPlugin } from '@guolao/vue-monaco-editor'
import { configureApi } from '@/api/client'
import App from './App.vue'
import router from './router'

// Configure the shared API client for standalone console mode.
// The base URL is injected by the Go server at runtime.
configureApi({
  baseUrl: (window as any).__ZITADEL_BASE_PATH__ || '',
  getOrgId: () => {
    try { return localStorage.getItem('zitadel_org') } catch { return null }
  },
})

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(VueMonacoEditorPlugin, {
  paths: {
    vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.52.2/min/vs',
  },
})
app.mount('#console-app')
