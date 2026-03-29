import { A as s, B as W, D as q, K as V, M, R as v, V as n, Y as l, c as U, f as $, g as d, h as u, n as K, o as f, p as e, r as Q, t as R, u as A, y as Y } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as Z, i as H, n as J, r as y } from "./wc-api-client-DbP47Lh1.js";
var O = { class: "space-y-4" }, X = { class: "flex items-center justify-between" }, ee = { class: "text-sm text-[var(--color-muted-foreground)]" }, re = {
  key: 0,
  class: "space-y-3"
}, oe = { class: "grid grid-cols-2 gap-3" }, te = ["onClick"], ae = { class: "text-2xl mb-2" }, le = { class: "text-sm font-semibold" }, se = { class: "text-xs text-[var(--color-muted-foreground)] mt-1" }, de = { class: "inline-flex items-center rounded-full border border-[var(--color-border)] px-2 py-0.5 text-[10px] font-medium uppercase mt-2" }, ie = {
  key: 1,
  class: "rounded-lg border border-[var(--color-border)] p-4 space-y-4 bg-[var(--color-card)]"
}, ne = { class: "text-sm font-semibold" }, ue = { class: "grid grid-cols-2 gap-4" }, ce = { class: "space-y-1.5" }, ve = { class: "space-y-1.5" }, pe = { class: "space-y-1.5" }, me = { class: "space-y-1.5" }, fe = { class: "space-y-1.5" }, ge = { class: "flex items-center gap-2 self-end pb-1" }, be = {
  key: 0,
  class: "rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700"
}, xe = { class: "flex justify-end gap-2" }, _e = ["disabled"], ye = {
  key: 2,
  class: "relative"
}, he = {
  key: 3,
  class: "flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, ke = {
  key: 4,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, we = {
  key: 5,
  class: "rounded-md border border-[var(--color-border)] overflow-hidden"
}, Ce = { class: "w-full text-sm" }, De = ["onClick"], Pe = { class: "p-4" }, ze = { class: "flex items-center gap-2" }, Be = { class: "font-medium" }, Se = { class: "p-4" }, Ve = { class: "inline-flex items-center rounded-full border border-[var(--color-border)] px-2 py-0.5 text-xs font-medium uppercase" }, Me = { class: "p-4 text-[var(--color-muted-foreground)]" }, Ue = { class: "p-4" }, $e = { class: "p-4 text-[var(--color-muted-foreground)]" }, Ae = { class: "p-4 text-right" }, Ee = { class: "flex items-center justify-end gap-1" }, Le = ["onClick"], Ne = ["onClick"], Te = {
  key: 6,
  class: "text-center py-12 text-sm text-[var(--color-muted-foreground)]"
}, h = "zitadel-provider-list", je = /* @__PURE__ */ Y({
  __name: "zitadel-provider-list.ce",
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
  setup(C) {
    const z = C, E = $(() => H(z.darkMode));
    let p;
    const g = n([]), B = n([]), b = n(!1), x = n(""), _ = n(""), i = n(!1), c = n(null), k = n(!1), m = n(""), a = W({
      name: "",
      issuer: "",
      client_id: "",
      client_secret: "",
      scopes: "openid email profile",
      auto_register: !0
    }), D = $(() => {
      if (!_.value.trim()) return g.value;
      const t = _.value.toLowerCase();
      return g.value.filter((r) => (r.name || "").toLowerCase().includes(t) || (r.protocol || "").toLowerCase().includes(t) || (r.template || "").toLowerCase().includes(t));
    });
    function S(t) {
      return {
        google: "🔵",
        entraid: "🟦",
        gitlab: "🦊",
        apple: "🍎",
        custom: "⚙"
      }[t] || "🔗";
    }
    function L(t) {
      return t ? new Date(t).toLocaleDateString() : "—";
    }
    function N(t) {
      y(h, "provider-selected", {
        id: t.id,
        name: t.name,
        protocol: t.protocol,
        enabled: t.enabled
      });
    }
    function T(t) {
      c.value = t, a.name = "", a.issuer = t.default_config?.issuer || "", a.scopes = t.default_config?.scopes || "openid email profile", a.client_id = "", a.client_secret = "", m.value = "";
    }
    async function j() {
      k.value = !0, m.value = "";
      try {
        y(h, "provider-created", {
          id: (await p.post("/v1/providers", {
            name: a.name,
            protocol: c.value?.protocol || "oidc",
            template: c.value?.id || "custom",
            config: {
              issuer: a.issuer,
              client_id: a.client_id,
              client_secret: a.client_secret,
              scopes: a.scopes
            },
            auto_register: a.auto_register
          })).id,
          name: a.name
        }), i.value = !1, c.value = null, await w();
      } catch (t) {
        m.value = t?.message || "Create failed", y(h, "provider-error", { error: m.value });
      } finally {
        k.value = !1;
      }
    }
    async function I(t) {
      try {
        await p.patch(`/v1/providers/${t.id}`, { enabled: !t.enabled }), y(h, "provider-toggled", {
          id: t.id,
          enabled: !t.enabled
        }), await w();
      } catch (r) {
        x.value = r?.message || "Toggle failed";
      }
    }
    async function F(t) {
      if (confirm(`Delete provider "${t.name}"?`))
        try {
          await p.delete(`/v1/providers/${t.id}`), y(h, "provider-deleted", { id: t.id }), await w();
        } catch (r) {
          x.value = r?.message || "Delete failed";
        }
    }
    async function w() {
      try {
        g.value = (await p.get("/v1/providers")).providers || [];
      } catch {
      }
    }
    async function G() {
      try {
        B.value = (await p.get("/v1/providers/templates")).templates || [];
      } catch {
      }
    }
    return q(async () => {
      p = J(Z(z.apiBaseUrl)), b.value = !0, await Promise.all([w(), G()]), b.value = !1;
    }), (t, r) => (s(), d("div", { class: V(["zitadel-wc", { dark: E.value }]) }, [e("div", O, [
      e("div", X, [e("div", null, [r[9] || (r[9] = e("h2", { class: "text-lg font-semibold tracking-tight" }, "Providers", -1)), e("p", ee, l(g.value.length) + " provider" + l(g.value.length !== 1 ? "s" : "") + " configured ", 1)]), C.showCreate ? (s(), d("button", {
        key: 0,
        class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity",
        onClick: r[0] || (r[0] = (o) => i.value = !i.value)
      }, l(i.value ? "Cancel" : "+ Add Provider"), 1)) : u("", !0)]),
      i.value && !c.value ? (s(), d("div", re, [r[10] || (r[10] = e("h3", { class: "text-sm font-semibold" }, "Choose a provider template", -1)), e("div", oe, [(s(!0), d(A, null, M(B.value, (o) => (s(), d("div", {
        key: o.id,
        class: "rounded-lg border border-[var(--color-border)] p-4 cursor-pointer hover:border-[var(--color-primary)] transition-colors bg-[var(--color-card)]",
        onClick: (P) => T(o)
      }, [
        e("div", ae, l(S(o.id)), 1),
        e("div", le, l(o.name), 1),
        e("p", se, l(o.description), 1),
        e("span", de, l(o.protocol), 1)
      ], 8, te))), 128))])])) : u("", !0),
      i.value && c.value ? (s(), d("div", ie, [
        e("h3", ne, "Configure " + l(c.value.name), 1),
        e("div", ue, [
          e("div", ce, [r[11] || (r[11] = e("label", { class: "text-sm font-medium" }, "Name", -1)), v(e("input", {
            "onUpdate:modelValue": r[1] || (r[1] = (o) => a.name = o),
            type: "text",
            placeholder: "e.g. Google Production",
            class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 512), [[f, a.name]])]),
          e("div", ve, [r[12] || (r[12] = e("label", { class: "text-sm font-medium" }, "Issuer", -1)), v(e("input", {
            "onUpdate:modelValue": r[2] || (r[2] = (o) => a.issuer = o),
            type: "text",
            placeholder: "https://accounts.google.com",
            class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 512), [[f, a.issuer]])]),
          e("div", pe, [r[13] || (r[13] = e("label", { class: "text-sm font-medium" }, "Client ID", -1)), v(e("input", {
            "onUpdate:modelValue": r[3] || (r[3] = (o) => a.client_id = o),
            type: "text",
            placeholder: "your-client-id",
            class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 512), [[f, a.client_id]])]),
          e("div", me, [r[14] || (r[14] = e("label", { class: "text-sm font-medium" }, "Client Secret", -1)), v(e("input", {
            "onUpdate:modelValue": r[4] || (r[4] = (o) => a.client_secret = o),
            type: "password",
            placeholder: "your-client-secret",
            class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 512), [[f, a.client_secret]])]),
          e("div", fe, [r[15] || (r[15] = e("label", { class: "text-sm font-medium" }, "Scopes", -1)), v(e("input", {
            "onUpdate:modelValue": r[5] || (r[5] = (o) => a.scopes = o),
            type: "text",
            placeholder: "openid email profile",
            class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 512), [[f, a.scopes]])]),
          e("div", ge, [v(e("input", {
            type: "checkbox",
            id: "wc-prov-auto",
            "onUpdate:modelValue": r[6] || (r[6] = (o) => a.auto_register = o),
            class: "accent-[var(--color-primary)]"
          }, null, 512), [[Q, a.auto_register]]), r[16] || (r[16] = e("label", {
            for: "wc-prov-auto",
            class: "text-sm cursor-pointer"
          }, "Auto-register users", -1))])
        ]),
        m.value ? (s(), d("div", be, l(m.value), 1)) : u("", !0),
        e("div", xe, [e("button", {
          class: "inline-flex items-center rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm hover:bg-[var(--color-muted)] transition-colors",
          onClick: r[7] || (r[7] = (o) => c.value = null)
        }, "← Back"), e("button", {
          class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50",
          disabled: !a.name || !a.issuer || !a.client_id || k.value,
          onClick: j
        }, l(k.value ? "Creating…" : "Create Provider"), 9, _e)])
      ])) : u("", !0),
      C.showSearch && !i.value ? (s(), d("div", ye, [v(e("input", {
        "onUpdate:modelValue": r[8] || (r[8] = (o) => _.value = o),
        type: "text",
        placeholder: "Search providers…",
        class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
      }, null, 512), [[f, _.value]])])) : u("", !0),
      b.value ? (s(), d("div", he, "Loading providers…")) : u("", !0),
      x.value ? (s(), d("div", ke, l(x.value), 1)) : u("", !0),
      !b.value && D.value.length && !i.value ? (s(), d("div", we, [e("table", Ce, [r[17] || (r[17] = e("thead", null, [e("tr", { class: "border-b bg-[var(--color-muted)]" }, [
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Name"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Protocol"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Template"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Status"),
        e("th", { class: "h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]" }, "Created"),
        e("th", { class: "h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]" }, "Actions")
      ])], -1)), e("tbody", null, [(s(!0), d(A, null, M(D.value, (o) => (s(), d("tr", {
        key: o.id,
        class: "border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors",
        onClick: (P) => N(o)
      }, [
        e("td", Pe, [e("div", ze, [e("span", null, l(S(o.template)), 1), e("span", Be, l(o.name), 1)])]),
        e("td", Se, [e("span", Ve, l(o.protocol), 1)]),
        e("td", Me, l(o.template), 1),
        e("td", Ue, [e("span", { class: V(["inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border", o.enabled ? "bg-green-50 text-green-700 border-green-200" : "bg-red-50 text-red-700 border-red-200"]) }, l(o.enabled ? "enabled" : "disabled"), 3)]),
        e("td", $e, l(L(o.created_at)), 1),
        e("td", Ae, [e("div", Ee, [e("button", {
          class: "inline-flex items-center rounded-md border border-[var(--color-border)] px-2 py-1 text-xs hover:bg-[var(--color-muted)] transition-colors",
          onClick: U((P) => I(o), ["stop"])
        }, l(o.enabled ? "Disable" : "Enable"), 9, Le), e("button", {
          class: "inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors",
          onClick: U((P) => F(o), ["stop"])
        }, "Delete", 8, Ne)])])
      ], 8, De))), 128))])])])) : u("", !0),
      !b.value && !x.value && !D.value.length && !i.value ? (s(), d("div", Te, l(_.value ? "No providers match your search." : "No providers configured yet."), 1)) : u("", !0)
    ])], 2));
  }
}), Ie = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", Fe = /* @__PURE__ */ R(je, [["styles", [Ie]]]), Ge = K(Fe);
customElements.define("zitadel-provider-list", Ge);
export {
  Ge as t
};

//# sourceMappingURL=zitadel-provider-list-wc-BhnxCPPb.js.map