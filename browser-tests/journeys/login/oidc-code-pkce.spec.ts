import { expect, test } from '@playwright/test'

import {
  completePasswordLogin,
  withIsolatedPage,
  waitForCallback,
} from '../../support/browser'
import { establishAuthenticatedBrowserSession } from '../../support/journeys/seed'
import { CallbackHarness } from '../../support/callback-harness'
import {
  createAuthorizationRequest,
  exchangeAuthorizationCode,
  fetchOpenIdUserInfo,
} from '../../support/op-client'

const appBaseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
const callbackOrigin = 'http://127.0.0.1:9876'
const redirectUri = `${callbackOrigin}/callback`
const userIdentifier = 'e2e-user@example.com'
const userPassword = 'password123'
const reusableSessionIdentifier = 'admin'
const reusableSessionPassword = 'admin123'

const callbackHarness = new CallbackHarness(callbackOrigin, 9876)

test.describe.serial('OIDC code + PKCE journey', () => {
  test.beforeAll(async () => {
    await callbackHarness.start()
  })

  test.afterAll(async () => {
    await callbackHarness.stop()
  })

  test.beforeEach(() => {
    callbackHarness.reset()
  })

  test('app can complete authorization_code + PKCE and fetch userinfo @smoke', async () => {
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

  test('existing browser session satisfies prompt=none silently', async () => {
    test.setTimeout(60_000)
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page, {
        userIdentifier: reusableSessionIdentifier,
        userPassword: reusableSessionPassword,
      })

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

  test('prompt=none without a browser session returns login_required', async () => {
    await withIsolatedPage(async (page) => {
      callbackHarness.reset()
      const auth = await createAuthorizationRequest({ redirectUri, prompt: 'none' })
      await page.goto(auth.url)

      const callbackURL = await waitForCallback(page, redirectUri, auth.state, () => callbackHarness.lastCallback())
      expect(callbackURL.searchParams.get('error')).toBe('login_required')
      expect(callbackURL.searchParams.get('code')).toBeNull()
    })
  })

  test('prompt=login forces fresh credentials even when a session exists', async () => {
    test.setTimeout(60_000)
    await withIsolatedPage(async (page) => {
      await establishAuthenticatedBrowserSession(page, {
        userIdentifier: reusableSessionIdentifier,
        userPassword: reusableSessionPassword,
      })

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

  test('unregistered redirect_uri is rejected before a code is issued', async () => {
    await withIsolatedPage(async (page) => {
      const auth = await createAuthorizationRequest({ redirectUri: 'http://127.0.0.1:9999/callback' })
      const response = await page.goto(auth.url, { waitUntil: 'domcontentloaded' })

      expect(response).not.toBeNull()
      expect(page.url()).not.toContain(redirectUri)
      expect(callbackHarness.lastCallback()).toBeNull()
      await expect(page.locator('body')).toContainText(/redirect|invalid|error/i)
    })
  })

  test('token exchange rejects the wrong PKCE verifier', async () => {
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

  test('authorization codes can be used only once', async () => {
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

  test('userinfo rejects an invalid bearer token', async () => {
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
