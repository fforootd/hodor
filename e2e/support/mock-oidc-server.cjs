#!/usr/bin/env node

const { createServer } = require('node:http')
const { generateKeyPairSync, createSign } = require('node:crypto')
const { once } = require('node:events')
const querystring = require('node:querystring')
const { Provider } = require('oidc-provider')

const host = process.env.MOCK_OIDC_HOST || '127.0.0.1'
const port = Number(process.env.MOCK_OIDC_PORT || 19998)
const zitadelCallback = process.env.MOCK_OIDC_REDIRECT_URI || 'http://127.0.0.1:18080/v1/auth/sso/callback'
const mockPassword = process.env.MOCK_OIDC_PASSWORD || 'password123'
const baseOrigin = `http://${host}:${port}`
const cookieKeys = ['dev-cookie-key-1', 'dev-cookie-key-2']

const scenarioDefinitions = [
  {
    id: 'default',
    prefix: '',
    defaultEmail: 'mock-rp-user@example.com',
  },
  {
    id: 'verified-email-existing-user',
    prefix: '/scenarios/verified-email-existing-user',
    defaultEmail: 'e2e-user@example.com',
  },
  {
    id: 'link-only-failure',
    prefix: '/scenarios/link-only-failure',
    defaultEmail: 'unlinked-rp-user@example.com',
  },
  {
    id: 'userinfo-only',
    prefix: '/scenarios/userinfo-only',
    defaultEmail: 'userinfo-rp-user@example.com',
    omitIdToken: true,
  },
  {
    id: 'nonce-mismatch',
    prefix: '/scenarios/nonce-mismatch',
    defaultEmail: 'nonce-rp-user@example.com',
    tamperNonce: true,
  },
  {
    id: 'token-failure',
    prefix: '/scenarios/token-failure',
    defaultEmail: 'token-failure-rp-user@example.com',
    tokenFailure: true,
  },
  {
    id: 'access-denied',
    prefix: '/scenarios/access-denied',
    defaultEmail: 'mock-rp-user@example.com',
    accessDenied: true,
  },
]

function html(body) {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Mock OIDC</title><style>
body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 32rem; padding: 0 1rem; }
form { display: grid; gap: 0.75rem; }
label { display: grid; gap: 0.25rem; font-size: 0.95rem; }
input { font: inherit; padding: 0.625rem 0.75rem; }
button { font: inherit; padding: 0.75rem 1rem; cursor: pointer; }
.meta { color: #555; font-size: 0.95rem; }
</style></head><body>${body}</body></html>`
}

function renderLoginPage(scenario, uid, email, message = '') {
  return html(`
    <h1>Mock OIDC Sign in</h1>
    <p class="meta">Scenario: <strong>${scenario.id}</strong></p>
    ${message ? `<p class="meta">${message}</p>` : ''}
    <form method="post" action="${scenario.prefix}/interaction/${uid}/login">
      <label>
        Email
        <input name="email" type="email" value="${escapeHtml(email)}" autocomplete="username email" />
      </label>
      <label>
        Password
        <input name="password" type="password" value="" autocomplete="current-password" />
      </label>
      <button type="submit">Sign in</button>
    </form>
  `)
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = []
    req.on('data', (chunk) => chunks.push(chunk))
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
    req.on('error', reject)
  })
}

function getScenarioAccount(scenario, email) {
  const identifier = (email || scenario.defaultEmail).trim() || scenario.defaultEmail
  return {
    accountId: identifier,
    sub: identifier,
    email: identifier,
    email_verified: true,
    name: identifier.split('@')[0],
    preferred_username: identifier.split('@')[0],
  }
}

function createScenarioKeys(scenarioId) {
  const { privateKey, publicKey } = generateKeyPairSync('rsa', { modulusLength: 2048 })
  const kid = `mock-${scenarioId}-rs256`
  const privateJwk = privateKey.export({ format: 'jwk' })
  const publicJwk = publicKey.export({ format: 'jwk' })

  return {
    privatePem: privateKey.export({ format: 'pem', type: 'pkcs8' }),
    jwks: {
      keys: [
        {
          ...privateJwk,
          alg: 'RS256',
          use: 'sig',
          kid,
        },
      ],
    },
    header: {
      alg: 'RS256',
      kid,
      typ: 'JWT',
    },
    publicJwk: {
      ...publicJwk,
      alg: 'RS256',
      use: 'sig',
      kid,
    },
  }
}

function base64urlEncode(value) {
  return Buffer.from(value).toString('base64url')
}

function decodeJwt(token) {
  const [encodedHeader, encodedPayload] = token.split('.')
  return {
    header: JSON.parse(Buffer.from(encodedHeader, 'base64url').toString('utf8')),
    payload: JSON.parse(Buffer.from(encodedPayload, 'base64url').toString('utf8')),
  }
}

function signJwt(header, payload, privatePem) {
  const encodedHeader = base64urlEncode(JSON.stringify(header))
  const encodedPayload = base64urlEncode(JSON.stringify(payload))
  const signer = createSign('RSA-SHA256')
  signer.update(`${encodedHeader}.${encodedPayload}`)
  signer.end()
  const signature = signer.sign(privatePem).toString('base64url')
  return `${encodedHeader}.${encodedPayload}.${signature}`
}

function withJsonResponseMutation(res, mutate) {
  const originalWrite = res.write.bind(res)
  const originalEnd = res.end.bind(res)
  const chunks = []

  res.write = function patchedWrite(chunk, encoding, callback) {
    if (chunk) {
      chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, encoding))
    }
    if (typeof callback === 'function') {
      callback()
    }
    return true
  }

  res.end = function patchedEnd(chunk, encoding, callback) {
    if (chunk) {
      chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, encoding))
    }

    const contentType = String(res.getHeader('content-type') || '')
    if (!contentType.includes('application/json')) {
      return originalEnd(Buffer.concat(chunks), encoding, callback)
    }

    try {
      const parsed = JSON.parse(Buffer.concat(chunks).toString('utf8'))
      const mutated = mutate(parsed)
      const body = Buffer.from(JSON.stringify(mutated))
      res.setHeader('content-length', body.length)
      return originalEnd(body, encoding, callback)
    } catch (error) {
      return originalEnd(Buffer.concat(chunks), encoding, callback)
    }
  }

  return { originalWrite, originalEnd }
}

function mountProvider(prefix, provider) {
  const handler = provider.callback()
  return (req, res) => {
    req.originalUrl = req.url
    if (prefix) {
      const nextUrl = req.url.slice(prefix.length) || '/'
      req.url = nextUrl.startsWith('/') ? nextUrl : `/${nextUrl}`
      req.baseUrl = prefix
    } else {
      req.baseUrl = ''
    }
    handler(req, res)
  }
}

async function finishConsent(provider, req, res) {
  const {
    prompt: { name, details },
    grantId,
    session,
    params,
  } = await provider.interactionDetails(req, res)

  if (name !== 'consent') {
    return false
  }

  let grant
  if (grantId) {
    grant = await provider.Grant.find(grantId)
  } else {
    grant = new provider.Grant({
      accountId: session.accountId,
      clientId: params.client_id,
    })
  }

  if (details.missingOIDCScope?.length) {
    grant.addOIDCScope(details.missingOIDCScope.join(' '))
  }
  if (details.missingOIDCClaims) {
    grant.addOIDCClaims(details.missingOIDCClaims)
  }
  if (details.missingResourceScopes) {
    for (const [indicator, scope] of Object.entries(details.missingResourceScopes)) {
      grant.addResourceScope(indicator, scope.join(' '))
    }
  }

  await provider.interactionFinished(
    req,
    res,
    { consent: { grantId: await grant.save() } },
    { mergeWithLastSubmission: true },
  )
  return true
}

async function handleInteractionGet(scenario, provider, req, res, pathname) {
  const match = pathname.match(/^\/interaction\/([^/]+)$/)
  if (!match) {
    return false
  }

  if (await finishConsent(provider, req, res)) {
    return true
  }

  const details = await provider.interactionDetails(req, res)
  if (details.prompt.name !== 'login') {
    res.writeHead(501, { 'content-type': 'text/plain; charset=utf-8' })
    res.end('unsupported prompt')
    return true
  }

  res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' })
  res.end(renderLoginPage(scenario, match[1], scenario.defaultEmail))
  return true
}

async function handleInteractionPost(scenario, provider, req, res, pathname) {
  const match = pathname.match(/^\/interaction\/([^/]+)\/login$/)
  if (!match) {
    return false
  }

  const rawBody = await readBody(req)
  const form = querystring.parse(rawBody)
  const email = String(form.email || scenario.defaultEmail)
  const password = String(form.password || '')

  if (password !== mockPassword) {
    res.writeHead(401, { 'content-type': 'text/html; charset=utf-8' })
    res.end(renderLoginPage(scenario, match[1], email, 'Invalid password'))
    return true
  }

  if (scenario.accessDenied) {
    await provider.interactionFinished(
      req,
      res,
      {
        error: 'access_denied',
        error_description: 'Scenario requested access denial',
      },
      { mergeWithLastSubmission: false },
    )
    return true
  }

  const account = getScenarioAccount(scenario, email)
  await provider.interactionFinished(
    req,
    res,
    {
      login: { accountId: account.accountId },
    },
    { mergeWithLastSubmission: false },
  )
  return true
}

function createScenarioProvider(scenario) {
  const keys = createScenarioKeys(scenario.id)
  const issuer = `${baseOrigin}${scenario.prefix}`

  const provider = new Provider(issuer, {
    clients: [
      {
        client_id: 'mock-client-id',
        client_secret: 'mock-client-secret',
        redirect_uris: [zitadelCallback],
        response_types: ['code'],
        grant_types: ['authorization_code'],
        token_endpoint_auth_method: 'client_secret_post',
      },
    ],
    cookies: {
      keys: cookieKeys,
      short: { secure: false },
      long: { secure: false },
    },
    claims: {
      openid: ['sub'],
      email: ['email', 'email_verified'],
      profile: ['name', 'preferred_username'],
    },
    features: {
      devInteractions: { enabled: false },
      revocation: { enabled: false },
      introspection: { enabled: false },
      rpInitiatedLogout: { enabled: false },
    },
    interactions: {
      url(_ctx, interaction) {
        return `${scenario.prefix}/interaction/${interaction.uid}`
      },
    },
    findAccount(_ctx, sub) {
      const account = getScenarioAccount(scenario, sub)
      return {
        accountId: account.accountId,
        async claims() {
          return {
            sub: account.sub,
            email: account.email,
            email_verified: account.email_verified,
            name: account.name,
            preferred_username: account.preferred_username,
          }
        },
      }
    },
    jwks: keys.jwks,
  })

  return {
    ...scenario,
    keys,
    provider,
    mount: mountProvider(scenario.prefix, provider),
  }
}

function scenarioForRequest(pathname, scenarios) {
  const exact = scenarios.find((scenario) => scenario.prefix && (pathname === scenario.prefix || pathname.startsWith(`${scenario.prefix}/`)))
  if (exact) {
    return exact
  }
  return scenarios.find((scenario) => scenario.prefix === '')
}

function relativePath(pathname, prefix) {
  if (!prefix) {
    return pathname
  }
  const trimmed = pathname.slice(prefix.length)
  return trimmed || '/'
}

async function start() {
  const scenarios = scenarioDefinitions.map(createScenarioProvider)
  const server = createServer(async (req, res) => {
    try {
      const url = new URL(req.url || '/', baseOrigin)
      if (url.pathname === '/healthz') {
        res.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' })
        res.end('ok')
        return
      }

      const scenario = scenarioForRequest(url.pathname, scenarios)
      if (!scenario) {
        res.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' })
        res.end('scenario not found')
        return
      }

      const pathname = relativePath(url.pathname, scenario.prefix)

      if (req.method === 'GET' && await handleInteractionGet(scenario, scenario.provider, req, res, pathname)) {
        return
      }

      if (req.method === 'POST' && await handleInteractionPost(scenario, scenario.provider, req, res, pathname)) {
        return
      }

      if (scenario.tokenFailure && req.method === 'POST' && pathname === '/token') {
        res.writeHead(400, { 'content-type': 'application/json; charset=utf-8' })
        res.end(JSON.stringify({
          error: 'invalid_grant',
          error_description: 'Scenario token failure',
        }))
        return
      }

      if ((scenario.omitIdToken || scenario.tamperNonce) && req.method === 'POST' && pathname === '/token') {
        withJsonResponseMutation(res, (payload) => {
          if (scenario.omitIdToken) {
            delete payload.id_token
            return payload
          }

          if (scenario.tamperNonce && typeof payload.id_token === 'string') {
            const token = decodeJwt(payload.id_token)
            payload.id_token = signJwt(
              scenario.keys.header,
              { ...token.payload, nonce: 'tampered-nonce' },
              scenario.keys.privatePem,
            )
          }
          return payload
        })
      }

      scenario.mount(req, res)
    } catch (error) {
      console.error('[mock-oidc] request failed', error)
      if (!res.headersSent) {
        res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' })
      }
      res.end('mock oidc error')
    }
  })

  server.listen(port, host)
  await once(server, 'listening')
  console.log(`[mock-oidc] listening on ${baseOrigin}`)

  const shutdown = () => {
    server.close(() => process.exit(0))
  }
  process.on('SIGINT', shutdown)
  process.on('SIGTERM', shutdown)
}

start().catch((error) => {
  console.error('[mock-oidc] failed to start', error)
  process.exit(1)
})
