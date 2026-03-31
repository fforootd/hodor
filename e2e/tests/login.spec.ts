import { test, expect } from '@playwright/test'

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
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'admin')
    await page.click('button[type="submit"]')
    await expect(page.locator('input[type="password"]')).toBeVisible({ timeout: 5000 })
    await page.fill('input[name="password"], input[type="password"]', 'admin123')
    await page.click('button[type="submit"]')
    await page.waitForURL(/\/console/, { timeout: 15000 })
  })
})
