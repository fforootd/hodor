import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'

const consoleKitSrc = resolve(__dirname, '../packages/console-kit/src')
const webSrc = resolve(__dirname, 'src')

// Resolves @/ imports per-package: files inside console-kit resolve @/ to
// console-kit/src, while files inside web/ try web/src first, then fall back
// to console-kit/src. This lets us move code to console-kit without changing
// any import paths — login/account SPAs transparently find moved files.
function perPackageAlias(): Plugin {
  return {
    name: 'per-package-alias',
    enforce: 'pre',
    async resolveId(source, importer) {
      if (!source.startsWith('@/') || !importer) return null
      const suffix = source.slice(2)
      if (importer.includes('packages/console-kit/')) {
        return this.resolve(resolve(consoleKitSrc, suffix), importer, { skipSelf: true })
      }
      // For web files: try web/src/ first, then fall back to console-kit/src/
      const webResult = await this.resolve(resolve(webSrc, suffix), importer, { skipSelf: true })
      if (webResult) return webResult
      return this.resolve(resolve(consoleKitSrc, suffix), importer, { skipSelf: true })
    },
  }
}

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

export default defineConfig(() => {
  const apiBase = process.env.ZITADEL_API_BASE || 'http://localhost:8080'

  return {
    plugins: [perPackageAlias(), vue(), tailwindcss(), spaFallback()],
    resolve: {
      alias: {
        '@zitadel/client-js': resolve(consoleKitSrc, 'lib/zitadel-client-stub.ts'),
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
          target: apiBase,
          changeOrigin: true,
        },
        '/v1': apiBase,
        '/openapi.json': {
          target: apiBase,
          changeOrigin: true,
        },
        '/.well-known': {
          target: apiBase,
          changeOrigin: true,
        },
        '/authorize': {
          target: apiBase,
          changeOrigin: true,
        },
        '/oauth': {
          target: apiBase,
          changeOrigin: true,
        },
        '/userinfo': {
          target: apiBase,
          changeOrigin: true,
        },
        '/keys': {
          target: apiBase,
          changeOrigin: true,
        },
        '/end_session': {
          target: apiBase,
          changeOrigin: true,
        },
        '/revoke': {
          target: apiBase,
          changeOrigin: true,
        },
        '/devicecode': {
          target: apiBase,
          changeOrigin: true,
        },
        '/healthz': apiBase,
        '/readyz': apiBase,
        '/assets': apiBase,
      },
    },
  }
})
