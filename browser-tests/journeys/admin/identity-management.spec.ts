import { test, expect } from '@playwright/test'

import { loginAsAdmin } from '../../support/journeys/browser-login'

test.describe('Admin identity management journey', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page)
  })

  test('admin can open the identity list', async ({ page }) => {
    await page.goto('/console/users')
    await expect(page.locator('table, .table-wrap')).toBeVisible({ timeout: 5000 })
  })

  test('admin can open the create identity form', async ({ page }) => {
    await page.goto('/console/users/new')
    await expect(page.getByRole('heading', { name: /Create/i })).toBeVisible({ timeout: 5000 })
    await expect(page.getByRole('button', { name: /Create /i })).toBeVisible({ timeout: 5000 })
  })
})
