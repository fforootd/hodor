/**
 * Vite config for building the Zitadel web component library.
 *
 * Produces per-component ES module bundles + a consolidated barrel.
 * CSS is inlined into each component (Vue CE mode handles this).
 *
 * Build:  npm run build:wc
 * Output: web/dist-wc/
 */

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [
    vue({
      // Treat .ce.vue files as custom elements
      customElement: /\.ce\.vue$/,
    }),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  build: {
    outDir: 'dist-wc',
    emptyOutDir: true,
    lib: {
      entry: {
        // Barrel (imports everything)
        'zitadel-ui': resolve(__dirname, 'src/wc/index.ts'),
        // Per-component entry points
        'zitadel-login': resolve(__dirname, 'src/login/zitadel-login-wc.ts'),
        'zitadel-identity-list': resolve(__dirname, 'src/wc/zitadel-identity-list-wc.ts'),
        'zitadel-identity-detail': resolve(__dirname, 'src/wc/zitadel-identity-detail-wc.ts'),
        'zitadel-identity-create': resolve(__dirname, 'src/wc/zitadel-identity-create-wc.ts'),
        'zitadel-account': resolve(__dirname, 'src/wc/zitadel-account-wc.ts'),
        'zitadel-session-manager': resolve(__dirname, 'src/wc/zitadel-session-manager-wc.ts'),
        'zitadel-user-invite': resolve(__dirname, 'src/wc/zitadel-user-invite-wc.ts'),
        // Admin components
        'zitadel-org-list': resolve(__dirname, 'src/wc/zitadel-org-list-wc.ts'),
        'zitadel-session-list': resolve(__dirname, 'src/wc/zitadel-session-list-wc.ts'),
        'zitadel-provider-list': resolve(__dirname, 'src/wc/zitadel-provider-list-wc.ts'),
      },
      formats: ['es'],
    },
    rollupOptions: {
      output: {
        // Keep Vue as external for consumers who already have it
        // Comment out the next line to bundle Vue runtime (~80KB)
        // external: ['vue'],
        entryFileNames: '[name].js',
        chunkFileNames: 'chunks/[name]-[hash].js',
      },
    },
    // Inline CSS into JS for shadow DOM usage
    cssCodeSplit: false,
    minify: 'esbuild',
    sourcemap: true,
  },
  define: {
    'process.env.NODE_ENV': JSON.stringify('production'),
  },
})
