#!/usr/bin/env node

const net = require('node:net')
const path = require('node:path')
const { createRequire } = require('node:module')

const rootDir = path.resolve(__dirname, '../..')
const requireFromRoot = createRequire(path.join(rootDir, 'package.json'))
const requiredPackages = ['oidc-provider', '@playwright/test']
const missing = []
const requiredUrls = [
  { label: 'Zitadel', value: process.env.BASE_URL || 'http://127.0.0.1:18080' },
  { label: 'Mock OIDC', value: process.env.MOCK_OIDC_URL || 'http://127.0.0.1:19998' },
]

for (const packageName of requiredPackages) {
  try {
    requireFromRoot.resolve(packageName)
  } catch {
    missing.push(packageName)
  }
}

if (missing.length > 0) {
  console.error(
    `[browser-tests] Missing workspace dependencies: ${missing.join(', ')}.\n` +
      `[browser-tests] Run "npm ci" in ${rootDir} to refresh workspace installs before starting Playwright.`,
  )
  process.exit(1)
}

async function ensurePortAvailable(target) {
  const url = new URL(target.value)
  const hostname = url.hostname
  const port = Number(url.port || (url.protocol === 'https:' ? 443 : 80))

  await new Promise((resolve, reject) => {
    const server = net.createServer()

    server.once('error', (error) => {
      reject(error)
    })

    server.listen({ host: hostname, port }, () => {
      server.close((closeError) => {
        if (closeError) {
          reject(closeError)
          return
        }
        resolve()
      })
    })
  }).catch((error) => {
    const details = error && typeof error === 'object' ? error : { code: 'UNKNOWN' }
    const reason =
      details.code === 'EADDRINUSE'
        ? 'another process is already using that port'
        : details.code === 'EPERM'
          ? 'this environment is not allowed to bind local test ports'
          : `${details.code || 'UNKNOWN'}${details.message ? `: ${details.message}` : ''}`

    console.error(
      `[browser-tests] ${target.label} cannot start on ${hostname}:${port} because ${reason}.\n` +
        `[browser-tests] Free the port or run the smoke suite in an environment that permits local listeners before starting Playwright.`,
    )
    process.exit(1)
  })
}

async function main() {
  for (const target of requiredUrls) {
    await ensurePortAvailable(target)
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error)
  console.error(`[browser-tests] Preflight failed: ${message}`)
  process.exit(1)
})
