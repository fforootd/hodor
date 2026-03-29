import { defineConfig } from '@hey-api/openapi-ts'

export default defineConfig({
  input: '../openapi.json',
  output: {
    path: 'src/generated',
    format: 'prettier',
    lint: false,
  },
  plugins: [
    '@hey-api/typescript',
    '@hey-api/sdk',
    '@hey-api/client-fetch',
  ],
})
