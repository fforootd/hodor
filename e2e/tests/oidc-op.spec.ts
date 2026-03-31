import { createHash, randomBytes, randomUUID } from 'node:crypto'
import { once } from 'node:events'
import { createServer, type Server } from 'node:http'

import { chromium, expect, test, type APIRequestContext, type Page } from '@playwright/test'

const clientId = 'e2e-browser-client'
const clientSecret = 'e2e-browser-secret'
const appBaseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
const callbackOrigin = 'http://127.0.0.1:9876'
const redirectUri = `${callbackOrigin}/callback`
const userIdentifier = 'e2e-user@example.com'
const userPassword = 'password123'
const pageDiagnostics = new WeakMap<Page, string[]>()

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function createPKCE() {
  const verifier = randomBytes(32).toString('base64url')
  const challenge = createHash('sha256').update(verifier).digest('base64url')
  return { verifier, challenge }
}

function buildAuthorizeURL(options?: { prompt?: string; redirectURI?: string }) {
  const pkce = createPKCE()
  const state = randomUUID()
  const nonce = randomUUID()
  const url = new URL('/authorize', appBaseURL)
  url.searchParams.set('client_id', clientId)
  url.searchParams.set('redirect_uri', options?.redirectURI || redirectUri)
  url.searchParams.set('response_type', 'code')
  url.searchParams.set('scope', 'openid profile email')
  url.searchParams.set('state', state)
  url.searchParams.set('nonce', nonce)
  url.searchParams.set('code_challenge', pkce.challenge)
  url.searchParams.set('code_challenge_method', 'S256')
  if (options?.prompt) {
    url.searchParams.set('prompt', options.prompt)
  }
  return { url: url.toString(), state, nonce, pkce }
}

async function readJSON(response: Awaited<ReturnType<APIRequestContext['post']>>) {
  const contentType = response.headers()['content-type'] || ''
  if (!contentType.includes('application/json')) {
    return null
  }
  return response.json()
}

async function exchangeAuthorizationCode(
  request: APIRequestContext,
  code: string,
  codeVerifier: string,
  redirectURI = redirectUri,
) {
  const response = await request.post('/oauth/token', {
    failOnStatusCode: false,
    form: {
      grant_type: 'authorization_code',
      code,
      client_id: clientId,
      client_secret: clientSecret,
      redirect_uri: redirectURI,
      code_verifier: codeVerifier,
    },
  })

  return {
    response,
    body: await readJSON(response),
  }
}

async function fetchUserInfo(request: APIRequestContext, accessToken: string) {
  const response = await request.get('/userinfo', {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  })

  return {
    response,
    body: await response.json(),
  }
}

async function completePasswordLogin(page: Page) {
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

async function detectOIDCEntryState(page: Page) {
  const loginInput = page
    .locator(
      'input[name="identifier"], input[type="text"], input[type="email"], input[name="password"], input[type="password"]',
    )
    .first()
  const sessionReuseHeading = page.getByRole('heading', { name: /Use your existing session\?/i })

  for (let attempt = 0; attempt < 75; attempt += 1) {
    if (page.url().startsWith(redirectUri)) {
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

class CallbackHarness {
  private server: Server | null = null
  private lastURL: string | null = null

  async start() {
    if (this.server) return

    this.server = createServer((req, res) => {
      const url = new URL(req.url || '/', callbackOrigin)

      if (url.pathname === '/callback') {
        this.lastURL = url.toString()
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        res.end('<html><body>OIDC callback received</body></html>')
        return
      }

      if (url.pathname === '/logout') {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        res.end('<html><body>OIDC logout received</body></html>')
        return
      }

      if (url.pathname === '/healthz') {
        res.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' })
        res.end('ok')
        return
      }

      res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' })
      res.end('not found')
    })

    this.server.listen(9876, '127.0.0.1')
    await once(this.server, 'listening')
  }

  reset() {
    this.lastURL = null
  }

  lastCallback(): URL | null {
    return this.lastURL ? new URL(this.lastURL) : null
  }

  async stop() {
    if (!this.server) return
    const server = this.server
    this.server = null
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error) {
          reject(error)
          return
        }
        resolve()
      })
    })
  }
}

const callbackHarness = new CallbackHarness()

async function waitForCallback(page: Page, state: string) {
  await page.waitForURL(new RegExp(`^${escapeRegex(redirectUri)}\\?`), { timeout: 15_000 })

  const callbackURL = new URL(page.url())
  expect(callbackURL.searchParams.get('state')).toBe(state)

  const harnessCallback = callbackHarness.lastCallback()
  expect(harnessCallback?.searchParams.get('state')).toBe(state)

  return callbackURL
}

async function withIsolatedPage(run: (page: Page) => Promise<void>) {
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

async function establishAuthenticatedBrowserSession(page: Page, request: APIRequestContext) {
  callbackHarness.reset()
  const auth = buildAuthorizeURL()
  await page.goto(auth.url)

  const entryState = await detectOIDCEntryState(page)
  if (entryState === 'login') {
    await completePasswordLogin(page)
  } else if (entryState === 'session_reuse') {
    await page.getByRole('button', { name: /Continue with this session/i }).click()
  }

  const callbackURL = await waitForCallback(page, auth.state)
  const code = callbackURL.searchParams.get('code')
  expect(code).toBeTruthy()

  const exchanged = await exchangeAuthorizationCode(request, code || '', auth.pkce.verifier)
  expect(exchanged.response.ok()).toBeTruthy()

  return { auth, callbackURL, exchanged }
}

test.describe.serial('OIDC OP end-to-end', () => {
  test.beforeAll(async () => {
    await callbackHarness.start()
  })

  test.afterAll(async () => {
    await callbackHarness.stop()
  })

  test.beforeEach(() => {
    callbackHarness.reset()
  })

  test('authorization_code + PKCE happy path returns code, tokens, and userinfo @smoke', async ({
    request,
  }) => {
    await withIsolatedPage(async (page) => {
      const auth = buildAuthorizeURL()

      await page.goto(auth.url)
      await completePasswordLogin(page)

      const callbackURL = await waitForCallback(page, auth.state)
      const code = callbackURL.searchParams.get('code')
      expect(code).toBeTruthy()

      const exchanged = await exchangeAuthorizationCode(request, code || '', auth.pkce.verifier)
      expect(exchanged.response.ok()).toBeTruthy()
      expect(exchanged.body?.access_token).toBeTruthy()
      expect(exchanged.body?.id_token).toBeTruthy()
      expect(exchanged.body?.token_type).toBe('Bearer')

      const userinfo = await fetchUserInfo(request, String(exchanged.body?.access_token || ''))
      expect(userinfo.response.ok()).toBeTruthy()
      expect(userinfo.body.email).toBe(userIdentifier)
      expect(userinfo.body.email_verified).toBe(true)
    })
  })

  test('reuses an existing session after explicit confirmation @full', async ({ request }) => {
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page, request)

      callbackHarness.reset()
      const auth = buildAuthorizeURL()
      await page.goto(auth.url)

      await expect(page.getByRole('heading', { name: /Use your existing session\?/i })).toBeVisible()
      await page.getByRole('button', { name: /Continue with this session/i }).click()

      const callbackURL = await waitForCallback(page, auth.state)
      const code = callbackURL.searchParams.get('code')
      expect(code).toBeTruthy()

      const exchanged = await exchangeAuthorizationCode(request, code || '', auth.pkce.verifier)
      expect(exchanged.response.ok()).toBeTruthy()
    })
  })

  test('prompt=none reuses an existing session silently @full', async ({ request }) => {
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page, request)

      callbackHarness.reset()
      const auth = buildAuthorizeURL({ prompt: 'none' })
      await page.goto(auth.url)

      const callbackURL = await waitForCallback(page, auth.state)
      const code = callbackURL.searchParams.get('code')
      expect(code).toBeTruthy()

      const exchanged = await exchangeAuthorizationCode(request, code || '', auth.pkce.verifier)
      expect(exchanged.response.ok()).toBeTruthy()
    })
  })

  test('prompt=login disables session reuse and requires credentials again @full', async ({ request }) => {
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page, request)

      callbackHarness.reset()
      const auth = buildAuthorizeURL({ prompt: 'login' })
      await page.goto(auth.url)

      await expect(page.getByRole('heading', { name: /Use your existing session\?/i })).toHaveCount(0)
      await expect(
        page.locator('input[name="identifier"], input[type="text"], input[type="email"]').first(),
      ).toBeVisible({ timeout: 15_000 })

      await completePasswordLogin(page)

      const callbackURL = await waitForCallback(page, auth.state)
      const code = callbackURL.searchParams.get('code')
      expect(code).toBeTruthy()

      const exchanged = await exchangeAuthorizationCode(request, code || '', auth.pkce.verifier)
      expect(exchanged.response.ok()).toBeTruthy()
    })
  })

  test('rejects an unregistered redirect_uri before issuing a code @full', async () => {
    await withIsolatedPage(async (page) => {
      const auth = buildAuthorizeURL({ redirectURI: 'http://127.0.0.1:9999/callback' })
      const response = await page.goto(auth.url, { waitUntil: 'domcontentloaded' })

      expect(response).not.toBeNull()
      expect(page.url()).not.toContain(redirectUri)
      expect(callbackHarness.lastCallback()).toBeNull()
      await expect(page.locator('body')).toContainText(/redirect|invalid|error/i)
    })
  })

  test('rejects token exchange when the PKCE verifier is wrong @full', async ({ request }) => {
    await withIsolatedPage(async (page) => {
      const auth = buildAuthorizeURL()

      await page.goto(auth.url)
      await completePasswordLogin(page)

      const callbackURL = await waitForCallback(page, auth.state)
      const code = callbackURL.searchParams.get('code')
      expect(code).toBeTruthy()

      const exchanged = await exchangeAuthorizationCode(
        request,
        code || '',
        `${auth.pkce.verifier}-wrong`,
      )
      expect(exchanged.response.ok()).toBeFalsy()
      expect(exchanged.body?.error).toBeTruthy()
    })
  })
})
