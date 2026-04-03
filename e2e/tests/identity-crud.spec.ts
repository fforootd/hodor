import { test, expect } from '@playwright/test'

import { loginAsAdmin } from '../support/browser'

test.describe('Identity CRUD', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page)
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
