# Dev Bootstrap Plan — Local Zitadel + Idempotent Provisioning

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a fully local, podman-friendly Zitadel stack for PIM development, plus a single Rust-based provisioning tool that performs **idempotent bootstrap** (projects, API apps, JWT keys, service account PAT, roles) for both dev and prod, and a **dev-only seed** step (test users, role assignments).

**Non-goals:**
- Productionising the Zitadel deployment itself (HA, backups, TLS termination) — out of scope
- Replacing Zitadel Cloud for prod — prod topology deferred; this plan only ensures prod bootstrap *semantics* are honoured by the same tool
- PIM-owned database seeds — PIM has no dedicated DB yet; revisit when it does

---

## Context

### Current State (verified on 2026-04-17)

- `libs/infra-auth` is a thin re-export of `zitadel::actix::introspection::{IntrospectedUser, IntrospectionConfig, IntrospectionConfigBuilder}` and `zitadel::credentials::Application`. **The IdP-replaceable abstraction is already in place.**
- `apps/api-gateway` consumes `ZitadelSettings { authority, key_file }`, builds `IntrospectionConfigBuilder::with_jwt_profile(...)` at startup. Token validation uses the `IntrospectedUser` extractor on protected handlers.
- `apps/user-service` is implemented as a Zitadel Management API v2 proxy and expects `USER_SERVICE__ZITADEL_AUTHORITY` + `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN` in its environment.
- `docker-compose.yml` at the repo root only declares `user-service` and `api-gateway`. **No Zitadel, no Postgres, no bootstrap tooling.**
- `zitadel-key.json` exists at the repo root. **It is NOT covered by `.gitignore`** (security hazard, must be fixed in Phase 6).
- Default `ZitadelSettings::authority` is the placeholder `https://localhost.zitadel.cloud`, which implies the team has been running against a real Zitadel Cloud tenant so far.

### Target State

- `podman compose up -d` (or `docker compose up -d`) brings up: `zitadel` + `zitadel-db` (Postgres) + `user-service` + `api-gateway`, all healthchecked and wired.
- A single Rust binary `tools/pim-bootstrap` reads a declarative TOML config and performs idempotent provisioning of every Zitadel resource PIM needs:
  - Project
  - API Application (JWT Profile) → emits `zitadel-key.json`
  - Service Account + PAT for `user-service` → emits `.env.local`
  - Role definitions
- Same binary has a `seed` subcommand, dev-only, that creates test humans and assigns roles.
- One-command developer onboarding: `just dev-up` → brings stack up, runs bootstrap, runs seed, prints next-step hints.
- One-command full reset: `just dev-reset` → tears volumes, re-bootstraps, re-seeds.

### Key Design Decisions (confirmed with user)

| # | Decision |
|---|----------|
| D1 | Bootstrap tool is a Rust workspace crate at `tools/pim-bootstrap`. |
| D2 | Entrypoint surface is a `justfile` at repo root. |
| D3 | **(Superseded by D12.)** Dev admin credential was originally going to use a pre-generated RSA machinekey via `ZITADEL_FIRSTINSTANCE_MACHINEKEYS`. Replaced after verifying Zitadel docs: FIRSTINSTANCE can dispense a PAT directly, which is strictly simpler. |
| D4 | `bootstrap` subcommand defaults to **create-or-skip**. `--sync` opts into update-on-drift. `--rotate-keys` opts into key regeneration. `--dry-run` exits 2 if changes would be made. |
| D5 | Bootstrap provisions: Project, API App + JWT key, Service Account + PAT, Role definitions. Seed provisions: test humans + role assignments. |
| D6 | Same binary, same code paths, for dev and prod — only the input config and output sinks differ. Seed refuses to run against a `prod` profile. |
| D7 | Dev `ZITADEL_EXTERNALDOMAIN` uses a `.localhost` subdomain (`pim.localhost`) for friendlier OIDC redirect testing with future frontends (browsers route `*.localhost` to loopback per RFC 6761). |
| D8 | Prod target is **Zitadel Cloud**. `bootstrap/prod.example.toml` points at a Cloud authority; admin credentials are supplied by the operator's secret manager at invocation time, never by this repo. |
| D9 | Drift detection is **not** wired into CI initially. The `pim-bootstrap diff` subcommand exists for operator use via a `just prod-diff` recipe, intended to run pre-release and on a periodic cadence. Revisit CI integration once prod topology is live and a secret-injection story exists. |
| D10 | **Configuration is layered by sensitivity, not unified**: (a) non-secret config (URLs, ports, paths) → `apps/<svc>/config.toml` (gitignored, committed as `.example.toml`); (b) non-symmetric key material (`zitadel-key.json`) → standalone gitignored file referenced by path in TOML; (c) symmetric secret strings (PAT, masterkey) → `.env.local` consumed via `env_file` in compose and `APP__*__*` env var convention in Rust. Reason: no single file type is right for all three sensitivities; the three sinks have disjoint responsibilities. |
| D11 | Compose topology is **derived from Zitadel's official compose pack** (Traefik + Zitadel API + Postgres) with the **Login service disabled** for now. Traefik is kept because Management Console needs gRPC-Web routing. Login service is deferred until a PIM frontend actually needs OIDC redirects — it can be re-enabled later with minimal churn. |
| D12 | **Supersedes D3.** Dev admin credential for `pim-bootstrap` is a PAT dispensed by Zitadel at first boot via `ZITADEL_FIRSTINSTANCE_ORG_MACHINE_*` env vars. Zitadel writes the PAT into a mounted volume on first startup. `pim-bootstrap` authenticates to the Management API with `Authorization: Bearer <PAT>`. This eliminates the need for an RSA machinekey generator, a Zitadel JSON-key schema dance, and any chance of schema drift. Prod Cloud keeps the JWT Profile path (operator provides key file out-of-band). |
| D13 | `ZITADEL_MASTERKEY` (32 chars, immutable post-init) is **generated by `just dev-up` on first run** and persisted to `.env.local`. Subsequent invocations detect and reuse it. `dev-reset` wipes it along with the Postgres volume so the next run gets a fresh instance with a fresh masterkey. |
| D14 | FirstInstance setup is driven by **`bootstrap/steps.yaml` passed via `--steps` CLI flag**, not by `ZITADEL_FIRSTINSTANCE_*` env vars. Reason: `FirstInstance` is not in `defaults.yaml` (it's a steps-file-only section per the comment at `cmd/defaults.yaml:997`), so env-var overrides rely on undocumented name mangling. YAML is self-documenting, matches Zitadel's official recommendation, and gives clearer schema errors on misconfig. The correct field name (verified against `cmd/setup/03.go` in Zitadel v4.13.0 and proven by smoke test on 2026-04-18) is **`FirstInstance.PatPath`**, not `MachinePatPath`. |
| D15 | **Traefik is dropped from the dev compose stack.** The official Zitadel pack wires Traefik via `/var/run/docker.sock` for dynamic service discovery, which is incompatible with podman rootless on macOS (the CNCF user socket lives at `/run/user/<uid>/podman/podman.sock` and Traefik's root process cannot read it; the VM-internal symlink at `/var/run/docker.sock` points at the unreadable path). Rather than sidegrade podman to rootful or use `--userns=keep-id`, we expose `zitadel-api` directly via a host port publish. This also eliminates the need for Management Console gRPC-Web routing in dev since we authenticate via PAT + REST, not the Console. Re-introduce a proxy layer only when a PIM frontend actually needs unified `pim.localhost` routing across services. |
| D16 | **Default dev host port is 18080**, not 8080. Reason: 8080 is commonly claimed by other local Node/Java services on developer machines (observed conflict with a coworker project's NestJS server on first smoke test). 18080 is high-numbered and less collision-prone while still memorable. `ZITADEL_EXTERNALPORT=18080` matches the host publish so Zitadel's self-generated issuer URLs resolve to the correct callable endpoint. |

### Idempotency Contract

The bootstrap tool must honour these invariants per resource:

1. **Lookup by stable natural key first** (project name, app name, service account username).
2. If missing → create → write any returned secret (key, PAT) to disk/stdout exactly once.
3. If present → skip (default). If `--sync` and drift detected → update in place.
4. If present and a secret-bearing resource (JWT key, PAT) where the secret was not previously persisted → treat as a **partial-provisioning failure** and surface a clear error instructing `--rotate-keys`.
5. Exit codes: `0` success (whether changed or not), `2` dry-run-with-planned-changes, non-zero success codes reserved for tooling, any other non-zero is failure.

---

## Phase 1 — Local Compose Stack (podman-first)

### Task 1.1: Gitignore hygiene and masterkey provisioning

**Files:**
- Modify: `.gitignore`
- Create: `.env.local.example`

- [ ] **Step 1: Add gitignore entries FIRST (security hygiene)** ✅ *completed 2026-04-17*

Appended to `.gitignore`:

```
# Zitadel secrets (never commit)
zitadel-key.json
**/zitadel-key.json
bootstrap/*.machinekey.json
.env.local
**/.env.local
```

Verified: `git check-ignore zitadel-key.json bootstrap/dev.machinekey.json .env.local` prints all three paths. `zitadel-key.json` is untracked in the working tree (never was committed) — no `git rm --cached` needed.

- [ ] **Step 2: Author `.env.local.example`**

Create a committed template showing every env var the compose stack consumes, with placeholder values and comments:

```dotenv
# Generated by `just dev-up` on first run; do NOT commit actual .env.local.
# Values marked [GENERATED] are produced automatically; values marked [STATIC]
# have safe dev defaults and can be overridden.

# Zitadel masterkey — 32 chars, immutable after first init. [GENERATED]
ZITADEL_MASTERKEY=

# PAT for the FirstInstance admin machine user; dispensed by Zitadel on
# first boot into a bind-mounted file, then read by `just dev-up` and
# written here. [GENERATED]
ZITADEL_ADMIN_PAT=

# PAT for the user-service service account; written by `pim-bootstrap`. [GENERATED]
USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN=
```

- [ ] **Step 3: Commit**

```bash
git add .gitignore .env.local.example
git commit -m "chore(bootstrap): gitignore Zitadel secrets, add env template"
```

> **Note:** The earlier plan draft included a `bootstrap/generate-machinekey.sh` for a pre-generated RSA machinekey. That approach was superseded by D12 after verifying Zitadel docs — FIRSTINSTANCE dispenses a PAT directly, so no RSA key material is needed for dev admin auth.

### Task 1.2: compose.yml derived from Zitadel's official pack (Login disabled)

**Files:**
- Delete: `docker-compose.yml`
- Create: `compose.yml` (compose v2 canonical; `docker compose` and `podman compose` both honour it)

**Source baseline:** `https://github.com/zitadel/zitadel/tree/main/deploy/compose` (pinned tags: `ZITADEL_VERSION=v4.13.0`, `traefik:v3.6.8`, `postgres:17.2-alpine`).

**Deviations from the official pack (per D11, D15, D16):**
1. `zitadel-login` service is removed entirely.
2. **`traefik` service is removed entirely (per D15).** All Traefik labels on `zitadel-api` are dropped; the API is exposed to the host via a direct port publish. This sidesteps the podman-rootless docker-socket mount incompatibility and keeps the dev stack reachable without a proxy layer.
3. `ZITADEL_FIRSTINSTANCE_LOGINCLIENT*` env vars and `ZITADEL_DEFAULTINSTANCE_FEATURES_LOGINV2_*` env vars are removed from `zitadel-api` (no Login consumer).
4. **Added** `bootstrap/steps.yaml` mounted read-only at `/bootstrap/steps.yaml` and referenced via `--steps` on the `start-from-init` command (per D14). The steps file declares `FirstInstance.Org.Machine.*` + `FirstInstance.PatPath`, so Zitadel provisions an admin machine user at first boot and writes its PAT to `/zitadel/bootstrap/pim-admin.pat` on the shared `zitadel-bootstrap` volume. `just dev-up` will later copy that PAT value into `.env.local` as `ZITADEL_ADMIN_PAT`.
5. `ZITADEL_EXTERNALDOMAIN` is `pim.localhost` (per D7) and `ZITADEL_EXTERNALPORT` is `18080` (per D16). The user-visible URL is `http://pim.localhost:18080`.
6. `pim-api-gateway` and `pim-user-service` are added to the same `zitadel` network, mount their service-specific `config.toml` (D10 Layer A), the gateway additionally mounts `zitadel-key.json` (D10 Layer B), user-service consumes `.env.local` via `env_file` (D10 Layer C).

- [x] **Step 1: Derive and write `compose.yml`** ✅ *completed 2026-04-18*

Wrote `compose.yml` with the six deviations above. Top-level shape:

```yaml
name: pim

services:
  zitadel-api:    # upstream minus LOGINCLIENT/LOGINV2/Traefik labels; mounts bootstrap/steps.yaml + --steps flag; ports: ["18080:8080"]
  postgres:       # verbatim from upstream
  api-gateway:
    image: pim/api-gateway:dev  # or build: apps/api-gateway
    volumes:
      - ./apps/api-gateway/config.toml:/app/config.toml:ro
      - ./zitadel-key.json:/app/zitadel-key.json:ro
    networks: [zitadel]
    depends_on:
      zitadel-api:
        condition: service_healthy
  user-service:
    image: pim/user-service:dev
    env_file: ./.env.local
    volumes:
      - ./apps/user-service/config.toml:/app/config.toml:ro
    networks: [zitadel]
    depends_on:
      zitadel-api:
        condition: service_healthy

networks:
  zitadel:
    name: zitadel

volumes:
  postgres-data:
  zitadel-bootstrap:
```

**FirstInstance setup via `bootstrap/steps.yaml`** (per D14). The `zitadel-api` service mounts `./bootstrap:/bootstrap:ro` and invokes `start-from-init --steps /bootstrap/steps.yaml`. The steps file:

```yaml
FirstInstance:
  PatPath: /zitadel/bootstrap/pim-admin.pat
  Org:
    Name: pim-dev
    Machine:
      Machine:
        Username: pim-admin
        Name: PIM Bootstrap Admin
      Pat:
        ExpirationDate: "2099-01-01T00:00:00Z"
```

> Field name `FirstInstance.PatPath` verified against `cmd/setup/03.go` in Zitadel v4.13.0 and smoke-test-proven on 2026-04-18 (72-char PAT written to volume, authenticates successfully against `/auth/v1/users/me`).

- [x] **Step 2: Drop the stale `docker-compose.yml`** ✅ *staged for deletion 2026-04-18*

```bash
git rm docker-compose.yml
```

- [x] **Step 3: Add `pim.localhost` loopback alias** ✅ *verified 2026-04-18*

`pim.localhost` resolves to `127.0.0.1` automatically on macOS per RFC 6761; no `/etc/hosts` edit needed. A note belongs in `docs/dev-bootstrap.md` when that file is authored in Phase 5.

- [x] **Step 4: Smoke-test boot (manual)** ✅ *all green 2026-04-18*

```bash
# One-time: generate masterkey (Phase 5 justfile will do this automatically)
ZITADEL_MASTERKEY="$(openssl rand -base64 32 | tr -d '=+/' | cut -c1-32)"
echo "ZITADEL_MASTERKEY=$ZITADEL_MASTERKEY" > .env.local

# Fresh boot (destructive — wipes volumes)
podman compose --env-file .env.local down -v
podman compose --env-file .env.local up -d postgres zitadel-api

# Wait for readiness
podman compose ps
podman compose logs zitadel-api | grep -m1 'server is listening'

# IMPORTANT: disable corporate/Clash HTTP proxies for loopback, or curl will 502
export no_proxy="localhost,127.0.0.1,pim.localhost,*.localhost"

# Health check
curl -sf http://pim.localhost:18080/debug/healthz     # expect: HTTP 200

# Read dispensed PAT from bootstrap volume (image is distroless — must use alpine sidecar)
PAT=$(podman run --rm -v pim_zitadel-bootstrap:/b:ro alpine:3 cat /b/pim-admin.pat)

# Verify PAT authenticates
curl -sf -H "Authorization: Bearer $PAT" http://pim.localhost:18080/auth/v1/users/me | jq .
# expect: userName=pim-admin, state=USER_STATE_ACTIVE, machine.name="PIM Bootstrap Admin"

podman compose down  # leaves volumes intact — masterkey and PAT survive
```

**Observed result (2026-04-18):** health 200; PAT auth 200 returning machine user `pim-admin` in org `pim-dev` (id `369117951578144771`, state `USER_STATE_ACTIVE`). `bootstrap/steps.yaml` → PAT path end-to-end confirmed working.

**Runtime gotchas discovered:**
- Zitadel image is distroless: no `sh`/`cat`/`ls` available inside. Read bootstrap files with a sidecar: `podman run --rm -v pim_zitadel-bootstrap:/b:ro alpine:3 cat /b/pim-admin.pat`.
- `podman compose down` without `-v` preserves volumes; `FirstInstance` only runs when the DB is empty, so *must* use `-v` for re-init.
- If a corporate/Clash HTTP proxy is exported in the shell, loopback curl requests are intercepted and return 502. Always set `no_proxy="localhost,127.0.0.1,pim.localhost,*.localhost"`. The Phase 5 justfile will set this automatically.

- [x] **Step 5: Commit** ✅ *2026-04-18*

```bash
git add compose.yml bootstrap/steps.yaml plans/006-dev-bootstrap.md
git rm docker-compose.yml
git commit -m "feat(compose): local Zitadel dev stack on port 18080 (no Login, no Traefik)"
```

---

## Phase 2 — `tools/pim-bootstrap` Crate Skeleton

### Task 2.1: Workspace crate and CLI scaffold

**Files:**
- Create: `tools/pim-bootstrap/Cargo.toml`
- Create: `tools/pim-bootstrap/src/main.rs`
- Create: `tools/pim-bootstrap/src/lib.rs`
- Create: `tools/pim-bootstrap/src/cli.rs`
- Create: `tools/pim-bootstrap/src/config.rs`
- Modify: `Cargo.toml` (workspace members)

- [x] **Step 1: Register crate in workspace**

Add `"tools/pim-bootstrap"` to `[workspace.members]`.

- [x] **Step 2: Declare crate**

`tools/pim-bootstrap/Cargo.toml`:

```toml
[package]
name = "pim-bootstrap"
version.workspace = true
edition.workspace = true

[[bin]]
name = "pim-bootstrap"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive", "env"] }
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
toml = "0.8"
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
tracing.workspace = true
tracing-subscriber = "0.3"
thiserror.workspace = true
anyhow = "1"
zitadel.workspace = true  # reuse credentials/JWT profile primitives
```

- [x] **Step 3: Define CLI surface**

`src/cli.rs` — three subcommands:

- `bootstrap --config <path> [--sync] [--rotate-keys] [--dry-run] [--env dev|prod]`
- `seed --config <path> [--dry-run] [--env dev]` (hard-fails on `prod`)
- `diff --config <path>` (reports drift without changes; always dry)

Global flags: `--zitadel-url`, `--admin-key-file` (falls back to env).

- [x] **Step 4: Define config schema**

`src/config.rs`:

```rust
#[derive(Deserialize)]
pub struct BootstrapConfig {
    pub env: Environment,                         // dev | prod (rejects seed on prod)
    pub zitadel: ZitadelTarget,
    pub project: ProjectSpec,
    pub api_app: ApiAppSpec,
    pub service_account: ServiceAccountSpec,
    pub roles: Vec<RoleSpec>,
    pub outputs: OutputSinks,
}

pub struct SeedConfig {
    pub users: Vec<HumanSpec>,
    pub role_assignments: Vec<RoleAssignmentSpec>,
}
```

`OutputSinks` describes where provisioned values land, split by sensitivity:

```rust
pub struct OutputSinks {
    /// Non-secret values written back into service config.toml files
    /// (gitignored real files; committed as .example.toml). Maps service name → path.
    pub service_configs: HashMap<String, PathBuf>,
    /// Non-symmetric secret key files (e.g. JWT app key). Written once, never overwritten
    /// without --rotate-keys.
    pub jwt_key_path: PathBuf,
    /// Symmetric secret strings (PATs). Appended/upserted into a single env file
    /// consumed by compose `env_file:` directives.
    pub env_file_path: PathBuf,
}
```

For prod, paths can be sentinel strings like `"stdout:jwt_key"` or `"stdout:pat"` so operators pipe to their secret manager instead of landing files on disk.

- [x] **Step 5: Smoke-test CLI**

Implement stub handlers that just log the parsed config. Run:

```bash
cargo run -p pim-bootstrap -- bootstrap --config bootstrap/dev.toml --dry-run
```

Expected: parses, logs, exits 0.

- [x] **Step 6: Commit**

```bash
git add Cargo.toml tools/pim-bootstrap/
git commit -m "feat(bootstrap): pim-bootstrap CLI skeleton with declarative config"
```

### Task 2.2: Dev and prod config files

**Files:**
- Create: `bootstrap/dev.toml`
- Create: `bootstrap/prod.example.toml`
- Create: `bootstrap/seed.dev.toml`

- [x] **Step 1: Author `bootstrap/dev.toml`**

Concrete, runnable values for local:

```toml
env = "dev"

[zitadel]
authority = "http://pim.localhost:18080"
# Dev admin auth mode: read PAT from env var (per D12). Prod flips to jwt_profile.
admin_auth = "pat"
admin_pat_env_var = "ZITADEL_ADMIN_PAT"

[project]
name = "pim"

[api_app]
name = "api-gateway"
auth_method = "jwt_profile"

[service_account]
username = "user-service-sa"
description = "Service account used by user-service to call Zitadel Management API"

[[roles]]
key = "admin"
display_name = "Administrator"

[[roles]]
key = "member"
display_name = "Member"

[outputs]
jwt_key_path = "zitadel-key.json"
env_file_path = ".env.local"

[outputs.service_configs]
api-gateway = "apps/api-gateway/config.toml"
user-service = "apps/user-service/config.toml"
```

The three output sinks correspond to D10's sensitivity layering: non-secret config lands in service `config.toml` files, the JWT app key lands in its own file, the PAT lands in `.env.local`.

- [x] **Step 2: Author `bootstrap/prod.example.toml`**

Same structure, placeholders for authority + admin credentials, outputs pointing to stdout markers (`"stdout:jwt_key"`) so prod operators pipe to their secret manager.

- [x] **Step 3: Author `bootstrap/seed.dev.toml`**

```toml
env = "dev"

[[users]]
username = "alice"
email = "alice@pim.dev"
given_name = "Alice"
family_name = "Tester"
initial_password = "Alice-Dev-Pass-1"  # dev only, documented as disposable
email_verified = true

[[users]]
username = "bob"
email = "bob@pim.dev"
given_name = "Bob"
family_name = "Tester"
initial_password = "Bob-Dev-Pass-1"
email_verified = true

[[users]]
username = "charlie"
email = "charlie@pim.dev"
given_name = "Charlie"
family_name = "Tester"
initial_password = "Charlie-Dev-Pass-1"
email_verified = true

[[role_assignments]]
user = "alice"
roles = ["admin"]

[[role_assignments]]
user = "bob"
roles = ["member"]

[[role_assignments]]
user = "charlie"
roles = ["member"]
```

- [x] **Step 4: Commit**

```bash
git add bootstrap/dev.toml bootstrap/prod.example.toml bootstrap/seed.dev.toml
git commit -m "feat(bootstrap): declarative dev/prod config and dev seed"
```

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

`Outcome { Created, Unchanged, Updated, DryRunWouldCreate, DryRunWouldUpdate, Skipped { reason } }`. Every op returns one; the top-level aggregator tallies.

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

- [ ] **Step 3: Secret-persist-exactly-once discipline (per D10 sensitivity layer)**

Three classes of output, three disciplines:

- **Non-secret values** (project ID, app ID, authority URL) → write into the relevant `apps/<svc>/config.toml` via `outputs.service_configs`. Safe to overwrite on every run; these are derivable from Zitadel state.
- **JWT app key** (non-symmetric, file) → write to `outputs.jwt_key_path`. If target file already has matching `keyId`, no-op. If file missing but app exists in Zitadel → error unless `--rotate-keys`.
- **Service account PAT** (symmetric, string) → upsert into `outputs.env_file_path` as a `KEY=VALUE` line. Same rule as JWT key: missing-but-resource-exists → error unless `--rotate-keys`.

Never write a secret to a service `config.toml`. Never write a non-secret to `.env.local`. The layering is load-bearing.

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

# Drift check against prod (operator-invoked, not CI; see D9).
# Requires PIM_PROD_ADMIN_KEY to point at an admin key JSON provided
# by the operator's secret manager at invocation time.
prod-diff:
    cargo run -p pim-bootstrap -- diff --config bootstrap/prod.toml

# Full teardown + wipe (clears Postgres volume + all three D10 output classes +
# ZITADEL_MASTERKEY + ZITADEL_ADMIN_PAT, so next `dev-up` regenerates everything).
dev-reset:
    podman compose down -v
    rm -f zitadel-key.json .env.local
    rm -f apps/api-gateway/config.toml apps/user-service/config.toml
    @echo "Run 'just dev-up' to reinitialise from scratch."
```

> Per D12/D13, there is no separate "regenerate machinekey" recipe anymore. The admin PAT is dispensed by Zitadel at first boot, and the masterkey is generated by `dev-up` on demand — both are wiped by `dev-reset` and re-minted on the next `dev-up`.

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
- Prod bootstrap contract (same binary, different config, secret outputs redirected)
- Troubleshooting matrix

- [ ] **Step 2: Cross-link from `README.md` and `docs/design.md`**

Add a "Development" section to README pointing to `docs/dev-bootstrap.md`. In `docs/design.md` section 7 (Future Evolution), move "Dev bootstrap" from future to done.

- [ ] **Step 3: Update `docs/configuration.md`**

Document the D10 three-layer configuration convention:

- **Layer A — non-secret service config**: `apps/<svc>/config.toml`, gitignored, committed as `.example.toml`. Generated by `pim-bootstrap` from Zitadel state. Safe to delete and regenerate.
- **Layer B — non-symmetric key files**: `zitadel-key.json` at repo root. Gitignored, written once, rotated via `--rotate-keys`.
- **Layer C — symmetric secret strings**: `.env.local` at repo root, consumed by compose `env_file:`. Gitignored, upserted by bootstrap, rotated via `--rotate-keys`.

State which app needs which layers (api-gateway: A+B; user-service: A+C).

- [ ] **Step 4: Update `AGENTS.md` if the `/docs/` map changes**

Add `docs/dev-bootstrap.md` entry if the documentation map references specific pages. (Currently lists only `design.md`; extend if needed.)

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
- Masterkey / admin PAT: teardown-required (see D13). `just dev-reset` wipes them along with the Postgres volume; next `just dev-up` mints fresh values.

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

### Phase 1
- [ ] `podman compose up -d postgres zitadel-api` starts cleanly; `zitadel-api` healthcheck passes within 60s
- [ ] `http://pim.localhost:18080/debug/healthz` returns 200 (direct publish, no Traefik — see D15/D16)
- [ ] `/zitadel/bootstrap/pim-admin.pat` inside the `zitadel-api` container contains a non-empty PAT (dispensed by `FirstInstance` in `bootstrap/steps.yaml`)
- [ ] `.env.local.example` is committed; `.env.local` is absent from `git ls-files`
- [ ] `git check-ignore` confirms `zitadel-key.json`, `.env.local`, and `bootstrap/*.machinekey.json` are all ignored

### Phase 2
- [ ] `cargo run -p pim-bootstrap -- --help` lists `bootstrap`, `seed`, `diff`
- [ ] Parsing `bootstrap/dev.toml` and `bootstrap/seed.dev.toml` succeeds

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
| Phase 1: Local compose stack | In Progress — Task 1.1 ✅, Task 1.2 ✅ (smoke test green 2026-04-18) |
| Phase 2: `pim-bootstrap` skeleton | ✅ Complete — Task 2.1 ✅, Task 2.2 ✅ (all subcommands dry-run green 2026-04-18) |
| Phase 3: Idempotent bootstrap ops | Not Started |
| Phase 4: Dev seed | Not Started |
| Phase 5: Developer ergonomics | Not Started |
| Phase 6: Secret hygiene and rotation | Not Started |

---

## Resolved Questions

The four questions raised in the first draft of this plan were resolved with the user on 2026-04-17 and folded into the Key Design Decisions table as D7–D10:

1. Dev `ZITADEL_EXTERNALDOMAIN` → `pim.localhost` (D7).
2. Prod topology → Zitadel Cloud first; `prod.example.toml` targets Cloud (D8).
3. `diff` in CI → deferred. Exposed as `just prod-diff` for manual/periodic operator use (D9).
4. Bootstrap managing PIM service config → yes, via the three-layer sink model (D10).
