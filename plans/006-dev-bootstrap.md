# Dev Bootstrap Plan — Local Zitadel + Idempotent Provisioning

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a fully local, podman-friendly Zitadel stack for PIM development, plus a single Rust-based provisioning tool that performs **idempotent bootstrap** (projects, API apps, JWT keys, service account PAT, roles) for both dev and prod, and a **dev-only seed** step (test users, role assignments).

**Non-goals:**
- Productionising the Zitadel deployment itself (HA, backups, TLS termination) — out of scope
- Replacing Zitadel Cloud for prod — prod topology deferred; this plan only ensures prod bootstrap *semantics* are honoured by the same tool
- PIM-owned database seeds — PIM has no dedicated DB yet; revisit when it does

---

## Architectural decisions (now tracked as ADRs)

The original draft of this plan carried a D1–D16 decision table and an idempotency contract inline. Those have been extracted into standalone ADRs under `docs/decisions/` and this plan no longer duplicates them. Consult the ADRs as the authoritative record:

- **ADR-0005** `docs/decisions/0005-bootstrap-local-zitadel-with-pim-bootstrap-tool.md` — tool shape, justfile entrypoint, create-or-skip/`--sync`/`--rotate-keys`/`--dry-run` semantics, compose deviations from upstream Zitadel pack (Login disabled, Traefik dropped, `pim.localhost:18080`), PAT-via-FirstInstance admin credential (supersedes the RSA machinekey approach), `bootstrap/steps.yaml` over env-var overrides, full idempotency contract per resource.
- **ADR-0012** `docs/decisions/0012-three-layer-config-split-by-sensitivity.md` — Layer A service `config.toml` / Layer B non-symmetric key files (`zitadel-key.json`) / Layer C symmetric secret strings (`.env.local`) and the `stdout:<tag>` sink DSL for prod.
- **ADR-0013** `docs/decisions/0013-dev-prod-parity-via-declarative-bootstrap.md` — same binary/config schema drives dev-self-hosted and prod-Zitadel-Cloud; drift is exposed via `pim-bootstrap diff` and invoked manually, not enforced in CI.

When implementing the remaining phases, treat those ADRs as the binding contract. If an implementation forces a decision to change, update the ADR first (or alongside) per `AGENTS.md`.

---

## Context

### Current State (snapshot used to author the original plan)

- `libs/infra-auth` is a thin re-export of `zitadel::actix::introspection::{IntrospectedUser, IntrospectionConfig, IntrospectionConfigBuilder}` and `zitadel::credentials::Application`. The IdP-replaceable abstraction is already in place.
- `apps/api-gateway` consumes `ZitadelSettings { authority, key_file }`, builds `IntrospectionConfigBuilder::with_jwt_profile(...)` at startup. Token validation uses the `IntrospectedUser` extractor on protected handlers.
- `apps/user-service` is implemented as a Zitadel Management API v2 proxy and expects `USER_SERVICE__ZITADEL_AUTHORITY` + `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN` in its environment.
- Repo-root `compose.yml` now declares `zitadel` + `zitadel-db` alongside the PIM services (Phase 1 output).
- `zitadel-key.json` is gitignored; `.gitignore:53-64` enforces Layer A/B/C exclusions from ADR-0012.

### Target State

- `podman compose up -d` (or `docker compose up -d`) brings up: `zitadel` + `zitadel-db` (Postgres) + `user-service` + `api-gateway`, all healthchecked and wired.
- A single Rust binary `tools/pim-bootstrap` reads a declarative TOML config and performs idempotent provisioning of every Zitadel resource PIM needs (project, API app + JWT key, service account + PAT, roles). Same binary has a `seed` subcommand, dev-only, for test humans and role assignments.
- One-command developer onboarding: `just dev-up` → brings stack up, runs bootstrap, runs seed, prints next-step hints.
- One-command full reset: `just dev-reset` → tears volumes, re-bootstraps, re-seeds.

---

## Phase 1 — Local Compose Stack (podman-first)

**Status:** Task 1.1 ✅ and Task 1.2 ✅ (smoke test green 2026-04-18). Outputs landed in `compose.yml`, `bootstrap/steps.yaml`, `.env.local.example`, `.gitignore`. Remaining work is Task 1.3 (the `justfile` and `bootstrap/wait-for-zitadel.sh`), which is now tracked under Phase 5 Task 5.1 to keep justfile work in one place. See ADR-0005 for the compose shape and PAT-dispense mechanism.

---

## Phase 2 — `tools/pim-bootstrap` Crate Skeleton

**Status:** ✅ Complete — Task 2.1 ✅, Task 2.2 ✅ (all subcommands dry-run green 2026-04-18). The crate exists at `tools/pim-bootstrap/` with `cli.rs`, `config.rs`, `lib.rs`, `main.rs`; subcommand surface and config schema match ADR-0005 §Decision and ADR-0012 §Decision. No further scaffolding work remains here — subsequent phases add real behaviour behind the skeleton.

---

## Phase 3 — Idempotent Bootstrap Operations

### Task 3.1: Zitadel API client module

**Files:**
- Create: `tools/pim-bootstrap/src/zitadel_client.rs`

- [ ] **Step 1: Implement typed client**

Provide thin typed wrappers per-resource. Each operation returns `Result<Option<Resource>>` for lookups, `Result<Resource>` for creates. Authenticate via one of two modes, selected by `bootstrap/*.toml` `[zitadel] admin_auth` field:

- `admin_auth = "pat"` (dev): add `Authorization: Bearer <ZITADEL_ADMIN_PAT>` header to every request. PAT comes from the env var named in `admin_pat_env_var`.
- `admin_auth = "jwt_profile"` (prod): use the Zitadel `zitadel::credentials::Application` JWT Profile flow with the operator-supplied key file (same primitives as the gateway — no custom JWT signing).

Operations needed:

- `GET /management/v1/projects/_search` filtered by name
- `POST /management/v1/projects`
- `GET /management/v1/projects/{id}/apps/_search` filtered by name
- `POST /management/v1/projects/{id}/apps/api` with `authMethodType: API_AUTH_METHOD_TYPE_PRIVATE_KEY_JWT`
- `POST /management/v1/projects/{id}/apps/{appId}/keys` (emits JWT key; download-once)
- `GET /management/v1/users/_search` filtered by username (for service account detection)
- `POST /management/v1/users/machine`
- `POST /management/v1/users/{id}/pats` (emits PAT; one-time reveal)
- `GET /management/v1/projects/{id}/roles/_search`
- `POST /management/v1/projects/{id}/roles`

- [ ] **Step 2: Unit tests with mock server**

Use `wiremock` for HTTP mocking. Cover: 200 OK parse, 404 not-found mapping to `Ok(None)`, 409 conflict handling, auth failure surfacing.

- [ ] **Step 3: Commit**

```bash
git add tools/pim-bootstrap/src/zitadel_client.rs tools/pim-bootstrap/Cargo.toml
git commit -m "feat(bootstrap): typed Zitadel Management API client with wiremock tests"
```

### Task 3.2: Idempotent provisioning pipeline

**Files:**
- Create: `tools/pim-bootstrap/src/ops/mod.rs`
- Create: `tools/pim-bootstrap/src/ops/project.rs`
- Create: `tools/pim-bootstrap/src/ops/api_app.rs`
- Create: `tools/pim-bootstrap/src/ops/service_account.rs`
- Create: `tools/pim-bootstrap/src/ops/roles.rs`
- Create: `tools/pim-bootstrap/src/outcome.rs`

- [ ] **Step 1: Define outcome type**

`Outcome { Created, Unchanged, Updated, DryRunWouldCreate, DryRunWouldUpdate, Skipped { reason } }`. Every op returns one; the top-level aggregator tallies. Rules per outcome match ADR-0005 §Idempotency contract.

- [ ] **Step 2: Implement each op as lookup→branch→persist**

Example contract for `ensure_api_app`:

```rust
pub async fn ensure_api_app(
    client: &ZitadelClient,
    project_id: &str,
    spec: &ApiAppSpec,
    opts: &OpOptions,
) -> Result<(Outcome, ApiAppState)>;

pub struct OpOptions { pub sync: bool, pub rotate_keys: bool, pub dry_run: bool }
pub struct ApiAppState { pub app_id: String, pub jwt_key_file_path: Option<PathBuf> }
```

- [ ] **Step 3: Secret-persist-exactly-once discipline (per ADR-0012)**

Three classes of output, three disciplines:

- **Layer A — non-secret values** (project ID, app ID, authority URL) → write into the relevant `apps/<svc>/config.toml` via `outputs.service_configs`. Safe to overwrite on every run; these are derivable from Zitadel state.
- **Layer B — JWT app key** (non-symmetric, file) → write to `outputs.jwt_key_path`. If target file already has matching `keyId`, no-op. If file missing but app exists in Zitadel → error unless `--rotate-keys`.
- **Layer C — service account PAT** (symmetric, string) → upsert into `outputs.env_file_path` as a `KEY=VALUE` line. Same rule as JWT key: missing-but-resource-exists → error unless `--rotate-keys`.

Never write a secret to a service `config.toml`. Never write a non-secret to `.env.local`. The layering is load-bearing (ADR-0012).

- [ ] **Step 4: Wire ops into `bootstrap` command**

Top-level flow: ensure_project → ensure_roles → ensure_api_app → ensure_service_account → ensure_service_account_pat. Print a structured summary table at end.

- [ ] **Step 5: Integration test against real Zitadel**

Add `tools/pim-bootstrap/tests/bootstrap_dev.rs` that expects `PIM_BOOTSTRAP_TEST=1` env and a running local stack. Runs full bootstrap twice, asserts second run is entirely `Unchanged`.

- [ ] **Step 6: Commit**

```bash
git add tools/pim-bootstrap/
git commit -m "feat(bootstrap): idempotent ops for project, api-app, service-account, roles"
```

---

## Phase 4 — Dev Seed Subcommand

### Task 4.1: Human user + role assignment ops

**Files:**
- Create: `tools/pim-bootstrap/src/ops/humans.rs`
- Create: `tools/pim-bootstrap/src/ops/role_assign.rs`

- [ ] **Step 1: Prod guard**

`seed` subcommand validates `config.env == Environment::Dev`. Any other value → immediate error with exit code 3 and message: `"seed refuses to run outside dev — refusing to provision dummy passwords in prod"`.

- [ ] **Step 2: Ensure humans**

Lookup by username. If absent → `POST /management/v1/users/human` with `initial_password` and `email_verified`. If present → skip (never reset a user's password; dev passwords are one-shot on creation).

Document this loudly: "seed initial passwords only apply to newly-created users. To reset, wipe the Zitadel DB (`just dev-reset`) or change the password manually."

- [ ] **Step 3: Ensure role grants**

Lookup existing grants via `POST /management/v1/users/{id}/grants/_search`. Compute diff against declared; add missing. `--sync` removes undeclared ones; default leaves them alone.

- [ ] **Step 4: Commit**

```bash
git add tools/pim-bootstrap/src/ops/
git commit -m "feat(bootstrap): dev seed ops (humans + role grants) with prod guard"
```

---

## Phase 5 — Developer Ergonomics

### Task 5.1: justfile entrypoints

**Files:**
- Create: `justfile`

- [ ] **Step 1: Author recipes**

```
default:
    @just --list

# Bring up local Zitadel + Postgres (no PIM services yet)
dev-infra-up:
    podman compose up -d zitadel-db zitadel
    ./bootstrap/wait-for-zitadel.sh

# Run bootstrap tool against local stack
dev-bootstrap: dev-infra-up
    cargo run -p pim-bootstrap -- bootstrap --config bootstrap/dev.toml

# Run dev seed
dev-seed:
    cargo run -p pim-bootstrap -- seed --config bootstrap/seed.dev.toml --env dev

# Full dev up: infra + bootstrap + PIM services + seed
dev-up: dev-bootstrap
    podman compose up -d user-service api-gateway
    just dev-seed

# Drift check against local dev stack (no changes)
dev-diff:
    cargo run -p pim-bootstrap -- diff --config bootstrap/dev.toml

# Drift check against prod (operator-invoked, not CI; see ADR-0013).
# Requires PIM_PROD_ADMIN_KEY to point at an admin key JSON provided
# by the operator's secret manager at invocation time.
prod-diff:
    cargo run -p pim-bootstrap -- diff --config bootstrap/prod.toml

# Full teardown + wipe (clears Postgres volume + all three ADR-0012 output classes +
# ZITADEL_MASTERKEY + ZITADEL_ADMIN_PAT, so next `dev-up` regenerates everything).
dev-reset:
    podman compose down -v
    rm -f zitadel-key.json .env.local
    rm -f apps/api-gateway/config.toml apps/user-service/config.toml
    @echo "Run 'just dev-up' to reinitialise from scratch."
```

> Per ADR-0005, there is no separate "regenerate machinekey" recipe. The admin PAT is dispensed by Zitadel at first boot, and the masterkey is generated by `dev-up` on demand — both are wiped by `dev-reset` and re-minted on the next `dev-up`.

- [ ] **Step 2: Author `bootstrap/wait-for-zitadel.sh`**

Polls `http://localhost:8080/debug/healthz` with exponential backoff, times out at 60s, fails loudly.

- [ ] **Step 3: Commit**

```bash
git add justfile bootstrap/wait-for-zitadel.sh
git commit -m "feat(dev): justfile recipes for bootstrap, seed, reset"
```

### Task 5.2: Documentation sync

**Files:**
- Modify: `docs/design.md`
- Modify: `docs/configuration.md`
- Modify: `README.md`
- Create: `docs/dev-bootstrap.md`

- [ ] **Step 1: Author `docs/dev-bootstrap.md`**

New page explaining:

- Prerequisites (podman, just, cargo)
- First-time setup (`cp .env.local.example .env.local` → `just dev-up`; masterkey and admin PAT auto-mint)
- Daily loop (`just dev-up`, `just dev-diff`)
- How to reset state
- How to add a new project/app/role (edit `bootstrap/dev.toml`, run `just dev-bootstrap` — idempotent)
- How to add a test user (edit `bootstrap/seed.dev.toml`, run `just dev-seed`)
- Prod bootstrap contract (same binary, different config, secret outputs redirected — see ADR-0013)
- Troubleshooting matrix

- [ ] **Step 2: Cross-link from `README.md` and `docs/design.md`**

Add a "Development" section to README pointing to `docs/dev-bootstrap.md`. In `docs/design.md` section 7 (Future Evolution), move "Dev bootstrap" from future to done.

- [ ] **Step 3: Update `docs/configuration.md`**

Document the ADR-0012 three-layer configuration convention and state which app needs which layers (api-gateway: A+B; user-service: A+C).

- [ ] **Step 4: Update `AGENTS.md` if the `/docs/` map changes**

Add `docs/dev-bootstrap.md` entry if the documentation map references specific pages.

- [ ] **Step 5: Commit**

```bash
git add docs/ README.md AGENTS.md
git commit -m "docs: dev bootstrap workflow and design updates"
```

---

## Phase 6 — Secret Hygiene and Rotation

### Task 6.1: Rotation workflow

**Files:**
- Modify: `docs/dev-bootstrap.md`
- Modify: `tools/pim-bootstrap/src/ops/api_app.rs` (verify `--rotate-keys` path)
- Modify: `tools/pim-bootstrap/src/ops/service_account.rs` (verify `--rotate-keys` path)

- [ ] **Step 1: Document rotation recipes**

In `docs/dev-bootstrap.md` add section "Rotating secrets":

- JWT app key: `cargo run -p pim-bootstrap -- bootstrap --config bootstrap/dev.toml --rotate-keys --sync` → revokes old key via `DELETE /management/v1/projects/{id}/apps/{appId}/keys/{keyId}`, creates new, overwrites `zitadel-key.json`.
- PAT: `--rotate-keys` also revokes the old PAT and writes a new one to `.env.local`.
- Masterkey / admin PAT: teardown-required (see ADR-0005). `just dev-reset` wipes them along with the Postgres volume; next `just dev-up` mints fresh values.

- [ ] **Step 2: Verify rotation ops and test**

Integration test: bootstrap → capture keyId → bootstrap --rotate-keys → assert new keyId differs, old key rejects auth against Zitadel.

- [ ] **Step 3: Commit**

```bash
git add tools/pim-bootstrap/ docs/
git commit -m "feat(bootstrap): secret rotation via --rotate-keys, documented"
```

### Task 6.2: Final hygiene audit

- [ ] **Step 1: Grep for accidental secret commits**

```bash
git grep -n "BEGIN RSA PRIVATE KEY" || true
git grep -n "keyId\":" -- '*.json' || true
git ls-files | grep -E '(zitadel-key\.json|\.env\.local|machinekey\.json)$' || true
```

All three must return empty.

- [ ] **Step 2: Delete legacy committed `zitadel-key.json` if still tracked**

```bash
git rm --cached zitadel-key.json 2>/dev/null || true
```

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: remove tracked dev key, rely on gitignored regeneration" --allow-empty
```

---

## Acceptance Criteria

### Phase 1 & 2
Already satisfied (see Status). Remaining acceptance checks for those phases that are still outstanding (justfile + wait-for-zitadel.sh) are absorbed into Phase 5 Task 5.1.

### Phase 3
- [ ] Fresh Zitadel → `pim-bootstrap bootstrap --config bootstrap/dev.toml` succeeds, emits `zitadel-key.json` and `.env.local`
- [ ] Second run without flags reports all `Unchanged`
- [ ] Run with `--sync` after editing a role's display name reports `Updated` for that role only
- [ ] `--dry-run` on clean state exits 0, on drift exits 2
- [ ] Deleting `zitadel-key.json` between runs causes a clear error instructing `--rotate-keys`

### Phase 4
- [ ] `pim-bootstrap seed --config bootstrap/seed.dev.toml --env dev` creates alice/bob/charlie, assigns roles
- [ ] Second run reports users unchanged (passwords not reset)
- [ ] `pim-bootstrap seed --env prod` refuses with exit code 3

### Phase 5
- [ ] `just dev-up` on a clean checkout completes successfully
- [ ] `just dev-reset && just dev-up` recreates a working stack
- [ ] `just dev-diff` on a converged stack reports no drift

### Phase 6
- [ ] `--rotate-keys` regenerates JWT app key and PAT; old credentials fail auth
- [ ] No secret files tracked by git

---

## Status

| Phase | Status |
|-------|--------|
| Phase 1: Local compose stack | In Progress — Task 1.1 ✅, Task 1.2 ✅; Task 1.3 (justfile) folded into Phase 5 |
| Phase 2: `pim-bootstrap` skeleton | ✅ Complete — all subcommands dry-run green 2026-04-18 |
| Phase 3: Idempotent bootstrap ops | Not Started |
| Phase 4: Dev seed | Not Started |
| Phase 5: Developer ergonomics | Not Started |
| Phase 6: Secret hygiene and rotation | Not Started |
