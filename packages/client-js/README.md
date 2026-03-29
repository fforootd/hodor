# @zitadel/client-js

Auto-generated TypeScript/JavaScript SDK for the [Zitadel](https://zitadel.com) API.

## Installation

```bash
npm install @zitadel/client-js
```

## Quick Start

```ts
import { client, listEntities, createEntity } from '@zitadel/client-js'

// Configure the global client
client.setConfig({
  baseUrl: 'https://your-instance.zitadel.cloud',
  headers: {
    Authorization: 'Bearer pat_...',
  },
})

// List entities
const { data, error } = await listEntities({ query: { limit: 10 } })

// Create an entity
const { data: newEntity } = await createEntity({
  body: {
    identifier: 'user@example.com',
    display_name: 'Jane Doe',
  },
})
```

## Custom Client

```ts
import { createClient } from '@zitadel/client-js'

const zitadel = createClient({
  baseUrl: '/api',
  token: () => getTokenFromStore(),
})
```

## Development

```bash
# Regenerate from OpenAPI spec
npm run generate

# Type-check
npm run typecheck

# Build for publishing
npm run build
```

## Generated from

This SDK is generated from the Zitadel OpenAPI 3.1 specification using
[@hey-api/openapi-ts](https://heyapi.dev). Do not edit files in `src/generated/` manually.
