import { expect, type Page } from '@playwright/test'

import { browserJSON } from '../browser'

export async function visitLogin(page: Page) {
  await page.goto('/login')
  await expect(
    page.locator('input[name="identifier"], input[type="text"], input[type="email"]').first(),
  ).toBeVisible({ timeout: 15_000 })
}

export async function submitIdentifierStep(page: Page, identifier: string) {
  const identifierInput = page
    .locator('input[name="identifier"], input[type="text"], input[type="email"]')
    .first()
  await expect(identifierInput).toBeVisible({ timeout: 15_000 })
  await identifierInput.fill(identifier)
  const continueButton = page.locator('button:visible').filter({ hasText: /^Continue$/i }).first()
  if (await continueButton.isVisible().catch(() => false)) {
    await continueButton.click()
  } else {
    await identifierInput.press('Enter')
  }
  await expect(
    page.locator('input[name="password"], input[type="password"]').first(),
  ).toBeVisible({ timeout: 15_000 })
}

export async function submitPasswordStep(page: Page, password: string) {
  const passwordInput = page.locator('input[name="password"], input[type="password"]').first()
  await expect(passwordInput).toBeVisible({ timeout: 15_000 })
  await passwordInput.fill(password)
  const signInButton = page.locator('button:visible').filter({ hasText: /^Sign in$/i }).first()
  if (await signInButton.isVisible().catch(() => false)) {
    await signInButton.click()
  } else {
    await passwordInput.press('Enter')
  }
}

export async function continueWithExistingSession(page: Page) {
  await expect(
    page.getByRole('heading', { name: /Use your existing session\?/i }),
  ).toBeVisible({ timeout: 15_000 })
  await page.getByRole('button', { name: /Continue with this session/i }).click()
  await expect.poll(() => page.url(), { timeout: 15_000 }).toMatch(/\/console\/?/)
}

async function maybeContinueExistingSession(page: Page) {
  const heading = page.getByRole('heading', { name: /Use your existing session\?/i })
  if (!(await heading.isVisible().catch(() => false))) {
    return false
  }
  await page.getByRole('button', { name: /Continue with this session/i }).click()
  return true
}

async function isConsoleShellVisible(page: Page) {
  return (
    (await page.getByRole('button', { name: /Search/i }).isVisible().catch(() => false))
    || (await page.getByRole('button', { name: /ZA Admin admin@localhost/i }).isVisible().catch(
      () => false,
    ))
  )
}

export async function revokeCurrentSession(page: Page) {
  const sessions = await browserJSON(page, '/v1/account/sessions')
  if (sessions.status !== 200 || !Array.isArray((sessions.body as any)?.sessions)) {
    throw new Error(`Unable to load current account sessions: ${JSON.stringify(sessions)}`)
  }

  const currentSession = (sessions.body as any).sessions.find(
    (session: { current?: boolean; id?: string }) => session.current && session.id,
  )
  if (!currentSession?.id) {
    throw new Error(`No current session returned from account API: ${JSON.stringify(sessions.body)}`)
  }

  const revokeResult = await page.evaluate(async (sessionId) => {
    const response = await fetch(`/v1/account/sessions/${sessionId}/revoke`, {
      method: 'POST',
      credentials: 'include',
      headers: {
        Accept: 'application/json',
      },
    })
    return { status: response.status }
  }, currentSession.id)

  if (revokeResult.status !== 204) {
    throw new Error(`Unable to revoke current session, got status ${revokeResult.status}`)
  }
}

export async function loginAsAdmin(
  page: Page,
  identifier = 'admin',
  password = 'admin123',
) {
  if (page.url().includes('/console') && (await isConsoleShellVisible(page))) {
    return
  }

  await page.goto('/login?redirect_uri=%2Fconsole')
  await expect(page).toHaveURL(/\/login/)
  if (await maybeContinueExistingSession(page)) {
    await expect.poll(() => page.url(), { timeout: 15_000 }).toMatch(/\/console\/?/)
    return
  }

  await submitIdentifierStep(page, identifier)
  await submitPasswordStep(page, password)

  if (await maybeContinueExistingSession(page)) {
    await expect.poll(() => page.url(), { timeout: 15_000 }).toMatch(/\/console\/?/)
    return
  }

  await expect.poll(async () => {
    if (page.url().includes('/console') && (await isConsoleShellVisible(page))) {
      return page.url()
    }
    return page.url()
  }, { timeout: 15_000 }).toMatch(/\/console\/?/)
}
