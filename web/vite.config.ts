import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'

// SPA history fallback: rewrite /console/*, /login/*, /account/* to
// their respective HTML entry points so Vue Router handles routing.
function spaFallback(): Plugin {
  return {
    name: 'spa-fallback',
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        const url = req.url || ''
        if (url.startsWith('/console') && !url.includes('.')) {
          req.url = '/src/console/index.html'
        } else if (url.startsWith('/login') && !url.includes('.')) {
          req.url = '/src/login/index.html'
        } else if (url.startsWith('/account') && !url.includes('.')) {
          req.url = '/src/account/index.html'
        }
        next()
      })
    },
  }
}

export default defineConfig({
  plugins: [vue(), tailwindcss(), spaFallback()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@zitadel/client-js': resolve(__dirname, 'src/lib/zitadel-client-stub.ts'),
    },
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
      // SSE endpoint — must be defined BEFORE /v1 so it takes precedence.
      // changeOrigin ensures the Host header matches the Go server.
      '/v1/events/stream': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/v1': 'http://localhost:8080',
      '/openapi.json': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/healthz': 'http://localhost:8080',
      '/readyz': 'http://localhost:8080',
      '/assets': 'http://localhost:8080',
      // OIDC endpoints (Go OIDC provider on :8080).
      '/.well-known': 'http://localhost:8080',
      '/authorize': 'http://localhost:8080',
      '/oauth': 'http://localhost:8080',
      '/userinfo': 'http://localhost:8080',
      '/end_session': 'http://localhost:8080',
      '/keys': 'http://localhost:8080',
      '/revoke': 'http://localhost:8080',
      '/devicecode': 'http://localhost:8080',
    },
  },
})
