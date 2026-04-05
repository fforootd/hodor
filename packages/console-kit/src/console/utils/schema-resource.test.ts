import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/api/resources', () => ({
  metaSchemaApi: {
    get: vi.fn(),
  },
  schemaApi: {
    listByType: vi.fn(),
  },
}))

import { metaSchemaApi, schemaApi } from '@/api/resources'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  collectSummaryFacts,
  extractSchemaFields,
  getValueAtPath,
  loadResourceSchemaContext,
  normalizeResourceData,
  stringifyResourceData,
} from './schema-resource'

describe('schema-resource utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(window as any).__ZITADEL_BASE_PATH__ = '/zitadel'
  })

  it('loads the preferred schema context with catalog display metadata', async () => {
    vi.mocked(metaSchemaApi.get).mockResolvedValue({
      'x-catalog': {
        group: { singular: 'Group', alias: 'Groups', path: 'groups' },
      },
    })
    vi.mocked(schemaApi.listByType).mockResolvedValue([
      { id: 'group_v2', version: 2, is_default: false, schema: { type: 'object' } } as any,
      { id: 'group_v1', version: 1, is_default: true, schema: { type: 'object', title: 'Group' } } as any,
    ])

    const context = await loadResourceSchemaContext('group', 'group_v2')

    expect(context.display.singular).toBe('Group')
    expect(context.schemaId).toBe('group_v2')
    expect(context.versions.map((item) => item.id)).toEqual(['group_v2', 'group_v1'])
  })

  it('extracts nested schema fields and annotation hints', () => {
    const fields = extractSchemaFields({
      type: 'object',
      required: ['name'],
      properties: {
        name: { type: 'string', description: 'Visible name' },
        api_key: { type: 'string', 'x-sensitive': true, 'x-editable': false },
        profile: {
          type: 'object',
          properties: {
            team: { type: 'string' },
          },
        },
        tags: {
          type: 'array',
          items: { type: 'string', enum: ['a', 'b'] },
        },
      },
    })

    expect(fields[0]).toMatchObject({ path: 'name', required: true, hidden: false })
    expect(fields[1]).toMatchObject({ path: 'api_key', sensitive: true, editable: false })
    expect(fields[2].properties?.[0]).toMatchObject({ path: 'profile.team', type: 'string' })
    expect(fields[3].item).toMatchObject({ path: 'tags.item', enum: ['a', 'b'] })
  })

  it('prioritizes identifier fields first and metadata last', () => {
    const fields = extractSchemaFields({
      type: 'object',
      properties: {
        description: { type: 'string' },
        metadata: { type: 'object', additionalProperties: true },
        name: { type: 'string', 'x-identifier': true },
      },
    })

    expect(fields.map((field) => field.name)).toEqual(['name', 'description', 'metadata'])
  })

  it('builds canonical write bodies for user and app resources', () => {
    const userBody = buildResourceWriteBody('user', 'human_user_v1', {
      email: 'alice@example.com',
      display_name: 'Alice',
      profile: { locale: 'en' },
    })
    const appBody = buildResourceWriteBody('app', 'app_v1', {
      client_name: 'Console',
      description: 'Admin app',
      redirect_uris: ['https://example.com/callback'],
      post_logout_redirect_uris: ['https://example.com/logout'],
      metadata: { tier: 'pro' },
    })

    expect(userBody).toMatchObject({
      schema_id: 'human_user_v1',
      identifier: 'alice@example.com',
      display_name: 'Alice',
    })
    expect(appBody).toMatchObject({
      schema_id: 'app_v1',
      name: 'Console',
      description: 'Admin app',
      redirect_uris: ['https://example.com/callback'],
      post_logout_redirect_uris: ['https://example.com/logout'],
      metadata: { tier: 'pro' },
    })
  })

  it('creates copyable cURL snippets with base path, org header, and body', () => {
    const snippets = buildCurlSnippets({
      path: '/v1/groups/group_123',
      body: { name: 'Core Team' },
      includeOrgHeader: true,
      orgId: 'org_123',
      methods: ['GET', 'PATCH'],
    })

    expect(snippets[0].title).toBe('GET')
    expect(snippets[0].command).toContain(`${window.location.origin}/zitadel/v1/groups/group_123`)
    expect(snippets[1].command).toContain(`X-Org-Id: org_123`)
    expect(snippets[1].command).toContain(`"name": "Core Team"`)
  })

  it('normalizes and stringifies resource data defensively', () => {
    const value = normalizeResourceData({ nested: { enabled: true } })

    expect(value).toEqual({ nested: { enabled: true } })
    expect(stringifyResourceData(value)).toContain('"enabled": true')
    expect(normalizeResourceData(null)).toEqual({})
  })

  it('collects summary facts while skipping hidden or sensitive values', () => {
    const facts = collectSummaryFacts({
      name: 'Console',
      metadata: { tier: 'pro' },
      profile: { locale: 'de-CH' },
      secret: 'hidden',
    }, {
      type: 'object',
      properties: {
        name: { type: 'string' },
        metadata: { type: 'object' },
        profile: {
          type: 'object',
          properties: {
            locale: { type: 'string' },
          },
        },
        secret: { type: 'string', 'x-sensitive': true },
      },
    })

    expect(facts).toEqual([
      { label: 'Name', value: 'Console' },
      { label: 'Locale', value: 'de-CH' },
    ])
  })

  it('reads nested values defensively', () => {
    expect(getValueAtPath({ profile: { locale: 'en' } }, 'profile.locale')).toBe('en')
    expect(getValueAtPath({ profile: null }, 'profile.locale')).toBeUndefined()
  })
})
