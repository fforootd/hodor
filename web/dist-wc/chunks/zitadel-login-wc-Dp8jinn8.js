import { A as c, B as Ne, C as Jt, D as kt, E as Zt, F as Fe, G as p, H as it, I as Yt, J as Se, K as te, L as C, M as me, N as E, O as Kt, P as lt, R as Ct, S as ge, T as be, U as Xt, V as X, W as Qt, Y as A, _ as M, b as Ue, c as er, d as tr, f as I, g as m, h as P, j as rr, k as ar, l as or, m as $, n as sr, o as nr, p as k, q as ir, r as lr, t as he, u as se, v as j, w as ye, x as cr, y as T, z as Be } from "./_plugin-vue_export-helper-DHhFP0j4.js";
var Xe = 1, dr = class {
  subscribers;
  toasts;
  dismissedToasts;
  constructor() {
    this.subscribers = [], this.toasts = [], this.dismissedToasts = /* @__PURE__ */ new Set();
  }
  subscribe = (e) => (this.subscribers.push(e), () => {
    const t = this.subscribers.indexOf(e);
    this.subscribers.splice(t, 1);
  });
  publish = (e) => {
    this.subscribers.forEach((t) => t(e));
  };
  addToast = (e) => {
    this.publish(e), this.toasts = [...this.toasts, e];
  };
  create = (e) => {
    const { message: t, ...r } = e, a = typeof e.id == "number" || e.id && e.id?.length > 0 ? e.id : Xe++, o = this.toasts.find((i) => i.id === a), s = e.dismissible === void 0 ? !0 : e.dismissible;
    return this.dismissedToasts.has(a) && this.dismissedToasts.delete(a), o ? this.toasts = this.toasts.map((i) => i.id === a ? (this.publish({
      ...i,
      ...e,
      id: a,
      title: t
    }), {
      ...i,
      ...e,
      id: a,
      dismissible: s,
      title: t
    }) : i) : this.addToast({
      title: t,
      ...r,
      dismissible: s,
      id: a
    }), a;
  };
  dismiss = (e) => (e ? (this.dismissedToasts.add(e), requestAnimationFrame(() => this.subscribers.forEach((t) => t({
    id: e,
    dismiss: !0
  })))) : this.toasts.forEach((t) => {
    this.subscribers.forEach((r) => r({
      id: t.id,
      dismiss: !0
    }));
  }), e);
  message = (e, t) => this.create({
    ...t,
    message: e,
    type: "default"
  });
  error = (e, t) => this.create({
    ...t,
    type: "error",
    message: e
  });
  success = (e, t) => this.create({
    ...t,
    type: "success",
    message: e
  });
  info = (e, t) => this.create({
    ...t,
    type: "info",
    message: e
  });
  warning = (e, t) => this.create({
    ...t,
    type: "warning",
    message: e
  });
  loading = (e, t) => this.create({
    ...t,
    type: "loading",
    message: e
  });
  promise = (e, t) => {
    if (!t) return;
    let r;
    t.loading !== void 0 && (r = this.create({
      ...t,
      promise: e,
      type: "loading",
      message: t.loading,
      description: typeof t.description != "function" ? t.description : void 0
    }));
    const a = Promise.resolve(e instanceof Function ? e() : e);
    let o = r !== void 0, s;
    const i = a.then(async (l) => {
      if (s = ["resolve", l], ye(l))
        o = !1, this.create({
          id: r,
          type: "default",
          message: l
        });
      else if (fr(l) && !l.ok) {
        o = !1;
        const d = typeof t.error == "function" ? await t.error(`HTTP error! status: ${l.status}`) : t.error, h = typeof t.description == "function" ? await t.description(`HTTP error! status: ${l.status}`) : t.description, S = typeof d == "object" && !ye(d) ? d : {
          message: d || "",
          id: r || ""
        };
        this.create({
          id: r,
          type: "error",
          description: h,
          ...S
        });
      } else if (l instanceof Error) {
        o = !1;
        const d = typeof t.error == "function" ? await t.error(l) : t.error, h = typeof t.description == "function" ? await t.description(l) : t.description, S = typeof d == "object" && !ye(d) ? d : {
          message: d || "",
          id: r || ""
        };
        this.create({
          id: r,
          type: "error",
          description: h,
          ...S
        });
      } else if (t.success !== void 0) {
        o = !1;
        const d = typeof t.success == "function" ? await t.success(l) : t.success, h = typeof t.description == "function" ? await t.description(l) : t.description, S = typeof d == "object" && !ye(d) ? d : {
          message: d || "",
          id: r || ""
        };
        this.create({
          id: r,
          type: "success",
          description: h,
          ...S
        });
      }
    }).catch(async (l) => {
      if (s = ["reject", l], t.error !== void 0) {
        o = !1;
        const d = typeof t.error == "function" ? await t.error(l) : t.error, h = typeof t.description == "function" ? await t.description(l) : t.description, S = typeof d == "object" && !ye(d) ? d : {
          message: d || "",
          id: r || ""
        };
        this.create({
          id: r,
          type: "error",
          description: h,
          ...S
        });
      }
    }).finally(() => {
      o && (this.dismiss(r), r = void 0), t.finally?.();
    }), u = () => new Promise((l, d) => i.then(() => s[0] === "reject" ? d(s[1]) : l(s[1])).catch(d));
    return typeof r != "string" && typeof r != "number" ? { unwrap: u } : Object.assign(r, { unwrap: u });
  };
  custom = (e, t) => {
    const r = t?.id || Xe++, a = this.toasts.find((s) => s.id === r), o = t?.dismissible === void 0 ? !0 : t.dismissible;
    return this.dismissedToasts.has(r) && this.dismissedToasts.delete(r), a ? this.toasts = this.toasts.map((s) => s.id === r ? (this.publish({
      ...s,
      component: e,
      dismissible: o,
      id: r,
      ...t
    }), {
      ...s,
      component: e,
      dismissible: o,
      id: r,
      ...t
    }) : s) : this.addToast({
      component: e,
      dismissible: o,
      id: r,
      ...t
    }), r;
  };
  getActiveToasts = () => this.toasts.filter((e) => !this.dismissedToasts.has(e.id));
}, ee = new dr();
function ur(e, t) {
  const r = t?.id || Xe++;
  return ee.create({
    message: e,
    id: r,
    type: "default",
    ...t
  }), r;
}
var fr = (e) => e && typeof e == "object" && "ok" in e && typeof e.ok == "boolean" && "status" in e && typeof e.status == "number", pr = ur, mr = () => ee.toasts, vr = () => ee.getActiveToasts(), gr = Object.assign(pr, {
  success: ee.success,
  info: ee.info,
  warning: ee.warning,
  error: ee.error,
  custom: ee.custom,
  message: ee.message,
  promise: ee.promise,
  dismiss: ee.dismiss,
  loading: ee.loading
}, {
  getHistory: mr,
  getToasts: vr
}), De = window.__ZITADEL_BASE_PATH__ || "", br = class extends Error {
  constructor(e, t, r) {
    super(e), this.name = "ApiError", this.status = t, this.code = r || `HTTP_${t}`;
  }
  get isUnauthorized() {
    return this.status === 401;
  }
  get isForbidden() {
    return this.status === 403;
  }
}, ct = !1;
function hr() {
  ct || (ct = !0, gr.error("Session expired", {
    description: "Your session has expired or is invalid. Redirecting to login…",
    duration: 4e3
  }), setTimeout(() => {
    const e = `${De}/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`;
    window.location.href = e;
  }, 1500));
}
function yr() {
  if (!De) return "same-origin";
  try {
    return new URL(De, window.location.origin).origin !== window.location.origin ? "include" : "same-origin";
  } catch {
    return "same-origin";
  }
}
async function _e(e, t = {}) {
  const r = await fetch(`${De}${e}`, {
    ...t,
    headers: {
      "Content-Type": "application/json",
      ...t.headers
    },
    credentials: yr()
  });
  if (!r.ok) {
    const o = await r.json().catch(() => ({ error: r.statusText })), s = o.error || `HTTP ${r.status}`, i = o.code || void 0;
    throw r.status === 401 && hr(), new br(s, r.status, i);
  }
  const a = await r.text();
  if (a)
    return JSON.parse(a);
}
var Je = {
  get: (e) => _e(e),
  post: (e, t) => _e(e, {
    method: "POST",
    body: JSON.stringify(t)
  }),
  put: (e, t) => _e(e, {
    method: "PUT",
    body: JSON.stringify(t)
  }),
  patch: (e, t) => _e(e, {
    method: "PATCH",
    body: JSON.stringify(t)
  }),
  delete: (e, t) => _e(e, {
    method: "DELETE",
    ...t ? { body: JSON.stringify(t) } : {}
  })
}, dt = {
  create: (e, t) => {
    const r = {};
    return e && (r.redirect_uri = e), t && (r.state = t), Je.post("/v1/login/flows", r);
  },
  submit: (e, t, r) => Je.post(`/v1/login/flows/${e}/submit`, {
    action: t,
    ...r
  }),
  get: (e) => Je.get(`/v1/login/flows/${e}`)
}, St = class {
  constructor(e) {
    this.spans = [], this.flushInterval = null, this.config = e, this.flushInterval = setInterval(() => this.flush(), 5e3), typeof window < "u" && window.performance && window.addEventListener("load", () => {
      const t = performance.getEntriesByType("navigation")[0];
      t && this.spans.push({
        name: "documentLoad",
        startTime: t.startTime,
        endTime: t.loadEventEnd,
        attributes: {
          "http.url": window.location.href,
          "document.load_ms": t.loadEventEnd - t.startTime,
          "document.dom_content_loaded_ms": t.domContentLoadedEventEnd - t.startTime
        }
      });
    });
  }
  getTracer(e) {
    return { startSpan: (t, r) => {
      const a = {
        name: t,
        startTime: performance.now(),
        endTime: void 0,
        attributes: r,
        end: () => {
          a.endTime = performance.now(), this.spans.push(a);
        },
        setAttribute: (o, s) => {
          a.attributes || (a.attributes = {}), a.attributes[o] = s;
        }
      };
      return a;
    } };
  }
  async flush() {
    if (this.spans.length === 0) return;
    const e = this.spans.splice(0), t = this.config.otelEndpoint || `${this.config.baseUrl}/v1/otel/traces`, r = { resourceSpans: [{
      resource: { attributes: [{
        key: "service.name",
        value: { stringValue: "zitadel-login-wc" }
      }, {
        key: "browser.language",
        value: { stringValue: navigator.language }
      }] },
      scopeSpans: [{ spans: e.map((a) => ({
        traceId: Sr(),
        spanId: Ar(),
        name: a.name,
        kind: 1,
        startTimeUnixNano: String(Math.floor((performance.timeOrigin + (a.startTime || 0)) * 1e6)),
        endTimeUnixNano: String(Math.floor((performance.timeOrigin + (a.endTime || performance.now())) * 1e6)),
        attributes: a.attributes ? Object.entries(a.attributes).map(([o, s]) => ({
          key: o,
          value: typeof s == "number" ? { intValue: String(s) } : { stringValue: String(s) }
        })) : []
      })) }]
    }] };
    try {
      await fetch(t, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...this.config.flowId ? { "X-Flow-ID": this.config.flowId } : {}
        },
        body: JSON.stringify(r),
        keepalive: !0
      });
    } catch {
    }
  }
  shutdown() {
    this.flush(), this.flushInterval && clearInterval(this.flushInterval);
  }
}, Y = null;
function _r(e) {
  return e.enabled === !1 ? null : Y || (Y = new St(e), Y);
}
function wr(e, t, r) {
  Y && Y.getTracer("zitadel-login").startSpan("login.flow.step_transition", {
    "flow.id": r,
    "flow.from_step": e,
    "flow.to_step": t
  }).end();
}
function xr(e, t) {
  if (Y)
    return Y.getTracer("zitadel-login").startSpan("login.flow.submit", {
      "flow.id": t,
      "flow.action": e
    });
}
function kr() {
  Y && (Y.shutdown(), Y = null);
}
function Cr(e) {
  Y && Y instanceof St && (Y.config.flowId = e);
}
function Sr() {
  return At(32);
}
function Ar() {
  return At(16);
}
function At(e) {
  const t = new Uint8Array(e / 2);
  return crypto.getRandomValues(t), Array.from(t).map((r) => r.toString(16).padStart(2, "0")).join("");
}
async function zr() {
  try {
    const e = await Function("p", "return import(p)")("@thumbmarkjs/thumbmarkjs");
    if (e && (e.getFingerprint || e.default?.getFingerprint)) {
      const t = await (e.getFingerprint || e.default.getFingerprint)();
      return {
        visitorId: typeof t == "string" ? t : t.hash || t.thumbmark || "",
        components: typeof t == "object" ? t.components || {} : {},
        collectedAt: Date.now()
      };
    }
  } catch {
  }
  return $r();
}
async function $r() {
  const e = {};
  try {
    const t = document.createElement("canvas");
    t.width = 200, t.height = 50;
    const r = t.getContext("2d");
    r && (r.textBaseline = "top", r.font = "14px Arial", r.fillStyle = "#f60", r.fillRect(100, 1, 62, 20), r.fillStyle = "#069", r.fillText("Zitadel fp 🔐", 2, 15), r.fillStyle = "rgba(102, 204, 0, 0.7)", r.fillText("canvas fp", 4, 35), e.canvas = await ut(t.toDataURL()));
  } catch {
    e.canvas = "unavailable";
  }
  try {
    const t = document.createElement("canvas"), r = t.getContext("webgl") || t.getContext("experimental-webgl");
    if (r && r instanceof WebGLRenderingContext) {
      const a = r.getExtension("WEBGL_debug_renderer_info");
      a && (e.webgl_renderer = r.getParameter(a.UNMASKED_RENDERER_WEBGL) || "", e.webgl_vendor = r.getParameter(a.UNMASKED_VENDOR_WEBGL) || "");
    }
  } catch {
    e.webgl_renderer = "unavailable";
  }
  return e.screen = `${screen.width}x${screen.height}x${screen.colorDepth}`, e.timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "", e.language = navigator.language || "", e.platform = navigator.platform || "", e.cores = String(navigator.hardwareConcurrency || 0), e.memory = String(navigator.deviceMemory || 0), {
    visitorId: await ut(Object.entries(e).sort(([t], [r]) => t.localeCompare(r)).map(([t, r]) => `${t}:${r}`).join("|")),
    components: e,
    collectedAt: Date.now()
  };
}
async function ut(e) {
  const t = new TextEncoder().encode(e), r = await crypto.subtle.digest("SHA-256", t);
  return Array.from(new Uint8Array(r)).map((a) => a.toString(16).padStart(2, "0")).join("");
}
async function Tr(e, t, r) {
  try {
    await fetch(`${e}/v1/login/flows/${t}/submit`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({
        action: "fingerprint_submit",
        visitor_id: r.visitorId,
        fingerprint_hash: r.visitorId
      })
    });
  } catch {
  }
}
function zt(e) {
  var t, r, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var o = e.length;
    for (t = 0; t < o; t++) e[t] && (r = zt(e[t])) && (a && (a += " "), a += r);
  } else for (r in e) e[r] && (a && (a += " "), a += r);
  return a;
}
function $t() {
  for (var e, t, r = 0, a = "", o = arguments.length; r < o; r++) (e = arguments[r]) && (t = zt(e)) && (a && (a += " "), a += t);
  return a;
}
var Or = (e, t) => {
  const r = new Array(e.length + t.length);
  for (let a = 0; a < e.length; a++) r[a] = e[a];
  for (let a = 0; a < t.length; a++) r[e.length + a] = t[a];
  return r;
}, Ir = (e, t) => ({
  classGroupId: e,
  validator: t
}), Tt = (e = /* @__PURE__ */ new Map(), t = null, r) => ({
  nextPart: e,
  validators: t,
  classGroupId: r
}), Ge = "-", ft = [], Er = "arbitrary..", Lr = (e) => {
  const t = Pr(e), { conflictingClassGroups: r, conflictingClassGroupModifiers: a } = e;
  return {
    getClassGroupId: (i) => {
      if (i.startsWith("[") && i.endsWith("]")) return jr(i);
      const u = i.split(Ge);
      return Ot(u, u[0] === "" && u.length > 1 ? 1 : 0, t);
    },
    getConflictingClassGroupIds: (i, u) => {
      if (u) {
        const l = a[i], d = r[i];
        return l ? d ? Or(d, l) : l : d || ft;
      }
      return r[i] || ft;
    }
  };
}, Ot = (e, t, r) => {
  if (e.length - t === 0) return r.classGroupId;
  const a = e[t], o = r.nextPart.get(a);
  if (o) {
    const l = Ot(e, t + 1, o);
    if (l) return l;
  }
  const s = r.validators;
  if (s === null) return;
  const i = t === 0 ? e.join(Ge) : e.slice(t).join(Ge), u = s.length;
  for (let l = 0; l < u; l++) {
    const d = s[l];
    if (d.validator(i)) return d.classGroupId;
  }
}, jr = (e) => e.slice(1, -1).indexOf(":") === -1 ? void 0 : (() => {
  const t = e.slice(1, -1), r = t.indexOf(":"), a = t.slice(0, r);
  return a ? Er + a : void 0;
})(), Pr = (e) => {
  const { theme: t, classGroups: r } = e;
  return Vr(r, t);
}, Vr = (e, t) => {
  const r = Tt();
  for (const a in e) {
    const o = e[a];
    et(o, r, a, t);
  }
  return r;
}, et = (e, t, r, a) => {
  const o = e.length;
  for (let s = 0; s < o; s++) {
    const i = e[s];
    Mr(i, t, r, a);
  }
}, Mr = (e, t, r, a) => {
  if (typeof e == "string") {
    Rr(e, t, r);
    return;
  }
  if (typeof e == "function") {
    Br(e, t, r, a);
    return;
  }
  Nr(e, t, r, a);
}, Rr = (e, t, r) => {
  const a = e === "" ? t : It(t, e);
  a.classGroupId = r;
}, Br = (e, t, r, a) => {
  if (Fr(e)) {
    et(e(a), t, r, a);
    return;
  }
  t.validators === null && (t.validators = []), t.validators.push(Ir(r, e));
}, Nr = (e, t, r, a) => {
  const o = Object.entries(e), s = o.length;
  for (let i = 0; i < s; i++) {
    const [u, l] = o[i];
    et(l, It(t, u), r, a);
  }
}, It = (e, t) => {
  let r = e;
  const a = t.split(Ge), o = a.length;
  for (let s = 0; s < o; s++) {
    const i = a[s];
    let u = r.nextPart.get(i);
    u || (u = Tt(), r.nextPart.set(i, u)), r = u;
  }
  return r;
}, Fr = (e) => "isThemeGetter" in e && e.isThemeGetter === !0, Ur = (e) => {
  if (e < 1) return {
    get: () => {
    },
    set: () => {
    }
  };
  let t = 0, r = /* @__PURE__ */ Object.create(null), a = /* @__PURE__ */ Object.create(null);
  const o = (s, i) => {
    r[s] = i, t++, t > e && (t = 0, a = r, r = /* @__PURE__ */ Object.create(null));
  };
  return {
    get(s) {
      let i = r[s];
      if (i !== void 0) return i;
      if ((i = a[s]) !== void 0)
        return o(s, i), i;
    },
    set(s, i) {
      s in r ? r[s] = i : o(s, i);
    }
  };
}, Qe = "!", pt = ":", Dr = [], mt = (e, t, r, a, o) => ({
  modifiers: e,
  hasImportantModifier: t,
  baseClassName: r,
  maybePostfixModifierPosition: a,
  isExternal: o
}), Gr = (e) => {
  const { prefix: t, experimentalParseClassName: r } = e;
  let a = (o) => {
    const s = [];
    let i = 0, u = 0, l = 0, d;
    const h = o.length;
    for (let z = 0; z < h; z++) {
      const V = o[z];
      if (i === 0 && u === 0) {
        if (V === pt) {
          s.push(o.slice(l, z)), l = z + 1;
          continue;
        }
        if (V === "/") {
          d = z;
          continue;
        }
      }
      V === "[" ? i++ : V === "]" ? i-- : V === "(" ? u++ : V === ")" && u--;
    }
    const S = s.length === 0 ? o : o.slice(l);
    let L = S, N = !1;
    S.endsWith(Qe) ? (L = S.slice(0, -1), N = !0) : S.startsWith(Qe) && (L = S.slice(1), N = !0);
    const q = d && d > l ? d - l : void 0;
    return mt(s, N, L, q);
  };
  if (t) {
    const o = t + pt, s = a;
    a = (i) => i.startsWith(o) ? s(i.slice(o.length)) : mt(Dr, !1, i, void 0, !0);
  }
  if (r) {
    const o = a;
    a = (s) => r({
      className: s,
      parseClassName: o
    });
  }
  return a;
}, qr = (e) => {
  const t = /* @__PURE__ */ new Map();
  return e.orderSensitiveModifiers.forEach((r, a) => {
    t.set(r, 1e6 + a);
  }), (r) => {
    const a = [];
    let o = [];
    for (let s = 0; s < r.length; s++) {
      const i = r[s], u = i[0] === "[", l = t.has(i);
      u || l ? (o.length > 0 && (o.sort(), a.push(...o), o = []), a.push(i)) : o.push(i);
    }
    return o.length > 0 && (o.sort(), a.push(...o)), a;
  };
}, Wr = (e) => ({
  cache: Ur(e.cacheSize),
  parseClassName: Gr(e),
  sortModifiers: qr(e),
  ...Lr(e)
}), Hr = /\s+/, Jr = (e, t) => {
  const { parseClassName: r, getClassGroupId: a, getConflictingClassGroupIds: o, sortModifiers: s } = t, i = [], u = e.trim().split(Hr);
  let l = "";
  for (let d = u.length - 1; d >= 0; d -= 1) {
    const h = u[d], { isExternal: S, modifiers: L, hasImportantModifier: N, baseClassName: q, maybePostfixModifierPosition: z } = r(h);
    if (S) {
      l = h + (l.length > 0 ? " " + l : l);
      continue;
    }
    let V = !!z, J = a(V ? q.substring(0, z) : q);
    if (!J) {
      if (!V) {
        l = h + (l.length > 0 ? " " + l : l);
        continue;
      }
      if (J = a(q), !J) {
        l = h + (l.length > 0 ? " " + l : l);
        continue;
      }
      V = !1;
    }
    const ne = L.length === 0 ? "" : L.length === 1 ? L[0] : s(L).join(":"), ae = N ? ne + Qe : ne, oe = ae + J;
    if (i.indexOf(oe) > -1) continue;
    i.push(oe);
    const O = o(J, V);
    for (let Z = 0; Z < O.length; ++Z) {
      const Q = O[Z];
      i.push(ae + Q);
    }
    l = h + (l.length > 0 ? " " + l : l);
  }
  return l;
}, Zr = (...e) => {
  let t = 0, r, a, o = "";
  for (; t < e.length; ) (r = e[t++]) && (a = Et(r)) && (o && (o += " "), o += a);
  return o;
}, Et = (e) => {
  if (typeof e == "string") return e;
  let t, r = "";
  for (let a = 0; a < e.length; a++) e[a] && (t = Et(e[a])) && (r && (r += " "), r += t);
  return r;
}, Yr = (e, ...t) => {
  let r, a, o, s;
  const i = (l) => (r = Wr(t.reduce((d, h) => h(d), e())), a = r.cache.get, o = r.cache.set, s = u, u(l)), u = (l) => {
    const d = a(l);
    if (d) return d;
    const h = Jr(l, r);
    return o(l, h), h;
  };
  return s = i, (...l) => s(Zr(...l));
}, Kr = [], R = (e) => {
  const t = (r) => r[e] || Kr;
  return t.isThemeGetter = !0, t;
}, Lt = /^\[(?:(\w[\w-]*):)?(.+)\]$/i, jt = /^\((?:(\w[\w-]*):)?(.+)\)$/i, Xr = /^\d+(?:\.\d+)?\/\d+(?:\.\d+)?$/, Qr = /^(\d+(\.\d+)?)?(xs|sm|md|lg|xl)$/, ea = /\d+(%|px|r?em|[sdl]?v([hwib]|min|max)|pt|pc|in|cm|mm|cap|ch|ex|r?lh|cq(w|h|i|b|min|max))|\b(calc|min|max|clamp)\(.+\)|^0$/, ta = /^(rgba?|hsla?|hwb|(ok)?(lab|lch)|color-mix)\(.+\)$/, ra = /^(inset_)?-?((\d+)?\.?(\d+)[a-z]+|0)_-?((\d+)?\.?(\d+)[a-z]+|0)/, aa = /^(url|image|image-set|cross-fade|element|(repeating-)?(linear|radial|conic)-gradient)\(.+\)$/, le = (e) => Xr.test(e), x = (e) => !!e && !Number.isNaN(Number(e)), ce = (e) => !!e && Number.isInteger(Number(e)), Ze = (e) => e.endsWith("%") && x(e.slice(0, -1)), ie = (e) => Qr.test(e), Pt = () => !0, oa = (e) => ea.test(e) && !ta.test(e), tt = () => !1, sa = (e) => ra.test(e), na = (e) => aa.test(e), ia = (e) => !v(e) && !g(e), la = (e) => de(e, Rt, tt), v = (e) => Lt.test(e), ue = (e) => de(e, Bt, oa), vt = (e) => de(e, ga, x), ca = (e) => de(e, Ft, Pt), da = (e) => de(e, Nt, tt), gt = (e) => de(e, Vt, tt), ua = (e) => de(e, Mt, na), Pe = (e) => de(e, Ut, sa), g = (e) => jt.test(e), we = (e) => fe(e, Bt), fa = (e) => fe(e, Nt), bt = (e) => fe(e, Vt), pa = (e) => fe(e, Rt), ma = (e) => fe(e, Mt), Ve = (e) => fe(e, Ut, !0), va = (e) => fe(e, Ft, !0), de = (e, t, r) => {
  const a = Lt.exec(e);
  return a ? a[1] ? t(a[1]) : r(a[2]) : !1;
}, fe = (e, t, r = !1) => {
  const a = jt.exec(e);
  return a ? a[1] ? t(a[1]) : r : !1;
}, Vt = (e) => e === "position" || e === "percentage", Mt = (e) => e === "image" || e === "url", Rt = (e) => e === "length" || e === "size" || e === "bg-size", Bt = (e) => e === "length", ga = (e) => e === "number", Nt = (e) => e === "family-name", Ft = (e) => e === "number" || e === "weight", Ut = (e) => e === "shadow", ba = () => {
  const e = R("color"), t = R("font"), r = R("text"), a = R("font-weight"), o = R("tracking"), s = R("leading"), i = R("breakpoint"), u = R("container"), l = R("spacing"), d = R("radius"), h = R("shadow"), S = R("inset-shadow"), L = R("text-shadow"), N = R("drop-shadow"), q = R("blur"), z = R("perspective"), V = R("aspect"), J = R("ease"), ne = R("animate"), ae = () => [
    "auto",
    "avoid",
    "all",
    "avoid-page",
    "page",
    "left",
    "right",
    "column"
  ], oe = () => [
    "center",
    "top",
    "bottom",
    "left",
    "right",
    "top-left",
    "left-top",
    "top-right",
    "right-top",
    "bottom-right",
    "right-bottom",
    "bottom-left",
    "left-bottom"
  ], O = () => [
    ...oe(),
    g,
    v
  ], Z = () => [
    "auto",
    "hidden",
    "clip",
    "visible",
    "scroll"
  ], Q = () => [
    "auto",
    "contain",
    "none"
  ], b = () => [
    g,
    v,
    l
  ], U = () => [
    le,
    "full",
    "auto",
    ...b()
  ], ze = () => [
    ce,
    "none",
    "subgrid",
    g,
    v
  ], $e = () => [
    "auto",
    { span: [
      "full",
      ce,
      g,
      v
    ] },
    ce,
    g,
    v
  ], K = () => [
    ce,
    "auto",
    g,
    v
  ], Te = () => [
    "auto",
    "min",
    "max",
    "fr",
    g,
    v
  ], pe = () => [
    "start",
    "end",
    "center",
    "between",
    "around",
    "evenly",
    "stretch",
    "baseline",
    "center-safe",
    "end-safe"
  ], _ = () => [
    "start",
    "end",
    "center",
    "stretch",
    "center-safe",
    "end-safe"
  ], y = () => ["auto", ...b()], n = () => [
    le,
    "auto",
    "full",
    "dvw",
    "dvh",
    "lvw",
    "lvh",
    "svw",
    "svh",
    "min",
    "max",
    "fit",
    ...b()
  ], F = () => [
    le,
    "screen",
    "full",
    "dvw",
    "lvw",
    "svw",
    "min",
    "max",
    "fit",
    ...b()
  ], w = () => [
    le,
    "screen",
    "full",
    "lh",
    "dvh",
    "lvh",
    "svh",
    "min",
    "max",
    "fit",
    ...b()
  ], f = () => [
    e,
    g,
    v
  ], D = () => [
    ...oe(),
    bt,
    gt,
    { position: [g, v] }
  ], G = () => ["no-repeat", { repeat: [
    "",
    "x",
    "y",
    "space",
    "round"
  ] }], Oe = () => [
    "auto",
    "cover",
    "contain",
    pa,
    la,
    { size: [g, v] }
  ], We = () => [
    Ze,
    we,
    ue
  ], W = () => [
    "",
    "none",
    "full",
    d,
    g,
    v
  ], H = () => [
    "",
    x,
    we,
    ue
  ], Ie = () => [
    "solid",
    "dashed",
    "dotted",
    "double"
  ], st = () => [
    "normal",
    "multiply",
    "screen",
    "overlay",
    "darken",
    "lighten",
    "color-dodge",
    "color-burn",
    "hard-light",
    "soft-light",
    "difference",
    "exclusion",
    "hue",
    "saturation",
    "color",
    "luminosity"
  ], B = () => [
    x,
    Ze,
    bt,
    gt
  ], nt = () => [
    "",
    "none",
    q,
    g,
    v
  ], Ee = () => [
    "none",
    x,
    g,
    v
  ], Le = () => [
    "none",
    x,
    g,
    v
  ], He = () => [
    x,
    g,
    v
  ], je = () => [
    le,
    "full",
    ...b()
  ];
  return {
    cacheSize: 500,
    theme: {
      animate: [
        "spin",
        "ping",
        "pulse",
        "bounce"
      ],
      aspect: ["video"],
      blur: [ie],
      breakpoint: [ie],
      color: [Pt],
      container: [ie],
      "drop-shadow": [ie],
      ease: [
        "in",
        "out",
        "in-out"
      ],
      font: [ia],
      "font-weight": [
        "thin",
        "extralight",
        "light",
        "normal",
        "medium",
        "semibold",
        "bold",
        "extrabold",
        "black"
      ],
      "inset-shadow": [ie],
      leading: [
        "none",
        "tight",
        "snug",
        "normal",
        "relaxed",
        "loose"
      ],
      perspective: [
        "dramatic",
        "near",
        "normal",
        "midrange",
        "distant",
        "none"
      ],
      radius: [ie],
      shadow: [ie],
      spacing: ["px", x],
      text: [ie],
      "text-shadow": [ie],
      tracking: [
        "tighter",
        "tight",
        "normal",
        "wide",
        "wider",
        "widest"
      ]
    },
    classGroups: {
      aspect: [{ aspect: [
        "auto",
        "square",
        le,
        v,
        g,
        V
      ] }],
      container: ["container"],
      columns: [{ columns: [
        x,
        v,
        g,
        u
      ] }],
      "break-after": [{ "break-after": ae() }],
      "break-before": [{ "break-before": ae() }],
      "break-inside": [{ "break-inside": [
        "auto",
        "avoid",
        "avoid-page",
        "avoid-column"
      ] }],
      "box-decoration": [{ "box-decoration": ["slice", "clone"] }],
      box: [{ box: ["border", "content"] }],
      display: [
        "block",
        "inline-block",
        "inline",
        "flex",
        "inline-flex",
        "table",
        "inline-table",
        "table-caption",
        "table-cell",
        "table-column",
        "table-column-group",
        "table-footer-group",
        "table-header-group",
        "table-row-group",
        "table-row",
        "flow-root",
        "grid",
        "inline-grid",
        "contents",
        "list-item",
        "hidden"
      ],
      sr: ["sr-only", "not-sr-only"],
      float: [{ float: [
        "right",
        "left",
        "none",
        "start",
        "end"
      ] }],
      clear: [{ clear: [
        "left",
        "right",
        "both",
        "none",
        "start",
        "end"
      ] }],
      isolation: ["isolate", "isolation-auto"],
      "object-fit": [{ object: [
        "contain",
        "cover",
        "fill",
        "none",
        "scale-down"
      ] }],
      "object-position": [{ object: O() }],
      overflow: [{ overflow: Z() }],
      "overflow-x": [{ "overflow-x": Z() }],
      "overflow-y": [{ "overflow-y": Z() }],
      overscroll: [{ overscroll: Q() }],
      "overscroll-x": [{ "overscroll-x": Q() }],
      "overscroll-y": [{ "overscroll-y": Q() }],
      position: [
        "static",
        "fixed",
        "absolute",
        "relative",
        "sticky"
      ],
      inset: [{ inset: U() }],
      "inset-x": [{ "inset-x": U() }],
      "inset-y": [{ "inset-y": U() }],
      start: [{
        "inset-s": U(),
        start: U()
      }],
      end: [{
        "inset-e": U(),
        end: U()
      }],
      "inset-bs": [{ "inset-bs": U() }],
      "inset-be": [{ "inset-be": U() }],
      top: [{ top: U() }],
      right: [{ right: U() }],
      bottom: [{ bottom: U() }],
      left: [{ left: U() }],
      visibility: [
        "visible",
        "invisible",
        "collapse"
      ],
      z: [{ z: [
        ce,
        "auto",
        g,
        v
      ] }],
      basis: [{ basis: [
        le,
        "full",
        "auto",
        u,
        ...b()
      ] }],
      "flex-direction": [{ flex: [
        "row",
        "row-reverse",
        "col",
        "col-reverse"
      ] }],
      "flex-wrap": [{ flex: [
        "nowrap",
        "wrap",
        "wrap-reverse"
      ] }],
      flex: [{ flex: [
        x,
        le,
        "auto",
        "initial",
        "none",
        v
      ] }],
      grow: [{ grow: [
        "",
        x,
        g,
        v
      ] }],
      shrink: [{ shrink: [
        "",
        x,
        g,
        v
      ] }],
      order: [{ order: [
        ce,
        "first",
        "last",
        "none",
        g,
        v
      ] }],
      "grid-cols": [{ "grid-cols": ze() }],
      "col-start-end": [{ col: $e() }],
      "col-start": [{ "col-start": K() }],
      "col-end": [{ "col-end": K() }],
      "grid-rows": [{ "grid-rows": ze() }],
      "row-start-end": [{ row: $e() }],
      "row-start": [{ "row-start": K() }],
      "row-end": [{ "row-end": K() }],
      "grid-flow": [{ "grid-flow": [
        "row",
        "col",
        "dense",
        "row-dense",
        "col-dense"
      ] }],
      "auto-cols": [{ "auto-cols": Te() }],
      "auto-rows": [{ "auto-rows": Te() }],
      gap: [{ gap: b() }],
      "gap-x": [{ "gap-x": b() }],
      "gap-y": [{ "gap-y": b() }],
      "justify-content": [{ justify: [...pe(), "normal"] }],
      "justify-items": [{ "justify-items": [..._(), "normal"] }],
      "justify-self": [{ "justify-self": ["auto", ..._()] }],
      "align-content": [{ content: ["normal", ...pe()] }],
      "align-items": [{ items: [..._(), { baseline: ["", "last"] }] }],
      "align-self": [{ self: [
        "auto",
        ..._(),
        { baseline: ["", "last"] }
      ] }],
      "place-content": [{ "place-content": pe() }],
      "place-items": [{ "place-items": [..._(), "baseline"] }],
      "place-self": [{ "place-self": ["auto", ..._()] }],
      p: [{ p: b() }],
      px: [{ px: b() }],
      py: [{ py: b() }],
      ps: [{ ps: b() }],
      pe: [{ pe: b() }],
      pbs: [{ pbs: b() }],
      pbe: [{ pbe: b() }],
      pt: [{ pt: b() }],
      pr: [{ pr: b() }],
      pb: [{ pb: b() }],
      pl: [{ pl: b() }],
      m: [{ m: y() }],
      mx: [{ mx: y() }],
      my: [{ my: y() }],
      ms: [{ ms: y() }],
      me: [{ me: y() }],
      mbs: [{ mbs: y() }],
      mbe: [{ mbe: y() }],
      mt: [{ mt: y() }],
      mr: [{ mr: y() }],
      mb: [{ mb: y() }],
      ml: [{ ml: y() }],
      "space-x": [{ "space-x": b() }],
      "space-x-reverse": ["space-x-reverse"],
      "space-y": [{ "space-y": b() }],
      "space-y-reverse": ["space-y-reverse"],
      size: [{ size: n() }],
      "inline-size": [{ inline: ["auto", ...F()] }],
      "min-inline-size": [{ "min-inline": ["auto", ...F()] }],
      "max-inline-size": [{ "max-inline": ["none", ...F()] }],
      "block-size": [{ block: ["auto", ...w()] }],
      "min-block-size": [{ "min-block": ["auto", ...w()] }],
      "max-block-size": [{ "max-block": ["none", ...w()] }],
      w: [{ w: [
        u,
        "screen",
        ...n()
      ] }],
      "min-w": [{ "min-w": [
        u,
        "screen",
        "none",
        ...n()
      ] }],
      "max-w": [{ "max-w": [
        u,
        "screen",
        "none",
        "prose",
        { screen: [i] },
        ...n()
      ] }],
      h: [{ h: [
        "screen",
        "lh",
        ...n()
      ] }],
      "min-h": [{ "min-h": [
        "screen",
        "lh",
        "none",
        ...n()
      ] }],
      "max-h": [{ "max-h": [
        "screen",
        "lh",
        ...n()
      ] }],
      "font-size": [{ text: [
        "base",
        r,
        we,
        ue
      ] }],
      "font-smoothing": ["antialiased", "subpixel-antialiased"],
      "font-style": ["italic", "not-italic"],
      "font-weight": [{ font: [
        a,
        va,
        ca
      ] }],
      "font-stretch": [{ "font-stretch": [
        "ultra-condensed",
        "extra-condensed",
        "condensed",
        "semi-condensed",
        "normal",
        "semi-expanded",
        "expanded",
        "extra-expanded",
        "ultra-expanded",
        Ze,
        v
      ] }],
      "font-family": [{ font: [
        fa,
        da,
        t
      ] }],
      "font-features": [{ "font-features": [v] }],
      "fvn-normal": ["normal-nums"],
      "fvn-ordinal": ["ordinal"],
      "fvn-slashed-zero": ["slashed-zero"],
      "fvn-figure": ["lining-nums", "oldstyle-nums"],
      "fvn-spacing": ["proportional-nums", "tabular-nums"],
      "fvn-fraction": ["diagonal-fractions", "stacked-fractions"],
      tracking: [{ tracking: [
        o,
        g,
        v
      ] }],
      "line-clamp": [{ "line-clamp": [
        x,
        "none",
        g,
        vt
      ] }],
      leading: [{ leading: [s, ...b()] }],
      "list-image": [{ "list-image": [
        "none",
        g,
        v
      ] }],
      "list-style-position": [{ list: ["inside", "outside"] }],
      "list-style-type": [{ list: [
        "disc",
        "decimal",
        "none",
        g,
        v
      ] }],
      "text-alignment": [{ text: [
        "left",
        "center",
        "right",
        "justify",
        "start",
        "end"
      ] }],
      "placeholder-color": [{ placeholder: f() }],
      "text-color": [{ text: f() }],
      "text-decoration": [
        "underline",
        "overline",
        "line-through",
        "no-underline"
      ],
      "text-decoration-style": [{ decoration: [...Ie(), "wavy"] }],
      "text-decoration-thickness": [{ decoration: [
        x,
        "from-font",
        "auto",
        g,
        ue
      ] }],
      "text-decoration-color": [{ decoration: f() }],
      "underline-offset": [{ "underline-offset": [
        x,
        "auto",
        g,
        v
      ] }],
      "text-transform": [
        "uppercase",
        "lowercase",
        "capitalize",
        "normal-case"
      ],
      "text-overflow": [
        "truncate",
        "text-ellipsis",
        "text-clip"
      ],
      "text-wrap": [{ text: [
        "wrap",
        "nowrap",
        "balance",
        "pretty"
      ] }],
      indent: [{ indent: b() }],
      "vertical-align": [{ align: [
        "baseline",
        "top",
        "middle",
        "bottom",
        "text-top",
        "text-bottom",
        "sub",
        "super",
        g,
        v
      ] }],
      whitespace: [{ whitespace: [
        "normal",
        "nowrap",
        "pre",
        "pre-line",
        "pre-wrap",
        "break-spaces"
      ] }],
      break: [{ break: [
        "normal",
        "words",
        "all",
        "keep"
      ] }],
      wrap: [{ wrap: [
        "break-word",
        "anywhere",
        "normal"
      ] }],
      hyphens: [{ hyphens: [
        "none",
        "manual",
        "auto"
      ] }],
      content: [{ content: [
        "none",
        g,
        v
      ] }],
      "bg-attachment": [{ bg: [
        "fixed",
        "local",
        "scroll"
      ] }],
      "bg-clip": [{ "bg-clip": [
        "border",
        "padding",
        "content",
        "text"
      ] }],
      "bg-origin": [{ "bg-origin": [
        "border",
        "padding",
        "content"
      ] }],
      "bg-position": [{ bg: D() }],
      "bg-repeat": [{ bg: G() }],
      "bg-size": [{ bg: Oe() }],
      "bg-image": [{ bg: [
        "none",
        {
          linear: [
            { to: [
              "t",
              "tr",
              "r",
              "br",
              "b",
              "bl",
              "l",
              "tl"
            ] },
            ce,
            g,
            v
          ],
          radial: [
            "",
            g,
            v
          ],
          conic: [
            ce,
            g,
            v
          ]
        },
        ma,
        ua
      ] }],
      "bg-color": [{ bg: f() }],
      "gradient-from-pos": [{ from: We() }],
      "gradient-via-pos": [{ via: We() }],
      "gradient-to-pos": [{ to: We() }],
      "gradient-from": [{ from: f() }],
      "gradient-via": [{ via: f() }],
      "gradient-to": [{ to: f() }],
      rounded: [{ rounded: W() }],
      "rounded-s": [{ "rounded-s": W() }],
      "rounded-e": [{ "rounded-e": W() }],
      "rounded-t": [{ "rounded-t": W() }],
      "rounded-r": [{ "rounded-r": W() }],
      "rounded-b": [{ "rounded-b": W() }],
      "rounded-l": [{ "rounded-l": W() }],
      "rounded-ss": [{ "rounded-ss": W() }],
      "rounded-se": [{ "rounded-se": W() }],
      "rounded-ee": [{ "rounded-ee": W() }],
      "rounded-es": [{ "rounded-es": W() }],
      "rounded-tl": [{ "rounded-tl": W() }],
      "rounded-tr": [{ "rounded-tr": W() }],
      "rounded-br": [{ "rounded-br": W() }],
      "rounded-bl": [{ "rounded-bl": W() }],
      "border-w": [{ border: H() }],
      "border-w-x": [{ "border-x": H() }],
      "border-w-y": [{ "border-y": H() }],
      "border-w-s": [{ "border-s": H() }],
      "border-w-e": [{ "border-e": H() }],
      "border-w-bs": [{ "border-bs": H() }],
      "border-w-be": [{ "border-be": H() }],
      "border-w-t": [{ "border-t": H() }],
      "border-w-r": [{ "border-r": H() }],
      "border-w-b": [{ "border-b": H() }],
      "border-w-l": [{ "border-l": H() }],
      "divide-x": [{ "divide-x": H() }],
      "divide-x-reverse": ["divide-x-reverse"],
      "divide-y": [{ "divide-y": H() }],
      "divide-y-reverse": ["divide-y-reverse"],
      "border-style": [{ border: [
        ...Ie(),
        "hidden",
        "none"
      ] }],
      "divide-style": [{ divide: [
        ...Ie(),
        "hidden",
        "none"
      ] }],
      "border-color": [{ border: f() }],
      "border-color-x": [{ "border-x": f() }],
      "border-color-y": [{ "border-y": f() }],
      "border-color-s": [{ "border-s": f() }],
      "border-color-e": [{ "border-e": f() }],
      "border-color-bs": [{ "border-bs": f() }],
      "border-color-be": [{ "border-be": f() }],
      "border-color-t": [{ "border-t": f() }],
      "border-color-r": [{ "border-r": f() }],
      "border-color-b": [{ "border-b": f() }],
      "border-color-l": [{ "border-l": f() }],
      "divide-color": [{ divide: f() }],
      "outline-style": [{ outline: [
        ...Ie(),
        "none",
        "hidden"
      ] }],
      "outline-offset": [{ "outline-offset": [
        x,
        g,
        v
      ] }],
      "outline-w": [{ outline: [
        "",
        x,
        we,
        ue
      ] }],
      "outline-color": [{ outline: f() }],
      shadow: [{ shadow: [
        "",
        "none",
        h,
        Ve,
        Pe
      ] }],
      "shadow-color": [{ shadow: f() }],
      "inset-shadow": [{ "inset-shadow": [
        "none",
        S,
        Ve,
        Pe
      ] }],
      "inset-shadow-color": [{ "inset-shadow": f() }],
      "ring-w": [{ ring: H() }],
      "ring-w-inset": ["ring-inset"],
      "ring-color": [{ ring: f() }],
      "ring-offset-w": [{ "ring-offset": [x, ue] }],
      "ring-offset-color": [{ "ring-offset": f() }],
      "inset-ring-w": [{ "inset-ring": H() }],
      "inset-ring-color": [{ "inset-ring": f() }],
      "text-shadow": [{ "text-shadow": [
        "none",
        L,
        Ve,
        Pe
      ] }],
      "text-shadow-color": [{ "text-shadow": f() }],
      opacity: [{ opacity: [
        x,
        g,
        v
      ] }],
      "mix-blend": [{ "mix-blend": [
        ...st(),
        "plus-darker",
        "plus-lighter"
      ] }],
      "bg-blend": [{ "bg-blend": st() }],
      "mask-clip": [{ "mask-clip": [
        "border",
        "padding",
        "content",
        "fill",
        "stroke",
        "view"
      ] }, "mask-no-clip"],
      "mask-composite": [{ mask: [
        "add",
        "subtract",
        "intersect",
        "exclude"
      ] }],
      "mask-image-linear-pos": [{ "mask-linear": [x] }],
      "mask-image-linear-from-pos": [{ "mask-linear-from": B() }],
      "mask-image-linear-to-pos": [{ "mask-linear-to": B() }],
      "mask-image-linear-from-color": [{ "mask-linear-from": f() }],
      "mask-image-linear-to-color": [{ "mask-linear-to": f() }],
      "mask-image-t-from-pos": [{ "mask-t-from": B() }],
      "mask-image-t-to-pos": [{ "mask-t-to": B() }],
      "mask-image-t-from-color": [{ "mask-t-from": f() }],
      "mask-image-t-to-color": [{ "mask-t-to": f() }],
      "mask-image-r-from-pos": [{ "mask-r-from": B() }],
      "mask-image-r-to-pos": [{ "mask-r-to": B() }],
      "mask-image-r-from-color": [{ "mask-r-from": f() }],
      "mask-image-r-to-color": [{ "mask-r-to": f() }],
      "mask-image-b-from-pos": [{ "mask-b-from": B() }],
      "mask-image-b-to-pos": [{ "mask-b-to": B() }],
      "mask-image-b-from-color": [{ "mask-b-from": f() }],
      "mask-image-b-to-color": [{ "mask-b-to": f() }],
      "mask-image-l-from-pos": [{ "mask-l-from": B() }],
      "mask-image-l-to-pos": [{ "mask-l-to": B() }],
      "mask-image-l-from-color": [{ "mask-l-from": f() }],
      "mask-image-l-to-color": [{ "mask-l-to": f() }],
      "mask-image-x-from-pos": [{ "mask-x-from": B() }],
      "mask-image-x-to-pos": [{ "mask-x-to": B() }],
      "mask-image-x-from-color": [{ "mask-x-from": f() }],
      "mask-image-x-to-color": [{ "mask-x-to": f() }],
      "mask-image-y-from-pos": [{ "mask-y-from": B() }],
      "mask-image-y-to-pos": [{ "mask-y-to": B() }],
      "mask-image-y-from-color": [{ "mask-y-from": f() }],
      "mask-image-y-to-color": [{ "mask-y-to": f() }],
      "mask-image-radial": [{ "mask-radial": [g, v] }],
      "mask-image-radial-from-pos": [{ "mask-radial-from": B() }],
      "mask-image-radial-to-pos": [{ "mask-radial-to": B() }],
      "mask-image-radial-from-color": [{ "mask-radial-from": f() }],
      "mask-image-radial-to-color": [{ "mask-radial-to": f() }],
      "mask-image-radial-shape": [{ "mask-radial": ["circle", "ellipse"] }],
      "mask-image-radial-size": [{ "mask-radial": [{
        closest: ["side", "corner"],
        farthest: ["side", "corner"]
      }] }],
      "mask-image-radial-pos": [{ "mask-radial-at": oe() }],
      "mask-image-conic-pos": [{ "mask-conic": [x] }],
      "mask-image-conic-from-pos": [{ "mask-conic-from": B() }],
      "mask-image-conic-to-pos": [{ "mask-conic-to": B() }],
      "mask-image-conic-from-color": [{ "mask-conic-from": f() }],
      "mask-image-conic-to-color": [{ "mask-conic-to": f() }],
      "mask-mode": [{ mask: [
        "alpha",
        "luminance",
        "match"
      ] }],
      "mask-origin": [{ "mask-origin": [
        "border",
        "padding",
        "content",
        "fill",
        "stroke",
        "view"
      ] }],
      "mask-position": [{ mask: D() }],
      "mask-repeat": [{ mask: G() }],
      "mask-size": [{ mask: Oe() }],
      "mask-type": [{ "mask-type": ["alpha", "luminance"] }],
      "mask-image": [{ mask: [
        "none",
        g,
        v
      ] }],
      filter: [{ filter: [
        "",
        "none",
        g,
        v
      ] }],
      blur: [{ blur: nt() }],
      brightness: [{ brightness: [
        x,
        g,
        v
      ] }],
      contrast: [{ contrast: [
        x,
        g,
        v
      ] }],
      "drop-shadow": [{ "drop-shadow": [
        "",
        "none",
        N,
        Ve,
        Pe
      ] }],
      "drop-shadow-color": [{ "drop-shadow": f() }],
      grayscale: [{ grayscale: [
        "",
        x,
        g,
        v
      ] }],
      "hue-rotate": [{ "hue-rotate": [
        x,
        g,
        v
      ] }],
      invert: [{ invert: [
        "",
        x,
        g,
        v
      ] }],
      saturate: [{ saturate: [
        x,
        g,
        v
      ] }],
      sepia: [{ sepia: [
        "",
        x,
        g,
        v
      ] }],
      "backdrop-filter": [{ "backdrop-filter": [
        "",
        "none",
        g,
        v
      ] }],
      "backdrop-blur": [{ "backdrop-blur": nt() }],
      "backdrop-brightness": [{ "backdrop-brightness": [
        x,
        g,
        v
      ] }],
      "backdrop-contrast": [{ "backdrop-contrast": [
        x,
        g,
        v
      ] }],
      "backdrop-grayscale": [{ "backdrop-grayscale": [
        "",
        x,
        g,
        v
      ] }],
      "backdrop-hue-rotate": [{ "backdrop-hue-rotate": [
        x,
        g,
        v
      ] }],
      "backdrop-invert": [{ "backdrop-invert": [
        "",
        x,
        g,
        v
      ] }],
      "backdrop-opacity": [{ "backdrop-opacity": [
        x,
        g,
        v
      ] }],
      "backdrop-saturate": [{ "backdrop-saturate": [
        x,
        g,
        v
      ] }],
      "backdrop-sepia": [{ "backdrop-sepia": [
        "",
        x,
        g,
        v
      ] }],
      "border-collapse": [{ border: ["collapse", "separate"] }],
      "border-spacing": [{ "border-spacing": b() }],
      "border-spacing-x": [{ "border-spacing-x": b() }],
      "border-spacing-y": [{ "border-spacing-y": b() }],
      "table-layout": [{ table: ["auto", "fixed"] }],
      caption: [{ caption: ["top", "bottom"] }],
      transition: [{ transition: [
        "",
        "all",
        "colors",
        "opacity",
        "shadow",
        "transform",
        "none",
        g,
        v
      ] }],
      "transition-behavior": [{ transition: ["normal", "discrete"] }],
      duration: [{ duration: [
        x,
        "initial",
        g,
        v
      ] }],
      ease: [{ ease: [
        "linear",
        "initial",
        J,
        g,
        v
      ] }],
      delay: [{ delay: [
        x,
        g,
        v
      ] }],
      animate: [{ animate: [
        "none",
        ne,
        g,
        v
      ] }],
      backface: [{ backface: ["hidden", "visible"] }],
      perspective: [{ perspective: [
        z,
        g,
        v
      ] }],
      "perspective-origin": [{ "perspective-origin": O() }],
      rotate: [{ rotate: Ee() }],
      "rotate-x": [{ "rotate-x": Ee() }],
      "rotate-y": [{ "rotate-y": Ee() }],
      "rotate-z": [{ "rotate-z": Ee() }],
      scale: [{ scale: Le() }],
      "scale-x": [{ "scale-x": Le() }],
      "scale-y": [{ "scale-y": Le() }],
      "scale-z": [{ "scale-z": Le() }],
      "scale-3d": ["scale-3d"],
      skew: [{ skew: He() }],
      "skew-x": [{ "skew-x": He() }],
      "skew-y": [{ "skew-y": He() }],
      transform: [{ transform: [
        g,
        v,
        "",
        "none",
        "gpu",
        "cpu"
      ] }],
      "transform-origin": [{ origin: O() }],
      "transform-style": [{ transform: ["3d", "flat"] }],
      translate: [{ translate: je() }],
      "translate-x": [{ "translate-x": je() }],
      "translate-y": [{ "translate-y": je() }],
      "translate-z": [{ "translate-z": je() }],
      "translate-none": ["translate-none"],
      accent: [{ accent: f() }],
      appearance: [{ appearance: ["none", "auto"] }],
      "caret-color": [{ caret: f() }],
      "color-scheme": [{ scheme: [
        "normal",
        "dark",
        "light",
        "light-dark",
        "only-dark",
        "only-light"
      ] }],
      cursor: [{ cursor: [
        "auto",
        "default",
        "pointer",
        "wait",
        "text",
        "move",
        "help",
        "not-allowed",
        "none",
        "context-menu",
        "progress",
        "cell",
        "crosshair",
        "vertical-text",
        "alias",
        "copy",
        "no-drop",
        "grab",
        "grabbing",
        "all-scroll",
        "col-resize",
        "row-resize",
        "n-resize",
        "e-resize",
        "s-resize",
        "w-resize",
        "ne-resize",
        "nw-resize",
        "se-resize",
        "sw-resize",
        "ew-resize",
        "ns-resize",
        "nesw-resize",
        "nwse-resize",
        "zoom-in",
        "zoom-out",
        g,
        v
      ] }],
      "field-sizing": [{ "field-sizing": ["fixed", "content"] }],
      "pointer-events": [{ "pointer-events": ["auto", "none"] }],
      resize: [{ resize: [
        "none",
        "",
        "y",
        "x"
      ] }],
      "scroll-behavior": [{ scroll: ["auto", "smooth"] }],
      "scroll-m": [{ "scroll-m": b() }],
      "scroll-mx": [{ "scroll-mx": b() }],
      "scroll-my": [{ "scroll-my": b() }],
      "scroll-ms": [{ "scroll-ms": b() }],
      "scroll-me": [{ "scroll-me": b() }],
      "scroll-mbs": [{ "scroll-mbs": b() }],
      "scroll-mbe": [{ "scroll-mbe": b() }],
      "scroll-mt": [{ "scroll-mt": b() }],
      "scroll-mr": [{ "scroll-mr": b() }],
      "scroll-mb": [{ "scroll-mb": b() }],
      "scroll-ml": [{ "scroll-ml": b() }],
      "scroll-p": [{ "scroll-p": b() }],
      "scroll-px": [{ "scroll-px": b() }],
      "scroll-py": [{ "scroll-py": b() }],
      "scroll-ps": [{ "scroll-ps": b() }],
      "scroll-pe": [{ "scroll-pe": b() }],
      "scroll-pbs": [{ "scroll-pbs": b() }],
      "scroll-pbe": [{ "scroll-pbe": b() }],
      "scroll-pt": [{ "scroll-pt": b() }],
      "scroll-pr": [{ "scroll-pr": b() }],
      "scroll-pb": [{ "scroll-pb": b() }],
      "scroll-pl": [{ "scroll-pl": b() }],
      "snap-align": [{ snap: [
        "start",
        "end",
        "center",
        "align-none"
      ] }],
      "snap-stop": [{ snap: ["normal", "always"] }],
      "snap-type": [{ snap: [
        "none",
        "x",
        "y",
        "both"
      ] }],
      "snap-strictness": [{ snap: ["mandatory", "proximity"] }],
      touch: [{ touch: [
        "auto",
        "none",
        "manipulation"
      ] }],
      "touch-x": [{ "touch-pan": [
        "x",
        "left",
        "right"
      ] }],
      "touch-y": [{ "touch-pan": [
        "y",
        "up",
        "down"
      ] }],
      "touch-pz": ["touch-pinch-zoom"],
      select: [{ select: [
        "none",
        "text",
        "all",
        "auto"
      ] }],
      "will-change": [{ "will-change": [
        "auto",
        "scroll",
        "contents",
        "transform",
        g,
        v
      ] }],
      fill: [{ fill: ["none", ...f()] }],
      "stroke-w": [{ stroke: [
        x,
        we,
        ue,
        vt
      ] }],
      stroke: [{ stroke: ["none", ...f()] }],
      "forced-color-adjust": [{ "forced-color-adjust": ["auto", "none"] }]
    },
    conflictingClassGroups: {
      overflow: ["overflow-x", "overflow-y"],
      overscroll: ["overscroll-x", "overscroll-y"],
      inset: [
        "inset-x",
        "inset-y",
        "inset-bs",
        "inset-be",
        "start",
        "end",
        "top",
        "right",
        "bottom",
        "left"
      ],
      "inset-x": ["right", "left"],
      "inset-y": ["top", "bottom"],
      flex: [
        "basis",
        "grow",
        "shrink"
      ],
      gap: ["gap-x", "gap-y"],
      p: [
        "px",
        "py",
        "ps",
        "pe",
        "pbs",
        "pbe",
        "pt",
        "pr",
        "pb",
        "pl"
      ],
      px: ["pr", "pl"],
      py: ["pt", "pb"],
      m: [
        "mx",
        "my",
        "ms",
        "me",
        "mbs",
        "mbe",
        "mt",
        "mr",
        "mb",
        "ml"
      ],
      mx: ["mr", "ml"],
      my: ["mt", "mb"],
      size: ["w", "h"],
      "font-size": ["leading"],
      "fvn-normal": [
        "fvn-ordinal",
        "fvn-slashed-zero",
        "fvn-figure",
        "fvn-spacing",
        "fvn-fraction"
      ],
      "fvn-ordinal": ["fvn-normal"],
      "fvn-slashed-zero": ["fvn-normal"],
      "fvn-figure": ["fvn-normal"],
      "fvn-spacing": ["fvn-normal"],
      "fvn-fraction": ["fvn-normal"],
      "line-clamp": ["display", "overflow"],
      rounded: [
        "rounded-s",
        "rounded-e",
        "rounded-t",
        "rounded-r",
        "rounded-b",
        "rounded-l",
        "rounded-ss",
        "rounded-se",
        "rounded-ee",
        "rounded-es",
        "rounded-tl",
        "rounded-tr",
        "rounded-br",
        "rounded-bl"
      ],
      "rounded-s": ["rounded-ss", "rounded-es"],
      "rounded-e": ["rounded-se", "rounded-ee"],
      "rounded-t": ["rounded-tl", "rounded-tr"],
      "rounded-r": ["rounded-tr", "rounded-br"],
      "rounded-b": ["rounded-br", "rounded-bl"],
      "rounded-l": ["rounded-tl", "rounded-bl"],
      "border-spacing": ["border-spacing-x", "border-spacing-y"],
      "border-w": [
        "border-w-x",
        "border-w-y",
        "border-w-s",
        "border-w-e",
        "border-w-bs",
        "border-w-be",
        "border-w-t",
        "border-w-r",
        "border-w-b",
        "border-w-l"
      ],
      "border-w-x": ["border-w-r", "border-w-l"],
      "border-w-y": ["border-w-t", "border-w-b"],
      "border-color": [
        "border-color-x",
        "border-color-y",
        "border-color-s",
        "border-color-e",
        "border-color-bs",
        "border-color-be",
        "border-color-t",
        "border-color-r",
        "border-color-b",
        "border-color-l"
      ],
      "border-color-x": ["border-color-r", "border-color-l"],
      "border-color-y": ["border-color-t", "border-color-b"],
      translate: [
        "translate-x",
        "translate-y",
        "translate-none"
      ],
      "translate-none": [
        "translate",
        "translate-x",
        "translate-y",
        "translate-z"
      ],
      "scroll-m": [
        "scroll-mx",
        "scroll-my",
        "scroll-ms",
        "scroll-me",
        "scroll-mbs",
        "scroll-mbe",
        "scroll-mt",
        "scroll-mr",
        "scroll-mb",
        "scroll-ml"
      ],
      "scroll-mx": ["scroll-mr", "scroll-ml"],
      "scroll-my": ["scroll-mt", "scroll-mb"],
      "scroll-p": [
        "scroll-px",
        "scroll-py",
        "scroll-ps",
        "scroll-pe",
        "scroll-pbs",
        "scroll-pbe",
        "scroll-pt",
        "scroll-pr",
        "scroll-pb",
        "scroll-pl"
      ],
      "scroll-px": ["scroll-pr", "scroll-pl"],
      "scroll-py": ["scroll-pt", "scroll-pb"],
      touch: [
        "touch-x",
        "touch-y",
        "touch-pz"
      ],
      "touch-x": ["touch"],
      "touch-y": ["touch"],
      "touch-pz": ["touch"]
    },
    conflictingClassGroupModifiers: { "font-size": ["leading"] },
    orderSensitiveModifiers: [
      "*",
      "**",
      "after",
      "backdrop",
      "before",
      "details-content",
      "file",
      "first-letter",
      "first-line",
      "marker",
      "placeholder",
      "selection"
    ]
  };
}, ha = /* @__PURE__ */ Yr(ba);
function re(...e) {
  return ha($t(e));
}
var ya = /* @__PURE__ */ T({
  __name: "Card",
  props: { class: { type: [
    Boolean,
    null,
    String,
    Object,
    Array
  ] } },
  setup(e) {
    const t = e;
    return (r, a) => (c(), m("div", {
      "data-slot": "card",
      class: te(p(re)("bg-card text-card-foreground flex flex-col gap-6 rounded-xl border py-6 shadow-sm", t.class))
    }, [E(r.$slots, "default")], 2));
  }
}), _a = ya, wa = /* @__PURE__ */ T({
  __name: "CardContent",
  props: { class: { type: [
    Boolean,
    null,
    String,
    Object,
    Array
  ] } },
  setup(e) {
    const t = e;
    return (r, a) => (c(), m("div", {
      "data-slot": "card-content",
      class: te(p(re)("px-6", t.class))
    }, [E(r.$slots, "default")], 2));
  }
}), xa = wa, ka = /* @__PURE__ */ T({
  __name: "CardHeader",
  props: { class: { type: [
    Boolean,
    null,
    String,
    Object,
    Array
  ] } },
  setup(e) {
    const t = e;
    return (r, a) => (c(), m("div", {
      "data-slot": "card-header",
      class: te(p(re)("@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6", t.class))
    }, [E(r.$slots, "default")], 2));
  }
}), Ca = ka, ht = (e) => typeof e == "boolean" ? `${e}` : e === 0 ? "0" : e, yt = $t, Dt = (e, t) => (r) => {
  var a;
  if (t?.variants == null) return yt(e, r?.class, r?.className);
  const { variants: o, defaultVariants: s } = t, i = Object.keys(o).map((l) => {
    const d = r?.[l], h = s?.[l];
    if (d === null) return null;
    const S = ht(d) || ht(h);
    return o[l][S];
  }), u = r && Object.entries(r).reduce((l, d) => {
    let [h, S] = d;
    return S === void 0 || (l[h] = S), l;
  }, {});
  return yt(e, i, t == null || (a = t.compoundVariants) === null || a === void 0 ? void 0 : a.reduce((l, d) => {
    let { class: h, className: S, ...L } = d;
    return Object.entries(L).every((N) => {
      let [q, z] = N;
      return Array.isArray(z) ? z.includes({
        ...s,
        ...u
      }[q]) : {
        ...s,
        ...u
      }[q] === z;
    }) ? [
      ...l,
      h,
      S
    ] : l;
  }, []), r?.class, r?.className);
};
function Sa(e, t) {
  const r = typeof e == "string" && !t ? `${e}Context` : t, a = Symbol(r);
  return [(i) => {
    const u = Jt(a, i);
    if (u || u === null) return u;
    throw new Error(`Injection \`${a.toString()}\` not found. Component must be used within ${Array.isArray(e) ? `one of the following components: ${e.join(", ")}` : `\`${e}\``}`);
  }, (i) => (rr(a, i), i)];
}
function Gt(e) {
  return e ? e.flatMap((t) => t.type === se ? Gt(t.children) : [t]) : [];
}
var qe = typeof window < "u" && typeof document < "u", Ds = typeof WorkerGlobalScope < "u" && globalThis instanceof WorkerGlobalScope;
function qt(e) {
  const t = /* @__PURE__ */ Object.create(null);
  return ((r) => t[r] || (t[r] = e(r)));
}
var Aa = /\B([A-Z])/g, Gs = qt((e) => e.replace(Aa, "-$1").toLowerCase()), za = /-(\w)/g, qs = qt((e) => e.replace(za, (t, r) => r ? r.toUpperCase() : "")), Ws = qe ? window.document : void 0, Hs = qe ? window.navigator : void 0, Js = qe ? window.location : void 0;
function $a(e) {
  var t;
  const r = Xt(e);
  return (t = r?.$el) !== null && t !== void 0 ? t : r;
}
var Zs = [
  {
    max: 6e4,
    value: 1e3,
    name: "second"
  },
  {
    max: 276e4,
    value: 6e4,
    name: "minute"
  },
  {
    max: 72e6,
    value: 36e5,
    name: "hour"
  },
  {
    max: 5184e5,
    value: 864e5,
    name: "day"
  },
  {
    max: 24192e5,
    value: 6048e5,
    name: "week"
  },
  {
    max: 28512e6,
    value: 2592e6,
    name: "month"
  },
  {
    max: Number.POSITIVE_INFINITY,
    value: 31536e6,
    name: "year"
  }
];
function rt() {
  const e = Ue(), t = X(), r = I(() => a());
  ar(() => {
    r.value !== a() && Qt(t);
  });
  function a() {
    return t.value && "$el" in t.value && ["#text", "#comment"].includes(t.value.$el.nodeName) ? t.value.$el.nextElementSibling : $a(t);
  }
  const o = Object.assign({}, e.exposed), s = {};
  for (const u in e.props) Object.defineProperty(s, u, {
    enumerable: !0,
    configurable: !0,
    get: () => e.props[u]
  });
  if (Object.keys(o).length > 0) for (const u in o) Object.defineProperty(s, u, {
    enumerable: !0,
    configurable: !0,
    get: () => o[u]
  });
  Object.defineProperty(s, "$el", {
    enumerable: !0,
    configurable: !0,
    get: () => e.vnode.el
  }), e.exposed = s;
  function i(u) {
    if (t.value = u, !!u && (Object.defineProperty(s, "$el", {
      enumerable: !0,
      configurable: !0,
      get: () => u instanceof Element ? u : u.$el
    }), !(u instanceof Element) && !Object.prototype.hasOwnProperty.call(u, "$el"))) {
      const l = u.$.exposed, d = Object.assign({}, s);
      for (const h in l) Object.defineProperty(d, h, {
        enumerable: !0,
        configurable: !0,
        get: () => l[h]
      });
      e.exposed = d;
    }
  }
  return {
    forwardRef: i,
    currentRef: t,
    currentElement: r
  };
}
var Ta = T({
  name: "PrimitiveSlot",
  inheritAttrs: !1,
  setup(e, { attrs: t, slots: r }) {
    return () => {
      if (!r.default) return null;
      const a = Gt(r.default()), o = a.findIndex((l) => l.type !== or);
      if (o === -1) return a;
      const s = a[o];
      delete s.props?.ref;
      const i = s.props ? be(t, s.props) : t, u = tr({
        ...s,
        props: {}
      }, i);
      return a.length === 1 ? u : (a[o] = u, a);
    };
  }
}), Oa = [
  "area",
  "img",
  "input"
], Ae = T({
  name: "Primitive",
  inheritAttrs: !1,
  props: {
    asChild: {
      type: Boolean,
      default: !1
    },
    as: {
      type: [String, Object],
      default: "div"
    }
  },
  setup(e, { attrs: t, slots: r }) {
    const a = e.asChild ? "template" : e.as;
    return typeof a == "string" && Oa.includes(a) ? () => ge(a, t) : a !== "template" ? () => ge(e.as, t, { default: r.default }) : () => ge(Ta, t, { default: r.default });
  }
}), [Ia, Ea] = Sa("AvatarRoot"), La = /* @__PURE__ */ T({
  __name: "AvatarRoot",
  props: {
    asChild: {
      type: Boolean,
      required: !1
    },
    as: {
      type: null,
      required: !1,
      default: "span"
    }
  },
  setup(e) {
    return rt(), Ea({ imageLoadingStatus: X("idle") }), (t, r) => (c(), $(p(Ae), {
      "as-child": t.asChild,
      as: t.as
    }, {
      default: C(() => [E(t.$slots, "default")]),
      _: 3
    }, 8, ["as-child", "as"]));
  }
}), ja = /* @__PURE__ */ T({
  __name: "AvatarFallback",
  props: {
    delayMs: {
      type: Number,
      required: !1
    },
    asChild: {
      type: Boolean,
      required: !1
    },
    as: {
      type: null,
      required: !1,
      default: "span"
    }
  },
  setup(e) {
    const t = e, r = Ia();
    rt();
    const a = X(t.delayMs === void 0);
    return Yt((o) => {
      if (t.delayMs && qe) {
        const s = window.setTimeout(() => {
          a.value = !0;
        }, t.delayMs);
        o(() => {
          window.clearTimeout(s);
        });
      }
    }), (o, s) => a.value && p(r).imageLoadingStatus.value !== "loaded" ? (c(), $(p(Ae), {
      key: 0,
      "as-child": o.asChild,
      as: o.as
    }, {
      default: C(() => [E(o.$slots, "default")]),
      _: 3
    }, 8, ["as-child", "as"])) : P("v-if", !0);
  }
}), Pa = /* @__PURE__ */ T({
  __name: "Label",
  props: {
    for: {
      type: String,
      required: !1
    },
    asChild: {
      type: Boolean,
      required: !1
    },
    as: {
      type: null,
      required: !1,
      default: "label"
    }
  },
  setup(e) {
    const t = e;
    return rt(), (r, a) => (c(), $(p(Ae), be(t, { onMousedown: a[0] || (a[0] = (o) => {
      !o.defaultPrevented && o.detail > 1 && o.preventDefault();
    }) }), {
      default: C(() => [E(r.$slots, "default")]),
      _: 3
    }, 16));
  }
}), Va = /* @__PURE__ */ T({
  __name: "BaseSeparator",
  props: {
    orientation: {
      type: String,
      required: !1,
      default: "horizontal"
    },
    decorative: {
      type: Boolean,
      required: !1
    },
    asChild: {
      type: Boolean,
      required: !1
    },
    as: {
      type: null,
      required: !1
    }
  },
  setup(e) {
    const t = e, r = ["horizontal", "vertical"];
    function a(u) {
      return r.includes(u);
    }
    const o = I(() => a(t.orientation) ? t.orientation : "horizontal"), s = I(() => o.value === "vertical" ? t.orientation : void 0), i = I(() => t.decorative ? { role: "none" } : {
      "aria-orientation": s.value,
      role: "separator"
    });
    return (u, l) => (c(), $(p(Ae), be({
      as: u.as,
      "as-child": u.asChild,
      "data-orientation": o.value
    }, i.value), {
      default: C(() => [E(u.$slots, "default")]),
      _: 3
    }, 16, [
      "as",
      "as-child",
      "data-orientation"
    ]));
  }
}), Ma = /* @__PURE__ */ T({
  __name: "Separator",
  props: {
    orientation: {
      type: String,
      required: !1,
      default: "horizontal"
    },
    decorative: {
      type: Boolean,
      required: !1
    },
    asChild: {
      type: Boolean,
      required: !1
    },
    as: {
      type: null,
      required: !1
    }
  },
  setup(e) {
    const t = e;
    return (r, a) => (c(), $(Va, ir(cr(t)), {
      default: C(() => [E(r.$slots, "default")]),
      _: 3
    }, 16));
  }
}), Ra = /* @__PURE__ */ T({
  __name: "Button",
  props: {
    variant: {},
    size: {},
    class: { type: [
      Boolean,
      null,
      String,
      Object,
      Array
    ] },
    asChild: { type: Boolean },
    as: { default: "button" }
  },
  setup(e) {
    const t = e;
    return (r, a) => (c(), $(p(Ae), {
      "data-slot": "button",
      "data-variant": e.variant,
      "data-size": e.size,
      as: e.as,
      "as-child": e.asChild,
      class: te(p(re)(p(Ba)({
        variant: e.variant,
        size: e.size
      }), t.class))
    }, {
      default: C(() => [E(r.$slots, "default")]),
      _: 3
    }, 8, [
      "data-variant",
      "data-size",
      "as",
      "as-child",
      "class"
    ]));
  }
}), ve = Ra, Ba = Dt("inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive", {
  variants: {
    variant: {
      default: "bg-primary text-primary-foreground hover:bg-primary/90",
      destructive: "bg-destructive text-white hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60",
      outline: "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/50",
      secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
      ghost: "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
      link: "text-primary underline-offset-4 hover:underline"
    },
    size: {
      default: "h-9 px-4 py-2 has-[>svg]:px-3",
      sm: "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
      lg: "h-10 rounded-md px-6 has-[>svg]:px-4",
      icon: "size-9",
      "icon-sm": "size-8",
      "icon-lg": "size-10"
    }
  },
  defaultVariants: {
    variant: "default",
    size: "default"
  }
});
function Na(e) {
  return typeof e == "function" ? e() : p(e);
}
function Fa(e) {
  return Be(e) ? Ne(new Proxy({}, {
    get(t, r, a) {
      return p(Reflect.get(e.value, r, a));
    },
    set(t, r, a) {
      return Be(e.value[r]) && !Be(a) ? e.value[r].value = a : e.value[r] = a, !0;
    },
    deleteProperty(t, r) {
      return Reflect.deleteProperty(e.value, r);
    },
    has(t, r) {
      return Reflect.has(e.value, r);
    },
    ownKeys() {
      return Object.keys(e.value);
    },
    getOwnPropertyDescriptor() {
      return {
        enumerable: !0,
        configurable: !0
      };
    }
  })) : Ne(e);
}
function Ua(e) {
  return Fa(I(e));
}
function at(e, ...t) {
  const r = t.flat(), a = r[0];
  return Ua(() => Object.fromEntries(typeof a == "function" ? Object.entries(it(e)).filter(([o, s]) => !a(Na(s), o)) : Object.entries(it(e)).filter((o) => !r.includes(o[0]))));
}
var ot = typeof window < "u" && typeof document < "u", Ys = typeof WorkerGlobalScope < "u" && globalThis instanceof WorkerGlobalScope, Da = (e) => typeof e < "u";
function Wt(e) {
  const t = /* @__PURE__ */ Object.create(null);
  return (r) => t[r] || (t[r] = e(r));
}
var Ga = /\B([A-Z])/g, Ks = Wt((e) => e.replace(Ga, "-$1").toLowerCase()), qa = /-(\w)/g, Xs = Wt((e) => e.replace(qa, (t, r) => r ? r.toUpperCase() : "")), Qs = ot ? window.document : void 0, en = ot ? window.navigator : void 0, tn = ot ? window.location : void 0;
function Wa(e) {
  return JSON.parse(JSON.stringify(e));
}
var rn = [
  {
    max: 6e4,
    value: 1e3,
    name: "second"
  },
  {
    max: 276e4,
    value: 6e4,
    name: "minute"
  },
  {
    max: 72e6,
    value: 36e5,
    name: "hour"
  },
  {
    max: 5184e5,
    value: 864e5,
    name: "day"
  },
  {
    max: 24192e5,
    value: 6048e5,
    name: "week"
  },
  {
    max: 28512e6,
    value: 2592e6,
    name: "month"
  },
  {
    max: Number.POSITIVE_INFINITY,
    value: 31536e6,
    name: "year"
  }
];
function Ha(e, t, r, a = {}) {
  var o, s, i, u, l;
  const { clone: d = !1, passive: h = !1, eventName: S, deep: L = !1, defaultValue: N, shouldEmit: q } = a, z = Ue(), V = r || z?.emit || ((o = z?.$emit) == null ? void 0 : o.bind(z)) || ((i = (s = z?.proxy) == null ? void 0 : s.$emit) == null ? void 0 : i.bind(z?.proxy));
  let J = S;
  t || (t = "modelValue"), J = J || `update:${t.toString()}`;
  const ne = (O) => d ? typeof d == "function" ? d(O) : Wa(O) : O, ae = () => Da(e[t]) ? ne(e[t]) : N, oe = (O) => {
    q ? q(O) && V(J, O) : V(J, O);
  };
  if (h) {
    const O = X(ae());
    let Z = !1;
    return Fe(() => e[t], (Q) => {
      Z || (Z = !0, O.value = ne(Q), Zt(() => Z = !1));
    }), Fe(O, (Q) => {
      !Z && (Q !== e[t] || L) && oe(Q);
    }, { deep: L }), O;
  } else return I({
    get() {
      return ae();
    },
    set(O) {
      oe(O);
    }
  });
}
var Ja = /* @__PURE__ */ T({
  __name: "Input",
  props: {
    defaultValue: {},
    modelValue: {},
    class: { type: [
      Boolean,
      null,
      String,
      Object,
      Array
    ] }
  },
  emits: ["update:modelValue"],
  setup(e, { emit: t }) {
    const r = e, a = Ha(r, "modelValue", t, {
      passive: !0,
      defaultValue: r.defaultValue
    });
    return (o, s) => Ct((c(), m("input", {
      "onUpdate:modelValue": s[0] || (s[0] = (i) => Be(a) ? a.value = i : null),
      "data-slot": "input",
      class: te(p(re)("file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm", "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]", "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive", r.class))
    }, null, 2)), [[nr, p(a)]]);
  }
}), Ye = Ja, Za = /* @__PURE__ */ T({
  __name: "Label",
  props: {
    for: {},
    asChild: { type: Boolean },
    as: {},
    class: { type: [
      Boolean,
      null,
      String,
      Object,
      Array
    ] }
  },
  setup(e) {
    const t = e, r = at(t, "class");
    return (a, o) => (c(), $(p(Pa), be({ "data-slot": "label" }, p(r), { class: p(re)("flex items-center gap-2 text-sm leading-none font-medium select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50", t.class) }), {
      default: C(() => [E(a.$slots, "default")]),
      _: 3
    }, 16, ["class"]));
  }
}), Me = Za, Ya = /* @__PURE__ */ T({
  __name: "Separator",
  props: {
    orientation: { default: "horizontal" },
    decorative: {
      type: Boolean,
      default: !0
    },
    asChild: { type: Boolean },
    as: {},
    class: { type: [
      Boolean,
      null,
      String,
      Object,
      Array
    ] }
  },
  setup(e) {
    const t = e, r = at(t, "class");
    return (a, o) => (c(), $(p(Ma), be({ "data-slot": "separator" }, p(r), { class: p(re)("bg-border shrink-0 data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px", t.class) }), null, 16, ["class"]));
  }
}), Ka = Ya, Xa = /* @__PURE__ */ T({
  __name: "Avatar",
  props: { class: { type: [
    Boolean,
    null,
    String,
    Object,
    Array
  ] } },
  setup(e) {
    const t = e;
    return (r, a) => (c(), $(p(La), {
      "data-slot": "avatar",
      class: te(p(re)("relative flex size-8 shrink-0 overflow-hidden rounded-full", t.class))
    }, {
      default: C(() => [E(r.$slots, "default")]),
      _: 3
    }, 8, ["class"]));
  }
}), Qa = Xa, eo = /* @__PURE__ */ T({
  __name: "AvatarFallback",
  props: {
    delayMs: {},
    asChild: { type: Boolean },
    as: {},
    class: { type: [
      Boolean,
      null,
      String,
      Object,
      Array
    ] }
  },
  setup(e) {
    const t = e, r = at(t, "class");
    return (a, o) => (c(), $(p(ja), be({ "data-slot": "avatar-fallback" }, p(r), { class: p(re)("bg-muted flex size-full items-center justify-center rounded-full", t.class) }), {
      default: C(() => [E(a.$slots, "default")]),
      _: 3
    }, 16, ["class"]));
  }
}), to = eo, ro = /* @__PURE__ */ T({
  __name: "Alert",
  props: {
    class: { type: [
      Boolean,
      null,
      String,
      Object,
      Array
    ] },
    variant: {}
  },
  setup(e) {
    const t = e;
    return (r, a) => (c(), m("div", {
      "data-slot": "alert",
      class: te(p(re)(p(oo)({ variant: e.variant }), t.class)),
      role: "alert"
    }, [E(r.$slots, "default")], 2));
  }
}), xe = ro, ao = /* @__PURE__ */ T({
  __name: "AlertDescription",
  props: { class: { type: [
    Boolean,
    null,
    String,
    Object,
    Array
  ] } },
  setup(e) {
    const t = e;
    return (r, a) => (c(), m("div", {
      "data-slot": "alert-description",
      class: te(p(re)("text-muted-foreground col-start-2 grid justify-items-start gap-1 text-sm [&_p]:leading-relaxed", t.class))
    }, [E(r.$slots, "default")], 2));
  }
}), ke = ao, oo = Dt("relative w-full rounded-lg border px-4 py-3 text-sm grid has-[>svg]:grid-cols-[calc(var(--spacing)*4)_1fr] grid-cols-[0_1fr] has-[>svg]:gap-x-3 gap-y-0.5 items-start [&>svg]:size-4 [&>svg]:translate-y-0.5 [&>svg]:text-current", {
  variants: { variant: {
    default: "bg-card text-card-foreground",
    destructive: "text-destructive bg-card [&>svg]:text-current *:data-[slot=alert-description]:text-destructive/90"
  } },
  defaultVariants: { variant: "default" }
}), so = (e) => {
  for (const t in e) if (t.startsWith("aria-") || t === "role" || t === "title") return !0;
  return !1;
}, _t = (e) => e === "", no = (...e) => e.filter((t, r, a) => !!t && t.trim() !== "" && a.indexOf(t) === r).join(" ").trim(), wt = (e) => e.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase(), io = (e) => e.replace(/^([A-Z])|[\s-_]+(\w)/g, (t, r, a) => a ? a.toUpperCase() : r.toLowerCase()), lo = (e) => {
  const t = io(e);
  return t.charAt(0).toUpperCase() + t.slice(1);
}, Ce = {
  xmlns: "http://www.w3.org/2000/svg",
  width: 24,
  height: 24,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  "stroke-width": 2,
  "stroke-linecap": "round",
  "stroke-linejoin": "round"
}, co = ({ name: e, iconNode: t, absoluteStrokeWidth: r, "absolute-stroke-width": a, strokeWidth: o, "stroke-width": s, size: i = Ce.width, color: u = Ce.stroke, ...l }, { slots: d }) => ge("svg", {
  ...Ce,
  ...l,
  width: i,
  height: i,
  stroke: u,
  "stroke-width": _t(r) || _t(a) || r === !0 || a === !0 ? Number(o || s || Ce["stroke-width"]) * 24 / Number(i) : o || s || Ce["stroke-width"],
  class: no("lucide", l.class, ...e ? [`lucide-${wt(lo(e))}-icon`, `lucide-${wt(e)}`] : ["lucide-icon"]),
  ...!d.default && !so(l) && { "aria-hidden": "true" }
}, [...t.map((h) => ge(...h)), ...d.default ? [d.default()] : []]), Ht = (e, t) => (r, { slots: a, attrs: o }) => ge(co, {
  ...o,
  ...r,
  iconNode: t,
  name: e
}, a), Ke = Ht("circle-alert", [
  ["circle", {
    cx: "12",
    cy: "12",
    r: "10",
    key: "1mglay"
  }],
  ["line", {
    x1: "12",
    x2: "12",
    y1: "8",
    y2: "12",
    key: "1pkeuh"
  }],
  ["line", {
    x1: "12",
    x2: "12.01",
    y1: "16",
    y2: "16",
    key: "4dfq90"
  }]
]), uo = Ht("loader-circle", [["path", {
  d: "M21 12a9 9 0 1 1-6.219-8.56",
  key: "13zald"
}]]), fo = /* @__PURE__ */ T({
  __name: "Spinner",
  props: { class: { type: [
    Boolean,
    null,
    String,
    Object,
    Array
  ] } },
  setup(e) {
    const t = e;
    return (r, a) => (c(), $(p(uo), {
      role: "status",
      "aria-label": "Loading",
      class: te(p(re)("size-4 animate-spin", t.class))
    }, null, 8, ["class"]));
  }
}), Re = fo, po = ["href"], mo = { class: "centered-inner" }, vo = /* @__PURE__ */ T({
  __name: "CenteredLayout",
  props: { branding: {} },
  setup(e) {
    const t = e, r = I(() => {
      const a = (t.branding?.colors || {}).background || "#f0f2ff";
      return {
        "--brand-bg": a,
        background: `linear-gradient(135deg, ${a} 0%, #fafbff 50%, #f5f3ff 100%)`,
        fontFamily: t.branding?.font_family || "Inter, system-ui, sans-serif"
      };
    });
    return (a, o) => (c(), m("div", {
      class: "login-layout-centered",
      style: Se(r.value)
    }, [
      e.branding?.font_url ? (c(), m("link", {
        key: 0,
        rel: "stylesheet",
        href: e.branding.font_url
      }, null, 8, po)) : P("", !0),
      k("div", mo, [E(a.$slots, "default", {}, void 0, !0)]),
      E(a.$slots, "footer", {}, void 0, !0)
    ], 4));
  }
}), xt = /* @__PURE__ */ he(vo, [["__scopeId", "data-v-38188fe8"]]), go = ["href"], bo = { class: "split-form-side" }, ho = { class: "split-brand" }, yo = {
  href: "#",
  class: "split-brand-link"
}, _o = ["src", "alt"], wo = {
  key: 1,
  class: "split-org-name"
}, xo = { class: "split-form-content" }, ko = { class: "split-form-inner" }, Co = { class: "split-cover-side" }, So = ["src"], Ao = {
  key: 1,
  class: "split-cover-placeholder"
}, zo = /* @__PURE__ */ T({
  __name: "SplitLayout",
  props: { branding: {} },
  setup(e) {
    const t = e, r = I(() => t.branding?.dark_mode === "dark"), a = I(() => r.value && t.branding?.logo_dark ? t.branding.logo_dark : t.branding?.logo_url || ""), o = I(() => ({ fontFamily: t.branding?.font_family || "Inter, system-ui, sans-serif" }));
    return (s, i) => (c(), m("div", {
      class: "login-layout-split",
      style: Se(o.value)
    }, [
      e.branding?.font_url ? (c(), m("link", {
        key: 0,
        rel: "stylesheet",
        href: e.branding.font_url
      }, null, 8, go)) : P("", !0),
      k("div", bo, [k("div", ho, [k("a", yo, [a.value ? (c(), m("img", {
        key: 0,
        src: a.value,
        alt: e.branding?.org_name || "Logo",
        class: "split-logo"
      }, null, 8, _o)) : (c(), m("span", wo, A(e.branding?.org_name || "Zitadel"), 1))])]), k("div", xo, [k("div", ko, [E(s.$slots, "default", {}, void 0, !0)])])]),
      k("div", Co, [e.branding?.cover_image ? (c(), m("img", {
        key: 0,
        src: e.branding.cover_image,
        alt: "",
        class: "split-cover-img"
      }, null, 8, So)) : (c(), m("div", Ao, [...i[0] || (i[0] = [k("svg", {
        viewBox: "0 0 24 24",
        class: "split-cover-icon",
        fill: "none",
        stroke: "currentColor",
        "stroke-width": "1"
      }, [
        k("rect", {
          x: "3",
          y: "3",
          width: "18",
          height: "18",
          rx: "2"
        }),
        k("circle", {
          cx: "8.5",
          cy: "8.5",
          r: "1.5"
        }),
        k("path", { d: "m21 15-5-5L5 21" })
      ], -1)])]))]),
      E(s.$slots, "footer", {}, void 0, !0)
    ], 4));
  }
}), $o = /* @__PURE__ */ he(zo, [["__scopeId", "data-v-db2c5bfb"]]), To = ["href"], Oo = { class: "muted-wrapper" }, Io = { class: "muted-brand" }, Eo = {
  href: "#",
  class: "muted-brand-link"
}, Lo = ["src", "alt"], jo = { class: "muted-org-name" }, Po = { class: "muted-card-wrap" }, Vo = /* @__PURE__ */ T({
  __name: "MutedLayout",
  props: { branding: {} },
  setup(e) {
    const t = e, r = I(() => t.branding?.dark_mode === "dark"), a = I(() => r.value && t.branding?.logo_dark ? t.branding.logo_dark : t.branding?.logo_url || ""), o = I(() => ({
      fontFamily: t.branding?.font_family || "Inter, system-ui, sans-serif",
      background: "hsl(var(--muted, 0 0% 96%))"
    }));
    return (s, i) => (c(), m("div", {
      class: "login-layout-muted",
      style: Se(o.value)
    }, [e.branding?.font_url ? (c(), m("link", {
      key: 0,
      rel: "stylesheet",
      href: e.branding.font_url
    }, null, 8, To)) : P("", !0), k("div", Oo, [
      k("div", Io, [k("a", Eo, [a.value ? (c(), m("img", {
        key: 0,
        src: a.value,
        alt: e.branding?.org_name || "Logo",
        class: "muted-logo"
      }, null, 8, Lo)) : P("", !0), k("span", jo, A(e.branding?.org_name || "Zitadel"), 1)])]),
      k("div", Po, [E(s.$slots, "default", {}, void 0, !0)]),
      E(s.$slots, "footer", {}, void 0, !0)
    ])], 4));
  }
}), Mo = /* @__PURE__ */ he(Vo, [["__scopeId", "data-v-f65c3936"]]), Ro = ["href"], Bo = { class: "card-image-outer" }, No = { class: "card-image-card" }, Fo = { class: "card-image-form" }, Uo = { class: "card-image-media" }, Do = ["src"], Go = {
  key: 1,
  class: "card-image-placeholder"
}, qo = /* @__PURE__ */ T({
  __name: "CardImageLayout",
  props: { branding: {} },
  setup(e) {
    const t = e, r = I(() => ({
      background: `linear-gradient(135deg, ${(t.branding?.colors || {}).background || "#f0f2ff"} 0%, #fafbff 50%, #f5f3ff 100%)`,
      fontFamily: t.branding?.font_family || "Inter, system-ui, sans-serif"
    }));
    return (a, o) => (c(), m("div", {
      class: "login-layout-card-image",
      style: Se(r.value)
    }, [e.branding?.font_url ? (c(), m("link", {
      key: 0,
      rel: "stylesheet",
      href: e.branding.font_url
    }, null, 8, Ro)) : P("", !0), k("div", Bo, [k("div", No, [k("div", Fo, [E(a.$slots, "default", {}, void 0, !0)]), k("div", Uo, [e.branding?.cover_image ? (c(), m("img", {
      key: 0,
      src: e.branding.cover_image,
      alt: "",
      class: "card-image-img"
    }, null, 8, Do)) : (c(), m("div", Go, [...o[0] || (o[0] = [k("svg", {
      viewBox: "0 0 24 24",
      class: "card-image-icon",
      fill: "none",
      stroke: "currentColor",
      "stroke-width": "1"
    }, [
      k("rect", {
        x: "3",
        y: "3",
        width: "18",
        height: "18",
        rx: "2"
      }),
      k("circle", {
        cx: "8.5",
        cy: "8.5",
        r: "1.5"
      }),
      k("path", { d: "m21 15-5-5L5 21" })
    ], -1)])]))])]), E(a.$slots, "footer", {}, void 0, !0)])], 4));
  }
}), Wo = /* @__PURE__ */ he(qo, [["__scopeId", "data-v-a6981d4a"]]), Ho = ["href"], Jo = { class: "minimal-inner" }, Zo = /* @__PURE__ */ T({
  __name: "MinimalLayout",
  props: { branding: {} },
  setup(e) {
    const t = e, r = I(() => ({ fontFamily: t.branding?.font_family || "Inter, system-ui, sans-serif" }));
    return (a, o) => (c(), m("div", {
      class: "login-layout-minimal",
      style: Se(r.value)
    }, [
      e.branding?.font_url ? (c(), m("link", {
        key: 0,
        rel: "stylesheet",
        href: e.branding.font_url
      }, null, 8, Ho)) : P("", !0),
      k("div", Jo, [E(a.$slots, "default", {}, void 0, !0)]),
      E(a.$slots, "footer", {}, void 0, !0)
    ], 4));
  }
}), Yo = /* @__PURE__ */ he(Zo, [["__scopeId", "data-v-502feaac"]]), Ko = ["href"], Xo = {
  key: 0,
  class: "flex justify-center mb-2"
}, Qo = ["src", "alt"], es = {
  key: 1,
  class: "text-xl font-bold tracking-tight mb-2"
}, ts = {
  key: 3,
  class: "flex justify-center py-8"
}, rs = {
  key: 0,
  class: "text-xl font-semibold text-center"
}, as = {
  key: 1,
  class: "text-sm text-muted-foreground text-center"
}, os = {
  key: 2,
  class: "flex flex-col items-center gap-1"
}, ss = {
  key: 0,
  class: "text-sm text-muted-foreground"
}, ns = {
  key: 3,
  class: "text-center text-3xl"
}, is = {
  key: 6,
  class: "flex justify-center py-4"
}, ls = {
  key: 7,
  class: "space-y-1.5"
}, cs = {
  key: 0,
  class: "flex items-center justify-between"
}, ds = {
  key: 0,
  class: "text-xs text-destructive"
}, us = {
  key: 8,
  class: "text-xs text-muted-foreground -mt-2 pl-0.5"
}, fs = {
  key: 9,
  class: "flex justify-end -mt-2"
}, ps = ["onClick"], ms = {
  key: 10,
  class: "flex items-start gap-2"
}, vs = [
  "id",
  "required",
  "onUpdate:modelValue"
], gs = ["for", "innerHTML"], bs = ["name", "value"], hs = {
  key: 13,
  class: "relative py-2"
}, ys = {
  key: 14,
  class: "space-y-2"
}, _s = { class: "text-base" }, ws = { class: "text-base" }, xs = {
  key: 19,
  class: "text-xs text-muted-foreground text-center pt-2"
}, ks = ["href"], Cs = ["href"], Ss = {
  key: 20,
  class: "space-y-2"
}, As = {
  key: 1,
  xmlns: "http://www.w3.org/2000/svg",
  viewBox: "0 0 20 20",
  fill: "currentColor",
  class: "size-5 text-green-500"
}, zs = { class: "text-muted-foreground text-xs" }, $s = {
  key: 21,
  class: "space-y-2"
}, Ts = { class: "flex items-center gap-2 rounded-md border border-input px-3 py-2 text-sm" }, Os = ["checked", "onClick"], Is = { class: "text-muted-foreground text-xs" }, Es = {
  key: 22,
  class: "hidden"
}, Ls = {
  key: 23,
  class: "space-y-4"
}, js = {
  key: 0,
  class: "space-y-1.5"
}, Ps = {
  key: 0,
  class: "mt-6 text-xs text-muted-foreground text-center"
}, Vs = /* @__PURE__ */ T({
  __name: "LoginApp",
  props: {
    apiBaseUrl: { default: "" },
    redirectUri: { default: "" },
    state: { default: "" },
    layoutOverride: { default: "" },
    darkModeOverride: { default: "" },
    coverImageOverride: { default: "" },
    primaryColorOverride: { default: "" }
  },
  emits: [
    "login-complete",
    "login-error",
    "login-redirect"
  ],
  setup(e, { emit: t }) {
    const r = {
      centered: xt,
      split: $o,
      muted: Mo,
      card_image: Wo,
      minimal: Yo
    }, a = e, o = t, s = X(null), i = X(null), u = X(""), l = X(!1), d = Ne({}), h = Ne({}), S = X(""), L = X(!1), N = X(!1), q = X(null), z = X(!1), V = I(() => {
      const _ = a.layoutOverride;
      return _ && r[_] ? _ : i.value?.layout || "centered";
    }), J = I(() => r[V.value] || xt), ne = I(() => a.darkModeOverride ? a.darkModeOverride : i.value?.dark_mode || "light");
    Fe(ne, (_) => {
      const y = document.documentElement;
      if (_ === "dark") y.classList.add("dark");
      else if (_ === "auto") {
        const n = window.matchMedia("(prefers-color-scheme: dark)");
        y.classList.toggle("dark", n.matches);
      } else y.classList.remove("dark");
    }, { immediate: !0 });
    const ae = I(() => ne.value === "dark" && i.value?.logo_dark ? i.value.logo_dark : i.value?.logo_url || ""), oe = I(() => V.value === "split" || V.value === "card_image" ? "" : "max-w-sm"), O = I(() => s.value?.step === "register"), Z = I(() => {
      if (!O.value) return !0;
      for (const _ of Object.keys(h)) if (h[_] && d[_] !== h[_]) return !1;
      return !0;
    }), Q = I(() => s.value ? s.value.nodes.findIndex((_) => _.type === "input") : -1), b = {
      google: "🔵",
      entraid: "🟦",
      gitlab: "🦊",
      apple: "🍎",
      github: "🐙",
      custom: "🔑"
    };
    function U(_) {
      return b[_] || "🔑";
    }
    function ze(_) {
      return _.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" class="underline underline-offset-4 hover:text-foreground">$1</a>');
    }
    kt(async () => {
      _r({
        baseUrl: a.apiBaseUrl || "",
        enabled: !0
      });
      try {
        const _ = await dt.create();
        s.value = _, i.value = _.branding, Cr(_.flow_id), a.primaryColorOverride && i.value && (i.value.colors = {
          ...i.value.colors,
          primary: a.primaryColorOverride
        }), a.coverImageOverride && i.value && (i.value.cover_image = a.coverImageOverride);
        for (const y of _.nodes) y.name && y.value && (d[y.name] = y.value);
        pe(_);
      } catch {
        u.value = "Failed to initialize login flow", o("login-error", {
          code: "init_failed",
          message: "Failed to initialize login flow"
        });
      }
    }), Kt(() => {
      kr();
    });
    async function $e() {
      await K(S.value || "identifier");
    }
    async function K(_, y) {
      if (!s.value) return;
      l.value = !0, u.value = "";
      const n = xr(_, s.value.flow_id), F = s.value.step;
      try {
        const w = {
          action: _,
          ...d,
          ...y
        }, f = await dt.submit(s.value.flow_id, _, w);
        if ("redirect_url" in f && f.redirect_url) {
          o("login-redirect", { redirect_url: f.redirect_url }), window.location.href = f.redirect_url;
          return;
        }
        if ("redirect_uri" in f && f.redirect_uri) {
          const G = f;
          o("login-complete", {
            session_id: String(G.session_id),
            redirect_uri: G.redirect_uri
          }), window.location.href = G.redirect_uri;
          return;
        }
        const D = f;
        s.value = D, D.branding && (i.value = D.branding), D.step !== F && wr(F || "unknown", D.step, D.flow_id), d.password && (d.password = ""), Object.keys(h).forEach((G) => {
          h[G] = "";
        });
        for (const G of D.nodes) G.name && G.value && !d[G.name] && (d[G.name] = G.value);
        pe(D), N.value = !1, L.value = !1;
      } catch (w) {
        const f = w.message || "Something went wrong";
        u.value = f, o("login-error", {
          code: "submit_failed",
          message: f
        });
      } finally {
        l.value = !1, S.value = "", n && n.end();
      }
    }
    async function Te() {
      if (!(!s.value || L.value || N.value)) {
        L.value = !0;
        try {
          const _ = a.apiBaseUrl || "", y = await (await fetch(`${_}/v1/captcha/challenge`, { credentials: "include" })).json(), n = performance.now();
          let F = -1;
          for (let f = 0; f <= y.maxnumber; f++) {
            const D = y.salt + String(f), G = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(D));
            if (Array.from(new Uint8Array(G)).map((Oe) => Oe.toString(16).padStart(2, "0")).join("") === y.challenge) {
              F = f;
              break;
            }
          }
          const w = Math.round(performance.now() - n);
          if (F === -1) {
            L.value = !1, u.value = "Captcha challenge could not be solved";
            return;
          }
          await K("captcha_submit", { altcha_payload: JSON.stringify({
            algorithm: y.algorithm,
            challenge: y.challenge,
            number: F,
            salt: y.salt,
            signature: y.signature,
            took: w
          }) }), N.value = !0;
        } catch {
          u.value = "Captcha verification failed";
        } finally {
          L.value = !1;
        }
      }
    }
    async function pe(_) {
      if (!z.value && _.nodes.some((y) => y.type === "fingerprint_collect"))
        try {
          const y = await zr();
          await Tr(a.apiBaseUrl || "", _.flow_id, y), z.value = !0;
        } catch {
        }
    }
    return (_, y) => (c(), $(lt(J.value), { branding: i.value }, {
      footer: C(() => [i.value?.hide_zitadel_branding ? P("", !0) : (c(), m("p", Ps, " Powered by Zitadel "))]),
      default: C(() => [
        i.value?.font_url ? (c(), m("link", {
          key: 0,
          rel: "stylesheet",
          href: i.value.font_url
        }, null, 8, Ko)) : P("", !0),
        i.value?.custom_css ? (c(), $(lt("style"), {
          key: 1,
          textContent: A(i.value.custom_css)
        }, null, 8, ["textContent"])) : P("", !0),
        j(p(_a), { class: te(["w-full", oe.value]) }, {
          default: C(() => [j(p(Ca), { class: "text-center" }, {
            default: C(() => [ae.value ? (c(), m("div", Xo, [k("img", {
              src: ae.value,
              alt: i.value?.org_name,
              class: "h-8"
            }, null, 8, Qo)])) : (c(), m("div", es, A(i.value?.org_name || "Zitadel"), 1))]),
            _: 1
          }), j(p(xa), null, {
            default: C(() => [
              u.value ? (c(), $(p(xe), {
                key: 0,
                variant: "destructive",
                class: "mb-4"
              }, {
                default: C(() => [j(p(Ke), { class: "size-4" }), j(p(ke), null, {
                  default: C(() => [M(A(u.value), 1)]),
                  _: 1
                })]),
                _: 1
              })) : P("", !0),
              s.value?.errors?.length ? (c(!0), m(se, { key: 1 }, me(s.value.errors, (n, F) => (c(), $(p(xe), {
                key: "ge-" + F,
                variant: "destructive",
                class: "mb-4"
              }, {
                default: C(() => [j(p(Ke), { class: "size-4" }), j(p(ke), null, {
                  default: C(() => [M(A(n.message), 1)]),
                  _: 2
                }, 1024)]),
                _: 2
              }, 1024))), 128)) : P("", !0),
              s.value?.messages?.length ? (c(!0), m(se, { key: 2 }, me(s.value.messages, (n, F) => (c(), $(p(xe), {
                key: "gm-" + F,
                class: te({
                  "mb-4": !0,
                  "border-green-200 bg-green-50 text-green-800": n.type === "success",
                  "border-yellow-200 bg-yellow-50 text-yellow-800": n.type === "warning"
                })
              }, {
                default: C(() => [j(p(ke), null, {
                  default: C(() => [M(A(n.text), 1)]),
                  _: 2
                }, 1024)]),
                _: 2
              }, 1032, ["class"]))), 128)) : P("", !0),
              s.value ? (c(), m("form", {
                key: 4,
                onSubmit: er($e, ["prevent"]),
                class: "space-y-4"
              }, [(c(!0), m(se, null, me(s.value.nodes, (n, F) => (c(), m(se, { key: F }, [n.type === "heading" ? (c(), m("h1", rs, A(n.text), 1)) : n.type === "description" ? (c(), m("p", as, A(n.text), 1)) : n.type === "avatar" ? (c(), m("div", os, [j(p(Qa), { class: "size-10" }, {
                default: C(() => [j(p(to), null, {
                  default: C(() => [M(A(n.initial), 1)]),
                  _: 2
                }, 1024)]),
                _: 2
              }, 1024), n.text ? (c(), m("span", ss, A(n.text), 1)) : P("", !0)])) : n.type === "icon" ? (c(), m("div", ns, A(n.text), 1)) : n.type === "info" ? (c(), $(p(xe), {
                key: 4,
                class: "text-sm"
              }, {
                default: C(() => [j(p(ke), null, {
                  default: C(() => [M(A(n.text), 1)]),
                  _: 2
                }, 1024)]),
                _: 2
              }, 1024)) : n.type === "error" ? (c(), $(p(xe), {
                key: 5,
                variant: "destructive",
                class: "text-sm"
              }, {
                default: C(() => [j(p(Ke), { class: "size-4" }), j(p(ke), null, {
                  default: C(() => [M(A(n.text), 1)]),
                  _: 2
                }, 1024)]),
                _: 2
              }, 1024)) : n.type === "spinner" ? (c(), m("div", is, [j(p(Re), { class: "size-6" })])) : n.type === "input" ? (c(), m("div", ls, [
                n.input_type === "password" ? (c(), m("div", cs, [j(p(Me), { for: n.name }, {
                  default: C(() => [M(A(n.label), 1)]),
                  _: 2
                }, 1032, ["for"])])) : (c(), $(p(Me), {
                  key: 1,
                  for: n.name
                }, {
                  default: C(() => [M(A(n.label), 1)]),
                  _: 2
                }, 1032, ["for"])),
                j(p(Ye), {
                  id: n.name,
                  modelValue: d[n.name],
                  "onUpdate:modelValue": (w) => d[n.name] = w,
                  type: n.input_type || "text",
                  placeholder: n.placeholder || "",
                  autocomplete: n.autocomplete || "off",
                  required: n.required,
                  disabled: n.disabled,
                  autofocus: F === Q.value,
                  minlength: n.min_length || void 0,
                  maxlength: n.max_length || void 0,
                  pattern: n.pattern || void 0
                }, null, 8, [
                  "id",
                  "modelValue",
                  "onUpdate:modelValue",
                  "type",
                  "placeholder",
                  "autocomplete",
                  "required",
                  "disabled",
                  "autofocus",
                  "minlength",
                  "maxlength",
                  "pattern"
                ]),
                n.input_type === "password" && O.value ? (c(), m(se, { key: 2 }, [
                  j(p(Me), {
                    for: n.name + "_confirm",
                    class: "mt-3"
                  }, {
                    default: C(() => [...y[0] || (y[0] = [M("Confirm Password", -1)])]),
                    _: 1
                  }, 8, ["for"]),
                  j(p(Ye), {
                    id: n.name + "_confirm",
                    modelValue: h[n.name],
                    "onUpdate:modelValue": (w) => h[n.name] = w,
                    type: "password",
                    placeholder: "Confirm your password",
                    autocomplete: "new-password",
                    required: "",
                    class: "mt-1.5"
                  }, null, 8, [
                    "id",
                    "modelValue",
                    "onUpdate:modelValue"
                  ]),
                  h[n.name] && d[n.name] !== h[n.name] ? (c(), m("p", ds, "Passwords do not match")) : P("", !0)
                ], 64)) : P("", !0),
                (c(!0), m(se, null, me(n.errors || [], (w, f) => (c(), m("p", {
                  key: f,
                  class: "text-xs text-destructive"
                }, A(w), 1))), 128))
              ])) : n.type === "field_description" ? (c(), m("p", us, A(n.text), 1)) : n.type === "password_hint" ? (c(), m("div", fs, [k("button", {
                type: "button",
                class: "text-xs text-muted-foreground underline-offset-4 hover:underline cursor-pointer",
                onClick: (w) => K(n.action || "forgot_password")
              }, A(n.label), 9, ps)])) : n.type === "consent_checkbox" ? (c(), m("div", ms, [Ct(k("input", {
                id: n.name,
                type: "checkbox",
                required: n.required,
                "onUpdate:modelValue": (w) => d[n.name] = w,
                class: "mt-0.5 accent-[var(--brand-primary,#6366f1)]"
              }, null, 8, vs), [[lr, d[n.name]]]), k("label", {
                for: n.name,
                class: "text-xs text-muted-foreground leading-relaxed",
                innerHTML: ze(n.label || "")
              }, null, 8, gs)])) : n.type === "hidden" ? (c(), m("input", {
                key: 11,
                type: "hidden",
                name: n.name,
                value: n.value || ""
              }, null, 8, bs)) : n.type === "submit" ? (c(), $(p(ve), {
                key: 12,
                type: "submit",
                class: "w-full",
                disabled: l.value || n.disabled || !Z.value,
                onClick: (w) => S.value = n.action || ""
              }, {
                default: C(() => [l.value ? (c(), $(p(Re), {
                  key: 0,
                  class: "size-4 mr-2"
                })) : P("", !0), M(" " + A(l.value ? "Loading..." : n.label), 1)]),
                _: 2
              }, 1032, ["disabled", "onClick"])) : n.type === "divider" ? (c(), m("div", hs, [j(p(Ka)), y[1] || (y[1] = k("span", { class: "absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 bg-card px-2 text-xs text-muted-foreground" }, "or", -1))])) : n.type === "social_group" ? (c(), m("div", ys, [(c(!0), m(se, null, me(n.children || [], (w, f) => (c(), $(p(ve), {
                key: "sg" + f,
                type: "button",
                variant: "outline",
                class: "w-full gap-2",
                onClick: (D) => K(w.action || "sso", { provider_id: w.provider_id || "" })
              }, {
                default: C(() => [k("span", _s, A(U(w.template || "")), 1), M(" " + A(w.label), 1)]),
                _: 2
              }, 1032, ["onClick"]))), 128))])) : n.type === "button" ? (c(), $(p(ve), {
                key: 15,
                type: "button",
                variant: "outline",
                class: "w-full",
                disabled: l.value || n.disabled,
                onClick: (w) => K(n.action || "")
              }, {
                default: C(() => [M(A(n.label), 1)]),
                _: 2
              }, 1032, ["disabled", "onClick"])) : n.type === "sso_button" ? (c(), $(p(ve), {
                key: 16,
                type: "button",
                variant: "outline",
                class: "w-full gap-2",
                onClick: (w) => K(n.action || "sso", { provider_id: n.provider_id || "" })
              }, {
                default: C(() => [k("span", ws, A(U(n.template || "")), 1), M(" " + A(n.label), 1)]),
                _: 2
              }, 1032, ["onClick"])) : n.type === "link" ? (c(), $(p(ve), {
                key: 17,
                type: "button",
                variant: "link",
                class: "w-full text-muted-foreground",
                onClick: (w) => K(n.action || "back")
              }, {
                default: C(() => [M(A(n.label), 1)]),
                _: 2
              }, 1032, ["onClick"])) : n.type === "registration_link" ? (c(), $(p(ve), {
                key: 18,
                type: "button",
                variant: "link",
                class: "w-full text-muted-foreground font-medium",
                onClick: (w) => K(n.action || "register")
              }, {
                default: C(() => [M(A(n.label), 1)]),
                _: 2
              }, 1032, ["onClick"])) : n.type === "terms_footer" ? (c(), m("p", xs, [
                y[2] || (y[2] = M(" By clicking continue, you agree to our ", -1)),
                n.attributes?.terms_url ? (c(), m("a", {
                  key: 0,
                  href: n.attributes.terms_url,
                  target: "_blank",
                  class: "underline underline-offset-4 hover:text-foreground"
                }, "Terms of Service", 8, ks)) : P("", !0),
                n.attributes?.terms_url && n.attributes?.privacy_url ? (c(), m(se, { key: 1 }, [M(" and ")], 64)) : P("", !0),
                n.attributes?.privacy_url ? (c(), m("a", {
                  key: 2,
                  href: n.attributes.privacy_url,
                  target: "_blank",
                  class: "underline underline-offset-4 hover:text-foreground"
                }, "Privacy Policy", 8, Cs)) : P("", !0),
                y[3] || (y[3] = M(". ", -1))
              ])) : n.type === "captcha_altcha" ? (c(), m("div", Ss, [k("div", {
                ref_for: !0,
                ref_key: "altchaCaptchaEl",
                ref: q,
                class: "flex items-center gap-2 rounded-md border border-input px-3 py-2 text-sm"
              }, [L.value ? (c(), $(p(Re), {
                key: 0,
                class: "size-4"
              })) : N.value ? (c(), m("svg", As, [...y[4] || (y[4] = [k("path", {
                "fill-rule": "evenodd",
                d: "M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z",
                "clip-rule": "evenodd"
              }, null, -1)])])) : (c(), m("div", {
                key: 2,
                class: "size-4 rounded border-2 border-muted-foreground/30 cursor-pointer",
                onClick: Te
              })), k("span", zs, A(L.value ? "Verifying..." : N.value ? "Verified" : "I am human"), 1)], 512)])) : n.type === "captcha_checkbox" ? (c(), m("div", $s, [k("div", Ts, [k("input", {
                type: "checkbox",
                class: "accent-[var(--brand-primary,#6366f1)]",
                checked: !!d[n.name],
                onClick: (w) => d[n.name] = "verified"
              }, null, 8, Os), k("span", Is, "I am human (" + A(n.attributes?.provider || "captcha") + ")", 1)])])) : n.type === "fingerprint_collect" ? (c(), m("div", Es)) : n.type === "group" ? (c(), m("div", Ls, [(c(!0), m(se, null, me(n.children || [], (w, f) => (c(), m(se, { key: "g" + f }, [w.type === "input" ? (c(), m("div", js, [j(p(Me), { for: w.name }, {
                default: C(() => [M(A(w.label), 1)]),
                _: 2
              }, 1032, ["for"]), j(p(Ye), {
                id: w.name,
                modelValue: d[w.name],
                "onUpdate:modelValue": (D) => d[w.name] = D,
                type: w.input_type || "text",
                placeholder: w.placeholder || "",
                required: w.required
              }, null, 8, [
                "id",
                "modelValue",
                "onUpdate:modelValue",
                "type",
                "placeholder",
                "required"
              ])])) : P("", !0)], 64))), 128))])) : P("", !0)], 64))), 128))], 32)) : (c(), m("div", ts, [j(p(Re), { class: "size-6" })]))
            ]),
            _: 1
          })]),
          _: 1
        }, 8, ["class"])
      ]),
      _: 1
    }, 8, ["branding"]));
  }
}), Ms = Vs, Rs = /* @__PURE__ */ T({
  __name: "LoginApp.ce",
  props: {
    apiBaseUrl: {
      default: "",
      type: String
    },
    redirectUri: {
      default: "",
      type: String
    },
    oidcState: {
      default: "",
      type: String
    },
    layout: {
      default: "",
      type: String
    },
    darkMode: {
      default: "",
      type: String
    },
    coverImage: {
      default: "",
      type: String
    },
    primaryColor: {
      default: "",
      type: String
    },
    customCss: {
      default: "",
      type: String
    }
  },
  setup(e) {
    const t = e, r = I(() => t.darkMode === "dark");
    let a = null;
    function o(d) {
      const h = Ue()?.proxy?.$el?.getRootNode();
      !h || !("adoptedStyleSheets" in h) || (a || (a = document.createElement("style"), a.setAttribute("data-custom-css", ""), h.appendChild(a)), a.textContent = d);
    }
    kt(() => {
      t.customCss && o(t.customCss);
    }), Fe(() => t.customCss, (d) => {
      d && o(d);
    });
    function s() {
      return Ue()?.proxy?.$el?.closest("zitadel-login") || null;
    }
    function i(d) {
      s()?.dispatchEvent(new CustomEvent("login-complete", {
        detail: d,
        bubbles: !0,
        composed: !0
      }));
    }
    function u(d) {
      s()?.dispatchEvent(new CustomEvent("login-error", {
        detail: d,
        bubbles: !0,
        composed: !0
      }));
    }
    function l(d) {
      s()?.dispatchEvent(new CustomEvent("login-redirect", {
        detail: d,
        bubbles: !0,
        composed: !0
      }));
    }
    return (d, h) => (c(), m("div", { class: te(["zitadel-login-ce", { dark: r.value }]) }, [j(Ms, {
      "api-base-url": e.apiBaseUrl,
      "redirect-uri": e.redirectUri,
      state: e.oidcState,
      "layout-override": e.layout,
      "dark-mode-override": e.darkMode,
      "cover-image-override": e.coverImage,
      "primary-color-override": e.primaryColor,
      onLoginComplete: i,
      onLoginError: u,
      onLoginRedirect: l
    }, null, 8, [
      "api-base-url",
      "redirect-uri",
      "state",
      "layout-override",
      "dark-mode-override",
      "cover-image-override",
      "primary-color-override"
    ])], 2));
  }
}), Bs = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-card-foreground:#09090b;--color-popover:#fff;--color-popover-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-secondary:#f4f4f5;--color-secondary-foreground:#18181b;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-accent:#f4f4f5;--color-accent-foreground:#18181b;--color-destructive:#ef4444;--color-destructive-foreground:#fafafa;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-card-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-secondary:#27272a;--color-secondary-foreground:#fafafa;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-accent:#27272a;--color-accent-foreground:#fafafa;--color-destructive:#7f1d1d;--color-destructive-foreground:#fafafa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-login-ce{color:var(--color-foreground);background:var(--color-background)}.zitadel-login-ce.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", Ns = /* @__PURE__ */ he(Rs, [["styles", [Bs]]]), Fs = sr(Ns);
customElements.define("zitadel-login", Fs);
export {
  Fs as t
};

//# sourceMappingURL=zitadel-login-wc-Dp8jinn8.js.map