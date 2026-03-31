<template>
  <div class="space-y-6">
    <div class="space-y-2">
      <h1 class="text-2xl font-semibold tracking-tight">API &amp; Protocols</h1>
      <p class="max-w-3xl text-sm text-muted-foreground">
        Explore Zitadel&apos;s machine-facing integration surfaces for REST, OpenID Connect, and
        schema discovery.
      </p>
    </div>

    <Tabs v-model="activeTab" class="space-y-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <TabsList class="w-fit">
          <TabsTrigger value="reference">REST API</TabsTrigger>
          <TabsTrigger value="openid">OpenID Connect</TabsTrigger>
          <TabsTrigger value="schema">Schema Discovery</TabsTrigger>
        </TabsList>

        <Button v-if="activeTab === 'reference'" variant="outline" size="sm" as-child>
          <a :href="apiSpecHref" target="_blank" rel="noreferrer">
            <ExternalLink class="mr-2 size-4" />
            Raw OpenAPI
          </a>
        </Button>
      </div>

      <TabsContent value="reference" class="mt-0 space-y-4">
        <div
          class="flex flex-col gap-3 rounded-2xl border bg-muted/30 px-4 py-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1">
            <p class="text-sm font-medium">Session-aware API reference</p>
            <p class="max-w-3xl text-sm text-muted-foreground">
              Requests run against this instance using your current console session. You can also
              test protected endpoints with a bearer token.
            </p>
          </div>
          <p class="text-xs text-muted-foreground sm:max-w-56 sm:text-right">
            Embedded directly in the console, with your selected org context forwarded on requests.
          </p>
        </div>

        <div class="overflow-hidden rounded-[20px] border bg-background shadow-sm">
          <ApiReference :configuration="scalarConfiguration" />
        </div>
      </TabsContent>

      <TabsContent value="openid" class="mt-0 space-y-4">
        <div class="space-y-1">
          <h2 class="text-lg font-semibold tracking-tight">OpenID Connect</h2>
          <p class="text-sm text-muted-foreground">
            Discover the current issuer and protocol endpoints published by this instance.
          </p>
        </div>

        <div v-if="oidcLoading" class="rounded-xl border bg-muted/20 px-4 py-3 text-sm text-muted-foreground">
          Loading discovery document…
        </div>
        <div
          v-else-if="oidcError"
          class="rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        >
          {{ oidcError }}
        </div>
        <div v-else class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <div
            v-for="endpoint in oidcEndpoints"
            :key="endpoint.key"
            class="rounded-2xl border bg-card p-4 shadow-sm"
          >
            <div class="flex items-start justify-between gap-3">
              <div class="space-y-1">
                <h3 class="text-sm font-medium">{{ endpoint.label }}</h3>
                <p class="text-xs leading-relaxed text-muted-foreground">
                  {{ endpoint.description }}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  class="size-8"
                  :aria-label="`Copy ${endpoint.label}`"
                  @click="copy(endpoint.url)"
                >
                  <Copy class="size-4" />
                </Button>
                <Button variant="ghost" size="icon" class="size-8" as-child>
                  <a
                    :href="endpoint.url"
                    target="_blank"
                    rel="noreferrer"
                    :aria-label="`Open ${endpoint.label}`"
                  >
                    <ExternalLink class="size-4" />
                  </a>
                </Button>
              </div>
            </div>
            <code
              class="mt-3 block rounded-xl bg-muted px-3 py-2 text-xs leading-relaxed break-all"
              >{{ endpoint.url }}</code
            >
          </div>
        </div>
      </TabsContent>

      <TabsContent value="schema" class="mt-0 space-y-4">
        <div class="space-y-1">
          <h2 class="text-lg font-semibold tracking-tight">Schema Discovery</h2>
          <p class="text-sm text-muted-foreground">
            Entry points for clients that need to discover Zitadel&apos;s schema catalog and
            meta-schema surface.
          </p>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div
            v-for="entry in schemaDiscoveryEntries"
            :key="entry.label"
            class="rounded-2xl border bg-card p-4 shadow-sm"
          >
            <div class="flex items-start justify-between gap-3">
              <div class="space-y-1">
                <h3 class="text-sm font-medium">{{ entry.label }}</h3>
                <p class="text-xs leading-relaxed text-muted-foreground">{{ entry.description }}</p>
              </div>
              <div class="flex shrink-0 items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  class="size-8"
                  :aria-label="`Copy ${entry.label}`"
                  @click="copy(entry.url)"
                >
                  <Copy class="size-4" />
                </Button>
                <Button variant="ghost" size="icon" class="size-8" as-child>
                  <a
                    :href="entry.url"
                    target="_blank"
                    rel="noreferrer"
                    :aria-label="`Open ${entry.label}`"
                  >
                    <ExternalLink class="size-4" />
                  </a>
                </Button>
              </div>
            </div>
            <code
              class="mt-3 block rounded-xl bg-muted px-3 py-2 text-xs leading-relaxed break-all"
              >{{ entry.url }}</code
            >
          </div>
        </div>
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
  import { computed, onMounted, ref } from 'vue'
  import { ApiReference, type ReferenceProps } from '@scalar/api-reference'
  import '@scalar/api-reference/style.css'
  import { credentialsMode, getApiBaseUrl, getCurrentOrgHeader } from '@/api/client'
  import { Button } from '@/components/ui/button'
  import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
  import { Copy, ExternalLink } from 'lucide-vue-next'

  interface OIDCDiscoveryDocument {
    issuer?: string
    authorization_endpoint?: string
    token_endpoint?: string
    userinfo_endpoint?: string
    jwks_uri?: string
    end_session_endpoint?: string
    revocation_endpoint?: string
    device_authorization_endpoint?: string
  }

  type EndpointEntry = {
    key: string
    label: string
    description: string
    url: string
  }

  const activeTab = ref('reference')
  const baseUrl = getApiBaseUrl()
  const oidcLoading = ref(true)
  const oidcError = ref('')
  const oidcDiscovery = ref<OIDCDiscoveryDocument | null>(null)

  function withApiBase(path: string): string {
    return baseUrl ? `${baseUrl}${path}` : path
  }

  function toAbsoluteUrl(pathOrUrl: string): string {
    return new URL(pathOrUrl, window.location.origin).toString()
  }

  function normalizeApiUrl(pathOrUrl: string): string {
    const url = new URL(pathOrUrl, window.location.origin)
    if (!baseUrl || url.origin !== window.location.origin) {
      return url.toString()
    }

    const isAlreadyPrefixed = url.pathname === baseUrl || url.pathname.startsWith(`${baseUrl}/`)
    const isApiPath =
      url.pathname === '/openapi.json' ||
      url.pathname.startsWith('/v1/') ||
      url.pathname === '/healthz' ||
      url.pathname === '/readyz'

    if (isApiPath && !isAlreadyPrefixed) {
      url.pathname = `${baseUrl}${url.pathname}`
    }

    return url.toString()
  }

  async function scalarFetch(input: string | URL | Request, init?: RequestInit): Promise<Response> {
    const request = input instanceof Request ? input : new Request(input, init)
    const headers = new Headers(request.headers)
    const orgId = getCurrentOrgHeader()
    if (orgId && !headers.has('X-Org-Id')) {
      headers.set('X-Org-Id', orgId)
    }

    return fetch(normalizeApiUrl(request.url), {
      method: request.method,
      headers,
      body: request.method === 'GET' || request.method === 'HEAD' ? undefined : request.body,
      cache: request.cache,
      credentials: credentialsMode(baseUrl),
      integrity: request.integrity,
      keepalive: request.keepalive,
      mode: request.mode,
      redirect: request.redirect,
      referrer: request.referrer,
      referrerPolicy: request.referrerPolicy,
      signal: request.signal,
    })
  }

  async function loadJSON<T>(url: string): Promise<T> {
    const response = await fetch(url, {
      credentials: credentialsMode(baseUrl),
      headers: { Accept: 'application/json' },
    })

    const raw = await response.text()
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`)
    }

    try {
      return JSON.parse(raw) as T
    } catch {
      if (raw.trim().startsWith('<')) {
        throw new Error('Expected JSON from the discovery endpoint, but received HTML instead.')
      }
      throw new Error('Expected JSON from the discovery endpoint, but received another format.')
    }
  }

  async function copy(text: string) {
    await navigator.clipboard.writeText(text)
  }

  const apiSpecPath = computed(() => withApiBase('/openapi.json'))
  const apiSpecHref = computed(() => toAbsoluteUrl(apiSpecPath.value))
  const oidcDiscoveryUrl = computed(() => toAbsoluteUrl('/.well-known/openid-configuration'))

  const scalarConfiguration = computed(() => {
    const configuration = {
      _integration: 'vue',
      authentication: {
        preferredSecurityScheme: 'cookieAuth',
      },
      documentDownloadType: 'direct',
      fetch: scalarFetch,
      forceDarkModeState: 'light',
      hideClientButton: true,
      hideDarkModeToggle: true,
      layout: 'classic',
      persistAuth: false,
      servers: [
        {
          url: baseUrl || '/',
          description: 'This instance',
        },
      ],
      showDeveloperTools: 'never',
      showSidebar: true,
      showToolbar: 'never',
      theme: 'default',
      title: 'Zitadel API',
      url: apiSpecPath.value,
      withDefaultFonts: false,
    }

    return configuration as NonNullable<ReferenceProps['configuration']>
  })

  const oidcEndpoints = computed<EndpointEntry[]>(() => {
    const doc = oidcDiscovery.value
    if (!doc) return []

    const candidates: EndpointEntry[] = [
      {
        key: 'issuer',
        label: 'Issuer',
        description:
          'The canonical issuer identifier used by relying parties and token validation.',
        url: doc.issuer || '',
      },
      {
        key: 'discovery',
        label: 'Discovery',
        description: 'The well-known OpenID Provider metadata document for this instance.',
        url: oidcDiscoveryUrl.value,
      },
      {
        key: 'authorize',
        label: 'Authorization',
        description: 'Starts the browser-based authorization flow for OIDC clients.',
        url: doc.authorization_endpoint || '',
      },
      {
        key: 'token',
        label: 'Token',
        description:
          'Issues access, ID, and refresh tokens after client authentication and grant validation.',
        url: doc.token_endpoint || '',
      },
      {
        key: 'userinfo',
        label: 'Userinfo',
        description: 'Returns user claims for a valid access token with the appropriate scopes.',
        url: doc.userinfo_endpoint || '',
      },
      {
        key: 'jwks',
        label: 'JWKS',
        description:
          'Publishes signing keys for token verification by clients and resource servers.',
        url: doc.jwks_uri || '',
      },
      {
        key: 'end-session',
        label: 'End Session',
        description: 'Terminates the OpenID Connect login session when supported by the provider.',
        url: doc.end_session_endpoint || '',
      },
      {
        key: 'revoke',
        label: 'Revoke',
        description: 'Revokes access or refresh tokens when clients need to invalidate them early.',
        url: doc.revocation_endpoint || '',
      },
      {
        key: 'device-code',
        label: 'Device Authorization',
        description: 'Initiates the device authorization flow for constrained-input clients.',
        url: doc.device_authorization_endpoint || '',
      },
    ]

    return candidates.filter((entry) => entry.url)
  })

  const schemaDiscoveryEntries = computed<EndpointEntry[]>(() => [
    {
      key: 'identity-schema',
      label: 'Well-known identity schema',
      description:
        'Stable discovery entry point that redirects clients to Zitadel’s schema metadata surface.',
      url: toAbsoluteUrl('/.well-known/zitadel-identity-schema'),
    },
    {
      key: 'meta-schema',
      label: 'Meta schema catalog',
      description:
        'Returns the schema catalog, navigation metadata, and meta-schema definitions used by the console.',
      url: toAbsoluteUrl(withApiBase('/v1/schemas/$meta')),
    },
  ])

  onMounted(async () => {
    try {
      oidcDiscovery.value = await loadJSON<OIDCDiscoveryDocument>(oidcDiscoveryUrl.value)
    } catch (err: any) {
      oidcError.value = err?.message || 'Failed to load discovery document'
    } finally {
      oidcLoading.value = false
    }
  })
</script>
