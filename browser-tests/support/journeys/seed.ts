import { expect, type Page } from '@playwright/test'

import {
  browserJSON,
  completePasswordLogin,
} from '../browser'

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
    await page.goto('/console')
    await expect(page).toHaveURL(/\/login/)
    await completePasswordLogin(page, 'admin', 'admin123')
    const sessions = await browserJSON(page, '/v1/sessions')
    expect(sessions.status).toBe(200)
    expect(Array.isArray(sessions.body.items)).toBe(true)
    expect(sessions.body.items.length).toBeGreaterThan(0)
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
    const sessions = await browserJSON(page, '/v1/sessions')
    if (sessions.status === 200 && Array.isArray(sessions.body.items) && sessions.body.items.length > 0) {
      return
    }
  }

  await page.goto('/login?redirect_uri=%2Fconsole')
  await completePasswordLogin(page, userIdentifier, userPassword)

  const sessions = await browserJSON(page, '/v1/sessions')
  expect(sessions.status).toBe(200)
  expect(Array.isArray(sessions.body.items)).toBe(true)
  expect(sessions.body.items.length).toBeGreaterThan(0)
}
