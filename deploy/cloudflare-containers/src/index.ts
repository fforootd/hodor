import { Container, ContainerProxy, getRandom } from "@cloudflare/containers";

export { ContainerProxy };

const CONTAINER_POOL_SIZE = 1;
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

interface Env {
  ZITADEL: DurableObjectNamespace<ZitadelContainer>;
  DB: D1Database;
  ZITADEL_ADMIN_PASSWORD?: string;
  ZITADEL_ADMIN_PAT?: string;
  ZITADEL_COOKIE_SECRETS?: string;
  ZITADEL_ADMIN_EMAIL?: string;
  ZITADEL_EXTERNAL_DOMAIN?: string;
  ZITADEL_TLS_MODE?: string;
  ZITADEL_PROXY_HEADER_MODE?: string;
  ZITADEL_TRUSTED_PROXIES?: string;
  ZITADEL_LOG_LEVEL?: string;
  ZITADEL_BASE_PATH?: string;
  ZITADEL_DATABASE_MIGRATE?: string;
  ZITADEL_DATABASE_BOOTSTRAP?: string;
  ZITADEL_ENCRYPTION_ACTIVE_KEY_ID?: string;
  ZITADEL_ENCRYPTION_KEYS?: string;
}

interface D1ProxyRequest {
  sql: string;
  params?: D1ProxyParam[];
}

interface D1BlobParam {
  __d1_type: "blob";
  base64: string;
}

interface D1ProxyMeta {
  columns?: string[];
  changes: number;
  last_row_id: number;
  changed_db: boolean;
  rows_read: number;
  rows_written: number;
  duration: number;
}

type D1ProxyParam = unknown | D1BlobParam;

// ── Container Durable Object ────────────────────────────────────────────────
//
// The SDK's built-in containerFetch() handles the full lifecycle:
//   start → wait for port → proxy request → renew activity timeout
//
// We only override fetch() to inject env vars on the first request before
// handing off to containerFetch(). No manual start/stop/retry logic needed.
// ─────────────────────────────────────────────────────────────────────────────

export class ZitadelContainer extends Container {
  defaultPort = 8080;
  sleepAfter = "10m";
  private readonly workerEnv: Env;
  private configured = false;

  constructor(ctx: any, env: Env) {
    super(ctx, env);
    this.workerEnv = env;
  }

  override onStart() {
    console.log("[zitadel] container started");
  }

  override onStop() {
    console.log("[zitadel] container stopped");
  }

  override onError(error: unknown) {
    console.error("[zitadel] container error:", error);
  }

  override async fetch(request: Request): Promise<Response> {
    // Lock env vars to the first request so concurrent requests during
    // startup don't race and overwrite each other.
    if (!this.configured) {
      this.envVars = buildContainerEnv(request, this.workerEnv);
      this.configured = true;

      // Explicit start with enableInternet and generous timeouts.
      // containerFetch() defaults don't pass enableInternet (blocks
      // non-intercepted outbound) and use short timeouts that can't
      // survive Zitadel's migration-heavy first boot.
      await this.startAndWaitForPorts({
        ports: this.defaultPort,
        startOptions: {
          envVars: this.envVars,
          enableInternet: true,
        },
        cancellationOptions: {
          instanceGetTimeoutMS: 300_000,
          portReadyTimeoutMS: 300_000,
          waitInterval: 2_000,
        },
      });
    }

    // Container is running — containerFetch sees it and just proxies.
    return this.containerFetch(request);
  }
}

// ── D1 Outbound Handler ─────────────────────────────────────────────────────
//
// Assigned outside the class body so the SDK's static getter/setter on the
// Container base class correctly registers the handler. Defining it as a
// static field *inside* the class can shadow the inherited getter/setter
// rather than calling it, causing the handler to be silently ignored.
//
// The handler signature is (request, env, ctx) per the SDK contract.
// ─────────────────────────────────────────────────────────────────────────────

ZitadelContainer.outboundByHost = {
  "d1.local": async (request: Request, env: Env, _ctx: any): Promise<Response> => {
    try {
      const payload = await parseD1Request(request);
      const params = payload.params?.map(decodeD1Param) ?? [];
      const statement =
        params.length > 0
          ? env.DB.prepare(payload.sql).bind(...params)
          : env.DB.prepare(payload.sql);
      const pathname = new URL(request.url).pathname;
      console.log(
        "[zitadel] d1 request",
        JSON.stringify({
          path: pathname,
          sql: summarizeSQL(payload.sql),
          params: payload.params?.length ?? 0,
        })
      );

      switch (pathname) {
        case "/query":
          return await executeQuery(statement);
        case "/exec":
          return await executeExec(statement);
        default:
          return jsonError(404, "not_found", "Unsupported D1 proxy endpoint.");
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[zitadel] d1 proxy error:", message);
      return Response.json(
        {
          success: false,
          error: message,
          results: [],
          meta: zeroMeta(),
        },
        { status: 500 }
      );
    }
  },
};

// ── D1 Proxy Helpers ────────────────────────────────────────────────────────

async function parseD1Request(request: Request): Promise<D1ProxyRequest> {
  const payload = (await request.json()) as Partial<D1ProxyRequest>;
  if (!payload || typeof payload.sql !== "string" || payload.sql.trim() === "") {
    throw new Error("invalid D1 proxy payload: expected non-empty sql");
  }
  if (payload.params !== undefined && !Array.isArray(payload.params)) {
    throw new Error("invalid D1 proxy payload: params must be an array");
  }
  return {
    sql: payload.sql,
    params: payload.params,
  };
}

function decodeD1Param(param: D1ProxyParam): unknown {
  if (isD1BlobParam(param)) {
    return Uint8Array.from(atob(param.base64), (char) => char.charCodeAt(0));
  }
  return param;
}

function isD1BlobParam(param: D1ProxyParam): param is D1BlobParam {
  return Boolean(
    param &&
      typeof param === "object" &&
      "__d1_type" in param &&
      (param as { __d1_type?: unknown }).__d1_type === "blob" &&
      "base64" in param &&
      typeof (param as { base64?: unknown }).base64 === "string"
  );
}

async function executeQuery(statement: D1PreparedStatement): Promise<Response> {
  const raw = await statement.raw({ columnNames: true });
  const columns = raw.length > 0 ? ((raw[0] as unknown[]) ?? []).map(String) : [];
  const rows = raw.slice(1);
  const results = rows.map((row) =>
    Object.fromEntries(columns.map((column, index) => [column, row[index] ?? null]))
  );
  return Response.json({
    success: true,
    results,
    meta: {
      ...zeroMeta(),
      columns,
      rows_read: results.length,
    },
  });
}

async function executeExec(statement: D1PreparedStatement): Promise<Response> {
  const result = await statement.run<Record<string, unknown>>();
  return Response.json({
    success: result.success,
    results: result.results ?? [],
    meta: {
      ...zeroMeta(),
      ...result.meta,
    },
  });
}

function zeroMeta(): D1ProxyMeta {
  return {
    changes: 0,
    last_row_id: 0,
    changed_db: false,
    rows_read: 0,
    rows_written: 0,
    duration: 0,
  };
}

function jsonError(status: number, error: string, message: string): Response {
  return new Response(JSON.stringify({ error, message }), {
    status,
    headers: {
      "Content-Type": "application/json",
    },
  });
}

function summarizeSQL(sql: string): string {
  return sql.replace(/\s+/g, " ").trim().slice(0, 200);
}

// ── Worker Config Helpers ───────────────────────────────────────────────────

function requiredWorkerConfig(env: Env): string[] {
  const missing: string[] = [];
  if (!env.ZITADEL_ADMIN_PASSWORD) {
    missing.push("ZITADEL_ADMIN_PASSWORD");
  }
  if (!env.ZITADEL_ADMIN_PAT) {
    missing.push("ZITADEL_ADMIN_PAT");
  }
  if (!env.ZITADEL_COOKIE_SECRETS) {
    missing.push("ZITADEL_COOKIE_SECRETS");
  }
  return missing;
}

function resolveExternalDomain(request: Request, env: Env): string {
  return env.ZITADEL_EXTERNAL_DOMAIN?.trim() || new URL(request.url).host;
}

function resolveTLSMode(request: Request, env: Env): string {
  return env.ZITADEL_TLS_MODE?.trim() || (new URL(request.url).protocol === "https:" ? "external" : "off");
}

function resolveProxyHeaderMode(request: Request, env: Env): string {
  if (env.ZITADEL_PROXY_HEADER_MODE?.trim()) {
    return env.ZITADEL_PROXY_HEADER_MODE.trim();
  }
  return request.headers.has("CF-Connecting-IP") ? "cloudflare" : "standard";
}

function buildContainerEnv(request: Request, env: Env): Record<string, string> {
  const vars: Record<string, string> = {
    ZITADEL_DATABASE_URL: "d1://d1.local",
    ZITADEL_SEED_FILE: "/etc/zitadel/seed.yaml",
    ZITADEL_ADMIN_PASSWORD: env.ZITADEL_ADMIN_PASSWORD ?? "",
    ZITADEL_ADMIN_EMAIL: env.ZITADEL_ADMIN_EMAIL?.trim() || DEFAULT_ADMIN_EMAIL,
    ZITADEL_ADMIN_PAT: env.ZITADEL_ADMIN_PAT ?? "",
    ZITADEL_COOKIE_SECRETS: env.ZITADEL_COOKIE_SECRETS ?? "",
    ZITADEL_EXTERNAL_DOMAIN: resolveExternalDomain(request, env),
    ZITADEL_TLS_MODE: resolveTLSMode(request, env),
    ZITADEL_PROXY_HEADER_MODE: resolveProxyHeaderMode(request, env),
    ZITADEL_TRUSTED_PROXIES: env.ZITADEL_TRUSTED_PROXIES?.trim() || DEFAULT_TRUSTED_PROXIES,
    ZITADEL_LOG_FORMAT: "json",
    ZITADEL_LOG_LEVEL: env.ZITADEL_LOG_LEVEL?.trim() || DEFAULT_LOG_LEVEL,
  };

  if (env.ZITADEL_BASE_PATH?.trim()) {
    vars.ZITADEL_BASE_PATH = env.ZITADEL_BASE_PATH.trim();
  }
  if (env.ZITADEL_DATABASE_MIGRATE?.trim()) {
    vars.ZITADEL_DATABASE_MIGRATE = env.ZITADEL_DATABASE_MIGRATE.trim();
  }
  if (env.ZITADEL_DATABASE_BOOTSTRAP?.trim()) {
    vars.ZITADEL_DATABASE_BOOTSTRAP = env.ZITADEL_DATABASE_BOOTSTRAP.trim();
  }
  if (env.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID?.trim()) {
    vars.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID = env.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID.trim();
  }
  if (env.ZITADEL_ENCRYPTION_KEYS?.trim()) {
    vars.ZITADEL_ENCRYPTION_KEYS = env.ZITADEL_ENCRYPTION_KEYS.trim();
  }

  return vars;
}

// ── Worker Entrypoint ───────────────────────────────────────────────────────

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/_health") {
      return new Response("ok", { status: 200 });
    }

    const missing = requiredWorkerConfig(env);
    if (missing.length > 0) {
      return jsonError(
        500,
        "misconfigured_worker",
        `Missing required Worker secrets: ${missing.join(", ")}`
      );
    }

    try {
      const container = await getRandom(env.ZITADEL, CONTAINER_POOL_SIZE);
      return await container.fetch(request);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const stack = err instanceof Error ? err.stack : undefined;
      console.error(
        "[zitadel] proxy error:",
        JSON.stringify({
          poolSize: CONTAINER_POOL_SIZE,
          error: message,
          stack,
          url: url.toString(),
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
