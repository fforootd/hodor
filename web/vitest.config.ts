import { defineConfig, type Plugin } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

const webSrc = resolve(__dirname, 'src')
const consoleKitSrc = resolve(__dirname, '../packages/console-kit/src')

// Same per-package alias as web/vite.config.ts — resolves @/ based on
// which package the importing file is in.
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
      const webResult = await this.resolve(resolve(webSrc, suffix), importer, { skipSelf: true })
      if (webResult) return webResult
      return this.resolve(resolve(consoleKitSrc, suffix), importer, { skipSelf: true })
    },
  }
}

export default defineConfig({
  plugins: [perPackageAlias(), vue()],
  resolve: {
    alias: {
      '@zitadel/client-js': resolve(consoleKitSrc, 'lib/zitadel-client-stub.ts'),
    },
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    include: [
      'src/**/*.test.ts',
      '../packages/console-kit/src/**/*.test.ts',
    ],
  },
})
