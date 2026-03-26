import { test, expect } from '@playwright/test'

test.describe('Identity CRUD', () => {
  test.beforeEach(async ({ page }) => {
    // Login as admin first.
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'admin@zitadel.local')
    await page.click('button[type="submit"]')
    await page.waitForURL(/\/(console|admin)/, { timeout: 10000 })
  })

  test('identity list page loads', async ({ page }) => {
    await page.goto('/console#/identities')
    await expect(page.locator('table, .table-wrap')).toBeVisible({ timeout: 5000 })
  })

  test('create identity form is accessible', async ({ page }) => {
    await page.goto('/console#/identities/new')
    await expect(page.locator('input, form')).toBeVisible({ timeout: 5000 })
  })
})
