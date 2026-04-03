import { Container, ContainerProxy, getContainer } from "@cloudflare/containers";
export { ContainerProxy };

// ── Constants ─────────────────────────────────────────────────────────────

const ROUTER_DO_NAME = "router";
const DEFAULT_ADMIN_EMAIL = "admin@example.com";
const DEFAULT_LOG_LEVEL = "info";
const DEFAULT_TRUSTED_PROXIES = [
  "127.0.0.1/32",
  "::1/128",
  "10.0.0.0/8",
  "100.64.0.0/10",
  "172.16.0.0/12",
  "192.168.0.0/16",
  "fc00::/7",
].join(",");

const STATIC_ASSET_PREFIX = "/assets/";
const DYNAMIC_ASSET_PREFIX = "/assets/login/";
const INTERNAL_ASSET_PREFIX = "/src/";

// ── Types ─────────────────────────────────────────────────────────────────

interface Env {
  ASSETS?: Fetcher;
  ZITADEL: DurableObjectNamespace<ZitadelContainer>;
  ROUTER: DurableObjectNamespace;

  // D1 REST API credentials (Worker secrets)
  CLOUDFLARE_ACCOUNT_ID?: string;
  CLOUDFLARE_API_TOKEN?: string;

  // Fallback config for single-instance mode (backward compat)
  ZITADEL_ADMIN_PASSWORD?: string;
  ZITADEL_ADMIN_PAT?: string;
  ZITADEL_COOKIE_SECRETS?: string;
  ZITADEL_STORAGE_STATEFUL_URL?: string;
  ZITADEL_DATABASE_AUTH_TOKEN?: string;
  ZITADEL_ADMIN_EMAIL?: string;
  ZITADEL_EXTERNAL_DOMAIN?: string;
  ZITADEL_TLS_MODE?: string;
  ZITADEL_PROXY_HEADER_MODE?: string;
  ZITADEL_TRUSTED_PROXIES?: string;
  ZITADEL_LOG_LEVEL?: string;
  ZITADEL_BASE_PATH?: string;
  ZITADEL_STORAGE_STATEFUL_MIGRATE?: string;
  ZITADEL_STORAGE_STATEFUL_BOOTSTRAP?: string;
  ZITADEL_ENCRYPTION_ACTIVE_KEY_ID?: string;
  ZITADEL_ENCRYPTION_KEYS?: string;
}

interface InstanceRoute {
  instanceId: string;
  customerId: string;
}

// ── ZitadelRouter DO ──────────────────────────────────────────────────────
//
// Singleton Durable Object that holds the domain → instance routing table.
// Queried on every request to resolve which container handles the hostname.

export class ZitadelRouter implements DurableObject {
  private schemaReady = false;
  private readonly state: DurableObjectState;
  private readonly workerEnv: Env;

  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    this.workerEnv = env;
  }

  private get sql() {
    return this.state.storage.sql;
  }

  private ensureSchema(): void {
    if (this.schemaReady) return;
    this.sql.exec(`
      CREATE TABLE IF NOT EXISTS domains (
        domain       TEXT PRIMARY KEY,
        instance_id  TEXT NOT NULL,
        customer_id  TEXT NOT NULL,
        is_primary   INTEGER NOT NULL DEFAULT 0,
        created_at   TEXT NOT NULL DEFAULT (datetime('now'))
      )
    `);
    this.sql.exec(
      `CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id)`
    );
    this.sql.exec(
      `CREATE INDEX IF NOT EXISTS idx_domains_customer ON domains(customer_id)`
    );
    this.schemaReady = true;
  }

  /** Resolve a hostname to an instance. */
  lookup(hostname: string): InstanceRoute | null {
    this.ensureSchema();
    const cursor = this.sql.exec<{
      instance_id: string;
      customer_id: string;
    }>(
      "SELECT instance_id, customer_id FROM domains WHERE domain = ?",
      hostname
    );
    const row = cursor.next();
    if (row.done) return null;
    return { instanceId: row.value.instance_id, customerId: row.value.customer_id };
  }

  /** Add a domain → instance mapping. */
  addDomain(
    domain: string,
    instanceId: string,
    customerId: string,
    isPrimary: boolean
  ): void {
    this.ensureSchema();
    this.sql.exec(
      `INSERT OR REPLACE INTO domains (domain, instance_id, customer_id, is_primary)
       VALUES (?, ?, ?, ?)`,
      domain,
      instanceId,
      customerId,
      isPrimary ? 1 : 0
    );
  }

  /** Remove a domain mapping. */
  removeDomain(domain: string): void {
    this.ensureSchema();
    this.sql.exec("DELETE FROM domains WHERE domain = ?", domain);
  }

  /** List all instances for a customer. */
  listInstances(customerId: string): Array<{ instanceId: string; domain: string }> {
    this.ensureSchema();
    const cursor = this.sql.exec<{
      instance_id: string;
      domain: string;
    }>(
      "SELECT instance_id, domain FROM domains WHERE customer_id = ? ORDER BY created_at",
      customerId
    );
    const results: Array<{ instanceId: string; domain: string }> = [];
    for (const row of cursor) {
      results.push({ instanceId: row.instance_id, domain: row.domain });
    }
    return results;
  }

  /** Handle management API requests routed to /_admin/... */
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    // Internal lookup endpoint (called by Worker fetch handler).
    if (request.method === "GET" && path === "/_lookup") {
      const domain = url.searchParams.get("domain");
      if (!domain) return jsonError(400, "bad_request", "domain query param required");
      const route = this.lookup(domain);
      if (!route) return jsonError(404, "not_found", "No instance for domain");
      return Response.json(route);
    }

    if (request.method === "POST" && path === "/_admin/instances") {
      return this.handleProvisionInstance(request);
    }
    if (request.method === "POST" && path === "/_admin/domains") {
      return this.handleAddDomain(request);
    }
    if (request.method === "DELETE" && path === "/_admin/domains") {
      return this.handleRemoveDomain(request);
    }
    if (request.method === "GET" && path === "/_admin/instances") {
      return this.handleListInstances(request);
    }

    return jsonError(404, "not_found", "Unknown admin endpoint");
  }

  private async handleProvisionInstance(request: Request): Promise<Response> {
    const body = (await request.json()) as {
      customer_id: string;
      instance_id?: string;
      domain: string;
      admin_email?: string;
      admin_password: string;
      admin_pat: string;
      cookie_secrets: string;
      encryption_keys?: string;
      encryption_key_id?: string;
      d1_database_id?: string;
    };

    if (!body.customer_id || !body.domain || !body.admin_password || !body.admin_pat || !body.cookie_secrets) {
      return jsonError(400, "bad_request", "customer_id, domain, admin_password, admin_pat, cookie_secrets required");
    }

    const instanceId = body.instance_id || `inst_${crypto.randomUUID().slice(0, 12)}`;

    // If no D1 database ID provided, create one via REST API.
    let d1DatabaseId = body.d1_database_id || "";
    if (!d1DatabaseId && this.workerEnv.CLOUDFLARE_ACCOUNT_ID && this.workerEnv.CLOUDFLARE_API_TOKEN) {
      const createResp = await fetch(
        `https://api.cloudflare.com/client/v4/accounts/${this.workerEnv.CLOUDFLARE_ACCOUNT_ID}/d1/database`,
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${this.workerEnv.CLOUDFLARE_API_TOKEN}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ name: `zitadel-${instanceId}` }),
        }
      );
      const createResult = (await createResp.json()) as {
        success: boolean;
        result?: { uuid: string };
        errors?: Array<{ message: string }>;
      };
      if (!createResult.success || !createResult.result?.uuid) {
        return jsonError(500, "d1_create_failed", createResult.errors?.[0]?.message || "Failed to create D1 database");
      }
      d1DatabaseId = createResult.result.uuid;
    }

    // Store config in the container's DO.
    const containerStub = this.workerEnv.ZITADEL.get(
      this.workerEnv.ZITADEL.idFromName(instanceId)
    );
    await containerStub.fetch(
      new Request("http://internal/_config", {
        method: "PUT",
        body: JSON.stringify({
          customer_id: body.customer_id,
          name: body.domain,
          database_type: "d1",
          d1_database_id: d1DatabaseId,
          admin_email: body.admin_email || DEFAULT_ADMIN_EMAIL,
          admin_password: body.admin_password,
          admin_pat: body.admin_pat,
          cookie_secrets: body.cookie_secrets,
          encryption_keys: body.encryption_keys || "",
          encryption_key_id: body.encryption_key_id || "",
          migrate: "auto",
          bootstrap: "auto",
          log_level: DEFAULT_LOG_LEVEL,
          version_channel: "stable",
          state: "active",
        }),
      })
    );

    // Store domain in container's DO.
    await containerStub.fetch(
      new Request("http://internal/_domains", {
        method: "PUT",
        body: JSON.stringify({ domain: body.domain, is_primary: true }),
      })
    );

    // Add domain → instance mapping in router.
    this.addDomain(body.domain, instanceId, body.customer_id, true);

    return Response.json({
      instance_id: instanceId,
      domain: body.domain,
      d1_database_id: d1DatabaseId,
    }, { status: 201 });
  }

  private async handleAddDomain(request: Request): Promise<Response> {
    const body = (await request.json()) as {
      domain: string;
      instance_id: string;
      customer_id: string;
      is_primary?: boolean;
    };

    if (!body.domain || !body.instance_id || !body.customer_id) {
      return jsonError(400, "bad_request", "domain, instance_id, customer_id required");
    }

    this.addDomain(body.domain, body.instance_id, body.customer_id, body.is_primary || false);

    // Also store in container's DO for reference.
    const containerStub = this.workerEnv.ZITADEL.get(
      this.workerEnv.ZITADEL.idFromName(body.instance_id)
    );
    await containerStub.fetch(
      new Request("http://internal/_domains", {
        method: "PUT",
        body: JSON.stringify({
          domain: body.domain,
          is_primary: body.is_primary || false,
        }),
      })
    );

    return Response.json({ status: "added", domain: body.domain });
  }

  private async handleRemoveDomain(request: Request): Promise<Response> {
    const body = (await request.json()) as { domain: string };
    if (!body.domain) {
      return jsonError(400, "bad_request", "domain required");
    }
    this.removeDomain(body.domain);
    return Response.json({ status: "removed", domain: body.domain });
  }

  private async handleListInstances(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const customerId = url.searchParams.get("customer_id");
    if (!customerId) {
      return jsonError(400, "bad_request", "customer_id query param required");
    }
    return Response.json(this.listInstances(customerId));
  }
}

// ── ZitadelContainer DO ──────────────────────────────────────────────────
//
// Per-instance Durable Object that holds the full instance configuration
// and runs the Zitadel container. Config is stored in DO SQLite and passed
// to the container as env vars at boot.

export class ZitadelContainer extends Container {
  defaultPort = 8080;
  sleepAfter = "10m";
  enableInternet = true;

  private configCache: Record<string, string> | null = null;
  private domainsCache: Array<{ domain: string; isPrimary: boolean }> | null = null;
  private schemaReady = false;

  // outboundByHost is registered after the class definition (see below).

  override onStart() {
    console.log("[zitadel] container started");
  }

  override onStop() {
    console.log("[zitadel] container stopped");
  }

  override onError(error: unknown) {
    console.error("[zitadel] container error:", error);
  }

  private ensureSchema(): void {
    if (this.schemaReady) return;
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS config (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
      )
    `);
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS instance_domains (
        domain     TEXT PRIMARY KEY,
        is_primary INTEGER NOT NULL DEFAULT 0
      )
    `);
    this.schemaReady = true;
  }

  private loadConfig(): Record<string, string> {
    if (this.configCache) return this.configCache;
    this.ensureSchema();
    const cursor = this.ctx.storage.sql.exec<{ key: string; value: string }>(
      "SELECT key, value FROM config"
    );
    const config: Record<string, string> = {};
    for (const row of cursor) {
      config[row.key] = row.value;
    }
    this.configCache = config;
    return config;
  }

  private loadDomains(): Array<{ domain: string; isPrimary: boolean }> {
    if (this.domainsCache) return this.domainsCache;
    this.ensureSchema();
    const cursor = this.ctx.storage.sql.exec<{
      domain: string;
      is_primary: number;
    }>("SELECT domain, is_primary FROM instance_domains");
    const domains: Array<{ domain: string; isPrimary: boolean }> = [];
    for (const row of cursor) {
      domains.push({ domain: row.domain, isPrimary: row.is_primary === 1 });
    }
    this.domainsCache = domains;
    return domains;
  }

  private getPrimaryDomain(request: Request): string {
    const domains = this.loadDomains();
    const primary = domains.find((d) => d.isPrimary);
    if (primary) return primary.domain;
    // Fallback: use request host
    return new URL(request.url).host;
  }

  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    // Internal config management endpoints (called by Router DO during provisioning).
    if (url.hostname === "internal") {
      return this.handleInternalRequest(request, url);
    }

    const config = this.loadConfig();

    // Check instance state.
    if (config.state === "suspended") {
      return jsonError(403, "instance_suspended", "This instance is suspended");
    }
    if (config.state === "deleted") {
      return jsonError(404, "instance_deleted", "This instance has been deleted");
    }

    // Build container env vars from stored config.
    const databaseType = config.database_type || "d1";
    const vars: Record<string, string> = {
      ZITADEL_SEED_FILE: "/etc/zitadel/seed.yaml",
      ZITADEL_EXTERNAL_DOMAIN: this.getPrimaryDomain(request),
      ZITADEL_TLS_MODE: resolveTLSMode(request),
      ZITADEL_PROXY_HEADER_MODE: resolveProxyHeaderMode(request),
      ZITADEL_TRUSTED_PROXIES: DEFAULT_TRUSTED_PROXIES,
      ZITADEL_LOG_FORMAT: "json",
      ZITADEL_LOG_LEVEL: config.log_level || DEFAULT_LOG_LEVEL,
      ZITADEL_ADMIN_EMAIL: config.admin_email || DEFAULT_ADMIN_EMAIL,
      ZITADEL_ADMIN_PASSWORD: config.admin_password || "",
      ZITADEL_ADMIN_PAT: config.admin_pat || "",
      ZITADEL_COOKIE_SECRETS: config.cookie_secrets || "",
    };

    if (databaseType === "d1") {
      // Encode the D1 database ID in the URL path so the Worker's fetch handler
      // can extract it from the container's outbound request to d1.local.
      const dbId = config.d1_database_id || "";
      vars.ZITADEL_STORAGE_STATEFUL_URL = `d1://d1.local/${dbId}`;
    } else {
      // BYODB: Turso, Postgres, or other direct URL.
      vars.ZITADEL_STORAGE_STATEFUL_URL = config.database_url || "";
      if (config.database_token) {
        vars.ZITADEL_DATABASE_AUTH_TOKEN = config.database_token;
      }
    }

    if (config.migrate) vars.ZITADEL_STORAGE_STATEFUL_MIGRATE = config.migrate;
    if (config.bootstrap) vars.ZITADEL_STORAGE_STATEFUL_BOOTSTRAP = config.bootstrap;
    if (config.encryption_keys) vars.ZITADEL_ENCRYPTION_KEYS = config.encryption_keys;
    if (config.encryption_key_id) vars.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID = config.encryption_key_id;

    this.envVars = vars;

    // Register the D1 outbound bridge before the container starts.
    // Uses runtime setOutboundByHost which takes priority over static registry.
    if (databaseType === "d1") {
      await this.setOutboundByHost("d1.local", "d1Bridge");
    }

    return super.fetch(request);
  }

  /** Handle internal management requests from the Router DO. */
  private async handleInternalRequest(
    request: Request,
    url: URL
  ): Promise<Response> {
    if (request.method === "PUT" && url.pathname === "/_config") {
      const body = (await request.json()) as Record<string, string>;
      this.ensureSchema();
      for (const [key, value] of Object.entries(body)) {
        this.ctx.storage.sql.exec(
          "INSERT OR REPLACE INTO config (key, value) VALUES (?, ?)",
          key,
          value
        );
      }
      this.configCache = null; // Invalidate cache.
      return Response.json({ status: "ok" });
    }

    if (request.method === "PUT" && url.pathname === "/_domains") {
      const body = (await request.json()) as {
        domain: string;
        is_primary: boolean;
      };
      this.ensureSchema();
      this.ctx.storage.sql.exec(
        "INSERT OR REPLACE INTO instance_domains (domain, is_primary) VALUES (?, ?)",
        body.domain,
        body.is_primary ? 1 : 0
      );
      this.domainsCache = null; // Invalidate cache.
      return Response.json({ status: "ok" });
    }

    if (request.method === "GET" && url.pathname === "/_config") {
      return Response.json(this.loadConfig());
    }

    return jsonError(404, "not_found", "Unknown internal endpoint");
  }
}

// ── D1 Bridge (outboundHandlers) ──────────────────────────────────────────
//
// Named handler registered via outboundHandlers and activated per-instance
// via setOutboundByHost("d1.local", "d1Bridge") in the Container's fetch().
// Uses ctx.containerId to look up the tenant's D1 database ID.

ZitadelContainer.outboundHandlers = {
  d1Bridge: async (
    req: Request,
    env: Env,
    ctx: { containerId: string }
  ): Promise<Response> => {
    const accountId = env.CLOUDFLARE_ACCOUNT_ID;
    const apiToken = env.CLOUDFLARE_API_TOKEN;

    if (!accountId || !apiToken) {
      return Response.json(
        { success: false, error: "D1 bridge: missing CLOUDFLARE_ACCOUNT_ID or CLOUDFLARE_API_TOKEN" },
        { status: 500 }
      );
    }

    // Look up the D1 database ID from the Container DO's own storage.
    const doId = env.ZITADEL.idFromString(ctx.containerId);
    const stub = env.ZITADEL.get(doId);
    const configResp = await stub.fetch(new Request("http://internal/_config"));
    const config = (await configResp.json()) as Record<string, string>;
    const databaseId = config.d1_database_id;

    if (!databaseId) {
      return Response.json(
        { success: false, error: "D1 bridge: no d1_database_id in instance config" },
        { status: 500 }
      );
    }

    const body = (await req.json()) as { sql: string; params?: any[] };

    const d1Resp = await fetch(
      `https://api.cloudflare.com/client/v4/accounts/${accountId}/d1/database/${databaseId}/query`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${apiToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          sql: body.sql,
          params: convertD1Params(body.params ?? []),
        }),
      }
    );

    const d1Result = (await d1Resp.json()) as {
      success: boolean;
      errors?: Array<{ message: string }>;
      result?: Array<{
        success: boolean;
        results: Array<Record<string, any>>;
        meta: Record<string, any>;
      }>;
    };

    const inner = d1Result.result?.[0];

    if (!inner || !d1Result.success) {
      return Response.json({
        success: false,
        error: d1Result.errors?.[0]?.message || "D1 query failed",
        results: [],
        meta: {},
      });
    }

    // Extract column names from first result row for d1driver's meta.columns.
    const columns =
      inner.results?.length > 0 ? Object.keys(inner.results[0]) : [];

    return Response.json({
      success: inner.success,
      results: inner.results ?? [],
      meta: {
        columns,
        changes: inner.meta?.changes ?? 0,
        last_row_id: inner.meta?.last_row_id ?? 0,
        changed_db: (inner.meta?.changes ?? 0) > 0,
        rows_read: inner.meta?.rows_read ?? 0,
        rows_written: inner.meta?.rows_written ?? 0,
        duration: inner.meta?.duration ?? 0,
      },
    });
  },
};

// ── D1 Bridge (Worker fetch fallback) ─────────────────────────────────────
//
// Handles container outbound HTTP to d1.local in the Worker's main fetch()
// handler. The D1 database ID is encoded in the URL path:
//   http://d1.local/<database-id>/query → D1 REST API
//
// This approach works regardless of outboundByHost interception status.

async function d1BridgeHandler(request: Request, env: Env): Promise<Response> {
  const accountId = env.CLOUDFLARE_ACCOUNT_ID;
  const apiToken = env.CLOUDFLARE_API_TOKEN;

  if (!accountId || !apiToken) {
    return Response.json(
      { success: false, error: "D1 bridge: missing CLOUDFLARE_ACCOUNT_ID or CLOUDFLARE_API_TOKEN" },
      { status: 500 }
    );
  }

  // Extract database ID from path: /7c1b4505-.../query → 7c1b4505-...
  const url = new URL(request.url);
  const pathParts = url.pathname.split("/").filter(Boolean);
  // pathParts: ["<database-id>", "query" or "exec"]
  if (pathParts.length < 2) {
    return Response.json(
      { success: false, error: "D1 bridge: invalid path, expected /<database-id>/query" },
      { status: 400 }
    );
  }
  const databaseId = pathParts[0];

  const body = (await request.json()) as { sql: string; params?: any[] };

  const d1Resp = await fetch(
    `https://api.cloudflare.com/client/v4/accounts/${accountId}/d1/database/${databaseId}/query`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        sql: body.sql,
        params: convertD1Params(body.params ?? []),
      }),
    }
  );

  const d1Result = (await d1Resp.json()) as {
    success: boolean;
    errors?: Array<{ message: string }>;
    result?: Array<{
      success: boolean;
      results: Array<Record<string, any>>;
      meta: Record<string, any>;
    }>;
  };

  const inner = d1Result.result?.[0];

  if (!inner || !d1Result.success) {
    const errMsg = d1Result.errors?.[0]?.message || "D1 query failed";
    console.error("[d1-bridge] query failed:", JSON.stringify({
      error: errMsg,
      sql: body.sql.substring(0, 200),
      d1Errors: d1Result.errors,
    }));
    return Response.json({
      success: false,
      error: errMsg,
      results: [],
      meta: {},
    });
  }

  // Extract column names from first result row for d1driver's meta.columns.
  const columns =
    inner.results?.length > 0 ? Object.keys(inner.results[0]) : [];

  return Response.json({
    success: inner.success,
    results: inner.results ?? [],
    meta: {
      columns,
      changes: inner.meta?.changes ?? 0,
      last_row_id: inner.meta?.last_row_id ?? 0,
      changed_db: (inner.meta?.changes ?? 0) > 0,
      rows_read: inner.meta?.rows_read ?? 0,
      rows_written: inner.meta?.rows_written ?? 0,
      duration: inner.meta?.duration ?? 0,
    },
  });
}

// ── D1 Param Conversion ───────────────────────────────────────────────────
//
// The Go d1driver encodes []byte params as {"__d1_type":"blob","base64":"..."}
// but the D1 REST API expects blob params as arrays of byte numbers [1,2,3].
// This function converts between the two formats.

function convertD1Params(params: any[]): any[] {
  return params.map((p) => {
    if (p && typeof p === "object" && p.__d1_type === "blob" && typeof p.base64 === "string") {
      // Decode base64 to array of byte numbers for D1 REST API.
      const binary = atob(p.base64);
      const bytes = new Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      return bytes;
    }
    return p;
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────

function resolveTLSMode(request: Request): string {
  return new URL(request.url).protocol === "https:" ? "external" : "off";
}

function resolveProxyHeaderMode(request: Request): string {
  return request.headers.has("CF-Connecting-IP") ? "cloudflare" : "standard";
}

function jsonError(status: number, error: string, message: string): Response {
  return new Response(JSON.stringify({ error, message }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function isDynamicAssetRequest(pathname: string): boolean {
  return pathname.startsWith(DYNAMIC_ASSET_PREFIX);
}

function isStaticAssetRequest(pathname: string): boolean {
  return pathname.startsWith(STATIC_ASSET_PREFIX) && !isDynamicAssetRequest(pathname);
}

function isInternalAssetRequest(pathname: string): boolean {
  return pathname.startsWith(INTERNAL_ASSET_PREFIX);
}

// ── Worker Entry ──────────────────────────────────────────────────────────

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    // Health check — no container needed.
    if (url.pathname === "/_health") {
      return new Response("ok", { status: 200 });
    }

    // Block direct access to internal build paths.
    if (isInternalAssetRequest(url.pathname)) {
      return new Response("Not found", { status: 404 });
    }

    // Serve hashed frontend bundles from Workers Assets.
    if (isStaticAssetRequest(url.pathname)) {
      if (!env.ASSETS) {
        return jsonError(500, "misconfigured_worker", "Missing ASSETS binding");
      }
      return env.ASSETS.fetch(request);
    }

    // Admin endpoints → Router DO.
    if (url.pathname.startsWith("/_admin/")) {
      const routerStub = env.ROUTER.get(env.ROUTER.idFromName(ROUTER_DO_NAME));
      return routerStub.fetch(request);
    }

    // D1 bridge — handle outbound d1.local requests from container.
    // Cloudflare Containers' outboundByHost isn't intercepting these, so we
    // catch them here in the Worker's main fetch handler instead.
    if (url.hostname === "d1.local") {
      return d1BridgeHandler(request, env);
    }

    // Domain lookup via Router DO.
    const routerStub = env.ROUTER.get(env.ROUTER.idFromName(ROUTER_DO_NAME));
    const lookupResp = await routerStub.fetch(
      new Request(`http://internal/_lookup?domain=${encodeURIComponent(url.hostname)}`)
    );
    if (lookupResp.status === 404) {
      return jsonError(404, "not_found", `No instance configured for ${url.hostname}`);
    }
    const route = (await lookupResp.json()) as InstanceRoute;

    // Route to the per-instance container.
    try {
      const container = getContainer(env.ZITADEL, route.instanceId);
      return await container.fetch(request);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error(
        "[zitadel] proxy error:",
        JSON.stringify({
          error: message,
          url: url.toString(),
          instance: route.instanceId,
        })
      );

      return new Response(
        JSON.stringify({
          error: "service_unavailable",
          message,
          detail: "Zitadel is starting up. Retry in a few seconds.",
        }),
        {
          status: 503,
          headers: {
            "Content-Type": "application/json",
            "Retry-After": "10",
          },
        }
      );
    }
  },
};
