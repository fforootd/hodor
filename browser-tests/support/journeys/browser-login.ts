import { expect, type Page } from '@playwright/test'

import { completePasswordLogin } from '../browser'

export async function loginAsAdmin(page: Page) {
  await page.goto('/console')
  await expect(page).toHaveURL(/\/login/)
  await completePasswordLogin(page, 'admin', 'admin123')
  await expect.poll(() => page.url(), { timeout: 15_000 }).toMatch(/\/console\/?/)
}
