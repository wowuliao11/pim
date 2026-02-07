# System Design Document

**Status:** Current Accepted Design
**Last Updated:** 2026-01-31

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

---

## 2. Architecture

```
                    ┌─────────────────┐
                    │   HTTP Client   │
                    └────────┬────────┘
                             │ HTTP/JSON
                    ┌────────▼────────┐
                    │   api-gateway   │  :8080
                    │   (Actix-web)   │
                    └────────┬────────┘
                             │ gRPC
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼────────┐    ...    ┌────────▼────────┐
     │  auth-service   │           │  user-service   │
     │    (Tonic)      │  :50051   │    (Tonic)      │  :50052
     └─────────────────┘           └─────────────────┘
```

### Layer Responsibilities

| Layer          | Crate                | Purpose                      |
| -------------- | -------------------- | ---------------------------- |
| Contract       | `proto/`             | Protobuf definitions (SSoT)  |
| Boundary       | `libs/rpc-proto`     | Generated gRPC code only     |
| Configuration  | `libs/infra-config`    | Config loading & environment |
| Observability  | `libs/infra-telemetry` | Metrics, tracing primitives  |
| Gateway        | `apps/api-gateway`   | HTTP↔gRPC translation        |
| Domain         | `apps/*-service`     | Business logic per domain    |

### Dependency Direction

```
apps/* → libs/rpc-proto → proto/
apps/* → libs/infra-config
apps/* → libs/infra-telemetry
```

Reverse dependencies are **FORBIDDEN**.

---

## 3. Domain Services

### auth-service (:50051)

**Responsibility:** Authentication and token management

**RPCs:**

- `Login` - Authenticate user, return JWT
- `Register` - Create new user account
- `ValidateToken` - Verify JWT validity
- `RefreshToken` - Generate new token from valid token

### user-service (:50052)

**Responsibility:** User data management

**RPCs:**

- `GetUser` - Retrieve user by ID
- `ListUsers` - Paginated user list
- `GetCurrentUser` - Current authenticated user
- `UpdateUser` - Modify user info
- `DeleteUser` - Remove user account

---

## 4. API Design

### External HTTP API (api-gateway)

| Method | Path                    | Description       |
| ------ | ----------------------- | ----------------- |
| GET    | `/health`               | Health check      |
| POST   | `/api/v1/auth/login`    | User login        |
| POST   | `/api/v1/auth/register` | User registration |
| GET    | `/api/v1/users`         | List users        |
| GET    | `/api/v1/users/{id}`    | Get user by ID    |
| GET    | `/api/v1/users/me`      | Current user      |

### gRPC APIs

Defined in `proto/` directory:

- `proto/auth/v1/auth.proto`
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
  - `auth-service`: `AUTH_SERVICE`
  - `user-service`: `USER_SERVICE`

**Examples:**

- `APP__APP__HOST=0.0.0.0` → `app.host`
- `APP__JWT__SECRET=my-key` → `jwt.secret`
- `AUTH_SERVICE__JWT_EXPIRATION_HOURS=48` → `jwt_expiration_hours`

### TOML Files

Each service may load optional TOML files from the `config/` directory (relative to repository root):

- `api-gateway`: `config/default.toml`, `config/local.toml`
- `auth-service`: `config/auth-service.toml`
- `user-service`: `config/user-service.toml`

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
- [ ] API Gateway → gRPC client integration
- [ ] Health checks / readiness probes
- [ ] Buf linting for proto changes
- [ ] Service mesh / observability
