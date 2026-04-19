# ADR-0006: Remove the auth-service and shrink infra-auth to a re-export shim

- **Status:** Accepted
- **Date:** 2026-03 (landed in commit b8fc04c)
- **Deciders:** PIM maintainers

## Context

The initial PIM scaffold included `apps/auth-service/` — a tonic gRPC
service exposing `Login`, `Register`, `ValidateToken`, and
`RefreshToken` RPCs, all stubbed with TODOs — plus `libs/infra-auth/`
containing a `JwtManager`, `Claims`, and error types for local JWT
handling.

With ADR-0003 (Zitadel owns identity) and ADR-0004 (gateway validates
via Zitadel introspection), `auth-service` has **nothing left to do**:

- `Login`/`Register` run in Zitadel's own UI and OIDC flows (clients
  talk directly — ADR-0008).
- `ValidateToken` is what the gateway's `IntrospectedUser` extractor
  does, in-process.
- `RefreshToken` is OIDC refresh-token flow, handled by client SDKs
  against Zitadel.

Two viable shapes for what remains:

1. **Keep auth-service as a thin passthrough** to Zitadel's
   Management API (future-proofing for when we want to add PIM-specific
   auth policies).
2. **Delete auth-service entirely** and have any code that needs to
   talk to Zitadel do so directly.

Option 1 preserves an abstraction boundary "in case we need it"; option
2 is YAGNI.

## Decision

Delete `apps/auth-service/` and `proto/auth/` entirely. Reduce
`libs/infra-auth/` to a thin re-export layer for the `zitadel` crate
types that the gateway uses.

The complete content of `libs/infra-auth/src/lib.rs` is now:

```rust
<!-- sketch -->
pub use zitadel::actix::introspection::{
    IntrospectedUser, IntrospectionConfig, IntrospectionConfigBuilder,
};
pub use zitadel::credentials::Application;
```

The crate is retained (rather than having the gateway depend on
`zitadel` directly) for two reasons: (a) it gives us a single seam to
add PIM-specific auth glue if it becomes needed later, and (b) it makes
the workspace's "our auth boundary" explicit to contributors browsing
`libs/`.

## Consequences

**Positive:**

- Minus one binary to build, deploy, observe, and secure.
- Minus one proto file pair (client + server) the build compiles.
- `libs/rpc-proto` no longer includes auth proto; its `build.rs` drops
  to the single `user` proto.
- New contributors see "gateway + user-service" as the entire service
  list, matching the actual architecture.

**Negative / accepted trade-offs:**

- If a future requirement demands a PIM-owned auth service (e.g. to
  enforce policies Zitadel doesn't model), we re-create the crate and
  proto from scratch. We judge this unlikely enough, and the re-
  creation cost small enough, that keeping a placeholder is waste.

**Locked in:**

- `libs/infra-auth` is a **boundary shim**, not a home for auth
  business logic. Any future code that's more than a re-export
  requires explicit reconsideration (likely a new ADR).
- No `auth.v1` or similar gRPC surface. If auth-related RPC is ever
  needed, the proto lives in a new namespace.

## Alternatives considered

### Option A — Keep auth-service as a Zitadel passthrough

Rejected. A passthrough with no PIM-side logic is cost without
benefit: extra RPC hop, extra service, extra proto to maintain. If we
ever need PIM-side logic, we add it then, not pre-emptively.

### Option B — Merge auth-service into user-service

Rejected. User management (ADR-0007) and token validation (ADR-0004)
have different call patterns: user management is infrequent and
admin-originated, token validation is on the hot path of every API
request. Conflating them would either drag user-service onto the hot
path or make the gateway talk to it for introspection (slow, adds a
hop). The current split — gateway does introspection directly,
user-service handles user CRUD — is the correct boundary.

## References

- Source code: `libs/infra-auth/src/lib.rs` (complete file — 11 lines).
- Deleted code: `apps/auth-service/` (whole directory) and
  `proto/auth/` (whole directory) — see commit `b8fc04c`.
- Originated from: `plans/003-zitadel-auth-integration.md` Phase 2 at
  commit `b8fc04c`.
- Related: ADR-0003 (Zitadel as IdP), ADR-0004 (introspection
  pattern), ADR-0007 (user-service as Zitadel proxy).
