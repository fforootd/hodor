import { A as a, D as E, K as L, M as S, R as k, V as i, Y as s, f as O, g as l, h as d, n as V, o as w, p as e, s as B, t as I, u as U, y as j } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as F, i as K, n as T, r as C } from "./wc-api-client-DbP47Lh1.js";
var W = { class: "space-y-4" }, $ = { class: "flex items-center justify-between" }, q = { class: "text-sm text-[var(--color-muted-foreground)]" }, G = {
  key: 0,
  class: "rounded-lg border border-[var(--color-border)] p-4 space-y-3 bg-[var(--color-card)]"
}, Q = { class: "space-y-1.5" }, R = { class: "space-y-1.5" }, Y = {
  key: 0,
  class: "rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700"
}, Z = ["disabled"], H = {
  key: 1,
  class: "relative"
}, J = {
  key: 2,
  class: "flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, P = {
  key: 3,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, X = {
  key: 4,
  class: "rounded-md border border-[var(--color-border)] overflow-hidden"
}, ee = { class: "w-full text-sm" }, re = ["onClick"], oe = { class: "p-4 font-medium" }, te = { class: "p-4 text-[var(--color-muted-foreground)]" }, ae = { class: "p-4 font-mono text-xs text-[var(--color-muted-foreground)]" }, le = {
  key: 5,
  class: "text-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, z = "zitadel-org-list", se = /* @__PURE__ */ j({
  __name: "zitadel-org-list.ce",
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
    showCreate: {
      type: Boolean,
      default: !0
    }
  },
  setup(b) {
    const D = b, M = O(() => K(D.darkMode));
    let h;
    const u = i([]), c = i(!1), v = i(""), f = i(""), m = i(!1), n = i(""), p = i(""), g = i(!1), y = i(""), x = O(() => {
      if (!f.value.trim()) return u.value;
      const o = f.value.toLowerCase();
      return u.value.filter((r) => (r.identifier || "").toLowerCase().includes(o) || (r.display_name || "").toLowerCase().includes(o));
    });
    function A(o) {
      C(z, "org-selected", {
        id: o.id,
        identifier: o.identifier,
        display_name: o.display_name
      });
    }
    async function _() {
      if (!(!n.value.trim() || g.value)) {
        g.value = !0, y.value = "";
        try {
          const o = { identifier: n.value.trim() };
          p.value.trim() && (o.display_name = p.value.trim());
          const r = await h.post("/v1/orgs", o);
          C(z, "org-created", {
            id: r.id,
            identifier: r.identifier
          }), n.value = "", p.value = "", m.value = !1, await N();
        } catch (o) {
          y.value = o?.message || "Failed to create organization", C(z, "org-error", { error: y.value });
        } finally {
          g.value = !1;
        }
      }
    }
    async function N() {
      c.value = !0, v.value = "";
      try {
        u.value = (await h.get("/v1/orgs")).items || [];
      } catch (o) {
        v.value = o?.message || "Failed to load organizations";
      } finally {
        c.value = !1;
      }
    }
    return E(() => {
      h = T(F(D.apiBaseUrl)), N();
    }), (o, r) => (a(), l("div", { class: L(["zitadel-wc", { dark: M.value }]) }, [e("div", W, [
      e("div", $, [e("div", null, [r[4] || (r[4] = e("h2", { class: "text-lg font-semibold tracking-tight" }, "Organizations", -1)), e("p", q, s(u.value.length) + " organization" + s(u.value.length !== 1 ? "s" : ""), 1)]), b.showCreate ? (a(), l("button", {
        key: 0,
        class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity",
        onClick: r[0] || (r[0] = (t) => m.value = !m.value)
      }, s(m.value ? "Cancel" : "+ New"), 1)) : d("", !0)]),
      m.value ? (a(), l("div", G, [
        e("div", Q, [r[5] || (r[5] = e("label", { class: "text-sm font-medium" }, "Identifier", -1)), k(e("input", {
          "onUpdate:modelValue": r[1] || (r[1] = (t) => n.value = t),
          type: "text",
          placeholder: "e.g. acme-corp",
          class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]",
          onKeyup: B(_, ["enter"])
        }, null, 544), [[w, n.value]])]),
        e("div", R, [r[6] || (r[6] = e("label", { class: "text-sm font-medium" }, "Display Name", -1)), k(e("input", {
          "onUpdate:modelValue": r[2] || (r[2] = (t) => p.value = t),
          type: "text",
          placeholder: "e.g. Acme Corporation",
          class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]",
          onKeyup: B(_, ["enter"])
        }, null, 544), [[w, p.value]])]),
        y.value ? (a(), l("div", Y, s(y.value), 1)) : d("", !0),
        e("button", {
          class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50",
          disabled: !n.value.trim() || g.value,
          onClick: _
        }, s(g.value ? "Creating…" : "Create Organization"), 9, Z)
      ])) : d("", !0),
      b.showSearch ? (a(), l("div", H, [k(e("input", {
        "onUpdate:modelValue": r[3] || (r[3] = (t) => f.value = t),
        type: "text",
        placeholder: "Search organizations…",
        class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
      }, null, 512), [[w, f.value]])])) : d("", !0),
      c.value ? (a(), l("div", J, " Loading organizations… ")) : d("", !0),
      v.value ? (a(), l("div", P, s(v.value), 1)) : d("", !0),
      !c.value && x.value.length ? (a(), l("div", X, [e("table", ee, [r[7] || (r[7] = e("thead", null, [e("tr", { class: "border-b bg-[var(--color-muted)]" }, [
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Identifier"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Display Name"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "ID")
      ])], -1)), e("tbody", null, [(a(!0), l(U, null, S(x.value, (t) => (a(), l("tr", {
        key: t.id,
        class: "border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors",
        onClick: (ue) => A(t)
      }, [
        e("td", oe, s(t.identifier), 1),
        e("td", te, s(t.display_name || "—"), 1),
        e("td", ae, s(t.id), 1)
      ], 8, re))), 128))])])])) : d("", !0),
      !c.value && !v.value && !x.value.length ? (a(), l("div", le, s(f.value ? "No organizations match your search." : "No organizations yet."), 1)) : d("", !0)
    ])], 2));
  }
}), ie = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-card-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-secondary:#f4f4f5;--color-secondary-foreground:#18181b;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-accent:#f4f4f5;--color-accent-foreground:#18181b;--color-destructive:#ef4444;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", de = /* @__PURE__ */ I(se, [["styles", [ie]]]), ne = V(de);
customElements.define("zitadel-org-list", ne);
export {
  ne as t
};

//# sourceMappingURL=zitadel-org-list-wc-Bmw7Tusz.js.map