import { Container, getContainer } from "@cloudflare/containers";

const ZITADEL_CONTAINER_NAME = "zitadel";
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
  ASSETS?: Fetcher;
  ZITADEL: DurableObjectNamespace<ZitadelContainer>;
  ZITADEL_ADMIN_PASSWORD?: string;
  ZITADEL_ADMIN_PAT?: string;
  ZITADEL_COOKIE_SECRETS?: string;
  ZITADEL_DATABASE_URL?: string;
  ZITADEL_DATABASE_AUTH_TOKEN?: string;
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

const STATIC_ASSET_PREFIX = "/assets/";
const DYNAMIC_ASSET_PREFIX = "/assets/login/";
const INTERNAL_ASSET_PREFIX = "/src/";

export class ZitadelContainer extends Container {
  defaultPort = 8080;
  sleepAfter = "10m";
  enableInternet = true;

  private readonly workerEnv: Env;

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
    // Refresh startup env on every request so secret rotations and
    // host-derived config changes are picked up by the next container start.
    this.envVars = buildContainerEnv(request, this.workerEnv);

    return super.fetch(request);
  }
}

function requiredWorkerConfig(env: Env): string[] {
  const missing: string[] = [];
  const databaseURL = env.ZITADEL_DATABASE_URL?.trim() || "";

  if (!env.ZITADEL_ADMIN_PASSWORD) {
    missing.push("ZITADEL_ADMIN_PASSWORD");
  }
  if (!env.ZITADEL_ADMIN_PAT) {
    missing.push("ZITADEL_ADMIN_PAT");
  }
  if (!env.ZITADEL_COOKIE_SECRETS) {
    missing.push("ZITADEL_COOKIE_SECRETS");
  }
  if (!databaseURL) {
    missing.push("ZITADEL_DATABASE_URL");
  }
  if (databaseRequiresAuthToken(databaseURL, env) && !env.ZITADEL_DATABASE_AUTH_TOKEN?.trim()) {
    missing.push("ZITADEL_DATABASE_AUTH_TOKEN");
  }

  return missing;
}

function databaseRequiresAuthToken(databaseURL: string, env: Env): boolean {
  if (!databaseURL || env.ZITADEL_DATABASE_AUTH_TOKEN?.trim()) {
    return false;
  }

  try {
    const url = new URL(databaseURL);
    const embeddedToken =
      url.searchParams.get("authToken") ||
      url.searchParams.get("auth_token") ||
      url.searchParams.get("jwt");

    if (embeddedToken?.trim()) {
      return false;
    }

    return url.hostname.endsWith(".turso.io");
  } catch {
    return false;
  }
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
    ZITADEL_DATABASE_URL: env.ZITADEL_DATABASE_URL?.trim() || "",
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
  if (env.ZITADEL_DATABASE_AUTH_TOKEN?.trim()) {
    vars.ZITADEL_DATABASE_AUTH_TOKEN = env.ZITADEL_DATABASE_AUTH_TOKEN.trim();
  }
  if (env.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID?.trim()) {
    vars.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID = env.ZITADEL_ENCRYPTION_ACTIVE_KEY_ID.trim();
  }
  if (env.ZITADEL_ENCRYPTION_KEYS?.trim()) {
    vars.ZITADEL_ENCRYPTION_KEYS = env.ZITADEL_ENCRYPTION_KEYS.trim();
  }

  return vars;
}

function serviceUnavailable(message: string): Response {
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

function jsonError(status: number, error: string, message: string): Response {
  return new Response(JSON.stringify({ error, message }), {
    status,
    headers: {
      "Content-Type": "application/json",
    },
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

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/_health") {
      return new Response("ok", { status: 200 });
    }

    // Block direct access to internal build paths like /src/console/index.html.
    if (isInternalAssetRequest(url.pathname)) {
      return new Response("Not found", { status: 404 });
    }

    // Serve hashed frontend bundles from Workers Assets so the container only
    // handles dynamic/API traffic.
    if (isStaticAssetRequest(url.pathname)) {
      if (!env.ASSETS) {
        return jsonError(500, "misconfigured_worker", "Missing ASSETS binding for static asset delivery");
      }
      return env.ASSETS.fetch(request);
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
      const container = getContainer(env.ZITADEL, ZITADEL_CONTAINER_NAME);
      return await container.fetch(request);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const stack = error instanceof Error ? error.stack : undefined;

      console.error(
        "[zitadel] proxy error:",
        JSON.stringify({
          error: message,
          stack,
          url: url.toString(),
          container: ZITADEL_CONTAINER_NAME,
        })
      );

      return serviceUnavailable(message);
    }
  },
};
