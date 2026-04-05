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
- **gRPC Framework:** Tonic (domain services)
- **Serialization:** Protobuf (service-to-service), JSON (external API)
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
                    └────────┬────────┘
                             │ gRPC
                    ┌────────▼────────┐
                    │  user-service   │
                    │    (Tonic)      │  :50052
                    │  Zitadel Proxy  │
                    └─────────────────┘
```

### Authentication Flow

1. **Clients** (Tauri mobile app, React admin panel) authenticate directly with Zitadel Cloud using Authorization Code + PKCE flow
2. **API Gateway** receives Bearer tokens and validates them via Zitadel's Token Introspection endpoint using the `zitadel` crate's `IntrospectedUser` actix extractor
3. **Protected handlers** include `IntrospectedUser` as a function parameter — no custom middleware needed
4. **user-service** proxies user queries to Zitadel's Management REST API v2

### Layer Responsibilities

| Layer          | Crate                  | Purpose                                       |
| -------------- | ---------------------- | --------------------------------------------- |
| Contract       | `proto/`               | Protobuf definitions (SSoT)                   |
| Boundary       | `libs/rpc-proto`       | Generated gRPC code only                      |
| Authentication | `libs/infra-auth`      | Zitadel OIDC re-exports (IntrospectedUser)    |
| Configuration  | `libs/infra-config`    | Config loading & environment                  |
| Observability  | `libs/infra-telemetry` | Metrics, tracing primitives                   |
| Gateway        | `apps/api-gateway`     | HTTP↔gRPC translation, token introspection    |
| Domain         | `apps/*-service`       | Business logic per domain (Zitadel API proxy) |

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

### user-service (:50052)

**Responsibility:** User data management — proxies to Zitadel Management REST API v2

**RPCs:**

- `GetUser` - Retrieve user by ID (via Zitadel `GET /v2/users/{id}`)
- `ListUsers` - Paginated user list (via Zitadel `POST /v2/users`)
- `GetCurrentUser` - Current authenticated user by user_id
- `UpdateUser` - Modify user info (via Zitadel `PUT /v2/users/{id}`)
- `DeleteUser` - Remove user account (via Zitadel `DELETE /v2/users/{id}`)

**Authentication to Zitadel:** Service account Personal Access Token (PAT)

---

## 4. API Design

### External HTTP API (api-gateway)

| Method | Path                      | Auth Required | Description                |
| ------ | ------------------------- | ------------- | -------------------------- |
| GET    | `/health`                 | No            | Health check               |
| GET    | `/api/v1/auth/userinfo`   | Yes           | Current user info (token)  |
| GET    | `/api/v1/users`           | Yes           | List users                 |
| GET    | `/api/v1/users/{id}`      | Yes           | Get user by ID             |
| GET    | `/api/v1/users/me`        | Yes           | Current user (full record) |

Authentication is handled by the `IntrospectedUser` extractor from the `zitadel` crate. Handlers that include this extractor automatically require a valid Bearer token. Handlers without it (e.g., `/health`) are public.

### gRPC APIs

Defined in `proto/` directory:

- `proto/user/v1/user.proto`

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
- `APP__ZITADEL__AUTHORITY=https://my.zitadel.cloud` → `zitadel.authority`
- `APP__ZITADEL__CLIENT_ID=my-client-id` → `zitadel.client_id`
- `APP__ZITADEL__CLIENT_SECRET=my-secret` → `zitadel.client_secret`
- `USER_SERVICE__ZITADEL_AUTHORITY=https://my.zitadel.cloud` → `zitadel_authority`
- `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN=pat-xxx` → `zitadel_service_account_token`

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

## 6. Future Evolution

- [ ] Database integration (per-service ownership)
- [ ] API Gateway → gRPC client integration (currently handlers use placeholder data)
- [ ] Health checks / readiness probes
- [ ] Buf linting for proto changes
- [ ] Service mesh / observability
- [x] ~~External identity provider integration~~ (Zitadel Cloud — implemented)
