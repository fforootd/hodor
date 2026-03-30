import { metaSchemaApi, schemaApi, type Schema } from '@/api/resources'

export interface CatalogDisplayMeta {
  alias?: string
  singular?: string
  path?: string
  route?: string
  icon?: string
}

export interface ResourceSchemaContext {
  display: CatalogDisplayMeta
  schema: Record<string, any> | null
  schemaId: string
  schemaType: string
  versions: Schema[]
}

export interface SchemaFieldDefinition {
  name: string
  path: string
  label: string
  description?: string
  type: string
  format?: string
  enum?: string[]
  required: boolean
  hidden: boolean
  editable: boolean
  sensitive: boolean
  identifier: boolean
  properties?: SchemaFieldDefinition[]
  item?: SchemaFieldDefinition | null
}

export interface CurlSnippet {
  title: string
  command: string
}

export type SchemaResourceType = 'user' | 'app' | 'org' | 'group' | 'project'

export async function loadResourceSchemaContext(
  schemaType: string,
  preferredSchemaId = '',
): Promise<ResourceSchemaContext> {
  const [meta, versions] = await Promise.all([
    metaSchemaApi.get(),
    schemaApi.listByType(schemaType),
  ])

  const display = ((meta?.['x-catalog'] || {})[schemaType] || {}) as CatalogDisplayMeta
  const sortedVersions = [...versions].sort((left, right) => right.version - left.version)
  const selected = sortedVersions.find((item) => item.id === preferredSchemaId)
    || sortedVersions.find((item) => item.is_default)
    || sortedVersions[0]

  return {
    display,
    schema: (selected?.schema as Record<string, any>) || null,
    schemaId: selected?.id || preferredSchemaId || '',
    schemaType,
    versions: sortedVersions,
  }
}

export function extractSchemaFields(
  schema: Record<string, any> | null | undefined,
  pathPrefix = '',
): SchemaFieldDefinition[] {
  if (!schema || typeof schema !== 'object') {
    return []
  }

  const properties = (schema.properties || {}) as Record<string, Record<string, any>>
  const required = new Set<string>(Array.isArray(schema.required) ? schema.required : [])

  return Object.entries(properties).map(([name, definition]) => {
    const path = pathPrefix ? `${pathPrefix}.${name}` : name
    const type = Array.isArray(definition.type) ? String(definition.type[0] || 'string') : String(definition.type || inferSchemaType(definition))
    const itemSchema = definition.items && typeof definition.items === 'object'
      ? definition.items as Record<string, any>
      : null

    return {
      name,
      path,
      label: formatFieldLabel(name),
      description: definition.description || '',
      type,
      format: definition.format,
      enum: Array.isArray(definition.enum) ? definition.enum.map(String) : undefined,
      required: required.has(name),
      hidden: Boolean(definition['x-hidden']),
      editable: definition['x-editable'] !== false,
      sensitive: Boolean(definition['x-sensitive']),
      identifier: Boolean(definition['x-identifier']),
      properties: type === 'object'
        ? extractSchemaFields(definition, path)
        : undefined,
      item: type === 'array' && itemSchema
        ? extractSchemaFields({ properties: { item: itemSchema }, required: [] }, path)[0] || null
        : null,
    }
  })
}

export function formatFieldLabel(name: string): string {
  return name
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (letter) => letter.toUpperCase())
}

export function normalizeResourceData(value: unknown): Record<string, any> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return {}
  }
  return JSON.parse(JSON.stringify(value))
}

export function stringifyResourceData(value: Record<string, any>): string {
  return JSON.stringify(normalizeResourceData(value), null, 2)
}

export function buildCurlSnippets(options: {
  path: string
  body?: Record<string, any> | null
  includeOrgHeader?: boolean
  orgId?: string | null
  methods: Array<'GET' | 'POST' | 'PATCH'>
}): CurlSnippet[] {
  const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''
  const url = `${window.location.origin}${basePath}${options.path}`

  return options.methods.map((method) => ({
    title: method,
    command: buildCurlCommand({
      method,
      url,
      body: method === 'GET' ? null : options.body || {},
      includeOrgHeader: options.includeOrgHeader !== false,
      orgId: options.orgId || null,
    }),
  }))
}

export function buildResourceWriteBody(
  resourceType: SchemaResourceType,
  schemaId: string,
  data: Record<string, any>,
): Record<string, any> {
  const safeData = normalizeResourceData(data)

  switch (resourceType) {
    case 'user': {
      const identifier = String(
        safeData.email
        || safeData.username
        || safeData.phone
        || safeData.identifier
        || '',
      ).trim()
      return {
        schema_id: schemaId,
        identifier,
        display_name: String(safeData.display_name || '').trim(),
        data: safeData,
      }
    }
    case 'app':
      return {
        schema_id: schemaId,
        name: String(safeData.client_name || '').trim(),
        description: String(safeData.description || '').trim(),
        app_type: String(safeData.app_type || '').trim(),
        redirect_uris: arrayValue(safeData.redirect_uris),
        post_logout_redirect_uris: arrayValue(safeData.post_logout_redirect_uris),
        grant_types: arrayValue(safeData.grant_types),
        response_types: arrayValue(safeData.response_types),
        logo_uri: String(safeData.logo_uri || '').trim(),
        metadata: objectValue(safeData.metadata),
        data: safeData,
      }
    case 'org':
      return {
        schema_id: schemaId,
        name: String(safeData.display_name || '').trim(),
        metadata: objectValue(safeData.metadata),
        data: safeData,
      }
    case 'group':
    case 'project':
      return {
        schema_id: schemaId,
        name: String(safeData.name || '').trim(),
        description: String(safeData.description || '').trim(),
        metadata: objectValue(safeData.metadata),
        data: safeData,
      }
    default:
      return {
        schema_id: schemaId,
        data: safeData,
      }
  }
}

function buildCurlCommand(options: {
  method: 'GET' | 'POST' | 'PATCH'
  url: string
  body: Record<string, any> | null
  includeOrgHeader: boolean
  orgId: string | null
}): string {
  const lines = [
    `curl --request ${options.method} \\`,
    `  --url '${options.url}' \\`,
    `  --header 'Authorization: Bearer <token>' \\`,
  ]

  if (options.includeOrgHeader && options.orgId) {
    lines.push(`  --header 'X-Org-Id: ${options.orgId}' \\`)
  }

  if (options.body) {
    lines.push(`  --header 'Content-Type: application/json' \\`)
    lines.push(`  --data '${JSON.stringify(options.body, null, 2)}'`)
  } else {
    lines[lines.length - 1] = lines[lines.length - 1].slice(0, -2)
  }

  return lines.join('\n')
}

function arrayValue(value: unknown): any[] {
  return Array.isArray(value) ? value : []
}

function objectValue(value: unknown): Record<string, any> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? normalizeResourceData(value)
    : {}
}

function inferSchemaType(definition: Record<string, any>): string {
  if (definition.properties) return 'object'
  if (definition.items) return 'array'
  if (definition.enum) return 'string'
  return 'string'
}
