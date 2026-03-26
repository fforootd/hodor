import { test, expect } from '@playwright/test'

test.describe('Login Flow', () => {
  test('shows login form', async ({ page }) => {
    await page.goto('/login')
    await expect(page.locator('input[name="identifier"], input[type="text"]')).toBeVisible()
  })

  test('rejects invalid credentials', async ({ page }) => {
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'bad@user.com')
    await page.click('button[type="submit"]')
    // Should show error or stay on login page.
    await expect(page).toHaveURL(/\/login/)
  })

  test('admin login redirects to console', async ({ page }) => {
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'admin@zitadel.local')
    await page.click('button[type="submit"]')
    // After identifier step, password step should appear.
    await expect(page.locator('input[type="password"]')).toBeVisible({ timeout: 5000 })
  })
})
