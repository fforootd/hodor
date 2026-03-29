import { A as l, D as M, K as T, M as C, R as x, V as i, Y as f, a as B, f as k, g as n, h as y, n as E, o as U, p as a, s as A, t as D, u as V, y as N } from "./_plugin-vue_export-helper-DHhFP0j4.js";
import { a as K, i as F, n as W, r as w } from "./wc-api-client-DbP47Lh1.js";
var $ = { class: "space-y-4" }, j = { class: "space-y-3" }, G = { class: "space-y-1.5" }, L = {
  key: 0,
  class: "space-y-1.5"
}, R = ["value"], Y = {
  key: 1,
  class: "rounded-md border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-700"
}, Z = {
  key: 2,
  class: "rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
}, q = ["disabled"], S = "zitadel-user-invite", H = /* @__PURE__ */ N({
  __name: "zitadel-user-invite.ce",
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
    }
  },
  setup(_) {
    const o = _, z = k(() => F(o.darkMode));
    let v;
    const t = i(""), g = i(o.schemaType), d = i([]), u = i(!1), c = i(""), p = i(""), h = k(() => t.value.includes("@") && t.value.includes("."));
    async function b() {
      if (!(!h.value || u.value)) {
        u.value = !0, c.value = "", p.value = "";
        try {
          const s = o.schemaType || g.value || "human_user", e = d.value.find((m) => m.type === s && m.is_default) || d.value.find((m) => m.type === s);
          if (!e) throw new Error(`No schema found for type "${s}"`);
          const r = {
            schema_id: e.id,
            profile: { email: t.value }
          };
          o.orgId && (r.org_ids = [o.orgId]);
          const I = await v.post("/v1/users", r);
          await v.post("/v1/auth/magic-link", {
            identifier: t.value,
            purpose: "invite"
          }), p.value = `Invitation sent to ${t.value}`, w(S, "invite-sent", {
            email: t.value,
            user_id: I.id,
            purpose: "invite"
          }), t.value = "";
        } catch (s) {
          c.value = s?.message || "Failed to send invitation", w(S, "invite-error", { error: c.value });
        } finally {
          u.value = !1;
        }
      }
    }
    return M(async () => {
      v = W(K(o.apiBaseUrl));
      try {
        d.value = (await v.get("/v1/schemas")).items || [];
      } catch {
      }
    }), (s, e) => (l(), n("div", { class: T(["zitadel-wc", { dark: z.value }]) }, [a("div", $, [e[5] || (e[5] = a("div", null, [a("h2", { class: "text-lg font-semibold tracking-tight" }, "Invite User"), a("p", { class: "text-sm text-[var(--color-muted-foreground)]" }, "Send an invitation email to a new user")], -1)), a("div", j, [
      a("div", G, [e[2] || (e[2] = a("label", { class: "text-sm font-medium" }, "Email Address", -1)), x(a("input", {
        "onUpdate:modelValue": e[0] || (e[0] = (r) => t.value = r),
        type: "email",
        placeholder: "user@example.com",
        class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]",
        onKeyup: A(b, ["enter"])
      }, null, 544), [[U, t.value]])]),
      !_.schemaType && d.value.length > 1 ? (l(), n("div", L, [e[4] || (e[4] = a("label", { class: "text-sm font-medium" }, "User Type", -1)), x(a("select", {
        "onUpdate:modelValue": e[1] || (e[1] = (r) => g.value = r),
        class: "w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm"
      }, [e[3] || (e[3] = a("option", { value: "" }, "Select type…", -1)), (l(!0), n(V, null, C(d.value, (r) => (l(), n("option", {
        key: r.type,
        value: r.type
      }, f(r.type), 9, R))), 128))], 512), [[B, g.value]])])) : y("", !0),
      p.value ? (l(), n("div", Y, f(p.value), 1)) : y("", !0),
      c.value ? (l(), n("div", Z, f(c.value), 1)) : y("", !0),
      a("button", {
        class: "inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50 w-full",
        disabled: !h.value || u.value,
        onClick: b
      }, f(u.value ? "Sending…" : "Send Invitation"), 9, q)
    ])])], 2));
  }
}), J = ":host{--color-background:#fff;--color-foreground:#09090b;--color-primary:#18181b;--color-primary-foreground:#fafafa;--color-muted:#f4f4f5;--color-muted-foreground:#71717a;--color-border:#e4e4e7;--color-input:#e4e4e7;--color-ring:#18181b;--radius:.5rem;font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;display:block}:host(.dark){--color-background:#09090b;--color-foreground:#fafafa;--color-primary:#fafafa;--color-primary-foreground:#18181b;--color-muted:#27272a;--color-muted-foreground:#a1a1aa;--color-border:#27272a;--color-input:#27272a;--color-ring:#d4d4d8}.zitadel-wc{color:var(--color-foreground);background:var(--color-background);padding:1rem}.zitadel-wc.dark{--lightningcss-light: ;--lightningcss-dark:initial;color-scheme:dark}", O = /* @__PURE__ */ D(H, [["styles", [J]]]), P = E(O);
customElements.define("zitadel-user-invite", P);
export {
  P as t
};

//# sourceMappingURL=zitadel-user-invite-wc-BOvUKTlH.js.map