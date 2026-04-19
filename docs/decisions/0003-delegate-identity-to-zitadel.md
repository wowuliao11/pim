# ADR-0003: Delegate identity and authentication to Zitadel

- **Status:** Accepted
- **Date:** 2026-03 (implementation landed in commits b8fc04c, 4aa2200)
- **Deciders:** PIM maintainers

## Context

Early PIM scaffolding included `libs/infra-auth/` with a hand-rolled
`JwtManager` (HMAC-SHA256 via `jsonwebtoken`), an `apps/auth-service/`
gRPC service with TODO `Login`/`Register`/`ValidateToken`/`RefreshToken`
methods, and a `JwtAuth` actix Transform middleware. None of it was
production-viable: no password hashing, no credential storage, no token
revocation, no password reset, no MFA, no audit trail, no user
management UI.

Implementing all of that correctly is at least a quarter of engineering
work and a permanent ongoing security liability. The alternative is to
adopt an external Identity Provider (IdP).

PIM's deployment model is a handful of Rust services with low
anticipated user counts (internal tooling / prosumer scale). It has no
existing identity stack to preserve, no SSO integrations to retain, and
no regulatory requirement that forbids external IdPs.

## Decision

**Zitadel** is PIM's Identity Provider. Zitadel owns users, credentials,
sessions, tokens, MFA, password policy, and the admin UI for all of
them. PIM services trust access tokens issued by Zitadel and never see
or store credentials.

Both a Zitadel Cloud SaaS instance and a self-hosted Zitadel (started by
`bootstrap/` — see ADR-0005) are supported; services connect to either
via the same OIDC discovery URL.

## Consequences

**Positive:**

- Zero auth-lifecycle code in PIM. Login, register, password reset,
  MFA, session management, SCIM, admin console — all free.
- Standard OIDC / OAuth 2.0 contracts; clients can use any conforming
  library.
- Security posture improves: we do not implement password hashing,
  token signing, or session revocation ourselves.
- Removes one entire PIM service (`auth-service`, see ADR-0006) and
  reduces `libs/infra-auth` to a thin re-export.

**Negative / accepted trade-offs:**

- External dependency: when Zitadel is down, PIM authentication is
  down. Mitigated by: (a) dev uses local self-hosted Zitadel, (b)
  production Zitadel Cloud SLA is acceptable for PIM's use case.
- Vendor coupling: migrating off Zitadel later would require rewriting
  the user-service proxy (ADR-0007) and any Zitadel-specific role/org
  modelling. We accept this as an explicit bet.
- User data lives outside PIM's database. Any PIM feature requiring
  "join user row to business data" proxies through Zitadel
  (ADR-0007), adding one network hop.

**Locked in:**

- OIDC as the auth protocol. No custom auth scheme.
- `zitadel` Rust crate (v5 at time of decision) as the integration
  library. See ADR-0004 for how it is used.

## Alternatives considered

### Option A — Build our own auth service

Rejected. Correct authentication is a deep specialty. We have no
differentiation to gain by implementing it. The ongoing security
maintenance burden alone outweighs any flexibility benefit.

### Option B — Auth0 / Okta / Firebase Auth

Rejected. All are viable, but Zitadel is open source (can self-host for
dev/test and if we need data sovereignty later), has a first-class
Rust crate, and its API model (orgs → projects → apps → roles)
matches how we intend to structure multi-tenancy. Pricing is also
developer-friendly at PIM's scale.

### Option C — Keycloak

Rejected. Keycloak is more mature and equally open source, but the
Rust ecosystem around it is thinner (no equivalent of the `zitadel`
crate's actix integration) and operating Keycloak is heavier (JVM, more
config surface). For PIM's size, the `zitadel` crate's near-zero
integration cost wins.

### Option D — Supabase Auth

Rejected. Supabase Auth is tightly coupled to Supabase Postgres and
its JS/TS client. For a Rust-first, Postgres-agnostic backend,
Supabase would pull PIM in a direction we do not want to go.

## References

- Source code:
  - `libs/infra-auth/src/lib.rs` — Zitadel re-exports
  - `apps/api-gateway/src/main.rs` — introspection config wiring
  - `apps/api-gateway/src/config/settings.rs` — `ZitadelSettings`
  - `apps/user-service/src/main.rs` — Zitadel Management API proxy
  - `tools/pim-bootstrap/` — local Zitadel provisioning
- External: [zitadel.com](https://zitadel.com/),
  [zitadel crate](https://crates.io/crates/zitadel)
- Originated from: `plans/003-zitadel-auth-integration.md` at commit
  `b8fc04c`. Current code also reflects ADR-0004 (JWT Profile) which
  landed later in commit `57b9cc9`.
- Related: ADR-0004 (how the gateway validates tokens), ADR-0005
  (bootstrapping local Zitadel), ADR-0006 (removing auth-service),
  ADR-0007 (user-service as Zitadel proxy), ADR-0008 (clients talk
  OIDC directly).
