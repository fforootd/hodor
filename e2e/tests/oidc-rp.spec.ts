import { createHash, randomBytes, randomUUID } from "node:crypto";
import { once } from "node:events";
import { createServer, type Server } from "node:http";

import {
  chromium,
  expect,
  test,
  type APIRequestContext,
  type Page,
} from "@playwright/test";

const appBaseURL = process.env.BASE_URL || "http://127.0.0.1:18080";
const mockOIDCPassword = "password123";
const opClientId = "e2e-browser-client";
const opClientSecret = "e2e-browser-secret";
const opCallbackOrigin = "http://127.0.0.1:9877";
const opRedirectURI = `${opCallbackOrigin}/callback`;
const providerIds = {
  happy: "prov_mock_oidc",
  existingUser: "prov_mock_oidc_existing_user",
  linkOnly: "prov_mock_oidc_link_only",
  userinfoOnly: "prov_mock_oidc_userinfo_only",
  nonceMismatch: "prov_mock_oidc_nonce_mismatch",
  tokenFailure: "prov_mock_oidc_token_failure",
  accessDenied: "prov_mock_oidc_access_denied",
} as const;

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function createPKCE() {
  const verifier = randomBytes(32).toString("base64url");
  const challenge = createHash("sha256").update(verifier).digest("base64url");
  return { verifier, challenge };
}

function buildAuthorizeURL(options?: { prompt?: string }) {
  const pkce = createPKCE();
  const state = randomUUID();
  const nonce = randomUUID();
  const url = new URL("/authorize", appBaseURL);
  url.searchParams.set("client_id", opClientId);
  url.searchParams.set("redirect_uri", opRedirectURI);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("scope", "openid profile email");
  url.searchParams.set("state", state);
  url.searchParams.set("nonce", nonce);
  url.searchParams.set("code_challenge", pkce.challenge);
  url.searchParams.set("code_challenge_method", "S256");
  if (options?.prompt) {
    url.searchParams.set("prompt", options.prompt);
  }
  return { url: url.toString(), state, pkce };
}

async function readJSON(
  response: Awaited<ReturnType<APIRequestContext["post"]>>,
) {
  const contentType = response.headers()["content-type"] || "";
  if (!contentType.includes("application/json")) {
    return null;
  }
  return response.json();
}

async function exchangeAuthorizationCode(
  request: APIRequestContext,
  code: string,
  codeVerifier: string,
) {
  const response = await request.post("/oauth/token", {
    failOnStatusCode: false,
    form: {
      grant_type: "authorization_code",
      code,
      client_id: opClientId,
      client_secret: opClientSecret,
      redirect_uri: opRedirectURI,
      code_verifier: codeVerifier,
    },
  });

  return {
    response,
    body: await readJSON(response),
  };
}

async function browserJSON(page: Page, path: string) {
  return page.evaluate(async (target) => {
    const response = await fetch(target, {
      credentials: "include",
      headers: {
        Accept: "application/json",
      },
    });
    const contentType = response.headers.get("content-type") || "";
    const body = contentType.includes("application/json")
      ? await response.json()
      : await response.text();
    return { status: response.status, body };
  }, path);
}

async function completeMockOIDCLogin(page: Page, email?: string) {
  await expect(page.locator('input[name="email"]')).toBeVisible({
    timeout: 15_000,
  });
  if (email) {
    await page.locator('input[name="email"]').fill(email);
  }
  await page.locator('input[name="password"]').fill(mockOIDCPassword);
  await page.getByRole("button", { name: /Sign in/i }).click();
}

async function startRPLogin(page: Page, providerID: string) {
  await page.goto(`${appBaseURL}/v1/auth/sso/${providerID}/start`);
}

async function expectExitState(page: Page, title = "Sign-in complete") {
  await page.waitForURL(/\/login\?/, { timeout: 15_000 });
  await expect(page.getByTestId("login-exit-state")).toBeVisible();
  await expect(page.getByTestId("login-exit-title")).toHaveText(title);
}

async function expectLoginError(page: Page, errorCode: string) {
  await page.waitForURL(
    new RegExp(`/login\\?.*error=${escapeRegex(errorCode)}`),
    { timeout: 15_000 },
  );
  await expect(page).toHaveURL(
    new RegExp(`/login\\?.*error=${escapeRegex(errorCode)}`),
  );
}

async function detectOIDCEntryState(page: Page) {
  const loginInput = page
    .locator(
      'input[name="identifier"], input[type="text"], input[type="email"], input[name="password"], input[type="password"]',
    )
    .first();
  const sessionReuseHeading = page.getByRole("heading", {
    name: /Use your existing session\?/i,
  });

  for (let attempt = 0; attempt < 75; attempt += 1) {
    if (page.url().startsWith(opRedirectURI)) {
      return "callback" as const;
    }
    if (await sessionReuseHeading.isVisible().catch(() => false)) {
      return "session_reuse" as const;
    }
    if (await loginInput.isVisible().catch(() => false)) {
      return "login" as const;
    }
    await page.waitForTimeout(200);
  }

  throw new Error(
    `OIDC flow did not reach login, session reuse, or callback. Current URL: ${page.url()}`,
  );
}

class CallbackHarness {
  private server: Server | null = null;
  private lastURL: string | null = null;

  async start() {
    if (this.server) return;

    this.server = createServer((req, res) => {
      const url = new URL(req.url || "/", opCallbackOrigin);
      if (url.pathname === "/callback") {
        this.lastURL = url.toString();
        res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
        res.end("<html><body>OIDC callback received</body></html>");
        return;
      }
      if (url.pathname === "/healthz") {
        res.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
        res.end("ok");
        return;
      }
      res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      res.end("not found");
    });

    this.server.listen(9877, "127.0.0.1");
    await once(this.server, "listening");
  }

  reset() {
    this.lastURL = null;
  }

  lastCallback() {
    return this.lastURL ? new URL(this.lastURL) : null;
  }

  async stop() {
    if (!this.server) return;
    const server = this.server;
    this.server = null;
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve();
      });
    });
  }
}

const callbackHarness = new CallbackHarness();

async function waitForCallback(page: Page, state: string) {
  await page.waitForURL(new RegExp(`^${escapeRegex(opRedirectURI)}\\?`), {
    timeout: 15_000,
  });
  const callbackURL = new URL(page.url());
  expect(callbackURL.searchParams.get("state")).toBe(state);
  const harnessCallback = callbackHarness.lastCallback();
  expect(harnessCallback?.searchParams.get("state")).toBe(state);
  return callbackURL;
}

async function withIsolatedPage(run: (page: Page) => Promise<void>) {
  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    await run(page);
  } finally {
    await context.close();
    await browser.close();
  }
}

test.describe.serial("OIDC RP end-to-end", () => {
  test.beforeAll(async () => {
    await callbackHarness.start();
  });

  test.afterAll(async () => {
    await callbackHarness.stop();
  });

  test.beforeEach(() => {
    callbackHarness.reset();
  });

  test("RP happy path lands on the login exit state and creates an SSO session @smoke", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.happy);
      await completeMockOIDCLogin(page);

      await expectExitState(page);

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(200);
      expect(profile.body.identity.identifier).toBe("mock-rp-user@example.com");

      const sessions = await browserJSON(page, "/v1/account/sessions");
      expect(sessions.status).toBe(200);
      expect(sessions.body.count).toBeGreaterThan(0);
      expect(sessions.body.sessions[0].auth_method).toBe("sso");
      expect(sessions.body.sessions[0].provider_id).toBe(providerIds.happy);
      expect(sessions.body.sessions[0].provider_kind).toBe("custom");
    });
  });

  test("create_or_link reuses the existing local user when the upstream email is verified @full", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.existingUser);
      await completeMockOIDCLogin(page, "e2e-user@example.com");

      await expectExitState(page);

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(200);
      expect(profile.body.identity.identifier).toBe("e2e-user@example.com");

      const sessions = await browserJSON(page, "/v1/account/sessions");
      expect(sessions.status).toBe(200);
      expect(sessions.body.sessions[0].provider_id).toBe(
        providerIds.existingUser,
      );
    });
  });

  test("link_only rejects users without an existing linked identity @full", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.linkOnly);
      await completeMockOIDCLogin(page, "unlinked-rp-user@example.com");

      await expectLoginError(page, "sso_link_failed");

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(401);
    });
  });

  test("userinfo fallback works when the upstream token response omits the ID token @full", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.userinfoOnly);
      await completeMockOIDCLogin(page, "userinfo-rp-user@example.com");

      await expectExitState(page);

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(200);
      expect(profile.body.identity.identifier).toBe(
        "userinfo-rp-user@example.com",
      );

      const sessions = await browserJSON(page, "/v1/account/sessions");
      expect(sessions.status).toBe(200);
      expect(sessions.body.sessions[0].auth_method).toBe("sso");
      expect(sessions.body.sessions[0].provider_id).toBe(
        providerIds.userinfoOnly,
      );
    });
  });

  test("nonce mismatch returns to login with sso_nonce @full", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.nonceMismatch);
      await completeMockOIDCLogin(page, "nonce-rp-user@example.com");

      await expectLoginError(page, "sso_nonce");

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(401);
    });
  });

  test("token exchange failure returns to login with sso_token @full", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.tokenFailure);
      await completeMockOIDCLogin(page, "token-failure-rp-user@example.com");

      await expectLoginError(page, "sso_token");

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(401);
    });
  });

  test("upstream access_denied returns to login with sso_failed @full", async () => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.accessDenied);
      await completeMockOIDCLogin(page);

      await expectLoginError(page, "sso_failed");

      const profile = await browserJSON(page, "/v1/account/profile");
      expect(profile.status).toBe(401);
    });
  });

  test("an SSO-created session is reusable when Zitadel acts as an OP later in the same browser context @smoke", async ({
    request,
  }) => {
    await withIsolatedPage(async (page) => {
      await startRPLogin(page, providerIds.happy);
      await completeMockOIDCLogin(page);
      await expectExitState(page);

      callbackHarness.reset();
      const auth = buildAuthorizeURL();
      await page.goto(auth.url);

      const entryState = await detectOIDCEntryState(page);
      expect(entryState).toBe("session_reuse");

      await page
        .getByRole("button", { name: /Continue with this session/i })
        .click();

      const callbackURL = await waitForCallback(page, auth.state);
      const code = callbackURL.searchParams.get("code");
      expect(code).toBeTruthy();

      const exchanged = await exchangeAuthorizationCode(
        request,
        code || "",
        auth.pkce.verifier,
      );
      expect(exchanged.response.ok()).toBeTruthy();
      expect(exchanged.body?.access_token).toBeTruthy();
      expect(exchanged.body?.id_token).toBeTruthy();
    });
  });
});
