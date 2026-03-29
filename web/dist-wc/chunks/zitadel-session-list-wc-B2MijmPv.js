import { A as l, D as A, K as h, M as B, R as E, V as g, Y as a, c as U, f as v, g as d, h as n, n as F, o as N, p as t, t as $, u as V, y as W } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as O, i as j, n as P, r as b } from "./wc-api-client-DbP47Lh1.js";
var R = { class: "space-y-4" }, T = { class: "flex items-center justify-between" }, q = { class: "text-sm text-[var(--color-muted-foreground)]" }, G = { class: "flex items-center gap-4 p-3 rounded-lg border border-[var(--color-border)] text-sm text-[var(--color-muted-foreground)] bg-[var(--color-card)]" }, K = { class: "flex items-center gap-1.5 text-green-700" }, Q = { class: "font-medium" }, X = { class: "flex items-center gap-1.5" }, Y = { class: "flex items-center gap-1.5 text-red-600" }, Z = {
  key: 0,
  class: "relative"
}, H = {
  key: 1,
  class: "flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, J = {
  key: 2,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, ee = {
  key: 3,
  class: "rounded-md border border-[var(--color-border)] overflow-hidden"
}, te = { class: "w-full text-sm" }, re = ["onClick"], oe = { class: "p-4 font-medium" }, se = { class: "p-4 font-mono text-xs text-[var(--color-muted-foreground)]" }, ae = { class: "p-4 text-[var(--color-muted-foreground)]" }, le = { class: "p-4" }, de = { class: "p-4 text-[var(--color-muted-foreground)]" }, ie = { class: "p-4 text-right" }, ne = ["onClick"], ue = {
  key: 4,
  class: "text-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, y = "zitadel-session-list", ce = /* @__PURE__ */ W({
  __name: "zitadel-session-list.ce",
  props: {
    apiBaseUrl: {
      default: "",
      type: String
    },
    darkMode: {
      default: "",
      type: String
    },
    showSearch: {
      type: Boolean,
      default: !0
    },
    userId: {
      default: "",
      type: String
    }
  },
  setup(_) {
    const f = _, k = v(() => j(f.darkMode));
    let m;
    const s = g([]), u = g(!1), i = g(""), c = g(""), x = v(() => s.value.filter((e) => e._state === "active").length), w = v(() => s.value.filter((e) => e._state === "expired").length), C = v(() => s.value.filter((e) => e._state === "revoked").length), p = v(() => {
      if (!c.value.trim()) return s.value;
      const e = c.value.toLowerCase();
      return s.value.filter((r) => (r.entity_id || "").toLowerCase().includes(e) || (r.ip_address || "").toLowerCase().includes(e) || (r.user_agent || "").toLowerCase().includes(e));
    });
    function S(e) {
      if (!e) return "Unknown";
      let r = "Unknown";
      e.includes("Firefox/") ? r = "Firefox" : e.includes("Edg/") ? r = "Edge" : e.includes("Chrome/") ? r = "Chrome" : e.includes("Safari/") && !e.includes("Chrome") ? r = "Safari" : e.includes("curl") && (r = "curl");
      let o = "";
      return e.includes("Mac OS X") || e.includes("Macintosh") ? o = "macOS" : e.includes("Windows") ? o = "Windows" : e.includes("Linux") && (o = "Linux"), o ? `${r} · ${o}` : r;
    }
    function D(e) {
      return e ? new Date(e).toLocaleDateString() : "—";
    }
    function z(e) {
      return e.revoked_at ? "revoked" : e.expires_at && new Date(e.expires_at) < /* @__PURE__ */ new Date() ? "expired" : "active";
    }
    function L(e) {
      b(y, "session-selected", {
        id: e.id,
        entity_id: e.entity_id,
        state: e._state
      });
    }
    async function M(e) {
      try {
        await m.delete(`/v1/sessions/${e}`);
        const r = s.value.findIndex((o) => o.id === e);
        r !== -1 && (s.value[r] = {
          ...s.value[r],
          _state: "revoked",
          revoked_at: (/* @__PURE__ */ new Date()).toISOString()
        }, s.value = [...s.value]), b(y, "session-revoked", { session_id: e });
      } catch (r) {
        i.value = r?.message || "Failed to revoke";
      }
    }
    return A(async () => {
      m = P(O(f.apiBaseUrl)), u.value = !0, i.value = "";
      try {
        s.value = ((await m.get("/v1/sessions")).items || []).map((e) => ({
          ...e,
          _state: z(e)
        })), f.userId && (s.value = s.value.filter((e) => e.entity_id === f.userId));
      } catch (e) {
        i.value = e?.message || "Failed to load sessions";
      } finally {
        u.value = !1;
      }
    }), (e, r) => (l(), d("div", { class: h(["zitadel-wc", { dark: k.value }]) }, [t("div", R, [
      t("div", T, [t("div", null, [r[1] || (r[1] = t("h2", { class: "text-lg font-semibold tracking-tight" }, "Sessions", -1)), t("p", q, a(x.value) + " active of " + a(s.value.length) + " total ", 1)])]),
      t("div", G, [
        t("div", K, [r[2] || (r[2] = t("span", { class: "w-2 h-2 rounded-full bg-green-500" }, null, -1)), t("span", Q, a(x.value) + " active", 1)]),
        r[5] || (r[5] = t("div", { class: "w-px h-4 bg-[var(--color-border)]" }, null, -1)),
        t("div", X, [r[3] || (r[3] = t("span", { class: "w-2 h-2 rounded-full bg-gray-400" }, null, -1)), t("span", null, a(w.value) + " expired", 1)]),
        r[6] || (r[6] = t("div", { class: "w-px h-4 bg-[var(--color-border)]" }, null, -1)),
        t("div", Y, [r[4] || (r[4] = t("span", { class: "w-2 h-2 rounded-full bg-red-500" }, null, -1)), t("span", null, a(C.value) + " revoked", 1)])
      ]),
      _.showSearch ? (l(), d("div", Z, [E(t("input", {
        "onUpdate:modelValue": r[0] || (r[0] = (o) => c.value = o),
        type: "text",
        placeholder: "Search by user, IP, or device…",
        class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
      }, null, 512), [[N, c.value]])])) : n("", !0),
      u.value ? (l(), d("div", H, "Loading sessions…")) : n("", !0),
      i.value ? (l(), d("div", J, a(i.value), 1)) : n("", !0),
      !u.value && p.value.length ? (l(), d("div", ee, [t("table", te, [r[7] || (r[7] = t("thead", null, [t("tr", { class: "border-b bg-[var(--color-muted)]" }, [
        t("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "User"),
        t("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "IP"),
        t("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Device"),
        t("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Status"),
        t("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Created"),
        t("th", { class: "h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]" }, "Action")
      ])], -1)), t("tbody", null, [(l(!0), d(V, null, B(p.value, (o) => (l(), d("tr", {
        key: o.id,
        class: "border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors",
        onClick: (I) => L(o)
      }, [
        t("td", oe, a(o.entity_id || "—"), 1),
        t("td", se, a(o.ip_address || "—"), 1),
        t("td", ae, a(S(o.user_agent)), 1),
        t("td", le, [t("span", { class: h(["inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border", o._state === "active" ? "bg-green-50 text-green-700 border-green-200" : o._state === "revoked" ? "bg-red-50 text-red-700 border-red-200" : "bg-gray-50 text-gray-600 border-gray-200"]) }, a(o._state), 3)]),
        t("td", de, a(D(o.created_at)), 1),
        t("td", ie, [o._state === "active" ? (l(), d("button", {
          key: 0,
          class: "inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors",
          onClick: U((I) => M(o.id), ["stop"])
        }, "Revoke", 8, ne)) : n("", !0)])
      ], 8, re))), 128))])])])) : n("", !0),
      !u.value && !i.value && !p.value.length ? (l(), d("div", ue, a(c.value ? "No sessions match your search." : "No sessions found."), 1)) : n("", !0)
    ])], 2));
  }
}), ve = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", fe = /* @__PURE__ */ $(ce, [["styles", [ve]]]), ge = F(fe);
customElements.define("zitadel-session-list", ge);
export {
  ge as t
};

//# sourceMappingURL=zitadel-session-list-wc-B2MijmPv.js.map