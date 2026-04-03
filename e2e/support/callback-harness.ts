import { once } from 'node:events'
import { createServer, type Server } from 'node:http'

export class CallbackHarness {
  private server: Server | null = null
  private lastURL: string | null = null

  constructor(
    private readonly origin: string,
    private readonly port: number,
  ) {}

  async start() {
    if (this.server) {
      return
    }

    this.server = createServer((req, res) => {
      const url = new URL(req.url || '/', this.origin)

      if (url.pathname === '/callback') {
        this.lastURL = url.toString()
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        res.end('<html><body>OIDC callback received</body></html>')
        return
      }

      if (url.pathname === '/logout') {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        res.end('<html><body>OIDC logout received</body></html>')
        return
      }

      if (url.pathname === '/healthz') {
        res.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' })
        res.end('ok')
        return
      }

      res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' })
      res.end('not found')
    })

    this.server.listen(this.port, '127.0.0.1')
    await once(this.server, 'listening')
  }

  reset() {
    this.lastURL = null
  }

  lastCallback(): URL | null {
    return this.lastURL ? new URL(this.lastURL) : null
  }

  async stop() {
    if (!this.server) {
      return
    }

    const server = this.server
    this.server = null
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error) {
          reject(error)
          return
        }
        resolve()
      })
    })
  }
}
