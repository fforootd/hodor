import { b as d } from "./_plugin-vue_export-helper-DHhFP0j4.js";
function f(t) {
  const n = d()?.proxy?.$el;
  return n && n.closest(t) || null;
}
function w(t, n, o) {
  const e = f(t);
  return e ? e.dispatchEvent(new CustomEvent(n, {
    detail: o,
    bubbles: !0,
    composed: !0
  })) : (console.warn(`[${t}] Could not find host element to dispatch "${n}"`), !1);
}
function g(t) {
  return t || window.__ZITADEL_BASE_PATH__ || "";
}
function m(t) {
  return t === "dark" ? !0 : t === "auto" ? window.matchMedia("(prefers-color-scheme: dark)").matches : !1;
}
function h(t) {
  if (!t) return "same-origin";
  try {
    return new URL(t, window.location.origin).origin !== window.location.origin ? "include" : "same-origin";
  } catch {
    return "same-origin";
  }
}
var l = class extends Error {
  constructor(t, n, o) {
    super(t), this.name = "WCApiError", this.status = n, this.code = o || `HTTP_${n}`;
  }
  get isUnauthorized() {
    return this.status === 401;
  }
  get isForbidden() {
    return this.status === 403;
  }
};
function y(t) {
  const n = h(t);
  async function o(e, r = {}) {
    const s = await fetch(`${t}${e}`, {
      ...r,
      headers: {
        "Content-Type": "application/json",
        ...r.headers
      },
      credentials: n
    });
    if (!s.ok) {
      const a = await s.json().catch(() => ({ error: s.statusText })), c = a.error || `HTTP ${s.status}`, u = a.code || void 0;
      throw new l(c, s.status, u);
    }
    const i = await s.text();
    if (i)
      return JSON.parse(i);
  }
  return {
    get: (e) => o(e),
    post: (e, r) => o(e, {
      method: "POST",
      body: JSON.stringify(r)
    }),
    put: (e, r) => o(e, {
      method: "PUT",
      body: JSON.stringify(r)
    }),
    patch: (e, r) => o(e, {
      method: "PATCH",
      body: JSON.stringify(r)
    }),
    delete: (e, r) => o(e, {
      method: "DELETE",
      ...r ? { body: JSON.stringify(r) } : {}
    })
  };
}
export {
  g as a,
  m as i,
  y as n,
  w as r,
  l as t
};

//# sourceMappingURL=wc-api-client-DbP47Lh1.js.map