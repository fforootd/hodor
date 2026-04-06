import { expect, type Page } from '@playwright/test'

import {
  completePasswordLogin,
} from '../browser'
import { loginAsAdmin } from './browser-login'

const sessionCookieName = '__zitadel_session'

async function hasBrowserSession(page: Page) {
  const cookies = await page.context().cookies()
  return cookies.some((cookie) => cookie.name === sessionCookieName && cookie.value.length > 0)
}

export async function postJSON<T>(
  page: Page,
  path: string,
  body: Record<string, unknown>,
) {
  return page.evaluate(
    async ({ target, payload }) => {
      const response = await fetch(target, {
        method: 'POST',
        credentials: 'include',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json',
        },
        body: JSON.stringify(payload),
      })

      const data = await response.json().catch(() => null)
      return { status: response.status, data }
    },
    { target: path, payload: body },
  ) as Promise<{ status: number; data: T }>
}

export async function establishAuthenticatedBrowserSession(
  page: Page,
  options: {
    userIdentifier: string
    userPassword: string
  },
) {
  const { userIdentifier, userPassword } = options

  // Seed reusable session state only for preconditions that are not the behavior under test.
  if (userIdentifier === 'admin' && userPassword === 'admin123') {
    await loginAsAdmin(page, 'admin', 'admin123')
    expect(await hasBrowserSession(page)).toBe(true)
    return
  }

  await page.goto('/login')

  const flowCreate = await postJSON<{ flow_id: string; step: string }>(page, '/v1/login/flows', {
    redirect_uri: '/console',
  })
  expect(flowCreate.status).toBeLessThan(400)

  let flowId = String(flowCreate.data?.flow_id || '')
  let step = String(flowCreate.data?.step || '')

  if (step === 'identifier') {
    const identifierSubmit = await postJSON<{ flow_id: string; step: string }>(
      page,
      `/v1/login/flows/${flowId}/submit`,
      {
        action: 'identifier',
        identifier: userIdentifier,
      },
    )
    expect(identifierSubmit.status).toBeLessThan(400)
    flowId = String(identifierSubmit.data?.flow_id || flowId)
    step = String(identifierSubmit.data?.step || '')
  }

  if (step === 'password') {
    const completion = await postJSON<{ redirect_uri?: string }>(
      page,
      `/v1/login/flows/${flowId}/submit`,
      {
        action: 'password',
        password: userPassword,
      },
    )
    expect(completion.status).toBeLessThan(400)
    if (await hasBrowserSession(page)) {
      return
    }
  }

  await page.goto('/login?redirect_uri=%2Fconsole')
  await completePasswordLogin(page, userIdentifier, userPassword)
  expect(await hasBrowserSession(page)).toBe(true)
}
