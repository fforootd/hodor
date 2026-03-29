import { A as o, B as P, D as W, K as B, M as C, V as c, Y as i, _ as w, f as g, g as s, h as f, n as O, p as a, t as G, u as S, y as K } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as R, i as Y, n as Z, r as F } from "./wc-api-client-DbP47Lh1.js";
var H = { class: "space-y-6" }, J = { class: "flex items-center justify-between" }, Q = { class: "text-lg font-semibold tracking-tight" }, X = { class: "flex items-center gap-2 text-xs text-[var(--color-muted-foreground)]" }, ee = { key: 0 }, re = { key: 1 }, te = {
  key: 0,
  class: "space-y-4"
}, ae = {
  key: 0,
  class: "text-sm text-[var(--color-muted-foreground)] py-4"
}, oe = {
  key: 1,
  class: "space-y-2"
}, se = ["onClick"], le = { class: "text-sm font-medium" }, ie = { class: "text-xs text-[var(--color-muted-foreground)]" }, ne = {
  key: 1,
  class: "space-y-4"
}, de = { class: "text-xs text-[var(--color-muted-foreground)] mt-0.5" }, ce = {
  key: 0,
  class: "text-sm text-[var(--color-muted-foreground)] py-4"
}, ue = {
  key: 1,
  class: "space-y-3"
}, ve = { class: "text-sm font-medium flex items-center gap-1.5" }, fe = {
  key: 0,
  class: "text-red-500 text-xs"
}, pe = ["value", "onChange"], me = [
  "value",
  "type",
  "placeholder",
  "onInput"
], ye = {
  key: 2,
  class: "text-xs text-[var(--color-muted-foreground)]"
}, ge = {
  key: 2,
  class: "space-y-4"
}, _e = { class: "rounded-lg border border-[var(--color-border)] overflow-hidden" }, be = { class: "grid grid-cols-[1fr_auto] p-3 border-b bg-[var(--color-muted)] text-sm" }, he = { class: "font-medium" }, xe = { class: "text-[var(--color-muted-foreground)] capitalize" }, ke = { class: "font-medium text-right max-w-[200px] truncate" }, Ce = {
  key: 0,
  class: "p-3 bg-red-50 text-red-700 text-sm rounded-md border border-red-200"
}, we = { class: "flex items-center justify-between pt-2 border-t border-[var(--color-border)]" }, Se = { key: 1 }, ze = ["disabled"], E = "zitadel-identity-create", Te = /* @__PURE__ */ K({
  __name: "zitadel-identity-create.ce",
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
    darkMode: {
      default: "",
      type: String
    },
    mode: {
      default: "wizard",
      type: String
    }
  },
  setup(j) {
    const d = j, q = g(() => Y(d.darkMode));
    let _;
    const p = c([]), b = c(""), h = c(null), x = c([]), u = P({}), z = c(!1), T = c(!1), k = c(!1), m = c(""), n = c(0), I = g(() => {
      const t = [];
      return d.schemaType ? t.push({ title: "Schema" }) : t.push({ title: "Schema" }), t.push({ title: "Profile" }), t.push({ title: "Create" }), t;
    }), A = g(() => d.schemaType ? d.schemaType.replace(/_/g, " ").replace(/\b\w/g, (t) => t.toUpperCase()) : "Identity"), D = g(() => {
      const t = {};
      for (const [e, r] of Object.entries(u)) r && (t[e] = r);
      return t;
    }), N = g(() => {
      if (n.value === 0) return !!b.value;
      if (n.value === 1) {
        for (const t of x.value) if (t.required && !u[t.name]) return !1;
        return !0;
      }
      return !0;
    });
    function U(t) {
      const e = [], r = t?.schema?.properties;
      if (!r) return e;
      const l = t?.schema?.required || [];
      for (const [y, v] of Object.entries(r))
        y.startsWith("$") || y === "id" || e.push({
          name: y,
          label: v.title || y.replace(/_/g, " ").replace(/\b\w/g, (L) => L.toUpperCase()),
          type: v.type || "string",
          inputType: v.format === "email" ? "email" : v.type === "number" ? "number" : "text",
          description: v.description || "",
          required: l.includes(y),
          enum: v.enum || null
        });
      return e;
    }
    async function M(t) {
      b.value = t.id, h.value = t, T.value = !0;
      try {
        x.value = U(await _.get(`/v1/schemas/${t.id}`));
      } catch {
        x.value = [];
      } finally {
        T.value = !1;
      }
    }
    async function V() {
      if (n.value < I.value.length - 1) n.value++;
      else {
        k.value = !0, m.value = "";
        try {
          const t = {
            schema_id: b.value,
            profile: { ...u }
          };
          d.orgId && (t.org_ids = [d.orgId]);
          const e = await _.post("/v1/users", t);
          F(E, "identity-created", {
            id: e.id,
            identifier: e.identifier
          });
        } catch (t) {
          m.value = t?.message || "Failed to create identity", F(E, "create-error", { error: m.value });
        } finally {
          k.value = !1;
        }
      }
    }
    function $() {
      F(E, "create-cancelled");
    }
    return W(async () => {
      _ = Z(R(d.apiBaseUrl)), z.value = !0;
      try {
        if (p.value = (await _.get("/v1/schemas")).items || [], d.schemaType) {
          const t = p.value.find((e) => e.type === d.schemaType && e.is_default) || p.value.find((e) => e.type === d.schemaType);
          t && (await M(t), n.value = 1);
        }
      } catch {
        p.value = [];
      } finally {
        z.value = !1;
      }
    }), (t, e) => (o(), s("div", { class: B(["zitadel-wc", { dark: q.value }]) }, [a("div", H, [
      a("div", J, [a("h2", Q, "Create " + i(A.value), 1), a("button", {
        class: "rounded-sm opacity-70 hover:opacity-100 transition-opacity text-[var(--color-muted-foreground)]",
        onClick: $
      }, "✕")]),
      a("div", X, [(o(!0), s(S, null, C(I.value, (r, l) => (o(), s("span", {
        key: l,
        class: B(["inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border transition-colors", l === n.value ? "border-[var(--color-primary)] bg-[var(--color-primary)] text-[var(--color-primary-foreground)]" : l < n.value ? "border-green-300 bg-green-50 text-green-700" : "border-[var(--color-border)]"])
      }, [l < n.value ? (o(), s("span", ee, "✓")) : (o(), s("span", re, i(l + 1), 1)), w(" " + i(r.title), 1)], 2))), 128))]),
      n.value === 0 ? (o(), s("div", te, [e[1] || (e[1] = a("div", null, [a("label", { class: "text-sm font-medium" }, "Schema Type"), a("p", { class: "text-xs text-[var(--color-muted-foreground)] mt-0.5" }, "Select the type of identity to create")], -1)), z.value ? (o(), s("div", ae, "Loading schemas…")) : (o(), s("div", oe, [(o(!0), s(S, null, C(p.value, (r) => (o(), s("div", {
        key: r.id,
        class: B(["rounded-lg border p-3 cursor-pointer transition-colors hover:bg-[var(--color-muted)]", b.value === r.id ? "border-[var(--color-primary)] bg-[var(--color-primary)]/5" : "border-[var(--color-border)]"]),
        onClick: (l) => M(r)
      }, [a("div", le, i(r.type), 1), a("div", ie, "v" + i(r.version) + i(r.is_default ? " (default)" : ""), 1)], 10, se))), 128))]))])) : f("", !0),
      n.value === 1 ? (o(), s("div", ne, [a("div", null, [e[4] || (e[4] = a("label", { class: "text-sm font-medium" }, "Profile Information", -1)), a("p", de, [
        e[2] || (e[2] = w("Fields from ", -1)),
        a("strong", null, i(h.value?.type), 1),
        e[3] || (e[3] = w(" schema", -1))
      ])]), T.value ? (o(), s("div", ce, "Loading fields…")) : (o(), s("div", ue, [(o(!0), s(S, null, C(x.value, (r) => (o(), s("div", {
        key: r.name,
        class: "space-y-1.5"
      }, [
        a("label", ve, [w(i(r.label) + " ", 1), r.required ? (o(), s("span", fe, "*")) : f("", !0)]),
        r.type === "boolean" ? (o(), s("select", {
          key: 0,
          value: u[r.name] || "",
          onChange: (l) => u[r.name] = l.target.value,
          class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm"
        }, [...e[5] || (e[5] = [
          a("option", { value: "" }, "—", -1),
          a("option", { value: "true" }, "true", -1),
          a("option", { value: "false" }, "false", -1)
        ])], 40, pe)) : (o(), s("input", {
          key: 1,
          value: u[r.name] || "",
          type: r.inputType || "text",
          placeholder: r.description || "",
          onInput: (l) => u[r.name] = l.target.value,
          class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        }, null, 40, me)),
        r.description ? (o(), s("p", ye, i(r.description), 1)) : f("", !0)
      ]))), 128))]))])) : f("", !0),
      n.value === 2 ? (o(), s("div", ge, [
        e[7] || (e[7] = a("h3", { class: "text-sm font-medium" }, "Review", -1)),
        a("div", _e, [a("div", be, [e[6] || (e[6] = a("span", { class: "text-[var(--color-muted-foreground)]" }, "Schema", -1)), a("span", he, i(h.value?.type) + " v" + i(h.value?.version), 1)]), (o(!0), s(S, null, C(D.value, (r, l) => (o(), s("div", {
          key: l,
          class: "grid grid-cols-[1fr_auto] p-3 border-b text-sm"
        }, [a("span", xe, i(String(l).replace(/_/g, " ")), 1), a("span", ke, i(r), 1)]))), 128))]),
        m.value ? (o(), s("div", Ce, i(m.value), 1)) : f("", !0)
      ])) : f("", !0),
      a("div", we, [n.value > 0 ? (o(), s("button", {
        key: 0,
        class: "inline-flex items-center rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm hover:bg-[var(--color-muted)] transition-colors",
        onClick: e[0] || (e[0] = (r) => n.value--)
      }, "Back")) : (o(), s("div", Se)), a("button", {
        class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50",
        disabled: !N.value || k.value,
        onClick: V
      }, i(n.value === I.value.length - 1 ? k.value ? "Creating…" : "Create" : "Continue"), 9, ze)])
    ])], 2));
  }
}), Ie = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-card-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-secondary:#f4f4f5;--color-secondary-foreground:#18181b;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-accent:#f4f4f5;--color-accent-foreground:#18181b;--color-destructive:#ef4444;--color-destructive-foreground:#fafafa;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-card-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-secondary:#27272a;--color-secondary-foreground:#fafafa;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-accent:#27272a;--color-accent-foreground:#fafafa;--color-destructive:#7f1d1d;--color-destructive-foreground:#fafafa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", Be = /* @__PURE__ */ G(Te, [["styles", [Ie]]]), Fe = O(Be);
customElements.define("zitadel-identity-create", Fe);
export {
  Fe as t
};

//# sourceMappingURL=zitadel-identity-create-wc-D3fDUvwP.js.map