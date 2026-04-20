# Plan 001 — `pim-bootstrap` Phase 3 Implementation

- **Status:** Proposed
- **Date:** 2026-04-19
- **Owner:** baozhiming
- **Governed by:** [ADR-0016](../docs/decisions/0016-rest-client-for-zitadel-management-api.md) (REST client), [ADR-0017](../docs/decisions/0017-ensure-ops-idempotent-state-machine.md) (four-state ensure-op driver). Existing ADRs 0005/0010/0012/0014 already cover the output layering and dev auth mode.
- **Goal:** Land the full Phase 3 of `tools/pim-bootstrap` so that a fresh clone running `just dev-up && just dev-bootstrap` produces a working `zitadel-key.json`, `.env.local`, and per-service `config.toml`, after which `cargo run -p api-gateway` starts successfully against the local Zitadel.
- **Non-goals:** Prod credential rotation, CI-driven prod bootstrap, proto changes, changes to `zitadel-api` container lifecycle, any Zitadel-side policy hardening beyond what ADR-0010/0012 already mandate.

---

## 1. Context (what lives where today)

| Concern | Current state | Reference |
|---|---|---|
| CLI surface (`bootstrap apply/plan`, `seed apply`, `diff`) | Scaffolded, parses config, then prints `"not implemented yet (Phase 3)"` | `tools/pim-bootstrap/src/main.rs:57,85` |
| Config structs (`BootstrapConfig`, `SeedConfig`) | Complete, includes `AdminAuthMode::{Pat, JwtProfile}`, `AppAuthMethod`, `OutputsConfig` | `tools/pim-bootstrap/src/config.rs:1` |
| Dev bootstrap seed | `env="dev"`, `admin_auth="pat"`, project `pim`, app `api-gateway` (jwt_profile), SA `user-service-sa`, roles `admin`/`member`, outputs pinned to `apps/<name>/config.toml`, `zitadel-key.json`, `.env.local` | `bootstrap/dev.toml:1` |
| Dev human seed | alice/bob/charlie + role assignments, `env="dev"` gate | `bootstrap/seed.dev.toml:1` |
| REST client | Does not exist; Cargo deps only (`reqwest`, `zitadel`, `tokio`) | `tools/pim-bootstrap/Cargo.toml` |
| Existing service configs | `apps/api-gateway/config.toml` is a literal duplicate of `config.example.toml` (no real IDs); `apps/user-service/config.toml` contains a stale, hand-pasted PAT; neither is tracked in git **but neither is gitignored either** — drift hazard | `apps/api-gateway/config.toml:1`, `apps/user-service/config.toml:1`, `.gitignore:1` |
| dev-up admin PAT | Minted by `bootstrap/steps.yaml` into `pim_zitadel-bootstrap` volume; exported into `.env.local` as `ZITADEL_ADMIN_PAT=...` | Prior bug-fix work, `justfile` `dev-up` recipe |

### Confirmed answers locking the design (from planning Q&A on 2026-04-19)

1. **Dev admin credential → PAT only.** `AdminCredential::JwtProfile` variant must still compile and be tested against a mocked server, but no dev code path exercises it. Prod flips to JWT Profile in a future plan.
2. **Layer A output path → `apps/<name>/config.toml`** (already declared in `bootstrap/dev.toml`). Plan must add these to `.gitignore` and keep `config.example.toml` as the committed template.
3. **`EnsureOp` trait → `async_trait` crate.** No native `async fn in trait` for now; keep `dyn EnsureOp` usable in a `Vec`.
4. **Seed re-run semantics → Ensure + skip password.** If user exists, patch missing `role_assignments` only; never rewrite `initial_password` / `email_verified` / profile fields.
5. **Phase G integration test → lightweight.** CI runs `just dev-up` → `pim-bootstrap bootstrap apply --sync` → `cargo check -p api-gateway -p user-service`. No full service startup, no `/health` poll.

---

## 2. Phase breakdown

Phases land as **separate commits on the current branch** (`docs/adr-0016-0017-phase3-design`). Every phase ends with a green `cargo check --workspace` and, where noted, `cargo test -p <crate>`. Phases B/C/D/E/F MUST NOT merge without Phase A landed first.

### Phase A — `libs/zitadel-rest-client` scaffolding + auth + error taxonomy

**Scope**
- Create crate `libs/zitadel-rest-client` with the layout from ADR-0016 §8 (`lib.rs`, `auth.rs`, `error.rs`, `pagination.rs`; domain modules deferred to Phase A.2).
- `AdminCredential` enum: `Pat(String)` and `JwtProfile { key_json: Vec<u8>, audience: String }`. Constructor helpers: `from_env_pat(var: &str)` and `from_jwt_key_path(path: &Path, audience: &str)`.
- `ZitadelClient::new(authority, AdminCredential) -> Result<Self>`. Internally owns a `reqwest::Client` with: HTTPS **not** required (dev uses `http://pim.localhost:18080`), 30s default timeout, `User-Agent: pim-bootstrap/<crate-version>`. Adds `Authorization: Bearer <token>`; for JWT Profile, mints a fresh `client_assertion` using the existing `zitadel` crate on every request-session (simple implementation first; TTL caching optional in a future plan).
- `ZitadelError` enum per ADR-0016 §5: `Transport`, `BadRequest`, `Unauthenticated`, `PermissionDenied`, `NotFound`, `AlreadyExists`, `InvalidArgument`, `FailedPrecondition`, `ResourceExhausted`, `Internal`, `Unknown(u16)`, `Deserialize`. `From<reqwest::Error>` for `Transport`. Status → variant mapping: 400→BadRequest, 401→Unauthenticated, 403→PermissionDenied, 404→NotFound, 409→AlreadyExists, 412→FailedPrecondition, 429→ResourceExhausted, 5xx→Internal, others→Unknown. 409 MUST NOT be demoted to a warning at this layer — ensure-op layer decides.
- `pagination::PageRequest { offset: u64, limit: u32, asc: bool }` and `Page<T> { items: Vec<T>, total: u64 }`. `ZitadelClient::list_all<T, F>(mut fetch: F) -> Result<Vec<T>, ZitadelError>` where `F: FnMut(PageRequest) -> Future<Output = Result<Page<T>, ZitadelError>>`. Default page size 100, cap at total.

**Deliverables**
- `libs/zitadel-rest-client/Cargo.toml` — deps: `reqwest` (rustls-tls), `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait`, `url`, `zitadel` (JWT minting only, feature-gated to avoid pulling gRPC), `tracing`.
- `libs/zitadel-rest-client/src/{lib,auth,error,pagination}.rs`.
- Unit tests: status-code → `ZitadelError` mapping; `list_all` terminates on short page; `AdminCredential::Pat` produces correct header.
- Workspace `Cargo.toml` member addition.

**Acceptance**
- `cargo check -p zitadel-rest-client` green.
- `cargo test -p zitadel-rest-client` green (target: ≥ 6 unit tests).
- `cargo deny check` stays green.
- No new proto dep; crate does not depend on `rpc-proto` (per copilot-instructions dependency rule).

---

### Phase A.2 — Domain endpoints (20 methods from ADR-0016 §3)

Split from A so Phase A lands a reviewable 300-line PR first. Lives in same crate.

**Scope (authoritative: ADR-0016 §3 table — 20 endpoints, 6 modules)**

- `project.rs` (4):
  - `get_project(id)` — `GET /management/v1/projects/{id}`
  - `list_projects(query)` — `POST /management/v1/projects/_search`
  - `create_project(req)` — `POST /management/v1/projects`
  - `update_project(id, req)` — `PUT /management/v1/projects/{id}`
- `app.rs` (4):
  - `get_app(project_id, app_id)` — `GET /management/v1/projects/{project_id}/apps/{app_id}`
  - `list_apps(project_id, query)` — `POST /management/v1/projects/{project_id}/apps/_search`
  - `create_api_app(project_id, req)` — `POST /management/v1/projects/{project_id}/apps/api`
  - `update_app(project_id, app_id, req)` — `PUT /management/v1/projects/{project_id}/apps/{app_id}`
- `user.rs` (2):
  - `list_users(query)` — `POST /management/v1/users/_search`
  - `create_machine_user(req)` — `POST /management/v1/users/machine`
- `user_key.rs` (3):
  - `list_machine_user_keys(user_id, query)` — `POST /management/v1/users/{user_id}/keys/_search`
  - `add_machine_user_key(user_id, req)` — `POST /management/v1/users/{user_id}/keys`
  - `remove_machine_user_key(user_id, key_id)` — `DELETE /management/v1/users/{user_id}/keys/{key_id}`
- `project_role.rs` (3):
  - `list_project_roles(project_id, query)` — `POST /management/v1/projects/{project_id}/roles/_search`
  - `add_project_role(project_id, req)` — `POST /management/v1/projects/{project_id}/roles`
  - `bulk_add_project_roles(project_id, req)` — `POST /management/v1/projects/{project_id}/roles/_bulk`
- `user_grant.rs` (4):
  - `list_user_grants(query)` — `POST /management/v1/user-grants/_search`
  - `create_user_grant(req)` — `POST /management/v1/user-grants`
  - `update_user_grant(grant_id, req)` — `PUT /management/v1/user-grants/{grant_id}`
  - `remove_user_grant(grant_id)` — `DELETE /management/v1/user-grants/{grant_id}`

**NOT in Phase A.2** — deferred until an amending ADR adds them (per ADR-0016
"New endpoints require an amending ADR"):
- Machine user PAT endpoints. If Phase E seed needs PATs, the seed step
  writes an amending ADR first.
- Human user creation endpoints. Phase E seed uses machine users only or
  takes the same amendment path.

**Deliverables**
- Domain modules behind `pub use` from `lib.rs`.
- Each method: `pub async fn …(&self, …) -> Result<T, ZitadelError>`, takes `&ZitadelClient`, returns strongly-typed response structs.
- Minimal `serde` structs — only fields the ensure-ops need. Extra Zitadel fields are ignored via `#[serde(default)]` / non-exhaustive parsing where possible.
- `wiremock`-based test per module: one happy-path, one 409 mapping. (No live Zitadel in unit tests.)

**Acceptance**
- `cargo test -p zitadel-rest-client` green with ≥ 14 unit tests total (A + A.2). Target ≥ 12 new tests (2 per module × 6 modules).
- Every method signature and endpoint URL has a one-line comment citing ADR-0016 §3 table row.

---

### Phase B — `EnsureOp` trait + driver in `pim-bootstrap`

**Scope**
- New module `tools/pim-bootstrap/src/ensure/mod.rs` defining:
  ```rust
  #[async_trait::async_trait]
  pub trait EnsureOp {
      type Desired;
      type Observed;
      fn name(&self) -> &str;                        // human-readable, e.g. "project:pim"
      async fn observe(&self, c: &ZitadelClient) -> Result<Option<Self::Observed>, ZitadelError>;
      fn classify(&self, obs: Option<&Self::Observed>) -> EnsureState;
      async fn act(&self, state: EnsureState, flags: Flags, c: &ZitadelClient) -> Result<EnsureOutcome, EnsureError>;
  }
  ```
- `EnsureState::{Missing, Match, Drift, Conflict}` and `Flags { sync: bool, rotate_keys: bool }` exactly per ADR-0017 §3 matrix.
- `EnsureOutcome` carries: side-effect summary (`Created { id }`, `Updated { id, fields }`, `NoChange { id }`, `Blocked { reason }`, `SecretsEmitted { count }`) + a structured payload the sink layer (Phase D) consumes.
- Driver `run_pipeline(ops: Vec<Box<dyn EnsureOp<…>>>, flags, client)` — executes in declared order, short-circuits on `EnsureError::Fatal`, collects per-op reports. Unit-testable with a mock client.
- `plan` vs `apply` distinction: `plan` runs `observe` + `classify` only, renders a text table, exits 0. `apply` additionally runs `act`. `--sync` and `--rotate-keys` gate writes exactly as ADR-0017 §3 mandates.

**Deliverables**
- `ensure/mod.rs`, `ensure/driver.rs`, `ensure/state.rs`, `ensure/report.rs`.
- Unit tests using a `MockZitadelClient` trait abstraction (introduced in Phase A behind a `#[cfg(any(test, feature = "mock"))]` flag, OR via `httpmock` against a real `ZitadelClient`; prefer the latter to avoid trait bifurcation).
- README-style doc-comment on `EnsureOp` linking to ADR-0017.

**Acceptance**
- `cargo test -p pim-bootstrap` green.
- `pim-bootstrap bootstrap plan --config bootstrap/dev.toml` runs end-to-end against a running dev Zitadel and prints a 5-row table (one per ensure-op, all rows = Missing on first run).

---

### Phase C — Concrete ensure-ops (project → api-app → SA → roles → grants; grants deferred)

**Scope**

Implement these `EnsureOp` impls in topological order (parent IDs flow into children via the shared pipeline context):

1. `ProjectEnsureOp` — natural key `project.name`. Match on name. Drift detection: none in v1 (name is the only field). Conflict: name collision with different pre-existing project (rare — treat as Drift blocked without `--sync`).
2. `ApiAppEnsureOp` — natural key `(project_id, api_app.name)`. Drift: `auth_method` mismatch. Conflict: app exists but of wrong type (OIDC vs API). Without `--sync`: Drift / Conflict abort with diff. **Key material emission**: on `Missing` creation OR `--rotate-keys`, call `add_app_key` (type JSON, 1 year expiry), capture JSON blob, hand to Phase D sink as `jwt_key_path` payload. One-shot rule enforced.
3. `ServiceAccountEnsureOp` — natural key `service_account.username`. Machine user w/ `access_token_type = JWT`. Drift: description mismatch. No Conflict path in dev.
4. `ProjectRolesEnsureOp` — batch: for each declared role, ensure presence. Missing → create. Match → no-op. Drift (display_name mismatch) → requires `--sync`. Per ADR-0017 §4, one Drift mid-batch does not block the others; all non-drifting members still reach Match.
5. `UserGrantEnsureOp` for the SA (grants SA no roles initially — gateway assigns at request time). **Deferred from v1 scope** unless seed needs it; leave as `todo!()` with a tracking note, ship Phase E's human-seed grants instead.

- Shared pipeline context struct holds resolved IDs (`project_id`, `api_app_id`, `sa_user_id`) between ops.

**Deliverables**
- One module per op under `tools/pim-bootstrap/src/ops/`.
- Each op: ≥ 4 unit tests against `httpmock` — Missing/Match/Drift/Conflict transitions per ADR-0017 §3.
- Hand-off struct documented.

**Acceptance**
- `cargo test -p pim-bootstrap` green with ≥ 20 new unit tests.
- `pim-bootstrap bootstrap apply --config bootstrap/dev.toml` against a fresh Zitadel produces: project, api app with JWT key, service account — verifiable via `pim-bootstrap bootstrap plan` reporting all Match on re-run.

---

### Phase D — Sink layer (TOML render + JSON key + env upsert)

**Scope**
- `sinks/service_config.rs` — renders each declared `service_configs` target. Strategy:
  - Load the existing `apps/<name>/config.toml` if present, else start from `apps/<name>/config.example.toml`.
  - Parse via `toml::Value` (preserve unknown keys). Set `[zitadel]` fields for api-gateway: `authority`, `key_file` (relative path to `zitadel-key.json`), `project_id`, `api_app_id`. For user-service: `zitadel_authority`, `zitadel_project_id`, `zitadel_sa_user_id`, **and**  `zitadel_service_account_token` **intentionally left absent** (populated out-of-band via env — existing user-service config expects this env var, NOT a hand-pasted token).
  - Atomic write: temp file + rename. Mode 0600. Create parent dir if missing.
- `sinks/jwt_key.rs` — writes the captured JSON blob to `outputs.jwt_key_path`. First-run behavior: refuse if file exists and `--rotate-keys` not set; `--rotate-keys` overwrites atomically. Mode 0600.
- `sinks/env_file.rs` — idempotent upsert into `outputs.env_file_path`:
  - Parse as line-oriented `KEY=VALUE`.
  - For each emitted symmetric secret (PAT for admin in prod path; NOT exercised in dev, but implement once), replace in-place or append. Preserve unrelated lines.
  - Mode 0600. Warn (not error) if file already has `KEY=` with a different value and `--sync` not set.
- `.gitignore` additions: `apps/*/config.toml`, `zitadel-key.json`, `.env.local`. These MUST land in Phase D's commit — running Phase C+D without this gitignore change would risk committing secrets.

**Deliverables**
- `sinks/` module + three files + a `render(report: PipelineReport, outputs: &OutputsConfig) -> Result<()>` entry point called after a successful `apply`.
- `.gitignore` patch.
- Integration-style tests using `tempfile::TempDir` to verify: fresh render, re-render is a no-op byte-for-byte, `--rotate-keys` overwrites JSON key, env upsert preserves unrelated lines.

**Acceptance**
- `cargo test -p pim-bootstrap` green.
- Running `just dev-bootstrap` on a fresh clone produces:
  - `apps/api-gateway/config.toml` with non-empty `[zitadel].project_id` and `api_app_id`.
  - `apps/user-service/config.toml` with non-empty `zitadel_project_id`.
  - `zitadel-key.json` (600 perms, valid JSON, has `keyId`/`key` fields).
  - `.env.local` still contains `ZITADEL_MASTERKEY` and `ZITADEL_ADMIN_PAT` from dev-up.

---

### Phase E — `seed apply` (human users + role assignments)

**Scope**
- Implement `SeedApplyCommand::run`.
- Gate: `SeedConfig.env != "dev"` → abort with clear error (already scaffolded; verify).
- For each `[[users]]`: `HumanUserEnsureOp`. Match on `username`. State machine:
  - Missing → create with `initial_password` + `email_verified` from seed.
  - Match (any existing user) → **skip password, skip profile**. Do not rewrite anything.
  - Drift / Conflict → log warning, continue.
- For each `[[role_assignments]]`: `UserGrantEnsureOp`. Match on `(user_id, project_id)`. Missing → create with declared roles. Match → verify role set equal; unequal → Drift, require `--sync` to patch.
- No output sinks — seed is purely Zitadel-side.
- Uses same pipeline driver from Phase B.

**Deliverables**
- `ops/human_user.rs`, finalize `ops/user_grant.rs`.
- `main.rs:85` stub replaced.
- Unit tests: first-run creates, second-run is no-op, added role on second run gets picked up under `--sync`.

**Acceptance**
- `cargo test -p pim-bootstrap` green.
- `just dev-seed` (new justfile recipe — add in this phase) runs cleanly twice in a row; second run reports all Match.

---

### Phase F — `diff` subcommand

**Scope**
- `pim-bootstrap diff --config bootstrap/dev.toml` → read-only pipeline (like `bootstrap plan` but outputs structured JSON by default; `--human` for text table).
- Exit code: 0 if all Match, 2 if any Missing/Drift/Conflict. Makes it CI-friendly.
- Re-uses Phase B driver with `act` disabled.

**Deliverables**
- `main.rs` diff arm replaced.
- Exit-code tests via `assert_cmd`.

**Acceptance**
- `cargo test -p pim-bootstrap` green.
- On clean dev env: `just dev-bootstrap && pim-bootstrap diff --config bootstrap/dev.toml` → exit 0.

---

### Phase G — Lightweight integration smoke + docs

**Scope**
- `justfile`: add `dev-smoke` recipe that runs `dev-up` → `dev-bootstrap` → `cargo check -p api-gateway -p user-service`.
- CI workflow file (GitHub Actions): `.github/workflows/bootstrap-smoke.yml`. Matrix: ubuntu-latest only. Steps: checkout → install rust stable → install `just` → `just dev-up` → `just dev-bootstrap` → `cargo check`. Tear down via `just dev-reset` in `always()`. Budget: 8 min.
- Update `docs/design.md` §bootstrap (or create the section) to describe the new flow, linking ADR-0016, ADR-0017, and this plan's resulting ADRs if any amendments surfaced.
- Add a `tools/pim-bootstrap/README.md` with the five-line quickstart, matrix of flags, and a pointer to ADR-0017.

**Deliverables**
- `.github/workflows/bootstrap-smoke.yml`.
- `docs/design.md` section.
- `tools/pim-bootstrap/README.md`.

**Acceptance**
- CI green on a PR that runs the new workflow.
- Reviewer can go from a fresh clone to running services using only the README + `just dev-smoke`.

---

## 3. Risks & mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Zitadel v4.13 Management API shape differs subtly from what ADR-0016 §3 assumes | Phase A.2 blocked | Write one `httpmock` fixture per endpoint using real response captured from dev Zitadel in Phase A.2 kickoff; correct the serde structs before writing ops. |
| `zitadel` crate pulls in tonic transitively, bloating `zitadel-rest-client` | Longer compile, deny violations | Feature-gate the `zitadel` crate to minting-only; if infeasible, hand-roll JWT assertion with `jsonwebtoken` crate (already in workspace, check). |
| Phase D overwrites user-modified `apps/*/config.toml` | Lost local edits | Preserve unknown keys via `toml::Value` round-trip; only set the specific fields the bootstrap owns; document in README. |
| `.gitignore` addition lands _after_ someone runs Phase C+D → secrets committed | Leaked PATs/keys | Land `.gitignore` patch in Phase D's **first** commit before any sink is wired up; Phase D PR gate: reviewer confirms `.gitignore` is in the diff. |
| `async_trait` creates object-safety friction down the road | Future refactor cost | Document the choice in an inline comment pointing at this plan; native `async fn in trait` is a drop-in migration when `dyn` gets full support. |
| Integration test in Phase G is flaky (Zitadel container start race) | CI red | Reuse the polling logic from `just dev-up` (already fixed bug 3); add a 120s outer timeout. |

## 4. Out of scope (explicit)

- Prod bootstrap flow (`env = "prod"`, JWT Profile dev admin, separate seed file).
- `pim-bootstrap bootstrap destroy` / cleanup command.
- Drift auto-repair without human-provided `--sync`.
- Reconciliation loop / continuous controller (one-shot CLI only).
- Renaming existing `apps/<name>/config.toml` → `config.local.toml` (rejected: bootstrap/dev.toml already pins these paths; changing them is a separate, ADR-worthy refactor).

## 5. Success criterion (overall)

A contributor clones the repo at this plan's final commit, runs:

```bash
just dev-up
just dev-bootstrap
cargo run -p api-gateway
```

and the gateway starts, authenticates to Zitadel via the freshly minted JWT key, and serves `/health` on `http://127.0.0.1:8080/health` returning 200 without any manual file editing.

## 6. Phase checkboxes

- [x] Phase A — `libs/zitadel-rest-client` scaffolding + auth + error taxonomy
- [x] Phase A.2 — Domain endpoints (20 methods)
- [x] Phase B — `EnsureOp` trait + driver
- [x] Phase C — Concrete ensure-ops (project → api-app → SA → roles)
- [x] Phase D — Sink layer + `.gitignore` patch
- [ ] Phase E — `seed apply`
- [ ] Phase F — `diff` subcommand
- [ ] Phase G — Smoke test + docs

When all boxes are checked and Success criterion verified, delete this file (per AGENTS.md §2.2).
