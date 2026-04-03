import { test, expect } from '@playwright/test'

import { loginAsAdmin } from '../support/browser'

test.describe('Login Flow', () => {
  test('shows login form @full', async ({ page }) => {
    await page.goto('/login')
    await expect(page.locator('input[name="identifier"], input[type="text"]')).toBeVisible()
  })

  test('rejects invalid credentials @full', async ({ page }) => {
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'bad@user.com')
    await page.click('button[type="submit"]')
    // Should show error or stay on login page.
    await expect(page).toHaveURL(/\/login/)
  })

  test('admin login redirects to console @smoke', async ({ page }) => {
    await loginAsAdmin(page)
    await expect(page).toHaveURL(/\/console/)
  })
})
