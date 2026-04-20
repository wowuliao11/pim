# ADR-0016: Talk to Zitadel Management API over hand-written REST

- **Status:** Proposed
- **Date:** 2026-04-19
- **Deciders:** PIM maintainers

## Context

ADR-0005 commits `pim-bootstrap` to creating and reconciling Zitadel
objects (project, API app, service account, keys, roles, user grants)
against a self-hosted Zitadel instance. Phase 3 of that tool — the
ensure-op dispatcher formalised in ADR-0017 — cannot land until we
pick a wire-protocol strategy. The choice matters because it locks in
crate surface, error taxonomy, retry semantics, and the authentication
path for every subsequent feature.

The candidate approaches are:

1. The community [`zitadel`](https://crates.io/crates/zitadel) Rust
   crate's `api::v1` client (gRPC + tonic, generated from Zitadel's
   proto files).
2. Hand-written reqwest + per-endpoint request/response models against
   the Management API's REST/JSON surface (the gRPC-Gateway transcoded
   HTTP routes Zitadel exposes at `/management/v1/...`).
3. Shelling out to `zitadel-tools` or a compiled CLI.

Constraints already decided:

- **Runtime target.** Zitadel v4.13.0 self-hosted. Every Management
  API RPC has a REST equivalent at `/management/v1/…`, authenticated
  with the same bearer token as gRPC (source survey in the librarian
  report referenced below).
- **Auth modes.** ADR-0012 plus `tools/pim-bootstrap/src/config.rs:33-38`
  enumerate two admin-auth modes: `Pat` (dev default) and
  `JwtProfile` (prod). The client must handle both, exchanging the
  JWT assertion at `/oauth/v2/token` for an access token when in
  `JwtProfile` mode.
- **Natural-key idempotency.** ADR-0005 and ADR-0017 both require the
  client to *list and filter by natural key* (`project.name`,
  `api_app.name`, `service_account.username`, `role.key`) — the
  server does not offer `get-by-name` endpoints, only list-and-search.
  The client must paginate exhaustively and deduplicate.
- **Error classification.** ADR-0017's `Conflict` state is driven by
  server-side HTTP status + error code. The client must surface both
  machine-readable codes (`ALREADY_EXISTS`, `NOT_FOUND`,
  `FAILED_PRECONDITION`, `PERMISSION_DENIED`, `UNAUTHENTICATED`) and
  the raw body for diagnostics.
- **Deployment shape.** `pim-bootstrap` is a single binary that ships
  in CI and dev; the client lives in-process. No sidecar gRPC proxy
  is acceptable.

Relevant research is captured in the librarian report surveyed for
this ADR (internal session `ses_25a56b916ffe7kI3Se5ywrfm12`,
2026-04-19); the report cites Zitadel source paths at tag `v4.13.0`
for every endpoint, auth flow, error mapping, and list-pagination
shape relied on below.

## Decision

**`pim-bootstrap` talks to Zitadel's Management API over hand-written
REST/JSON using `reqwest`, with per-endpoint request and response
structs defined in a new crate `libs/zitadel-rest-client`.** The
crate owns authentication, pagination, retries, error taxonomy, and
logging. It exposes one typed method per Zitadel operation the tool
actually calls, nothing more. gRPC, code generation, and
`zitadel`-crate dependencies are explicitly out of scope.

### Scope: endpoint surface

The client implements exactly these endpoints in its first landing,
one typed method per row. (Full request/response shapes come from the
librarian report and go into module-level rustdoc on each method.)

| Kind           | Method | Path                                                   |
|----------------|--------|--------------------------------------------------------|
| Project        | GET    | `/management/v1/projects/{id}`                         |
| Project        | POST   | `/management/v1/projects/_search`                      |
| Project        | POST   | `/management/v1/projects`                              |
| Project        | PUT    | `/management/v1/projects/{id}`                         |
| API app        | GET    | `/management/v1/projects/{project_id}/apps/{app_id}`   |
| API app        | POST   | `/management/v1/projects/{project_id}/apps/_search`    |
| API app        | POST   | `/management/v1/projects/{project_id}/apps/api`        |
| API app        | PUT    | `/management/v1/projects/{project_id}/apps/{app_id}`   |
| Machine user   | POST   | `/management/v1/users/_search`                         |
| Machine user   | POST   | `/management/v1/users/machine`                         |
| Machine key    | POST   | `/management/v1/users/{user_id}/keys/_search`          |
| Machine key    | POST   | `/management/v1/users/{user_id}/keys`                  |
| Machine key    | DELETE | `/management/v1/users/{user_id}/keys/{key_id}`         |
| Project role   | POST   | `/management/v1/projects/{project_id}/roles/_search`   |
| Project role   | POST   | `/management/v1/projects/{project_id}/roles`           |
| Project role   | POST   | `/management/v1/projects/{project_id}/roles/_bulk`     |
| User grant     | POST   | `/management/v1/user-grants/_search`                   |
| User grant     | POST   | `/management/v1/user-grants`                           |
| User grant     | PUT    | `/management/v1/user-grants/{grant_id}`                |
| User grant     | DELETE | `/management/v1/user-grants/{grant_id}`                |

New endpoints require an amending ADR or a revision here. Every
endpoint is v1 Management API. Zitadel's v2 user service exists but
the ensure-ops we need (machine user + keys) all live under v1 and we
stay uniform.

### Authentication

The client accepts one `AdminCredential` enum, mirroring
`AdminAuthMode` in `tools/pim-bootstrap/src/config.rs:33-38`:

- `Pat(String)` — passed through as
  `Authorization: Bearer <pat>` on every request. The PAT is read
  from the env var named by `ZitadelTarget.admin_pat_env_var`
  (`config.rs:57-59`); the client never touches environment directly.
- `JwtProfile { key_path, authority }` — the client loads the JSON
  key file on construction, builds and signs an RS256 JWT assertion
  (`iss`, `sub` = machine-user ID from the JSON; `aud` = authority;
  `exp` = now + 55 min; `iat` = now; `jti` = UUID v4), POSTs to
  `/oauth/v2/token` with
  `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer` and
  `scope=openid urn:zitadel:iam:org:project:id:zitadel:aud`, then
  caches the resulting access token until 60 seconds before its
  `expires_in`. On cache miss the client re-exchanges.

No dev/prod switching logic in the client; the caller supplies the
credential. This keeps the client stateless about environment and
testable with either mode against a fake.

### Error model

The client exposes a single `ZitadelError` enum whose variants are
classified by the server's gRPC code in the JSON error body (`code`
field, values per the standard gRPC-to-HTTP mapping documented in the
librarian report):

| Variant                  | HTTP | gRPC code | Meaning                                       |
|--------------------------|------|-----------|-----------------------------------------------|
| `InvalidArgument`        | 400  | 3         | Malformed request                             |
| `PermissionDenied`       | 403  | 7         | Caller lacks scopes                           |
| `NotFound`               | 404  | 5         | Natural-key lookup got no match               |
| `AlreadyExists`          | 409  | 6         | Create collided on server uniqueness          |
| `FailedPrecondition`     | 400  | 9         | e.g. delete with dependencies                 |
| `Aborted`                | 409  | 10        | Concurrent modification                       |
| `Unauthenticated`        | 401  | 16        | Bad / expired bearer                          |
| `ResourceExhausted`      | 429  | 8         | Rate limited                                  |
| `Unavailable`            | 503  | 14        | Server side; retriable                        |
| `Internal`               | 500  | 13        | Server side; not retriable by client          |
| `Transport(reqwest::Error)` | — | —         | Connection / TLS / timeout                    |
| `Decode { body, err }`   | —    | —         | JSON deserialisation failure (never silent)   |

Every error carries the raw response body as a `String` for
diagnostics. The classifier reads `body.code` first, falls back to
HTTP status when the body is not JSON. These variants are what
ADR-0017's `Conflict` state machine switches on.

### Retries, timeouts, logging

- **Request timeout:** 30 s per call (configurable on
  `ClientBuilder`).
- **Retries:** up to 3 retries with exponential backoff (200 ms,
  500 ms, 1200 ms) on `Transport`, `ResourceExhausted`, and
  `Unavailable`. No retries on any other variant — in particular
  `AlreadyExists` and `NotFound` are returned immediately because
  ADR-0017 wants them classified, not hidden.
- **Logging:** structured `tracing` events at `debug` on request,
  `info` on non-retried response, `warn` on retried response,
  `error` on terminal failure. Bearer tokens and JWT private keys
  are never logged; request bodies are logged at `trace` only.
- **Idempotency headers:** none. Zitadel does not honour
  `Idempotency-Key`; natural-key lookup in ADR-0017 is our
  idempotency mechanism.

### Pagination

All `_search` endpoints use the same request shape
(`{query, offset, limit, asc}`) and response shape
(`{result[], details.{total_result,processed_sequence,view_timestamp}}`).
The client exposes `list_all` helpers that paginate with
`limit = 100` and drain until `offset + len(result) >=
details.total_result`. Consumers never touch `offset` directly.

### Crate layout

`libs/zitadel-rest-client/` is a new workspace member following the
monorepo's layer rules (`.github/copilot-instructions.md` §Dependency
Rules: `libs/*` may depend on `proto/` only). The crate has zero
dependency on `proto/` because Management v1 has no `.proto` in our
tree — it is a pure HTTP client.

- `src/lib.rs` — public surface: `Client`, `ClientBuilder`,
  `AdminCredential`, `ZitadelError`, `Details`, per-endpoint
  request/response structs grouped by resource module.
- `src/auth.rs` — PAT pass-through + JWT Profile token exchange +
  token cache.
- `src/error.rs` — `ZitadelError`, classifier, JSON error-body
  struct.
- `src/pagination.rs` — `list_all` helper, `ListRequest` /
  `ListResponse` generics.
- `src/{project,app,user,user_key,project_role,user_grant}.rs` —
  one file per resource, typed methods.

No feature flags on first landing. Add them only when an actual
consumer needs a compile-time toggle.

## Consequences

**Positive:**

- The entire wire surface is 20 methods (table above). Easy to audit,
  easy to mock for ADR-0017's ensure-op unit tests.
- No build-time proto compilation or tonic toolchain in the bootstrap
  pipeline. `libs/zitadel-rest-client` compiles with stable Rust and
  `reqwest` only.
- Error taxonomy is typed at the variant level, not at the `.code()`
  integer level. ADR-0017's `Conflict` classifier matches on
  `ZitadelError::AlreadyExists` / `FailedPrecondition` / etc., never
  on raw integers.
- JWT Profile key handling is ours end-to-end, so the ADR-0012
  `stdout:<tag>` prod sink model works: the caller decides how the
  JSON key reaches `JwtProfile.key_path`.
- The crate is reusable beyond `pim-bootstrap`: the api-gateway or a
  future management CLI can depend on it without dragging in
  bootstrap-specific types.

**Negative / accepted trade-offs:**

- We write and maintain request/response structs by hand. When
  Zitadel adds fields we do not model, we ignore them on read
  (`#[serde(default)]` + `deny_unknown_fields` *disabled*); when
  Zitadel changes a field name we only notice at runtime. Mitigation:
  integration-test smoke against the real Zitadel in dev compose.
- JSON Management v1 is Zitadel's older surface. Some v2 endpoints
  exist (e.g. user service) with richer shapes; adopting them later
  is a new ADR, not an in-place migration.
- The one-shot secret properties (API app `client_secret`, machine
  key private key returned only on create) are handled inside the
  response struct for those two endpoints. If we lose the response,
  the secret is gone — rotation is the recovery path (ADR-0017).

**Locked in:**

- `reqwest` is the HTTP client. Switching to `hyper` directly or
  `ureq` later is a new ADR.
- `tracing` is the log/telemetry channel. Matches
  `libs/infra-telemetry`.
- Management API v1 JSON is the protocol. v2 adoption is out of
  scope until there is a concrete reason to migrate.
- `libs/zitadel-rest-client` is a PIM-internal crate; no publishing
  to crates.io.

**Follow-up:**

- Create `libs/zitadel-rest-client/` with the 20-method surface, the
  auth module, the error enum, and the `list_all` helper. Unit tests
  use `wiremock` or `mockito` for the HTTP contract; no real Zitadel
  in unit tests.
- Integration smoke test lives in `tools/pim-bootstrap`: exercise
  project create + lookup against the dev Zitadel brought up by
  `just dev-up`. Run only when `ZITADEL_INTEGRATION=1`, not in CI by
  default, matching ADR-0013's drift-observable stance.
- Extend the surface *only* when ADR-0017 grows a new ensure-op. Do
  not pre-emptively add endpoints.

## Alternatives considered

### Option A — `zitadel` community crate (gRPC + tonic)

Rejected. Pulls tonic, prost, and the full Zitadel proto tree into
`pim-bootstrap`'s compile time and final binary. The crate's auth
surface (`zitadel::credentials::Application`) is tied to actix-web
IntrospectedUser flows and does not cleanly map to a headless admin
tool. Error handling is a single `tonic::Status` rather than a typed
enum, so ADR-0017's `Conflict` classifier would switch on string
codes. The perceived benefit of "generated types = no drift" is
partially illusory: we still need hand-written glue for
paginate-until-done and for the PAT-vs-JWT-Profile admin toggle, and
the generated types include fields we ignore. Not worth the build
complexity for a 20-method surface.

### Option B — Community crate for auth only, reqwest for Management

Rejected. The split produces two error types (tonic status vs our
enum) that have to be unified upstream, and the `zitadel` crate's
auth helpers are oriented at inbound token validation (OIDC
introspection) rather than outbound service-user token minting. The
JWT Profile flow is a ~40-line hand-written block; owning it is
cheaper than wrapping it.

### Option C — Shell out to `zitadelctl` / `zitadel-tools`

Rejected. We would move structured errors into stdout parsing,
introduce a CLI binary as a runtime dependency of our bootstrap
binary, and lose type safety. ADR-0017's matrix needs machine-
readable error classification, which subprocess stdout does not
provide reliably.

### Option D — Wait for Zitadel v2 REST surface to fully replace v1

Rejected. Zitadel has not signalled deprecation of Management v1.
v2 migration can be a future ADR once v2 covers every endpoint in
the table above; today it does not (no v2 parity for
project-role `_bulk`, for one).

## Implementation notes

<!-- sketch -->

```rust
// libs/zitadel-rest-client/src/lib.rs
pub struct Client {
    inner: reqwest::Client,
    authority: Url,
    credential: CredentialSource,
}

pub enum AdminCredential {
    Pat(SecretString),
    JwtProfile { key_path: PathBuf, authority: Url },
}

pub enum ZitadelError {
    InvalidArgument { body: String },
    NotFound { body: String },
    AlreadyExists { body: String },
    FailedPrecondition { body: String },
    PermissionDenied { body: String },
    Unauthenticated { body: String },
    Aborted { body: String },
    ResourceExhausted { body: String },
    Unavailable { body: String },
    Internal { body: String },
    Transport(reqwest::Error),
    Decode { body: String, err: serde_json::Error },
}

impl Client {
    pub async fn list_projects_by_name(&self, name: &str) -> Result<Vec<Project>, ZitadelError> { ... }
    pub async fn add_project(&self, req: &AddProjectRequest) -> Result<AddProjectResponse, ZitadelError> { ... }
    // ... one typed method per row of the endpoint table above
}
```

## References

- Source code:
  - `tools/pim-bootstrap/src/config.rs:33-38` — `AdminAuthMode` enum
    that the client's `AdminCredential` mirrors
  - `tools/pim-bootstrap/src/config.rs:49-64` — `ZitadelTarget`
    (authority + admin credential source) that the client consumes
  - `tools/pim-bootstrap/src/main.rs:57` — stub call site that this
    ADR together with ADR-0017 unblocks
  - `.github/copilot-instructions.md` §Dependency Rules — layering
    constraint that places `libs/zitadel-rest-client` in `libs/`
- Related ADRs:
  - ADR-0005 (bootstrap tool contract; establishes natural-key
    lookup requirement this client satisfies)
  - ADR-0012 (three-layer sink model; the client has no opinion
    about sinks but its one-shot secret responses feed them)
  - ADR-0013 (dev/prod parity; the client is the same in both
    environments, only credentials differ)
  - ADR-0017 (ensure-op state machine; consumes this client's
    typed error variants to classify the `Conflict` state)
- External: Zitadel v4.13.0 source tree
  ([github.com/zitadel/zitadel](https://github.com/zitadel/zitadel/tree/v4.13.0)),
  specifically `proto/zitadel/management.proto`,
  `internal/api/grpc/management/`, `internal/api/http/error.go`, and
  `internal/api/oidc/token_jwt_profile.go` for endpoint, error, and
  JWT Profile flow references consumed above.
