import { A as t, D as b, K as w, M as C, V as u, Y as f, f as S, g as a, h as l, n as z, p as s, t as A, u as M, y as E } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as B, i as F, n as D, r as g } from "./wc-api-client-DbP47Lh1.js";
var U = { class: "space-y-4" }, L = { class: "flex items-center justify-between" }, N = {
  key: 0,
  class: "flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, W = {
  key: 1,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, $ = {
  key: 2,
  class: "rounded-md border border-[var(--color-border)] overflow-hidden"
}, O = { class: "w-full text-sm" }, V = { class: "p-4" }, j = { class: "text-sm font-medium" }, I = {
  key: 0,
  class: "ml-2 inline-flex items-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] px-2 py-0.5 text-[10px] font-medium"
}, R = { class: "p-4 text-[var(--color-muted-foreground)] font-mono text-xs" }, T = { class: "p-4 text-right" }, G = ["onClick"], K = {
  key: 3,
  class: "text-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, p = "zitadel-session-manager", P = /* @__PURE__ */ E({
  __name: "zitadel-session-manager.ce",
  props: {
    apiBaseUrl: {
      default: "",
      type: String
    },
    darkMode: {
      default: "",
      type: String
    }
  },
  setup(_) {
    const v = _, x = S(() => F(v.darkMode));
    let c;
    const n = u([]), d = u(!1), i = u("");
    function h(e) {
      if (!e) return "Unknown device";
      let r = "Unknown";
      e.includes("Firefox/") ? r = "Firefox" : e.includes("Edg/") ? r = "Edge" : e.includes("Chrome/") ? r = "Chrome" : e.includes("Safari/") && !e.includes("Chrome") && (r = "Safari");
      let o = "";
      return e.includes("Mac OS X") || e.includes("Macintosh") ? o = "macOS" : e.includes("Windows") ? o = "Windows" : e.includes("Linux") && (o = "Linux"), o ? `${r} on ${o}` : r;
    }
    async function m() {
      d.value = !0, i.value = "";
      try {
        n.value = (await c.get("/v1/account/sessions")).sessions || [];
      } catch (e) {
        i.value = e?.message || "Failed to load sessions";
      } finally {
        d.value = !1;
      }
    }
    async function y(e) {
      try {
        await c.post(`/v1/account/sessions/${e}/revoke`, {}), n.value = n.value.filter((r) => r.id !== e), g(p, "session-revoked", { session_id: e });
      } catch (r) {
        i.value = r?.message || "Failed to revoke session";
      }
    }
    async function k() {
      try {
        await c.post("/v1/account/sessions/revoke-others", {}), g(p, "all-sessions-revoked"), await m();
      } catch (e) {
        i.value = e?.message || "Failed to revoke sessions";
      }
    }
    return b(() => {
      c = D(B(v.apiBaseUrl)), m();
    }), (e, r) => (t(), a("div", { class: w(["zitadel-wc", { dark: x.value }]) }, [s("div", U, [
      s("div", L, [r[0] || (r[0] = s("h2", { class: "text-lg font-semibold tracking-tight" }, "Active Sessions", -1)), n.value.length > 1 ? (t(), a("button", {
        key: 0,
        class: "inline-flex items-center rounded-md border border-red-200 px-3 py-1.5 text-xs font-medium text-red-600 hover:bg-red-50 transition-colors",
        onClick: k
      }, "Revoke all others")) : l("", !0)]),
      d.value ? (t(), a("div", N, " Loading sessions… ")) : l("", !0),
      i.value ? (t(), a("div", W, f(i.value), 1)) : l("", !0),
      !d.value && n.value.length ? (t(), a("div", $, [s("table", O, [r[1] || (r[1] = s("thead", null, [s("tr", { class: "border-b bg-[var(--color-muted)]" }, [
        s("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Device"),
        s("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "IP Address"),
        s("th", { class: "h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]" }, "Action")
      ])], -1)), s("tbody", null, [(t(!0), a(M, null, C(n.value, (o) => (t(), a("tr", {
        key: o.id,
        class: "border-b last:border-0"
      }, [
        s("td", V, [s("span", j, f(h(o.user_agent)), 1), o.current ? (t(), a("span", I, "This device")) : l("", !0)]),
        s("td", R, f(o.ip_address || "—"), 1),
        s("td", T, [o.current ? l("", !0) : (t(), a("button", {
          key: 0,
          class: "inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors",
          onClick: (q) => y(o.id)
        }, "Revoke", 8, G))])
      ]))), 128))])])])) : l("", !0),
      !d.value && !n.value.length && !i.value ? (t(), a("div", K, " No active sessions. ")) : l("", !0)
    ])], 2));
  }
}), X = ":host{--color-background:#fff;--color-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", Y = /* @__PURE__ */ A(P, [["styles", [X]]]), Z = z(Y);
customElements.define("zitadel-session-manager", Z);
export {
  Z as t
};

//# sourceMappingURL=zitadel-session-manager-wc-CPeogCGn.js.map