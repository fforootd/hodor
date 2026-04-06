import { expect, test } from '@playwright/test'

import {
  loginAsAdmin,
  submitIdentifierStep,
  submitPasswordStep,
  visitLogin,
} from '../../support/journeys/browser-login'

test.describe('Password login journey', () => {
  test('visitor can reach the login form', async ({ page }) => {
    await visitLogin(page)
  })

  test('identifier step advances to password', async ({ page }) => {
    await visitLogin(page)
    await submitIdentifierStep(page, 'admin')
    await expect(page.locator('input[name="password"], input[type="password"]').first()).toBeVisible()
  })

  test('admin can sign in and land on the console @smoke', async ({ page }) => {
    await loginAsAdmin(page, 'admin', 'admin123')
    await expect(page).toHaveURL(/\/console/)
  })

  test('signing out from the console returns to the login flow', async ({ page }) => {
    await loginAsAdmin(page, 'e2e-user@example.com', 'password123')
    await page.goto('/logout')
    await expect(page).toHaveURL(/\/login/)
  })

  test('existing sessions open a second console page without signing in again', async ({ page }) => {
    await loginAsAdmin(page, 'reviewer@example.com', 'password123')
    const secondPage = await page.context().newPage()
    try {
      await secondPage.goto('/console')
      await expect(secondPage).toHaveURL(/\/console/)
    } finally {
      await secondPage.close()
    }
  })

  test('invalid password keeps the visitor in the login flow', async ({ page }) => {
    await visitLogin(page)
    await submitIdentifierStep(page, 'admin')
    await submitPasswordStep(page, 'wrong-password')
    await expect(page).toHaveURL(/\/login/)
    await expect(page.locator('input[name="password"], input[type="password"]').first()).toBeVisible()
  })
})
