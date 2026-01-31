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

| Layer          | Crate              | Purpose                     |
| -------------- | ------------------ | --------------------------- |
| Contract       | `proto/`           | Protobuf definitions (SSoT) |
| Boundary       | `libs/rpc-proto`   | Generated gRPC code only    |
| Infrastructure | `libs/common`      | Cross-cutting utilities     |
| Gateway        | `apps/api-gateway` | HTTP↔gRPC translation       |
| Domain         | `apps/*-service`   | Business logic per domain   |

### Dependency Direction

```
apps/* → libs/rpc-proto → proto/
apps/* → libs/common
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

## 5. Configuration

Each service loads configuration from:

1. Default values in code
2. Config files (`config/*.toml`)
3. Environment variables (highest priority)

### Environment Variable Prefixes

| Service      | Prefix          |
| ------------ | --------------- |
| api-gateway  | `APP_`          |
| auth-service | `AUTH_SERVICE_` |
| user-service | `USER_SERVICE_` |

---

## 6. Future Evolution

- [ ] Database integration (per-service ownership)
- [ ] API Gateway → gRPC client integration
- [ ] Health checks / readiness probes
- [ ] Buf linting for proto changes
- [ ] Service mesh / observability
