import { t as i } from "./chunks/zitadel-login-wc-Dp8jinn8.js";
import { n, t as h } from "./chunks/wc-api-client-DbP47Lh1.js";
import { t as m } from "./chunks/zitadel-identity-list-wc-D6AhzNd1.js";
import { t as p } from "./chunks/zitadel-identity-detail-wc-G836vHcz.js";
import { t as S } from "./chunks/zitadel-identity-create-wc-D3fDUvwP.js";
import { t as v } from "./chunks/zitadel-account-wc-CfZu3ssJ.js";
import { t as L } from "./chunks/zitadel-session-manager-wc-CPeogCGn.js";
import { t as C } from "./chunks/zitadel-user-invite-wc-BOvUKTlH.js";
import { t as b } from "./chunks/zitadel-org-list-wc-Bmw7Tusz.js";
import { t as O } from "./chunks/zitadel-session-list-wc-B2MijmPv.js";
import { t as k } from "./chunks/zitadel-provider-list-wc-BhnxCPPb.js";
var e = `
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(240 10% 3.9%);
  --color-card: hsl(0 0% 100%);
  --color-card-foreground: hsl(240 10% 3.9%);
  --color-popover: hsl(0 0% 100%);
  --color-popover-foreground: hsl(240 10% 3.9%);
  --color-primary: hsl(240 5.9% 10%);
  --color-primary-foreground: hsl(0 0% 98%);
  --color-secondary: hsl(240 4.8% 95.9%);
  --color-secondary-foreground: hsl(240 5.9% 10%);
  --color-muted: hsl(240 4.8% 95.9%);
  --color-muted-foreground: hsl(240 3.8% 46.1%);
  --color-accent: hsl(240 4.8% 95.9%);
  --color-accent-foreground: hsl(240 5.9% 10%);
  --color-destructive: hsl(0 84.2% 60.2%);
  --color-destructive-foreground: hsl(0 0% 98%);
  --color-border: hsl(240 5.9% 90%);
  --color-input: hsl(240 5.9% 90%);
  --color-ring: hsl(240 5.9% 10%);
  --radius: 0.5rem;
`, l = `
  --color-background: hsl(240 10% 3.9%);
  --color-foreground: hsl(0 0% 98%);
  --color-card: hsl(240 10% 3.9%);
  --color-card-foreground: hsl(0 0% 98%);
  --color-primary: hsl(0 0% 98%);
  --color-primary-foreground: hsl(240 5.9% 10%);
  --color-secondary: hsl(240 3.7% 15.9%);
  --color-secondary-foreground: hsl(0 0% 98%);
  --color-muted: hsl(240 3.7% 15.9%);
  --color-muted-foreground: hsl(240 5% 64.9%);
  --color-accent: hsl(240 3.7% 15.9%);
  --color-accent-foreground: hsl(0 0% 98%);
  --color-destructive: hsl(0 62.8% 30.6%);
  --color-destructive-foreground: hsl(0 0% 98%);
  --color-border: hsl(240 3.7% 15.9%);
  --color-input: hsl(240 3.7% 15.9%);
  --color-ring: hsl(240 4.9% 83.9%);
`, t = "'Inter', ui-sans-serif, system-ui, -apple-system, sans-serif", s = `
:host {
  display: block;
  font-family: ${t};
  ${e}
}

:host(.dark) {
  ${l}
}
`;
function c() {
  const r = new CSSStyleSheet();
  return r.replaceSync(s), r;
}
var o = null;
function E() {
  return o || (o = c()), o;
}
export {
  h as WCApiError,
  v as ZitadelAccount,
  S as ZitadelIdentityCreate,
  p as ZitadelIdentityDetail,
  m as ZitadelIdentityList,
  i as ZitadelLogin,
  b as ZitadelOrgList,
  k as ZitadelProviderList,
  O as ZitadelSessionList,
  L as ZitadelSessionManager,
  C as ZitadelUserInvite,
  c as createSharedStyleSheet,
  n as createWCApiClient,
  E as getSharedStyleSheet
};

//# sourceMappingURL=zitadel-ui.js.map