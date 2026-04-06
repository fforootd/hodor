import { expect, test, type Page } from '@playwright/test'

import {
  browserJSON,
  completeMockOidcLogin,
  detectInternalOidcEntryState,
  escapeRegex,
  waitForCallback,
  withIsolatedPage,
} from '../../support/browser'
import { CallbackHarness } from '../../support/callback-harness'
import { createAuthorizationRequest, exchangeAuthorizationCode } from '../../support/op-client'

const appBaseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
const mockOIDCPassword = 'password123'
const opCallbackOrigin = 'http://127.0.0.1:9877'
const opRedirectURI = `${opCallbackOrigin}/callback`
const sessionCookieName = '__zitadel_session'
const providerIds = {
  happy: 'prov_mock_oidc',
  existingUser: 'prov_mock_oidc_existing_user',
  linkOnly: 'prov_mock_oidc_link_only',
  userinfoOnly: 'prov_mock_oidc_userinfo_only',
  nonceMismatch: 'prov_mock_oidc_nonce_mismatch',
  tokenFailure: 'prov_mock_oidc_token_failure',
  accessDenied: 'prov_mock_oidc_access_denied',
} as const

const callbackHarness = new CallbackHarness(opCallbackOrigin, 9877)

async function startRPLogin(page: Page, providerID: string) {
  await page.goto(`${appBaseURL}/v1/auth/sso/${providerID}/start`)
}

async function expectExitState(page: Page, title = 'Sign-in complete') {
  await page.waitForURL(/\/login\?/, { timeout: 15_000 })
  const exitState = page.getByTestId('login-exit-state')
  if (await exitState.isVisible().catch(() => false)) {
    await expect(exitState).toBeVisible()
    await expect(page.getByTestId('login-exit-title')).toHaveText(title)
    return
  }

  const currentUrl = new URL(page.url())
  expect(currentUrl.searchParams.get('error')).toBeNull()
}

async function expectLoginError(page: Page, errorCode: string) {
  await page.waitForURL(
    new RegExp(`/login\\?.*error=${escapeRegex(errorCode)}`),
    { timeout: 15_000 },
  )
  await expect(page).toHaveURL(
    new RegExp(`/login\\?.*error=${escapeRegex(errorCode)}`),
  )
}

async function expectAuthenticatedApiSession(page: Page) {
  const cookies = await page.context().cookies()
  expect(cookies.some((cookie) => cookie.name === sessionCookieName && cookie.value.length > 0)).toBe(true)

  const sessions = await browserJSON(page, '/v1/sessions')
  expect(sessions.status).toBe(200)
  expect(Array.isArray(sessions.body.items)).toBe(true)
  expect(sessions.body.items.length).toBeGreaterThan(0)
}

async function expectUnauthenticatedApiSession(page: Page) {
  const sessions = await browserJSON(page, '/v1/sessions')
  expect(sessions.status).toBe(401)
}

test.describe.serial('Upstream OIDC login journey', () => {
  test.beforeAll(async () => {
    await callbackHarness.start()
  })

  test.afterAll(async () => {
    await callbackHarness.stop()
  })

  test.beforeEach(() => {
    callbackHarness.reset()
  })

  test('upstream OIDC login completes and creates an SSO session @smoke', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.happy)
      await completeMockOidcLogin(page, mockOIDCPassword)

      await expectExitState(page)
      await expectAuthenticatedApiSession(page)
    })
  })

  test('verified upstream email can reuse the existing linked local user', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.existingUser)
      await completeMockOidcLogin(page, mockOIDCPassword, 'e2e-user@example.com')

      await expectExitState(page)
      await expectAuthenticatedApiSession(page)
    })
  })

  test('link_only rejects users without an existing linked identity', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.linkOnly)
      await completeMockOidcLogin(page, mockOIDCPassword, 'unlinked-rp-user@example.com')

      await expectLoginError(page, 'sso_link_failed')
      await expectUnauthenticatedApiSession(page)
    })
  })

  test('userinfo fallback succeeds when the upstream token response omits the ID token', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.userinfoOnly)
      await completeMockOidcLogin(page, mockOIDCPassword, 'userinfo-rp-user@example.com')

      await expectExitState(page)
      await expectAuthenticatedApiSession(page)
    })
  })

  test('nonce mismatch returns to login with sso_nonce', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.nonceMismatch)
      await completeMockOidcLogin(page, mockOIDCPassword, 'nonce-rp-user@example.com')

      await expectLoginError(page, 'sso_nonce')
      await expectUnauthenticatedApiSession(page)
    })
  })

  test('token exchange failure returns to login with sso_token', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.tokenFailure)
      await completeMockOidcLogin(page, mockOIDCPassword, 'token-failure-rp-user@example.com')

      await expectLoginError(page, 'sso_token')
      await expectUnauthenticatedApiSession(page)
    })
  })

  test('upstream access_denied returns to login with sso_failed', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.accessDenied)
      await completeMockOidcLogin(page, mockOIDCPassword)

      await expectLoginError(page, 'sso_failed')
      await expectUnauthenticatedApiSession(page)
    })
  })

  test('an SSO-created session can be reused when Zitadel later acts as an OP @smoke', async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.happy)
      await completeMockOidcLogin(page, mockOIDCPassword)
      await expectExitState(page)

      callbackHarness.reset()
      const auth = await createAuthorizationRequest({ redirectUri: opRedirectURI })
      await page.goto(auth.url)

      const entryState = await detectInternalOidcEntryState(page, opRedirectURI)
      expect(entryState).toBe('session_reuse')

      await page
        .getByRole('button', { name: /Continue with this session/i })
        .click()

      const callbackURL = await waitForCallback(page, opRedirectURI, auth.state, () => callbackHarness.lastCallback())
      const exchanged = await exchangeAuthorizationCode(
        callbackURL,
        opRedirectURI,
        auth.state,
        auth.nonce,
        auth.codeVerifier,
      )
      expect(exchanged.access_token).toBeTruthy()
      expect(exchanged.id_token).toBeTruthy()
    })
  })
})
