import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: { '@': resolve(__dirname, 'src') },
  },
  build: {
    outDir: 'dist',
    rollupOptions: {
      input: {
        login: resolve(__dirname, 'src/login/index.html'),
        console: resolve(__dirname, 'src/console/index.html'),
        account: resolve(__dirname, 'src/account/index.html'),
      },
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/v1': 'http://localhost:8080',
      '/login': 'http://localhost:8080',
      '/admin': 'http://localhost:8080',
      '/console': 'http://localhost:8080',
      '/account': 'http://localhost:8080',
      '/healthz': 'http://localhost:8080',
      '/readyz': 'http://localhost:8080',
      '/assets': 'http://localhost:8080',
    },
  },
})
