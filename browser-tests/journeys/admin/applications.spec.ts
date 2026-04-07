import { expect, test } from '@playwright/test'

import {
  deleteCockpitResource,
  openResourceEditTab,
} from '../../support/journeys/admin-resources'
import { loginAsAdmin } from '../../support/journeys/browser-login'

test.describe('Admin applications journey', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page, 'admin-apps', 'admin123')
  })

  test('admin can create and delete an application while exposing the current edit boundary @smoke', async ({ page }) => {
    const suffix = `${Date.now()}`
    const name = `Journey Application ${suffix}`

    await page.goto('/console/applications')
    await page.getByRole('button', { name: /New Application/i }).click()
    // Wizard step 0: select application type (default: web) → next
    await page.getByRole('button', { name: /Next/i }).click()
    // Wizard step 1: fill name
    await page.getByLabel(/Application Name/i).fill(name)
    await page.getByRole('button', { name: /Next/i }).click()
    // Wizard step 2: confirm
    await page.getByRole('button', { name: /Create Application/i }).click()

    await expect(page).toHaveURL(/\/console\/applications\/.+/)
    await expect(page.getByRole('heading', { name })).toBeVisible({ timeout: 15_000 })

    await openResourceEditTab(page)
    await expect(page.getByRole('button', { name: /Save changes/i })).toBeVisible({
      timeout: 15_000,
    })

    await deleteCockpitResource(page, 'Application')
    await expect(page).toHaveURL(/\/console\/applications\/?$/)
    await expect(page.getByText(name)).toHaveCount(0)
  })
})
