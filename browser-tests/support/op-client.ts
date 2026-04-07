import {
  ClientSecretPost,
  allowInsecureRequests,
  authorizationCodeGrant,
  buildAuthorizationUrl,
  calculatePKCECodeChallenge,
  discovery,
  fetchUserInfo,
  randomPKCECodeVerifier,
  randomState,
  skipSubjectCheck,
  type Configuration,
} from 'openid-client'

const baseURL = process.env.BASE_URL || 'http://127.0.0.1:18080'
const clientId = 'e2e-browser-client'
const clientSecret = 'e2e-browser-secret'

type AuthorizationRequestOptions = {
  prompt?: string
  redirectUri: string
}

type AuthorizationRequest = {
  url: string
  state: string
  nonce: string
  codeVerifier: string
}

let configPromise: Promise<Configuration> | null = null

function describeClientError(error: unknown) {
  if (!(error instanceof Error)) {
    return String(error)
  }

  const details = [error.message]
  const withCode = error as Error & { code?: string; cause?: unknown }
  if (withCode.code) {
    details.push(`code=${withCode.code}`)
  }
  if (withCode.cause instanceof Error && withCode.cause.message) {
    details.push(`cause=${withCode.cause.message}`)
  } else if (withCode.cause) {
    details.push(`cause=${String(withCode.cause)}`)
  }
  return details.join(' ')
}

async function getConfig() {
  if (!configPromise) {
    configPromise = discovery(
      new URL(baseURL),
      clientId,
      {
        client_secret: clientSecret,
        redirect_uris: [],
        response_types: ['code'],
      },
      ClientSecretPost(clientSecret),
      {
        execute: [allowInsecureRequests],
      },
    )
  }
  return configPromise
}

export async function createAuthorizationRequest(options: AuthorizationRequestOptions): Promise<AuthorizationRequest> {
  const config = await getConfig()
  const codeVerifier = randomPKCECodeVerifier()
  const codeChallenge = await calculatePKCECodeChallenge(codeVerifier)
  const state = randomState()
  const nonce = randomState()
  const url = buildAuthorizationUrl(config, {
    client_id: clientId,
    redirect_uri: options.redirectUri,
    response_type: 'code',
    scope: 'openid profile email',
    state,
    nonce,
    code_challenge: codeChallenge,
    code_challenge_method: 'S256',
    ...(options.prompt ? { prompt: options.prompt } : {}),
  })

  return {
    url: url.toString(),
    state,
    nonce,
    codeVerifier,
  }
}

export async function exchangeAuthorizationCode(
  callbackUrl: URL,
  redirectUri: string,
  state: string,
  nonce: string,
  codeVerifier: string,
) {
  const config = await getConfig()
  try {
    return await authorizationCodeGrant(config, callbackUrl, {
      expectedState: state,
      expectedNonce: nonce,
      pkceCodeVerifier: codeVerifier,
    })
  } catch (error) {
    throw new Error(
      `authorization code exchange failed for ${callbackUrl.toString()}: ${describeClientError(error)}`,
      { cause: error instanceof Error ? error : undefined },
    )
  }
}

export async function fetchOpenIdUserInfo(accessToken: string) {
  const config = await getConfig()
  return fetchUserInfo(config, accessToken, skipSubjectCheck)
}
