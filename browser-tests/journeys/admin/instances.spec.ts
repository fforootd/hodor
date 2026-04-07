import { expect, test } from '@playwright/test'

import { loginAsAdmin } from '../../support/journeys/browser-login'

test.describe('Admin instances journey', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page, 'admin-instances', 'admin123')
  })

  test('admin can create an instance and enter its scoped console boundary @smoke', async ({ page }) => {
    const suffix = `${Date.now()}`
    const instanceId = `journey-${suffix}`
    const domain = `${instanceId}.zitadel.cloud`

    await page.goto('/console/instances')
    await page.getByRole('button', { name: /New Instance/i }).click()
    // Wizard step 0: fill instance details
    await page.getByLabel(/Instance Name/i).fill(instanceId)
    await page.getByLabel(/Subdomain/i).fill(domain)
    await page.getByRole('button', { name: /Next/i }).click()
    // Wizard step 1: confirm
    await page.getByRole('button', { name: /Create Instance/i }).click()

    await expect(page).toHaveURL(/\/console\/instances\/?$/)
    const instanceLink = page.getByRole('link', { name: domain }).first()
    await expect(instanceLink).toBeVisible({ timeout: 15_000 })

    const href = await instanceLink.getAttribute('href')
    expect(href).toMatch(/^\/console\/instances\/[^/]+$/)

    await instanceLink.click()
    await expect(page).toHaveURL(new RegExp(`${href}$`))
    await expect(page.getByRole('button', { name: domain })).toBeVisible({ timeout: 15_000 })
    await expect(page.getByRole('link', { name: /Users/i })).toBeVisible({ timeout: 15_000 })
    await expect(page.getByRole('link', { name: /Applications/i })).toBeVisible({
      timeout: 15_000,
    })
  })
})
