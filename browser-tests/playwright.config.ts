import { defineConfig } from '@playwright/test'

const baseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
process.env.BASE_URL = baseURL

export default defineConfig({
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'journeys-admin',
      testDir: './journeys/admin',
    },
    {
      name: 'journeys-login-oidc',
      testDir: './journeys/login',
    },
  ],
  webServer: {
    command: './browser-tests/scripts/start-stack.sh',
    cwd: '..',
    url: `${baseURL}/healthz`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
})
