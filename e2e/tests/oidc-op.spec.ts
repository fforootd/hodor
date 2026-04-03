import { expect, test, type Page } from '@playwright/test'

import {
  completePasswordLogin,
  detectInternalOidcEntryState,
  waitForCallback,
  withIsolatedPage,
} from '../support/browser'
import { CallbackHarness } from '../support/callback-harness'
import {
  createAuthorizationRequest,
  exchangeAuthorizationCode,
  fetchOpenIdUserInfo,
} from '../support/op-client'

const appBaseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
const callbackOrigin = 'http://127.0.0.1:9876'
const redirectUri = `${callbackOrigin}/callback`
const userIdentifier = 'e2e-user@example.com'
const userPassword = 'password123'

const callbackHarness = new CallbackHarness(callbackOrigin, 9876)

async function establishAuthenticatedBrowserSession(page: Page) {
  callbackHarness.reset()
  const auth = await createAuthorizationRequest({ redirectUri })
  await page.goto(auth.url)

  const entryState = await detectInternalOidcEntryState(page, redirectUri)
  if (entryState === 'login') {
    await completePasswordLogin(page, userIdentifier, userPassword)
  } else if (entryState === 'session_reuse') {
    await page.getByRole('button', { name: /Continue with this session/i }).click()
  }

  const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
  const exchanged = await exchangeAuthorizationCode(
    callbackURL,
    redirectUri,
    auth.state,
    auth.nonce,
    auth.codeVerifier,
  )

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

  test('authorization_code + PKCE happy path returns code, tokens, and userinfo @smoke', async () => {
    await withIsolatedPage(async (page) => {
      const auth = await createAuthorizationRequest({ redirectUri })

      await page.goto(auth.url)
      await completePasswordLogin(page, userIdentifier, userPassword)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      const exchanged = await exchangeAuthorizationCode(
        callbackURL,
        redirectUri,
        auth.state,
        auth.nonce,
        auth.codeVerifier,
      )

      expect(exchanged.access_token).toBeTruthy()
      expect(exchanged.id_token).toBeTruthy()
      expect(String(exchanged.token_type).toLowerCase()).toBe('bearer')
      expect(exchanged.claims()?.email).toBe(userIdentifier)

      const userinfo = await fetchOpenIdUserInfo(String(exchanged.access_token || ''))
      expect(userinfo.email).toBe(userIdentifier)
      expect(userinfo.email_verified).toBe(true)
    })
  })

  test('reuses an existing session after explicit confirmation @full', async () => {
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page)

      callbackHarness.reset()
      const auth = await createAuthorizationRequest({ redirectUri })
      await page.goto(auth.url)

      await expect(page.getByRole('heading', { name: /Use your existing session\?/i })).toBeVisible()
      await page.getByRole('button', { name: /Continue with this session/i }).click()

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      const exchanged = await exchangeAuthorizationCode(
        callbackURL,
        redirectUri,
        auth.state,
        auth.nonce,
        auth.codeVerifier,
      )
      expect(exchanged.access_token).toBeTruthy()
    })
  })

  test('prompt=none reuses an existing session silently @full', async () => {
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page)

      callbackHarness.reset()
      const auth = await createAuthorizationRequest({ redirectUri, prompt: 'none' })
      await page.goto(auth.url)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      const exchanged = await exchangeAuthorizationCode(
        callbackURL,
        redirectUri,
        auth.state,
        auth.nonce,
        auth.codeVerifier,
      )
      expect(exchanged.access_token).toBeTruthy()
    })
  })

  test('prompt=none without a session returns login_required @full', async () => {
    await withIsolatedPage(async (page) => {
      callbackHarness.reset()
      const auth = await createAuthorizationRequest({ redirectUri, prompt: 'none' })
      await page.goto(auth.url)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      expect(callbackURL.searchParams.get('error')).toBe('login_required')
      expect(callbackURL.searchParams.get('code')).toBeNull()
    })
  })

  test('prompt=login disables session reuse and requires credentials again @full', async () => {
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page)

      callbackHarness.reset()
      const auth = await createAuthorizationRequest({ redirectUri, prompt: 'login' })
      await page.goto(auth.url)

      await expect(page.getByRole('heading', { name: /Use your existing session\?/i })).toHaveCount(0)
      await expect(
        page.locator('input[name="identifier"], input[type="text"], input[type="email"]').first(),
      ).toBeVisible({ timeout: 15_000 })

      await completePasswordLogin(page, userIdentifier, userPassword)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      const exchanged = await exchangeAuthorizationCode(
        callbackURL,
        redirectUri,
        auth.state,
        auth.nonce,
        auth.codeVerifier,
      )
      expect(exchanged.access_token).toBeTruthy()
    })
  })

  test('rejects an unregistered redirect_uri before issuing a code @full', async () => {
    await withIsolatedPage(async (page) => {
      const auth = await createAuthorizationRequest({ redirectUri: 'http://127.0.0.1:9999/callback' })
      const response = await page.goto(auth.url, { waitUntil: 'domcontentloaded' })

      expect(response).not.toBeNull()
      expect(page.url()).not.toContain(redirectUri)
      expect(callbackHarness.lastCallback()).toBeNull()
      await expect(page.locator('body')).toContainText(/redirect|invalid|error/i)
    })
  })

  test('rejects token exchange when the PKCE verifier is wrong @full', async () => {
    await withIsolatedPage(async (page) => {
      const auth = await createAuthorizationRequest({ redirectUri })

      await page.goto(auth.url)
      await completePasswordLogin(page, userIdentifier, userPassword)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())

      await expect(
        exchangeAuthorizationCode(
          callbackURL,
          redirectUri,
          auth.state,
          auth.nonce,
          `${auth.codeVerifier}-wrong`,
        ),
      ).rejects.toThrow()
    })
  })

  test('authorization codes are one-time use @full', async () => {
    await withIsolatedPage(async (page) => {
      const auth = await createAuthorizationRequest({ redirectUri })

      await page.goto(auth.url)
      await completePasswordLogin(page, userIdentifier, userPassword)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      const exchanged = await exchangeAuthorizationCode(
        callbackURL,
        redirectUri,
        auth.state,
        auth.nonce,
        auth.codeVerifier,
      )
      expect(exchanged.access_token).toBeTruthy()

      await expect(
        exchangeAuthorizationCode(
          callbackURL,
          redirectUri,
          auth.state,
          auth.nonce,
          auth.codeVerifier,
        ),
      ).rejects.toThrow()
    })
  })

  test('userinfo rejects an invalid bearer token @full', async () => {
    const response = await fetch(new URL('/userinfo', appBaseURL), {
      headers: {
        Authorization: 'Bearer invalid-token',
      },
    })

    expect(response.ok).toBeFalsy()
    expect(response.status).toBeGreaterThanOrEqual(400)
    expect(response.status).toBeLessThan(500)
  })
})
