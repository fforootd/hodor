import { A as a, D as U, F as j, K as x, M as C, R as A, V as p, Y as o, f as n, g as l, h as i, n as E, o as P, p as e, t as R, u as I, y as V } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as F, i as W, n as q, r as S } from "./wc-api-client-DbP47Lh1.js";
var G = { class: "space-y-4" }, K = { class: "flex items-center justify-between" }, Q = { class: "text-lg font-semibold tracking-tight" }, Y = { class: "text-sm text-muted-foreground" }, Z = {
  key: 0,
  class: "relative"
}, H = ["placeholder"], J = {
  key: 1,
  class: "flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, O = {
  key: 2,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, X = {
  key: 3,
  class: "rounded-md border border-[var(--color-border)] overflow-hidden"
}, ee = { class: "w-full text-sm" }, re = ["onClick"], te = { class: "p-4 font-medium" }, oe = { class: "p-4 text-[var(--color-muted-foreground)]" }, ae = { class: "p-4" }, le = { class: "p-4 text-[var(--color-muted-foreground)]" }, se = {
  key: 4,
  class: "text-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, de = {
  key: 5,
  class: "flex items-center justify-between pt-2"
}, ne = { class: "text-xs text-[var(--color-muted-foreground)]" }, ie = { class: "flex gap-1" }, ce = ["onClick"], z = "zitadel-identity-list", ue = /* @__PURE__ */ V({
  __name: "zitadel-identity-list.ce",
  props: {
    apiBaseUrl: {
      default: "",
      type: String
    },
    schemaType: {
      default: "",
      type: String
    },
    orgId: {
      default: "",
      type: String
    },
    pageSize: {
      default: 20,
      type: Number
    },
    darkMode: {
      default: "",
      type: String
    },
    showSearch: {
      type: Boolean,
      default: !0
    },
    showCreate: {
      type: Boolean,
      default: !0
    }
  },
  setup(y) {
    const s = y, L = n(() => W(s.darkMode)), m = p([]), f = p(!1), v = p(""), c = p(""), g = p(1), h = n(() => s.schemaType ? s.schemaType.replace(/_/g, " ").replace(/\b\w/g, (t) => t.toUpperCase()) + "s" : "Identities"), D = n(() => h.value.replace(/s$/, "")), u = n(() => {
      if (!c.value.trim()) return m.value;
      const t = c.value.toLowerCase();
      return m.value.filter((d) => (d.identifier || "").toLowerCase().includes(t) || (d.display_name || "").toLowerCase().includes(t));
    }), _ = n(() => Number(s.pageSize) || 20), k = n(() => Math.ceil(u.value.length / _.value)), b = n(() => (g.value - 1) * _.value), w = n(() => b.value + _.value), M = n(() => u.value.slice(b.value, w.value));
    j(c, () => {
      g.value = 1;
    });
    function N(t) {
      return t ? new Date(t).toLocaleDateString() : "—";
    }
    function T(t) {
      S(z, "identity-selected", {
        id: t.id,
        identifier: t.identifier,
        schema_type: t.schema_name || s.schemaType
      });
    }
    function B() {
      S(z, "identity-create", { schema_type: s.schemaType });
    }
    return U(async () => {
      f.value = !0, v.value = "";
      try {
        const t = q(F(s.apiBaseUrl));
        let d = "/v1/users";
        const r = [];
        s.schemaType && r.push(`schema_type=${encodeURIComponent(s.schemaType)}`), s.orgId && r.push(`org_id=${encodeURIComponent(s.orgId)}`), r.length && (d += "?" + r.join("&")), m.value = (await t.get(d)).items || [];
      } catch (t) {
        v.value = t?.message || "Failed to load identities";
      } finally {
        f.value = !1;
      }
    }), (t, d) => (a(), l("div", { class: x(["zitadel-wc", { dark: L.value }]) }, [e("div", G, [
      e("div", K, [e("div", null, [e("h2", Q, o(h.value), 1), e("p", Y, o(m.value.length) + " total", 1)]), y.showCreate ? (a(), l("button", {
        key: 0,
        class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity",
        onClick: B
      }, "+ New " + o(D.value), 1)) : i("", !0)]),
      y.showSearch ? (a(), l("div", Z, [A(e("input", {
        "onUpdate:modelValue": d[0] || (d[0] = (r) => c.value = r),
        type: "text",
        placeholder: `Search ${h.value.toLowerCase()}…`,
        class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
      }, null, 8, H), [[P, c.value]])])) : i("", !0),
      f.value ? (a(), l("div", J, " Loading… ")) : i("", !0),
      v.value ? (a(), l("div", O, o(v.value), 1)) : i("", !0),
      !f.value && u.value.length ? (a(), l("div", X, [e("table", ee, [d[1] || (d[1] = e("thead", null, [e("tr", { class: "border-b bg-[var(--color-muted)]" }, [
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Identifier"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Display Name"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "State"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Created")
      ])], -1)), e("tbody", null, [(a(!0), l(I, null, C(M.value, (r) => (a(), l("tr", {
        key: r.id,
        class: "border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors",
        onClick: ($) => T(r)
      }, [
        e("td", te, o(r.identifier), 1),
        e("td", oe, o(r.display_name || "—"), 1),
        e("td", ae, [e("span", { class: x(["inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border", r.state === "active" ? "bg-green-50 text-green-700 border-green-200" : "bg-red-50 text-red-700 border-red-200"]) }, o(r.state), 3)]),
        e("td", le, o(N(r.created_at)), 1)
      ], 8, re))), 128))])])])) : i("", !0),
      !f.value && !v.value && !u.value.length ? (a(), l("div", se, o(c.value ? "No results found." : `No ${h.value.toLowerCase()} yet.`), 1)) : i("", !0),
      k.value > 1 ? (a(), l("div", de, [e("p", ne, " Showing " + o(b.value + 1) + "–" + o(Math.min(w.value, u.value.length)) + " of " + o(u.value.length), 1), e("div", ie, [(a(!0), l(I, null, C(k.value, (r) => (a(), l("button", {
        key: r,
        class: x(["h-8 w-8 rounded-md text-xs font-medium transition-colors", r === g.value ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]" : "border border-[var(--color-border)] hover:bg-[var(--color-muted)]"]),
        onClick: ($) => g.value = r
      }, o(r), 11, ce))), 128))])])) : i("", !0)
    ])], 2));
  }
}), fe = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-card-foreground:#09090b;--color-popover:#fff;--color-popover-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-secondary:#f4f4f5;--color-secondary-foreground:#18181b;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-accent:#f4f4f5;--color-accent-foreground:#18181b;--color-destructive:#ef4444;--color-destructive-foreground:#fafafa;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-card-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-secondary:#27272a;--color-secondary-foreground:#fafafa;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-accent:#27272a;--color-accent-foreground:#fafafa;--color-destructive:#7f1d1d;--color-destructive-foreground:#fafafa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", ve = /* @__PURE__ */ R(ue, [["styles", [fe]]]), pe = E(ve);
customElements.define("zitadel-identity-list", pe);
export {
  pe as t
};

//# sourceMappingURL=zitadel-identity-list-wc-D6AhzNd1.js.map