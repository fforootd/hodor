/**
 * @zitadel/client-js — Generated TypeScript SDK for the Zitadel API.
 *
 * This package is auto-generated from the OpenAPI 3.1 specification.
 * Do not edit files in the `generated/` directory manually.
 *
 * @example
 * ```ts
 * import { client, listEntities, getEntity } from '@zitadel/client-js'
 *
 * // Configure the client
 * client.setConfig({ baseUrl: 'https://your-instance.zitadel.cloud' })
 *
 * // Use generated SDK methods
 * const { data } = await listEntities({ query: { limit: 10 } })
 * ```
 */

// Re-export everything from the generated SDK.
export * from './generated/types.gen.js'
export * from './generated/sdk.gen.js'

// Re-export the generated client instance for configuration.
export { client } from './generated/client.gen.js'

// Re-export the configurable client factory.
export { createClient } from './client.js'
