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
  page.on('response', (response) => {
    if (response.status() >= 400) {
      void response
        .text()
        .then((body) => {
          const preview = body.replace(/\s+/g, ' ').trim().slice(0, 240)
          diagnostics.push(
            `response:${response.status()} ${response.request().method()} ${response.url()}${preview ? ` body:${preview}` : ''}`,
          )
        })
        .catch(() => {
          diagnostics.push(`response:${response.status()} ${response.request().method()} ${response.url()}`)
        })
    }
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
  const visibleIdentifierSelector =
    'input[name="identifier"]:visible, input[type="text"]:visible, input[type="email"]:visible'
  const visiblePasswordSelector =
    'input[name="password"]:visible, input[type="password"]:visible'
  const passwordInput = page.locator(visiblePasswordSelector).first()
  const identifierInput = page.locator(visibleIdentifierSelector).first()
  const continueButton = page.locator('button:visible').filter({ hasText: /^Continue$/i }).first()
  const signInButton = page.locator('button:visible').filter({ hasText: /Sign in/i }).first()

  await expect(
    page.locator(`${visibleIdentifierSelector}, ${visiblePasswordSelector}`).first(),
  ).toBeVisible({ timeout: 15_000 })

  if (await identifierInput.isVisible()) {
    await identifierInput.fill(userIdentifier)
    await identifierInput.press('Enter')
    const passwordAppeared = await passwordInput.isVisible().catch(() => false)
    if (
      !passwordAppeared &&
      page.url().includes('/login') &&
      (await continueButton.isVisible().catch(() => false))
    ) {
      await continueButton.click()
    }
  }

  await expect(passwordInput).toBeVisible({ timeout: 15_000 })
  await passwordInput.fill(userPassword)

  if (await signInButton.isVisible().catch(() => false)) {
    await signInButton.click()
  } else {
    await passwordInput.press('Enter')
  }

  // Give the frontend time to process the response and trigger navigation.
  for (let poll = 0; poll < 150; poll += 1) {
    if (!page.url().includes('/login')) {
      return
    }
    if (!(await passwordInput.isVisible().catch(() => false))) {
      return
    }
    await page.waitForTimeout(100)
  }

  const bodyText = await page.locator('body').innerText().catch(() => '')
  const diagnostics = pageDiagnostics.get(page) || []
  throw new Error(
    `Password login did not advance. Current URL: ${page.url()}. Body: ${bodyText.slice(0, 300)}. Diagnostics: ${diagnostics.slice(-10).join(' | ')}`,
  )
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
  const deadline = Date.now() + 15_000

  while (Date.now() < deadline) {
    if (page.url().startsWith(`${redirectUri}?`)) {
      const callbackURL = new URL(page.url())
      expect(callbackURL.searchParams.get('state')).toBe(state)
      return callbackURL
    }

    const harnessCallback = harnessLast()
    if (harnessCallback?.toString().startsWith(`${redirectUri}?`)) {
      expect(harnessCallback.searchParams.get('state')).toBe(state)
      return harnessCallback
    }

    await page.waitForTimeout(100)
  }

  const bodyText = await page.locator('body').innerText().catch(() => '')
  const diagnostics = pageDiagnostics.get(page) || []
  throw new Error(
    `OIDC flow did not reach callback. Current URL: ${page.url()}. Body: ${bodyText.slice(0, 300)}. Diagnostics: ${diagnostics.slice(-10).join(' | ')}`,
  )
}
