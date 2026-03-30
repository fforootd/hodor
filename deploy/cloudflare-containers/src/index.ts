/**
 * Zitadel — Cloudflare Workers + Containers + D1
 *
 * Architecture:
 *
 *   Client → CF Edge (Worker) → Durable Object → Container (:8080)
 *                                                       ↓
 *                                               http://d1.local/*
 *                                                       ↓
 *                                          outboundByHost handler
 *                                                       ↓
 *                                                 env.DB (D1)
 *
 * The Worker serves two roles:
 *   1. Edge proxy — routes HTTP traffic to the Zitadel container
 *   2. D1 proxy — intercepts outbound container HTTP to http://d1.local
 *      and forwards SQL queries to the D1 binding
 *
 * Inside the container, Zitadel connects with:
 *   ZITADEL_DATABASE_URL=d1://d1.local
 *
 * The Go d1driver package (internal/database/d1driver) sends SQL over HTTP
 * to the virtual hostname, which the outboundByHost handler intercepts.
 */

import { Container } from "@cloudflare/containers";

// NOTE: When @cloudflare/containers adds outbound support, re-enable:
// import { Container, ContainerProxy } from "@cloudflare/containers";
// export { ContainerProxy };

// ── Container Class ────────────────────────────────────────────────────────

export class ZitadelContainer extends Container {
  defaultPort = 8080;
  sleepAfter = "30m";

  // Use SQLite on the container filesystem for now.
  // D1 can be enabled once outboundByHost support is confirmed.
  //
  // Admin credentials: set via `wrangler secret put` or override below.
  // The seed file at /etc/zitadel/seed.yaml reads these env vars
  // via ${VAR} substitution to create a deterministic admin user.
  envVars = {
    ZITADEL_DATABASE_URL: "sqlite:///data/zitadel.db",
    ZITADEL_DEV_SEED_FILE: "/etc/zitadel/seed.yaml",
    ZITADEL_ADMIN_PASSWORD: "CHANGE-ME-on-first-login-2026!",
    ZITADEL_ADMIN_EMAIL: "admin@zitadel.cloud",
    ZITADEL_ADMIN_PAT: "zitadel-cloud-bootstrap-pat",
    ZITADEL_OBSERVABILITY_LOG_FORMAT: "json",
    ZITADEL_OBSERVABILITY_LOG_LEVEL: "info",
  };

  override onStart() {
    console.log("[zitadel] container started, database: D1 via outbound proxy");
  }

  override onStop() {
    console.log("[zitadel] container stopped (scale-to-zero)");
  }

  override onError(error: unknown) {
    console.error("[zitadel] container error:", error);
  }
}

// ── D1 Outbound Proxy (disabled — awaiting @cloudflare/containers support) ──
// When the package adds outboundByHost support, uncomment to enable D1:
//
// ZitadelContainer.outboundByHost = {
//   "d1.local": async (request: Request, env: Env) => {
//     const body = await request.json();
//     const stmt = env.DB.prepare(body.sql);
//     const bound = body.params?.length ? stmt.bind(...body.params) : stmt;
//     const result = await bound.run();
//     return Response.json({ success: result.success, results: result.results ?? [], meta: result.meta });
//   },
// };

// ── Types ──────────────────────────────────────────────────────────────────

interface Env {
  ZITADEL: DurableObjectNamespace;
  DB: D1Database;
}

// ── Session Affinity ───────────────────────────────────────────────────────

const COOKIE = "__zitadel_cid";

function readCookie(req: Request): string | null {
  const hdr = req.headers.get("Cookie") || "";
  const m = hdr.match(new RegExp(`${COOKIE}=([^;]+)`));
  return m ? m[1] : null;
}

function setCookie(res: Response, id: string): Response {
  const headers = new Headers(res.headers);
  headers.append(
    "Set-Cookie",
    `${COOKIE}=${id}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400`
  );
  return new Response(res.body, {
    status: res.status,
    statusText: res.statusText,
    headers,
  });
}

// ── Worker Fetch Handler ───────────────────────────────────────────────────

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    // /_health — lightweight check (doesn't wake the container).
    if (url.pathname === "/_health") {
      return new Response("ok", { status: 200 });
    }

    // Route to a named container instance.
    const instanceId = readCookie(request) || "primary";

    try {
      const container = env.ZITADEL.getByName(instanceId);
      const response = await container.fetch(request);
      return setCookie(response, instanceId);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const stack = err instanceof Error ? err.stack : undefined;
      console.error("[zitadel] proxy error:", JSON.stringify({
        instanceId,
        error: message,
        stack,
        url: url.toString(),
      }));
      return new Response(
        JSON.stringify({
          error: "service_unavailable",
          message,
          detail: "Zitadel container is starting up. Please retry in a few seconds.",
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
