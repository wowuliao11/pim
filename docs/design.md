# System Design Document

**Status:** Current Accepted Design
**Last Updated:** 2026-04-05

> **Notice:** This document reflects the stabilizing architecture of the system.
> Future code implementations MUST follow this design unless a new Plan is approved and this document is updated.

---

## 1. System Overview

PIM is a Rust microservices monorepo following the **1 × HTTP Gateway + N × gRPC Services** architecture pattern.

### Key Characteristics

- **Language:** Rust (2021 edition)
- **HTTP Framework:** Actix-web (api-gateway)
- **gRPC Framework:** Tonic 0.14 (domain services)
- **Serialization:** Protobuf with prost 0.14 (service-to-service), JSON (external API)
- **Build System:** Cargo workspace
- **Identity Provider:** Zitadel Cloud (OIDC Token Introspection)

---

## 2. Architecture

```
                    ┌─────────────────┐
                    │   HTTP Client   │
                    │ (Tauri / React) │
                    └────────┬────────┘
                             │ OIDC Auth Code + PKCE
                    ┌────────▼────────┐
                    │  Zitadel Cloud  │  (External IdP)
                    └────────┬────────┘
                             │ Bearer Token
                    ┌────────▼────────┐
                    │   api-gateway   │  :8080
                    │   (Actix-web)   │
                    │  Token Introsp. │
                    │  gRPC Client    │───── user-service gRPC ─────┐
                    └────────┬────────┘                             │
                             │                              ┌───────▼───────┐
                             │                              │ user-service  │
                             │                              │   (Tonic)     │  :50051
                             │                              │ Zitadel Proxy │
                             │                              └───────────────┘
```

### Authentication Flow

1. **Clients** (Tauri mobile app, React admin panel) authenticate directly with Zitadel Cloud using Authorization Code + PKCE flow
2. **API Gateway** receives Bearer tokens and validates them via Zitadel's Token Introspection endpoint using JWT Profile authentication (via the `zitadel` crate's `IntrospectedUser` actix extractor)
3. **Protected handlers** include `IntrospectedUser` as a function parameter — no custom middleware needed
4. **Gateway proxies** user requests to user-service over gRPC, passing the authenticated user ID
5. **user-service** proxies user queries to Zitadel's Management REST API v2 using a service account PAT

### Layer Responsibilities

| Layer          | Crate                  | Purpose                                              |
| -------------- | ---------------------- | ---------------------------------------------------- |
| Contract       | `proto/`               | Protobuf definitions (SSoT)                          |
| Boundary       | `libs/rpc-proto`       | Generated gRPC code only (tonic-prost-build)         |
| Authentication | `libs/infra-auth`      | Zitadel OIDC re-exports (IntrospectedUser)           |
| Configuration  | `libs/infra-config`    | Config loading & environment                         |
| Observability  | `libs/infra-telemetry` | Metrics (re-exports `metrics` crate), tracing, gRPC/HTTP metric layers |
| Gateway        | `apps/api-gateway`     | HTTP→gRPC translation, token introspection           |
| Domain         | `apps/*-service`       | Business logic per domain (Zitadel API proxy)        |

### Dependency Direction

```
apps/* → libs/rpc-proto → proto/
apps/* → libs/infra-auth
apps/* → libs/infra-config
apps/* → libs/infra-telemetry
```

Reverse dependencies are **FORBIDDEN**.

---

## 3. Domain Services

### user-service (:50051)

**Responsibility:** User data management — proxies to Zitadel Management REST API v2

**RPCs:**

- `GetUser` - Retrieve user by ID (via Zitadel `GET /v2/users/{id}`)
- `ListUsers` - Paginated user list (via Zitadel `POST /v2/users`)
- `GetCurrentUser` - Current authenticated user by user_id
- `UpdateUser` - Modify user info (via Zitadel `PUT /v2/users/{id}`)
- `DeleteUser` - Remove user account (via Zitadel `DELETE /v2/users/{id}`)

**Authentication to Zitadel:** Service account Personal Access Token (PAT)

**Security measures:**
- User ID format validation (`validate_user_id`) — only alphanumeric IDs accepted to prevent SSRF
- Generic gRPC error messages — internal details logged but not exposed to callers
- Credentials redacted in Debug output
- HTTP client configured with connect (5s) and request (10s) timeouts

**Data mapping:** Typed `serde::Deserialize` structs for Zitadel v2 JSON responses (`ZitadelUser`, `ZitadelHuman`, `ZitadelProfile`, etc.) mapped to proto `User` with `google.protobuf.Timestamp` fields.

---

## 4. API Design

### External HTTP API (api-gateway)

| Method | Path                      | Auth Required | Description                |
| ------ | ------------------------- | ------------- | -------------------------- |
| GET    | `/health`                 | No            | Health check               |
| GET    | `/api/v1/auth/userinfo`   | Yes           | Current user info (token)  |
| GET    | `/api/v1/users`           | Yes           | List users (via gRPC)      |
| GET    | `/api/v1/users/{id}`      | Yes           | Get user by ID (via gRPC)  |
| GET    | `/api/v1/users/me`        | Yes           | Current user (via gRPC)    |

Authentication is handled by the `IntrospectedUser` extractor from the `zitadel` crate. Handlers that include this extractor automatically require a valid Bearer token. Handlers without it (e.g., `/health`) are public.

### gRPC Client Integration

The gateway establishes a `UserServiceClient<Channel>` connection at startup and shares it across handlers via `actix_web::web::Data`. Proto `User` messages (with `prost_types::Timestamp`) are converted to gateway DTOs (`UserResponse` with `chrono::DateTime<Utc>`) before returning JSON.

### gRPC APIs

Defined in `proto/` directory:

- `proto/user/v1/user.proto` — Uses `google.protobuf.Timestamp` for temporal fields

---

## 5. Configuration Management

All services use a **shared configuration loader** from `libs/infra-config` that supports TOML files and environment variables.

### Configuration Sources (Priority Order)

1. **Environment variables** (highest priority)
2. **TOML configuration files** (optional)
3. **Default values** from Rust `Default` trait (lowest priority)

### Environment Variable Convention

- **Nesting separator:** `__` (double underscore)
- **Service-specific prefixes:**
  - `api-gateway`: `APP`
  - `user-service`: `USER_SERVICE`

**Examples:**

- `APP__APP__HOST=0.0.0.0` → `app.host`
- `APP__APP__USER_SERVICE_URL=http://user-svc:50051` → `app.user_service_url`
- `APP__ZITADEL__AUTHORITY=https://my.zitadel.cloud` → `zitadel.authority`
- `APP__ZITADEL__KEY_FILE=./keys/api-gateway.json` → `zitadel.key_file`
- `USER_SERVICE__ZITADEL_AUTHORITY=https://my.zitadel.cloud` → `zitadel_authority`
- `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN=pat-xxx` → `zitadel_service_account_token`

### Gateway-specific Settings

| Setting              | Default                    | Description                     |
| -------------------- | -------------------------- | ------------------------------- |
| `app.user_service_url` | `http://127.0.0.1:50051` | gRPC endpoint of user-service  |
| `app.host`           | `127.0.0.1`                | HTTP bind host                  |
| `app.port`           | `8080`                     | HTTP bind port                  |
| `app.metrics_port`   | `60080`                    | Prometheus metrics port         |

### TOML Files

Each service may load optional TOML files from its directory:

- `api-gateway`: `apps/api-gateway/config.toml`
- `user-service`: `apps/user-service/config.toml`

**Note:** `.env` files are **NOT** supported. Use environment variables or TOML files directly.

### Implementation

Services define their own `Settings` structs and call `infra_config::load_config()`:

```rust
use infra_config::load_config;
use config::ConfigError;

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("APP", "config.toml")
}
```

For detailed usage and migration notes, see [`docs/configuration.md`](./configuration.md).

---

## 6. Observability

### Metrics

- `infra-telemetry` re-exports the `metrics` crate under `#[cfg(feature = "prometheus")]`
- All workspace crates MUST use `infra_telemetry::metrics` instead of depending on `metrics` directly (prevents version conflicts)
- Gateway: HTTP metrics via `HttpMetrics` middleware
- user-service: gRPC metrics via `GrpcMetricsLayer`

### Tracing

- Structured logging via `tracing` + `tracing-subscriber`
- Credentials are never logged (custom `Debug` impls on `Settings` structs)

---

## 7. Release Automation

### Tool

[`release-plz`](https://github.com/release-plz/release-plz) via the official `release-plz/action@v0.5` GitHub Action. Workflow lives at `.github/workflows/release-plz.yml`, configuration at `release-plz.toml`.

### Rationale

release-plz is Rust-native and compares local `Cargo.toml` values against crates.io, so it transparently supports `version.workspace = true` inheritance. The previous tool (release-please) parses member manifests directly and fails on inherited `[package.version]` — see Plan 007 for the migration record and upstream issue `googleapis/release-please#2111`.

### Flow

1. On every push to `main`, `release-plz-pr` job opens or updates a Release PR containing version bumps and CHANGELOG entries derived from Conventional Commits.
2. When that Release PR is merged, `release-plz-release` job creates git tags and GitHub Releases for each bumped crate.
3. `publish = false` in `release-plz.toml` — crates are not pushed to crates.io. Flip per-package when/if publication becomes a goal.

### Constraints

- Commit messages MUST follow Conventional Commits (enforced on PR titles by `.github/workflows/pr-title.yml`, `amannn/action-semantic-pull-request@v5`).
- Workspace version inheritance (`version.workspace = true` in member crates, canonical `[workspace.package] version` in root `Cargo.toml`) is the supported pattern and MUST be preserved.

---

## 8. Development Workflow

PIM follows **Trunk-Based Development**. The operational handbook lives in
[`CONTRIBUTING.md`](../CONTRIBUTING.md); this section records the
architectural decisions that the workflow depends on.

### 8.1 Trunk and integration

- `main` is the single integration branch. Branch protection enforces:
  squash-only merges, linear history, no force-push, no self-approve, and
  6 required status checks (Rustfmt, Clippy, Test, Buf, Cargo Deny,
  Conventional Commit).
- All work happens on short-lived branches (target lifetime < 2 working
  days, target diff < ~400 LOC).
- Branch naming follows Conventional Commit types: `feat/`, `fix/`,
  `refactor/`, `docs/`, `ci/`, `chore/`, `test/`. Dependabot branches are
  exempt.

### 8.2 Hiding incomplete work

To avoid long-lived branches, incomplete work lands on `main` behind runtime
feature flags. The mechanism lives in `libs/infra-config::features`:

- Flags are read from environment variables of the form
  `APP_FEATURE_<UPPERCASE_NAME>=true`.
- Code uses `infra_config::features::is_enabled("flag_name")`.
- Flags are debt: each new flag has a documented owner and removal criterion
  in the introducing PR.

### 8.3 Plan-required threshold

Most changes ship without a plan file. A `/plans/NNN-*.md` is mandatory only
when the change is genuinely large or cross-cutting (see `AGENTS.md §3.1` for
the authoritative triggers). The threshold exists to keep architectural
decisions reviewable, not to gate routine work.

### 8.4 Pull request template

Located at `.github/pull_request_template.md` (auto-applied by GitHub). The
template enforces declaration of: purpose, proposed changes, test plan,
breaking-change status, and the TBD discipline checks (short-lived branch,
size limit, feature-flag gating).

---

## 9. Future Evolution

- [ ] Database integration (per-service ownership)
- [ ] Health checks / readiness probes
- [ ] Buf linting for proto changes
- [ ] Service mesh / observability dashboards
- [x] ~~External identity provider integration~~ (Zitadel Cloud — implemented)
- [x] ~~API Gateway → gRPC client integration~~ (implemented)
- [x] ~~Unified tonic/prost versions~~ (0.14)
