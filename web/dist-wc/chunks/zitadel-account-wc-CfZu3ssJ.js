import { A as a, B as P, D as R, K as U, M as h, R as G, V as d, Y as l, _ as C, f as k, g as s, h as n, i as H, n as K, p as o, t as X, u as f, y as Y } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as q, i as J, n as Q, r as A } from "./wc-api-client-DbP47Lh1.js";
var ee = {
  key: 0,
  class: "flex justify-center py-16 text-sm text-[var(--color-muted-foreground)]"
}, oe = {
  key: 1,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 text-center"
}, re = {
  key: 2,
  class: "space-y-6"
}, te = { class: "flex items-center justify-between border-b border-[var(--color-border)] pb-4" }, ae = { class: "flex items-center gap-3" }, se = { class: "flex h-10 w-10 items-center justify-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] font-semibold text-sm" }, ie = { class: "text-sm font-semibold" }, le = { class: "text-xs text-[var(--color-muted-foreground)]" }, ne = { class: "flex border-b border-[var(--color-border)]" }, de = ["onClick"], ce = {
  key: 0,
  class: "space-y-4"
}, ue = { class: "text-sm font-medium flex items-center gap-2" }, ve = ["title"], fe = { class: "flex items-center gap-2" }, pe = [
  "onUpdate:modelValue",
  "type",
  "placeholder"
], me = {
  key: 1,
  class: "flex-1 rounded-md border bg-[var(--color-muted)] px-3 py-2 text-sm text-[var(--color-muted-foreground)]"
}, be = ["onClick"], ye = {
  key: 0,
  class: "text-xs text-[var(--color-muted-foreground)] italic"
}, _e = ["disabled"], xe = {
  key: 1,
  class: "space-y-4"
}, ge = { class: "rounded-md border border-[var(--color-border)] overflow-hidden" }, he = { class: "w-full text-sm" }, ke = { class: "p-4" }, we = { class: "text-sm font-medium" }, Se = {
  key: 0,
  class: "ml-2 inline-flex items-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] px-2 py-0.5 text-[10px] font-medium"
}, Ce = { class: "p-4 text-[var(--color-muted-foreground)] font-mono text-xs" }, Ae = { class: "p-4 text-right" }, ze = ["onClick"], Me = { key: 0 }, Be = {
  key: 2,
  class: "space-y-4"
}, Ee = { class: "rounded-md border border-[var(--color-border)] overflow-hidden" }, Pe = { class: "w-full text-sm" }, Ue = { class: "p-4" }, je = { class: "inline-flex items-center rounded-md border border-[var(--color-border)] px-2 py-0.5 text-xs font-mono" }, De = { class: "p-4 text-right text-sm text-[var(--color-muted-foreground)]" }, Oe = { key: 0 }, z = "zitadel-account", Ne = /* @__PURE__ */ Y({
  __name: "zitadel-account.ce",
  props: {
    apiBaseUrl: {
      default: "",
      type: String
    },
    darkMode: {
      default: "",
      type: String
    },
    showSessions: {
      type: Boolean,
      default: !0
    },
    showActivity: {
      type: Boolean,
      default: !0
    }
  },
  setup(j) {
    const v = j, D = k(() => J(v.darkMode));
    let c;
    const p = d("profile"), M = d({ org_name: "Zitadel" }), u = d(null), _ = d({}), x = d({}), m = P({}), b = P({}), y = d([]), w = d([]), g = d(!1), S = d(!1), O = k(() => {
      const r = [{
        id: "profile",
        label: "Profile"
      }];
      return v.showSessions && r.push({
        id: "sessions",
        label: "Sessions"
      }), v.showActivity && r.push({
        id: "activity",
        label: "Activity"
      }), r;
    }), N = k(() => ((u.value?.display_name || u.value?.identifier || "?")[0] || "?").toUpperCase()), T = k(() => {
      const r = {};
      for (const [t, e] of Object.entries(x.value)) e.hidden || (r[t] = e);
      return r;
    });
    function V(r) {
      return r.replace(/_/g, " ").replace(/\b\w/g, (t) => t.toUpperCase());
    }
    function $(r) {
      if (!r) return "Unknown device";
      let t = "Unknown";
      r.includes("Firefox/") ? t = "Firefox" : r.includes("Edg/") ? t = "Edge" : r.includes("Chrome/") ? t = "Chrome" : r.includes("Safari/") && !r.includes("Chrome") && (t = "Safari");
      let e = "";
      return r.includes("Mac OS X") || r.includes("Macintosh") ? e = "macOS" : r.includes("Windows") ? e = "Windows" : r.includes("Linux") ? e = "Linux" : r.includes("Android") ? e = "Android" : (r.includes("iPhone") || r.includes("iPad")) && (e = "iOS"), e ? `${t} on ${e}` : t;
    }
    async function B() {
      try {
        const r = await c.get("/v1/account/profile");
        u.value = r.identity, _.value = r.identity.profile || {}, x.value = r.field_permissions || {};
        for (const [t, e] of Object.entries(x.value)) e.editable && (m[t] = _.value[t] || "");
      } catch {
        S.value = !0;
      }
    }
    async function F() {
      g.value = !0;
      try {
        const r = {};
        for (const [e, i] of Object.entries(x.value)) i.editable && m[e] !== (_.value[e] || "") && (r[e] = m[e]);
        const t = {};
        Object.keys(r).length && (t.profile = r), await c.patch("/v1/account/profile", t), A(z, "profile-updated", { changes: r }), await B();
      } catch {
      }
      g.value = !1;
    }
    async function L() {
      try {
        y.value = (await c.get("/v1/account/sessions")).sessions || [];
      } catch {
      }
    }
    async function W() {
      try {
        w.value = (await c.get("/v1/account/activity?limit=10")).events || [];
      } catch {
      }
    }
    async function Z(r) {
      try {
        await c.post(`/v1/account/sessions/${r}/revoke`, {}), y.value = y.value.filter((t) => t.id !== r), A(z, "session-revoked", { session_id: r });
      } catch {
      }
    }
    function I() {
      A(z, "sign-out");
    }
    return R(async () => {
      c = Q(q(v.apiBaseUrl));
      try {
        M.value = await c.get("/v1/branding");
      } catch {
      }
      await Promise.all([
        B(),
        ...v.showSessions ? [L()] : [],
        ...v.showActivity ? [W()] : []
      ]);
    }), (r, t) => (a(), s("div", { class: U(["zitadel-wc", { dark: D.value }]) }, [
      !u.value && !S.value ? (a(), s("div", ee, " Loading… ")) : n("", !0),
      S.value ? (a(), s("div", oe, [...t[0] || (t[0] = [o("p", { class: "font-semibold mb-1" }, "Session expired", -1), o("p", null, "Please sign in to access your account.", -1)])])) : n("", !0),
      u.value ? (a(), s("div", re, [
        o("div", te, [o("div", ae, [o("div", se, l(N.value), 1), o("div", null, [o("p", ie, l(u.value.display_name || u.value.identifier || "My Account"), 1), o("p", le, l(M.value.org_name || "Zitadel"), 1)])]), o("button", {
          class: "inline-flex items-center rounded-md border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--color-muted)] transition-colors",
          onClick: I
        }, "Sign out")]),
        o("div", ne, [(a(!0), s(f, null, h(O.value, (e) => (a(), s("button", {
          key: e.id,
          class: U(["px-4 py-2 text-sm font-medium border-b-2 -mb-[1px] transition-colors", p.value === e.id ? "border-[var(--color-primary)] text-[var(--color-foreground)]" : "border-transparent text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]"]),
          onClick: (i) => p.value = e.id
        }, l(e.label), 11, de))), 128))]),
        p.value === "profile" ? (a(), s("div", ce, [(a(!0), s(f, null, h(T.value, (e, i) => (a(), s("div", {
          key: i,
          class: "space-y-1.5"
        }, [
          o("label", ue, [C(l(V(i)) + " ", 1), e.editable ? n("", !0) : (a(), s("span", {
            key: 0,
            class: "text-xs",
            title: "Set by " + e.source
          }, "🔒", 8, ve))]),
          o("div", fe, [e.editable ? G((a(), s("input", {
            key: 0,
            "onUpdate:modelValue": (E) => m[i] = E,
            type: e.sensitive && !b[i] ? "password" : "text",
            placeholder: i,
            class: "flex-1 h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 8, pe)), [[H, m[i]]]) : (a(), s("div", me, [e.sensitive && !b[i] ? (a(), s(f, { key: 0 }, [C("•••••")], 64)) : (a(), s(f, { key: 1 }, [C(l(_.value[i] || "—"), 1)], 64))])), e.sensitive ? (a(), s("button", {
            key: 2,
            class: "inline-flex items-center rounded-md border border-[var(--color-border)] px-2 py-1.5 text-xs hover:bg-[var(--color-muted)] transition-colors",
            onClick: (E) => b[i] = !b[i]
          }, l(b[i] ? "Hide" : "Show"), 9, be)) : n("", !0)]),
          !e.editable && e.source !== "user" ? (a(), s("p", ye, " Set by " + l(e.source), 1)) : n("", !0)
        ]))), 128)), o("button", {
          class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50 mt-2",
          disabled: g.value,
          onClick: F
        }, l(g.value ? "Saving…" : "Save changes"), 9, _e)])) : n("", !0),
        p.value === "sessions" ? (a(), s("div", xe, [o("div", ge, [o("table", he, [t[2] || (t[2] = o("thead", null, [o("tr", { class: "border-b bg-[var(--color-muted)]" }, [
          o("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Device"),
          o("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "IP"),
          o("th", { class: "h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]" }, "Action")
        ])], -1)), o("tbody", null, [(a(!0), s(f, null, h(y.value, (e) => (a(), s("tr", {
          key: e.id,
          class: "border-b last:border-0"
        }, [
          o("td", ke, [o("span", we, l($(e.user_agent)), 1), e.current ? (a(), s("span", Se, "This device")) : n("", !0)]),
          o("td", Ce, l(e.ip_address || "—"), 1),
          o("td", Ae, [e.current ? n("", !0) : (a(), s("button", {
            key: 0,
            class: "inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors",
            onClick: (i) => Z(e.id)
          }, "Revoke", 8, ze))])
        ]))), 128)), y.value.length ? n("", !0) : (a(), s("tr", Me, [...t[1] || (t[1] = [o("td", {
          colspan: "3",
          class: "text-center text-[var(--color-muted-foreground)] py-8"
        }, "No active sessions", -1)])]))])])])])) : n("", !0),
        p.value === "activity" ? (a(), s("div", Be, [o("div", Ee, [o("table", Pe, [t[4] || (t[4] = o("thead", null, [o("tr", { class: "border-b bg-[var(--color-muted)]" }, [o("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Event"), o("th", { class: "h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]" }, "Time")])], -1)), o("tbody", null, [(a(!0), s(f, null, h(w.value, (e) => (a(), s("tr", {
          key: e.id,
          class: "border-b last:border-0"
        }, [o("td", Ue, [o("span", je, l(e.event_type), 1)]), o("td", De, l(e.time_ago), 1)]))), 128)), w.value.length ? n("", !0) : (a(), s("tr", Oe, [...t[3] || (t[3] = [o("td", {
          colspan: "2",
          class: "text-center text-[var(--color-muted-foreground)] py-8"
        }, "No activity yet", -1)])]))])])])])) : n("", !0)
      ])) : n("", !0)
    ], 2));
  }
}), Te = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-card-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-secondary:#f4f4f5;--color-secondary-foreground:#18181b;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-accent:#f4f4f5;--color-accent-foreground:#18181b;--color-destructive:#ef4444;--color-destructive-foreground:#fafafa;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-card-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-secondary:#27272a;--color-secondary-foreground:#fafafa;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-accent:#27272a;--color-accent-foreground:#fafafa;--color-destructive:#7f1d1d;--color-destructive-foreground:#fafafa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", Ve = /* @__PURE__ */ X(Ne, [["styles", [Te]]]), $e = K(Ve);
customElements.define("zitadel-account", $e);
export {
  $e as t
};

//# sourceMappingURL=zitadel-account-wc-CfZu3ssJ.js.map