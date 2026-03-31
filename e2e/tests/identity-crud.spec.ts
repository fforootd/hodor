import { test, expect } from '@playwright/test'

test.describe('Identity CRUD', () => {
  test.beforeEach(async ({ page }) => {
    // Login as admin first.
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'admin')
    await page.click('button[type="submit"]')
    await page.fill('input[name="password"], input[type="password"]', 'admin123')
    await page.click('button[type="submit"]')
    await page.waitForURL(/\/console/, { timeout: 10000 })
  })

  test('identity list page loads @full', async ({ page }) => {
    await page.goto('/console/users')
    await expect(page.locator('table, .table-wrap')).toBeVisible({ timeout: 5000 })
  })

  test('create identity form is accessible @full', async ({ page }) => {
    await page.goto('/console/users/new')
    await expect(page.getByRole('heading', { name: /Create/i })).toBeVisible({ timeout: 5000 })
    await expect(page.getByRole('button', { name: /Create /i })).toBeVisible({ timeout: 5000 })
  })
})
