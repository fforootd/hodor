import { expect, type Locator, type Page } from '@playwright/test'

import { browserJSON } from '../browser'

async function loginPageDiagnostics(page: Page) {
  const [title, bodyText, headings, buttons] = await Promise.all([
    page.title().catch(() => ''),
    page.locator('body').innerText().catch(() => ''),
    page
      .locator('h1, h2, h3, [role="heading"]')
      .evaluateAll((nodes) =>
        nodes
          .map((node) => node.textContent?.trim() || '')
          .filter(Boolean)
          .slice(0, 8),
      )
      .catch(() => [] as string[]),
    page
      .locator('button, [role="button"]')
      .evaluateAll((nodes) =>
        nodes
          .map((node) => node.textContent?.trim() || '')
          .filter(Boolean)
          .slice(0, 8),
      )
      .catch(() => [] as string[]),
  ])

  return [
    `URL: ${page.url()}`,
    title ? `Title: ${title}` : '',
    headings.length > 0 ? `Headings: ${headings.join(' | ')}` : '',
    buttons.length > 0 ? `Buttons: ${buttons.join(' | ')}` : '',
    `Body: ${bodyText.slice(0, 800)}`,
  ]
    .filter(Boolean)
    .join('\n')
}

async function expectVisibleWithDiagnostics(page: Page, locator: Locator, target: string) {
  try {
    await expect(locator).toBeVisible({ timeout: 15_000 })
  } catch (error) {
    const details = await loginPageDiagnostics(page)
    const reason = error instanceof Error ? error.message : String(error)
    throw new Error(`${target} did not appear on the login page.\n${details}\nCause: ${reason}`)
  }
}

export async function visitLogin(page: Page) {
  await page.goto('/login')
  await expectVisibleWithDiagnostics(
    page,
    page.locator('input[name="identifier"], input[type="text"], input[type="email"]').first(),
    'identifier input',
  )
}

export async function submitIdentifierStep(page: Page, identifier: string) {
  const identifierInput = page
    .locator('input[name="identifier"], input[type="text"], input[type="email"]')
    .first()
  await expectVisibleWithDiagnostics(page, identifierInput, 'identifier input')
  await identifierInput.fill(identifier)
  const continueButton = page.locator('button:visible').filter({ hasText: /^Continue$/i }).first()
  if (await continueButton.isVisible().catch(() => false)) {
    await continueButton.click()
  } else {
    await identifierInput.press('Enter')
  }
  await expectVisibleWithDiagnostics(
    page,
    page.locator('input[name="password"], input[type="password"]').first(),
    'password input',
  )
}

export async function submitPasswordStep(page: Page, password: string) {
  const passwordInput = page.locator('input[name="password"], input[type="password"]').first()
  await expectVisibleWithDiagnostics(page, passwordInput, 'password input')
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
