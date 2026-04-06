import { defineConfig } from '@playwright/test'

const baseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
process.env.BASE_URL = baseURL

export default defineConfig({
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
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
      name: 'journeys-login',
      testMatch: '**/journeys/login/password-login.spec.ts',
    },
    {
      name: 'journeys-login-oidc',
      testMatch: '**/journeys/login/oidc-*.spec.ts',
    },
  ],
  webServer: {
    command: './browser-tests/scripts/start-stack.sh',
    cwd: '..',
    url: `${baseURL}/healthz`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
})
