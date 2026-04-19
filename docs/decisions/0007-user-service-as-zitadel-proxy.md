# ADR-0007: Model user-service as a stateless Zitadel Management API proxy

- **Status:** Accepted
- **Date:** 2026-03 (landed in commit b8fc04c; refined through subsequent commits)
- **Deciders:** PIM maintainers

## Context

Given ADR-0003 (Zitadel owns identity), user data lives in Zitadel, not
in a PIM-owned database table. But PIM still exposes a user-facing API
surface (`GET /api/v1/users`, `GET /api/v1/users/{id}`,
`GET /api/v1/users/me`) and the gateway needs a gRPC backend to service
those routes.

Three shapes for how user-service should relate to Zitadel:

1. **Proxy**: user-service holds no local user state; each RPC
   translates directly to a Zitadel Management REST API v2 call.
2. **Cache**: user-service has a local Postgres table of users,
   synchronised from Zitadel via webhooks or periodic pulls; reads hit
   the cache, writes propagate to Zitadel.
3. **Local replica**: user-service owns the user table, uses Zitadel
   only for authentication, maintains its own user records
   independently.

Option 3 is ruled out by ADR-0003 (Zitadel is the single source of
truth). The meaningful choice is proxy vs cache.

## Decision

user-service is a **stateless proxy**. It holds a `reqwest::Client`, the
Zitadel authority URL, and a service-account PAT, and translates every
gRPC RPC into a Zitadel Management REST API v2 call inline. No local
database, no cache, no sync job.

Deserialization uses purpose-shaped structs (`ZitadelUser`,
`ZitadelListResponse`, etc.) rather than `serde_json::Value` traversal,
so field-shape drift from Zitadel surfaces at build/test time rather
than as runtime `None`s.

```rust
<!-- sketch -->
async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<GetUserResponse>, Status> {
    let url = format!("{}/v2/users/{}", self.zitadel_authority, req.id);
    let resp: ZitadelUserResponse = self.http_client
        .get(&url)
        .bearer_auth(&self.service_account_token)
        .send().await?
        .json().await?;
    Ok(Response::new(GetUserResponse {
        user: Some(Self::zitadel_user_to_proto(&resp.user)),
    }))
}
```

Authentication to Zitadel uses a long-lived service-account PAT
(configured as `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN`). The PAT
is distinct from the gateway's JWT Profile key (ADR-0004) — the gateway
uses introspection (a read-only verification operation), user-service
uses Management API (mutation capability), and they should rotate
independently.

## Consequences

**Positive:**

- Single source of truth is preserved. There is no way for PIM's view
  of a user to drift from Zitadel's.
- No migration, no background sync job, no cache-invalidation
  correctness problem, no database column to keep in step with
  Zitadel's v2 API.
- user-service is ~400 lines total (`apps/user-service/src/main.rs`).
  The entire "service" is HTTP client + type mapping + gRPC shim.
- Tests (`deserialize_zitadel_user_response`,
  `deserialize_zitadel_list_response_empty`,
  `zitadel_user_to_proto_*`) pin the Zitadel response shape we depend
  on; if Zitadel changes their API, deserialization tests fail before
  we ship a broken build.

**Negative / accepted trade-offs:**

- Every user read is one round-trip to Zitadel. For list-users
  endpoints this is fine at PIM scale; if we ever grow past ~100 QPS
  on user reads, a caching layer becomes justified.
- PIM cannot easily add per-user fields that Zitadel doesn't support
  (e.g. application-specific preferences). When that need arises, we
  introduce a side table keyed by Zitadel user_id, not a full user
  replica.
- If Zitadel is unreachable, user endpoints fail. For PIM this is the
  same failure mode as auth being down; they are coupled by design.

**Locked in:**

- user-service holds no durable state. Adding any storage to it is a
  decision reversal and needs a new ADR.
- Zitadel v2 API response shape is load-bearing: `userId`,
  `human.email.email`, `human.profile.givenName|familyName`,
  `details.creationDate|changeDate`. Changes here break PIM.
  Compensated by deserialization tests.
- Service-account PAT is the user-service → Zitadel credential.
  Rotation changes `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN` with
  no code change.

## Alternatives considered

### Option A — Local cache synced from Zitadel

Rejected for now. Cache correctness at the "user disabled in Zitadel 5
seconds ago" level is hard, and we have no performance evidence that a
cache is needed. Deferred until measurements justify it.

### Option B — user-service owns user records, Zitadel handles only auth

Rejected. Contradicts ADR-0003. Splits the source of truth and creates
a class of "which side is right" bugs that are expensive to debug in
production.

### Option C — Skip user-service, have the gateway call Zitadel directly

Rejected. Keeping user-service preserves a service boundary we can
later add business logic to (e.g. org-scoped user filtering, audit
logging on user reads) without rewriting the gateway. The cost —
one gRPC hop — is acceptable and matches the pattern the rest of PIM
follows.

## References

- Source code:
  - `apps/user-service/src/main.rs:119-398` — `UserServiceImpl`,
    proxy methods, `zitadel_user_to_proto` mapping
  - `apps/user-service/src/main.rs:491-563` — deserialization and
    mapping unit tests
  - `apps/user-service/src/config.rs:16-44` — Zitadel settings
  - `apps/user-service/config.example.toml:28-29` — config example
- External: [Zitadel User v2 API](https://zitadel.com/docs/apis/resources/user_service_v2/user-service-get-user-by-id)
- Originated from: `plans/003-zitadel-auth-integration.md` Phase 3 at
  commit `b8fc04c`.
- Related: ADR-0003 (Zitadel as IdP), ADR-0006 (no auth-service).
