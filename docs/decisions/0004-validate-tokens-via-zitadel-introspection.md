# ADR-0004: Validate tokens via Zitadel introspection with JWT Profile auth

- **Status:** Accepted
- **Date:** 2026-03 (introspection landed in b8fc04c; JWT Profile in 57b9cc9)
- **Deciders:** PIM maintainers
- **Supersedes:** none (this ADR consolidates two related decisions that
  landed in quick succession)

## Context

Given ADR-0003 (Zitadel as IdP), the API gateway must verify each
inbound request's bearer token. There are two orthogonal choices:

1. **Verification mechanism** — how does the gateway decide a token is
   valid?
   - Local JWT signature verification with JWKS rotation
   - OIDC Token Introspection (call Zitadel on every request)
2. **Gateway-to-Zitadel authentication** — how does the gateway
   authenticate itself when calling Zitadel's introspection endpoint?
   - HTTP Basic (client_id + client_secret)
   - JWT Profile (client assertion signed with a private key)

The two choices compose: we need to pick one from each axis.

For axis 1, local JWT verification is faster (no network hop) but has
three practical problems for Zitadel + access tokens:
- Zitadel by default issues **opaque** access tokens, not JWTs, unless
  explicitly configured per-project.
- Even with JWT access tokens, local verification cannot detect
  server-side revocation (password changed, session terminated, user
  disabled) until the token expires.
- JWKS key rotation, caching, and clock-skew handling all become
  PIM's responsibility to get right.

Introspection inverts the trade-off: one network round-trip per
request, but Zitadel is the single source of truth for every
validation.

For axis 2, HTTP Basic is simpler to configure but ships the client
secret in plaintext over TLS on every introspection call, and the secret
lives in an environment variable. JWT Profile replaces the shared secret
with an asymmetric signing key: Zitadel keeps only the public key; the
gateway signs short-lived client assertions locally. The
long-lived secret on the gateway side is a private key file, not a
password that also grants admin API access.

## Decision

**Axis 1: Use OIDC Token Introspection** for every protected request.
Accept the per-request network hop in exchange for instant revocation
semantics and zero JWKS code in PIM.

**Axis 2: Authenticate to Zitadel via JWT Profile**, not HTTP Basic.
The gateway loads a Zitadel-issued key file (`zitadel-key.json`) at
startup and uses the `zitadel` crate's `Application::load_from_file`
to build the introspection config.

Token validation in handlers uses the `zitadel` crate's
`IntrospectedUser` **extractor** (actix `FromRequest` impl), not a
custom `Transform` middleware:

```rust
<!-- sketch -->
// Protected: handler takes IntrospectedUser. Extractor runs before the
// handler body; absent or invalid token returns 401 automatically.
async fn userinfo(user: IntrospectedUser) -> HttpResponse { /* ... */ }

// Public: same router, different handler signature — no IntrospectedUser
// parameter, no validation runs.
async fn health() -> &'static str { "OK" }
```

The main.rs wiring:

```rust
<!-- sketch -->
let application = Application::load_from_file(config.zitadel_key_file())?;
let introspection_config = IntrospectionConfigBuilder::new(
    config.zitadel_authority()
)
    .with_jwt_profile(application)  // JWT Profile, not Basic
    .build()
    .await?;

App::new().app_data(introspection_config.clone()) /* ... */
```

## Consequences

**Positive:**

- Revocation is instant. Disabling a user in Zitadel immediately
  invalidates their existing tokens on next request.
- JWKS, clock-skew, token-format changes are Zitadel's problem, not
  PIM's.
- Extractor-based validation scales linearly per route: public routes
  are public by not having the extractor in their signature. There is
  no global middleware decision to maintain, no allowlist regex to
  keep in sync.
- JWT Profile: client secret rotation no longer needs gateway redeploy
  — just swap the key file. Leaking the key file still requires
  filesystem access, a higher bar than a leaked env var.
- `apps/api-gateway/src/middlewares/auth.rs` is reduced to a 30-line
  `AuthenticatedUser` convenience wrapper around `IntrospectedUser`.

**Negative / accepted trade-offs:**

- Every protected request pays one introspection round-trip to
  Zitadel. At PIM scale (internal / prosumer) this is fine;
  sub-millisecond p50 when Zitadel is co-located.
- Gateway must be reachable from Zitadel's IdP (or vice versa) on
  every request. A Zitadel outage is a PIM auth outage.
- JWT Profile adds a private-key-management concern: the key file
  must be distributed to the gateway and protected at rest. Paths and
  secret-management plumbing live in `bootstrap/` and
  `apps/api-gateway/config.example.toml`.
- The extractor pattern cannot express "all routes under /admin
  require authentication" declaratively. Protection is per-handler.
  We consider this a feature: it makes protection visible in code
  review.

**Locked in:**

- `IntrospectedUser` is the canonical actix extractor for
  authenticated requests. Handlers that need auth MUST accept it (or a
  wrapper derived from it).
- `zitadel-key.json`-style JWT Profile key file as the gateway's
  Zitadel credential. Distribution/rotation happens via
  `bootstrap/`.
- No custom JWT middleware. If future requirements demand one (e.g.
  offline token verification), it must be a new ADR superseding this.

## Alternatives considered

### Option A — Local JWT verification with JWKS

Rejected. See Context: opaque tokens by default, no instant
revocation, JWKS plumbing as PIM's code. We can always add a local
verification fast-path later if introspection RTT becomes a bottleneck
(which at current scale it is not).

### Option B — Custom actix Transform middleware wrapping scopes

Rejected. The `zitadel` crate's extractor does exactly what we would
write ourselves, tested and maintained upstream. Writing our own
middleware adds code to own and misses the extractor's "visible in
handler signature" benefit.

### Option C — HTTP Basic auth to Zitadel introspection endpoint

Rejected. A shared secret in an env var is strictly worse than JWT
Profile's asymmetric key model. The integration complexity difference
(`with_basic_auth(id, secret)` vs `with_jwt_profile(application)`) is
a one-line code change. There is no reason to prefer the weaker model.

### Option D — Introspection cache (memoize results for N seconds)

Not done at this time. If introspection RTT becomes a problem we
reconsider; a short cache (1-5s) preserves most of the "instant
revocation" benefit while amortising the RTT. Deferred until measured.

## Implementation notes

- Re-exports live in `libs/infra-auth/src/lib.rs:8,11`.
- Gateway wiring: `apps/api-gateway/src/main.rs:5,37,46`.
- Settings model: `apps/api-gateway/src/config/settings.rs` —
  `ZitadelSettings { authority, key_file }`.
- Example config: `apps/api-gateway/config.example.toml:29-34`.
- Convenience wrapper (extracts PIM-shape user from IntrospectedUser):
  `apps/api-gateway/src/middlewares/auth.rs:21-35`.

## References

- Source code: paths above.
- External: [Zitadel Token Introspection
  docs](https://zitadel.com/docs/apis/openidoauth/endpoints#introspection_endpoint),
  [Zitadel JWT
  Profile](https://zitadel.com/docs/guides/integrate/private-key-jwt),
  [`zitadel` crate actix module](https://docs.rs/zitadel/latest/zitadel/actix/introspection/index.html).
- Originated from: `plans/003-zitadel-auth-integration.md` (introspection
  pattern) and `plans/004-jwt-profile-introspection.md` (JWT Profile
  upgrade) at commits `b8fc04c` and `57b9cc9`.
- Related: ADR-0003 (why Zitadel), ADR-0006 (why no auth-service).
