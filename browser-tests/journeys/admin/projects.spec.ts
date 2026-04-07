import { expect, test } from '@playwright/test'

import {
  deleteCockpitResource,
  openResourceEditTab,
} from '../../support/journeys/admin-resources'
import { loginAsAdmin } from '../../support/journeys/browser-login'

test.describe('Admin projects journey', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page, 'admin-projects', 'admin123')
  })

  test('admin can create and delete a project while exposing the current edit boundary @smoke', async ({ page }) => {
    const suffix = `${Date.now()}`
    const name = `Journey Project ${suffix}`

    await page.goto('/console/projects/new')
    // Step 0: Project details
    await page.getByPlaceholder('My Project').fill(name)
    await page.getByRole('button', { name: /Continue/i }).click()
    // Step 1: Confirmation — submit
    await page.getByRole('button', { name: /Create Project/i }).click()

    await expect(page).toHaveURL(/\/console\/projects\/.+/)
    await expect(page.getByRole('heading', { name })).toBeVisible({ timeout: 15_000 })

    await openResourceEditTab(page)
    await expect(page.getByRole('button', { name: /Save changes/i })).toBeVisible({
      timeout: 15_000,
    })

    await deleteCockpitResource(page, 'Project')
    await expect(page).toHaveURL(/\/console\/projects\/?$/)
    await expect(page.getByText(name)).toHaveCount(0)
  })
})
