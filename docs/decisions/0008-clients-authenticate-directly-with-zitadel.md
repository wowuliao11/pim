# ADR-0008: Clients authenticate with Zitadel directly via OIDC + PKCE

- **Status:** Accepted
- **Date:** 2026-03 (architectural decision; client code lives outside this repo)
- **Deciders:** PIM maintainers

## Context

Given ADR-0003 (Zitadel owns identity), there is still a choice about
how PIM clients (the Tauri mobile/desktop app and the React admin
panel) obtain bearer tokens:

1. **Direct client ↔ Zitadel**: clients run OIDC Authorization Code +
   PKCE directly against Zitadel. The PIM API gateway never sees
   credentials; it only sees the bearer token on subsequent API calls.
2. **Gateway-mediated**: the gateway exposes `/api/v1/auth/login`,
   receives username+password, forwards to Zitadel's token endpoint on
   the user's behalf, returns the token.

Option 2 existed in the original scaffold (as TODO endpoints on
`auth-service`) and is sometimes called "backend for frontend"
authentication. It is superficially attractive because clients stay
simple: they talk only to PIM, never to Zitadel.

The counter-arguments are that option 2:
- makes PIM briefly handle plaintext credentials (violates one of the
  main reasons to adopt an IdP, ADR-0003);
- reimplements OIDC in PIM — including MFA challenges, captcha, email
  verification redirects, password-reset flows, and every edge case
  Zitadel has already solved correctly;
- blocks use of standard OIDC client libraries on the client side;
- makes it impossible to use Zitadel's hosted login UI, which is the
  primary UX reason to adopt Zitadel at all.

## Decision

Clients authenticate with Zitadel **directly** using OIDC Authorization
Code + PKCE. PIM's API gateway has no login, register, password-reset,
or token-refresh routes. It only serves resources that require a valid
bearer token and a single `GET /api/v1/auth/userinfo` that echoes back
what the current token's introspection revealed.

- **Tauri app**: uses a system-browser-based OIDC flow with PKCE (no
  client secret stored in the app). Access and refresh tokens live in
  OS-level secure storage.
- **React admin panel**: uses a standard browser OIDC+PKCE flow with a
  public-client Zitadel application, storing tokens in memory
  (refresh via silent iframe or refresh token as appropriate to the
  deployment).

PIM does not define or enforce the client implementation; the contract
is "send a valid Zitadel-issued bearer token in the `Authorization`
header." Any client that can do OIDC correctly can talk to PIM.

## Consequences

**Positive:**

- PIM never touches user credentials. Phishing, credential stuffing,
  password storage, MFA prompts — all outside PIM's attack surface.
- Clients use industry-standard OIDC libraries (e.g. `oauth4webapi`,
  `oidc-client-ts`, platform-native OIDC SDKs). No PIM-specific SDK
  to maintain.
- Zitadel's hosted login page, MFA flows, password reset, and branding
  all work out of the box. Changing login UX is a Zitadel console
  task, not a PIM deploy.
- The gateway's responsibility surface shrinks to "receive bearer
  token, authorize resource access." Clear boundary, easy to audit.

**Negative / accepted trade-offs:**

- Client integration is non-trivial: clients must handle OIDC redirect
  URIs, PKCE challenge generation, token storage, and refresh flows.
  This is mitigated by mature client libraries; the alternative
  (option 2) moves that work to PIM, where it is worse.
- CORS configuration on Zitadel must allow the React panel's origin;
  mobile app deep-link / custom-URL-scheme registration is required.
  These are one-time setup costs.
- The gateway cannot intercept login to add PIM-specific login audit
  logs. If we need that, we subscribe to Zitadel's event stream, not
  inject ourselves into the auth flow.

**Locked in:**

- No `/api/v1/auth/login`, `/api/v1/auth/register`, or similar
  credential-receiving routes on the gateway. Adding one requires a
  new ADR superseding this.
- OIDC + PKCE as the client auth flow (not implicit flow, not ROPC).

## Alternatives considered

### Option A — Gateway-mediated login (backend-for-frontend)

Rejected. See Context above. The usability argument (simpler clients)
does not hold up once MFA, password reset, and email verification are
considered — any correct BFF has to proxy all of those flows too,
which means implementing them in PIM, which is exactly what ADR-0003
exists to avoid.

### Option B — Implicit flow (legacy SPA auth)

Rejected. Deprecated by the OAuth 2.0 Security BCP. PKCE supersedes it
for all use cases.

### Option C — ROPC (Resource Owner Password Credentials)

Rejected. Also deprecated. Same concerns as option A plus it is only
permitted for first-party clients and forecloses social login and MFA.

## References

- Source code (PIM-side contract):
  - `apps/api-gateway/src/api/v1/routes.rs` — only `/auth/userinfo`
    exists under `/auth`, no login/register routes
  - `apps/api-gateway/src/api/v1/handlers/auth.rs` — `userinfo`
    handler uses `IntrospectedUser`
- External: [RFC 7636 PKCE](https://datatracker.ietf.org/doc/html/rfc7636),
  [Zitadel OIDC flow guide](https://zitadel.com/docs/guides/integrate/login/oidc/login-users)
- Originated from: `plans/003-zitadel-auth-integration.md` Architecture
  section at commit `b8fc04c`. Client implementation lives in separate
  repositories and is not tracked by this ADR.
- Related: ADR-0003 (Zitadel as IdP), ADR-0004 (how the gateway
  validates the tokens clients obtain via this flow).
