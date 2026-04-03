import { chromium, expect, type Page } from '@playwright/test'

const pageDiagnostics = new WeakMap<Page, string[]>()

export function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

export async function withIsolatedPage(run: (page: Page) => Promise<void>) {
  const browser = await chromium.launch()
  const context = await browser.newContext()
  const page = await context.newPage()
  const diagnostics: string[] = []
  pageDiagnostics.set(page, diagnostics)
  page.on('console', (message) => {
    diagnostics.push(`console:${message.type()}:${message.text()}`)
  })
  page.on('pageerror', (error) => {
    diagnostics.push(`pageerror:${error.message}`)
  })
  page.on('requestfailed', (request) => {
    diagnostics.push(`requestfailed:${request.method()} ${request.url()} ${request.failure()?.errorText || 'unknown'}`)
  })

  try {
    await run(page)
  } finally {
    pageDiagnostics.delete(page)
    await context.close()
    await browser.close()
  }
}

export async function browserJSON(page: Page, path: string) {
  return page.evaluate(async (target) => {
    const response = await fetch(target, {
      credentials: 'include',
      headers: {
        Accept: 'application/json',
      },
    })
    const contentType = response.headers.get('content-type') || ''
    const body = contentType.includes('application/json')
      ? await response.json()
      : await response.text()
    return { status: response.status, body }
  }, path)
}

export async function completePasswordLogin(
  page: Page,
  userIdentifier: string,
  userPassword: string,
) {
  const passwordInput = page.locator('input[name="password"], input[type="password"]').first()
  const identifierInput = page
    .locator('input[name="identifier"], input[type="text"], input[type="email"]')
    .first()

  await expect(
    page
      .locator(
        'input[name="identifier"], input[type="text"], input[type="email"], input[name="password"], input[type="password"]',
      )
      .first(),
  ).toBeVisible({ timeout: 15_000 })

  if (await identifierInput.isVisible()) {
    await identifierInput.fill(userIdentifier)
    await page.locator('button[type="submit"]').click()
  }

  await expect(passwordInput).toBeVisible({ timeout: 15_000 })
  await passwordInput.fill(userPassword)
  await page.locator('button[type="submit"]').click()
}

export async function loginAsAdmin(page: Page) {
  await page.goto('/login')
  await completePasswordLogin(page, 'admin', 'admin123')
  await page.waitForURL(/\/console/, { timeout: 15_000 })
}

export async function completeMockOidcLogin(page: Page, password: string, email?: string) {
  const emailInput = page.locator('input[name="email"]').first()
  try {
    await expect(emailInput).toBeVisible({
      timeout: 15_000,
    })
  } catch (error) {
    const bodyText = await page.locator('body').innerText().catch(() => '')
    throw new Error(
      `Mock OIDC login UI did not appear. Current URL: ${page.url()}. Body: ${bodyText.slice(0, 400)}`,
    )
  }
  if (email) {
    await emailInput.fill(email)
  }
  await page.locator('input[name="password"]').fill(password)
  await page.getByRole('button', { name: /Sign in/i }).click()
}

export async function detectInternalOidcEntryState(page: Page, callbackUrlPrefix: string) {
  const loginInput = page
    .locator(
      'input[name="identifier"], input[type="text"], input[type="email"], input[name="password"], input[type="password"]',
    )
    .first()
  const sessionReuseHeading = page.getByRole('heading', { name: /Use your existing session\?/i })

  for (let attempt = 0; attempt < 75; attempt += 1) {
    if (page.url().startsWith(callbackUrlPrefix)) {
      return 'callback' as const
    }
    if (await sessionReuseHeading.isVisible().catch(() => false)) {
      return 'session_reuse' as const
    }
    if (await loginInput.isVisible().catch(() => false)) {
      return 'login' as const
    }
    await page.waitForTimeout(200)
  }

  const bodyText = await page.locator('body').innerText().catch(() => '')
  const diagnostics = pageDiagnostics.get(page) || []
  throw new Error(
    `OIDC flow did not reach login, session reuse, or callback. Current URL: ${page.url()}. Body: ${bodyText.slice(0, 300)}. Diagnostics: ${diagnostics.slice(-10).join(' | ')}`,
  )
}

export async function waitForCallback(page: Page, redirectUri: string, state: string, harnessLast: () => URL | null) {
  await page.waitForURL(new RegExp(`^${escapeRegex(redirectUri)}\\?`), { timeout: 15_000 })

  const callbackURL = new URL(page.url())
  expect(callbackURL.searchParams.get('state')).toBe(state)

  const harnessCallback = harnessLast()
  expect(harnessCallback?.searchParams.get('state')).toBe(state)

  return callbackURL
}
