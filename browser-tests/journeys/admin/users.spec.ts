import { expect, test } from '@playwright/test'

import { loginAsAdmin } from '../../support/journeys/browser-login'

test.describe('Admin users journey', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page, 'admin-users', 'admin123')
  })

  test('admin can create and delete a user while the detail cockpit exposes the current edit boundary @smoke', async ({ page }) => {
    const suffix = `${Date.now()}`
    const identifier = `journey-user-${suffix}@example.com`
    const displayName = `Journey User ${suffix}`

    await page.goto('/console/users/new')
    await page.getByLabel(/Email or username/i).fill(identifier)
    await page.getByLabel(/Display name/i).fill(displayName)
    await page.getByLabel(/^Password/i).fill('password123')
    await page.getByRole('button', { name: /Create User/i }).click()

    await expect(page).toHaveURL(/\/console\/users\/.+/)
    await expect(page.getByRole('heading', { name: displayName })).toBeVisible({ timeout: 15_000 })

    await page.getByTestId('edit-user').click()
    await expect(page.getByTestId('edit-api-section')).toBeVisible({ timeout: 15_000 })
    await expect(page.getByText(/No schema fields available/i)).toBeVisible({ timeout: 15_000 })

    await page.getByTestId('delete-user').click()
    await page.getByTestId('confirm-delete-user').click()
    await expect(page).toHaveURL(/\/console\/users\/?$/)
    await expect(page.getByText(displayName)).toHaveCount(0)
  })
})
