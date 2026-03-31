export interface ProviderRecord {
  id: string
  display_name?: string
  name?: string
  kind?: string
  protocol?: string
  enabled: boolean
  connection?: Record<string, any>
  config?: Record<string, any>
  mapping?: { claims?: Record<string, string> }
  claim_overrides?: Record<string, string>
  catalog_ref?: {
    template_id?: string
    template_version?: string
    official?: boolean
  }
  template?: string
  target?: { schema_type?: string; schema_id?: string }
  linking?: { mode?: string; match_by?: string }
  created_at?: string
  updated_at?: string
}

export interface ProviderTemplateSummary {
  id: string
  name: string
  description: string
  protocol?: string
}

export function providerDisplayName(provider: ProviderRecord): string {
  return provider.display_name || provider.name || 'Unnamed provider'
}

export function providerTemplateLabel(provider: ProviderRecord): string {
  return provider.catalog_ref?.template_id || provider.template || provider.kind || 'custom'
}

export function providerConnection(provider: ProviderRecord): Record<string, any> {
  return provider.connection || provider.config || {}
}

export function providerClaimMappings(provider: ProviderRecord): [string, string][] {
  const claims = provider.mapping?.claims || provider.claim_overrides || {}
  return Object.entries(claims)
}

export function providerScopesLabel(provider: ProviderRecord): string {
  const scopes = providerConnection(provider).scopes
  if (Array.isArray(scopes)) return scopes.join(', ')
  if (typeof scopes === 'string' && scopes.trim()) return scopes
  return '—'
}

export function providerIcon(id: string): string {
  const icons: Record<string, string> = {
    google: '🔵',
    'google-oidc': '🔵',
    entra: '🟦',
    'entra-id': '🟦',
    github: '🐙',
    gitlab: '🦊',
    'custom-oidc': '⚙',
    apple: '🍎',
    custom: '⚙',
  }
  return icons[id] || '🔗'
}

export function formatProviderFieldLabel(key: string): string {
  return key
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase())
    .replace('Url', 'URL')
    .replace('Id', 'ID')
}

export function humanizeProviderValue(value?: string): string {
  if (!value) return '—'
  return formatProviderFieldLabel(value)
}

export function humanizeProviderLinking(mode?: string, matchBy?: string): string {
  if (!mode && !matchBy) return 'Default'
  if (mode === 'create_or_link' && matchBy === 'verified_email') {
    return 'Create or link by verified email'
  }
  if (mode === 'link_only' && matchBy === 'verified_email') {
    return 'Link only by verified email'
  }
  if (mode === 'create_or_link' && matchBy) {
    return `Create or link by ${humanizeProviderValue(matchBy).toLowerCase()}`
  }
  if (mode === 'link_only' && matchBy) {
    return `Link only by ${humanizeProviderValue(matchBy).toLowerCase()}`
  }
  return humanizeProviderValue(mode)
}

export function sortProviderConnectionKeys(keys: string[]): string[] {
  const order = [
    'issuer',
    'authorization_url',
    'token_url',
    'userinfo_url',
    'tenant_id',
    'client_id',
    'client_secret',
    'scopes',
  ]
  const rank = new Map(order.map((key, index) => [key, index]))
  return [...keys].sort((left, right) => {
    const leftRank = rank.get(left) ?? order.length + 1
    const rightRank = rank.get(right) ?? order.length + 1
    if (leftRank !== rightRank) return leftRank - rightRank
    return left.localeCompare(right)
  })
}
