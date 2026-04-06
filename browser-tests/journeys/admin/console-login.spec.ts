import { test, expect } from '@playwright/test'

import { loginAsAdmin } from '../../support/journeys/browser-login'

test.describe('Admin console login journey', () => {
  test('visitor can reach the login form', async ({ page }) => {
    await page.goto('/login')
    await expect(page.locator('input[name="identifier"], input[type="text"]')).toBeVisible()
  })

  test('invalid credentials keep the visitor on the login journey', async ({ page }) => {
    await page.goto('/login')
    await page.fill('input[name="identifier"], input[type="text"]', 'bad@user.com')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(/\/login/)
  })

  test('admin can sign in and land on the console @smoke', async ({ page }) => {
    await loginAsAdmin(page)
    await expect(page).toHaveURL(/\/console/)
  })
})
