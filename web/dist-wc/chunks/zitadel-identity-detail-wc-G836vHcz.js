import { A as d, B as F, D as M, F as A, K as k, M as E, V as y, Y as a, f as b, g as i, h as s, n as L, p as e, t as N, u as O, y as V } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as R, i as $, n as T, r as C } from "./wc-api-client-DbP47Lh1.js";
var W = {
  key: 0,
  class: "flex justify-center py-16 text-sm text-[var(--color-muted-foreground)]"
}, G = {
  key: 1,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, K = {
  key: 2,
  class: "space-y-6"
}, P = { class: "flex items-center justify-between border-b border-[var(--color-border)] pb-4" }, Y = { class: "flex items-center gap-3" }, Z = { class: "flex h-10 w-10 items-center justify-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] font-semibold text-sm" }, q = { class: "text-lg font-semibold tracking-tight" }, H = { class: "flex items-center gap-2 text-sm text-[var(--color-muted-foreground)]" }, J = { class: "space-y-4" }, Q = { class: "text-sm font-medium" }, X = {
  key: 0,
  class: "flex gap-2"
}, ee = ["value", "onInput"], re = {
  key: 1,
  class: "flex-1 rounded-md border bg-[var(--color-muted)] px-3 py-2 text-sm text-[var(--color-muted-foreground)]"
}, te = {
  key: 0,
  class: "text-sm text-[var(--color-muted-foreground)] italic py-4 text-center"
}, oe = {
  key: 0,
  class: "flex justify-end pt-2 border-t border-[var(--color-border)]"
}, ae = ["disabled"], de = { class: "pt-4 border-t border-[var(--color-border)] space-y-2" }, ie = { class: "grid grid-cols-2 gap-2 text-sm" }, le = { class: "font-mono text-xs break-all" }, I = "zitadel-identity-detail", ne = /* @__PURE__ */ V({
  __name: "zitadel-identity-detail.ce",
  props: {
    apiBaseUrl: {
      default: "",
      type: String
    },
    identityId: {
      default: "",
      type: String
    },
    editable: {
      type: Boolean,
      default: !0
    },
    darkMode: {
      default: "",
      type: String
    }
  },
  setup(v) {
    const l = v, w = b(() => $(l.darkMode)), t = y(null), p = y(!1), m = y(!1), n = y(""), u = F({});
    let c;
    const D = b(() => ((t.value?.display_name || t.value?.identifier || "?")[0] || "?").toUpperCase()), x = b(() => t.value?.profile ? t.value.profile : {}), S = b(() => {
      if (!t.value?.profile) return !1;
      for (const [o, r] of Object.entries(u)) if (r !== (t.value.profile[o] ?? "")) return !0;
      return !1;
    });
    function z(o) {
      return o.replace(/_/g, " ").replace(/\b\w/g, (r) => r.toUpperCase());
    }
    function h(o) {
      return o ? new Date(o).toLocaleDateString() : "—";
    }
    async function _() {
      if (l.identityId) {
        p.value = !0, n.value = "";
        try {
          t.value = await c.get(`/v1/users/${encodeURIComponent(l.identityId)}`);
          for (const [o, r] of Object.entries(t.value.profile || {})) u[o] = String(r ?? "");
        } catch (o) {
          n.value = o?.message || "Failed to load identity";
        } finally {
          p.value = !1;
        }
      }
    }
    async function j() {
      m.value = !0;
      try {
        const o = {};
        for (const [r, f] of Object.entries(u)) f !== (t.value?.profile?.[r] ?? "") && (o[r] = f);
        await c.patch(`/v1/users/${encodeURIComponent(l.identityId)}`, { profile: o }), C(I, "identity-updated", {
          id: l.identityId,
          changes: o
        }), await _();
      } catch (o) {
        n.value = o?.message || "Failed to save";
      } finally {
        m.value = !1;
      }
    }
    async function U() {
      if (confirm("Delete this identity? This cannot be undone."))
        try {
          await c.delete(`/v1/users/${encodeURIComponent(l.identityId)}`), C(I, "identity-deleted", { id: l.identityId });
        } catch (o) {
          n.value = o?.message || "Failed to delete";
        }
    }
    return M(() => {
      c = T(R(l.apiBaseUrl)), _();
    }), A(() => l.identityId, () => {
      c && _();
    }), (o, r) => (d(), i("div", { class: k(["zitadel-wc", { dark: w.value }]) }, [
      p.value ? (d(), i("div", W, " Loading identity… ")) : s("", !0),
      n.value ? (d(), i("div", G, a(n.value), 1)) : s("", !0),
      !p.value && t.value ? (d(), i("div", K, [
        e("div", P, [e("div", Y, [e("div", Z, a(D.value), 1), e("div", null, [e("h2", q, a(t.value.display_name || t.value.identifier), 1), e("div", H, [e("span", null, a(t.value.identifier), 1), e("span", { class: k(["inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border", t.value.state === "active" ? "bg-green-50 text-green-700 border-green-200" : "bg-red-50 text-red-700 border-red-200"]) }, a(t.value.state), 3)])])]), v.editable ? (d(), i("button", {
          key: 0,
          class: "inline-flex items-center rounded-md border border-red-200 bg-transparent px-3 py-1.5 text-xs font-medium text-red-600 hover:bg-red-50 transition-colors",
          onClick: U
        }, "Delete")) : s("", !0)]),
        e("div", J, [
          r[0] || (r[0] = e("h3", { class: "text-sm font-medium text-[var(--color-muted-foreground)] uppercase tracking-wider" }, "Profile", -1)),
          (d(!0), i(O, null, E(x.value, (f, g) => (d(), i("div", {
            key: g,
            class: "space-y-1.5"
          }, [e("label", Q, a(z(String(g))), 1), v.editable ? (d(), i("div", X, [e("input", {
            value: u[String(g)] ?? f ?? "",
            onInput: (B) => u[String(g)] = B.target.value,
            class: "flex-1 h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
          }, null, 40, ee)])) : (d(), i("div", re, a(f || "—"), 1))]))), 128)),
          Object.keys(x.value).length === 0 ? (d(), i("div", te, " No profile fields found for this identity. ")) : s("", !0)
        ]),
        v.editable && S.value ? (d(), i("div", oe, [e("button", {
          class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50",
          disabled: m.value,
          onClick: j
        }, a(m.value ? "Saving…" : "Save Changes"), 9, ae)])) : s("", !0),
        e("div", de, [r[5] || (r[5] = e("h3", { class: "text-sm font-medium text-[var(--color-muted-foreground)] uppercase tracking-wider" }, "Details", -1)), e("div", ie, [
          r[1] || (r[1] = e("span", { class: "text-[var(--color-muted-foreground)]" }, "ID", -1)),
          e("span", le, a(t.value.id), 1),
          r[2] || (r[2] = e("span", { class: "text-[var(--color-muted-foreground)]" }, "Schema", -1)),
          e("span", null, a(t.value.schema_name || "—"), 1),
          r[3] || (r[3] = e("span", { class: "text-[var(--color-muted-foreground)]" }, "Created", -1)),
          e("span", null, a(h(t.value.created_at)), 1),
          r[4] || (r[4] = e("span", { class: "text-[var(--color-muted-foreground)]" }, "Updated", -1)),
          e("span", null, a(h(t.value.updated_at)), 1)
        ])])
      ])) : s("", !0)
    ], 2));
  }
}), se = ":host{--color-background:#fff;--color-foreground:#09090b;--color-card:#fff;--color-card-foreground:#09090b;--color-popover:#fff;--color-popover-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-secondary:#f4f4f5;--color-secondary-foreground:#18181b;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-accent:#f4f4f5;--color-accent-foreground:#18181b;--color-destructive:#ef4444;--color-destructive-foreground:#fafafa;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-card:#09090b;--color-card-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-secondary:#27272a;--color-secondary-foreground:#fafafa;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-accent:#27272a;--color-accent-foreground:#fafafa;--color-destructive:#7f1d1d;--color-destructive-foreground:#fafafa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", ue = /* @__PURE__ */ N(ne, [["styles", [se]]]), ce = L(ue);
customElements.define("zitadel-identity-detail", ce);
export {
  ce as t
};

//# sourceMappingURL=zitadel-identity-detail-wc-G836vHcz.js.map