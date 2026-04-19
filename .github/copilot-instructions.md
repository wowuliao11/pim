# Copilot Instructions - Project Architecture Guide

> This document defines the folder responsibilities and architectural constraints for the PIM monorepo.

## Directory Structure

```
pim/
├── proto/                     # API Contract Layer (Source of Truth)
│   ├── auth/v1/               # Auth domain gRPC definitions
│   └── user/v1/               # User domain gRPC definitions
│
├── libs/                      # Library Layer (Atomic, Reusable crates)
│   ├── rpc-proto/             # Boundary Layer - Generated gRPC code only
│   ├── infra-config/          # Configuration loading & environment utilities
│   └── infra-telemetry/       # Metrics, tracing, observability primitives
│
├── apps/                      # Application Layer (Deployable binaries)
│   ├── api-gateway/           # HTTP Gateway (Actix-web)
│   ├── auth-service/          # Auth gRPC Service (tonic)
│   └── user-service/          # User gRPC Service (tonic)
│
├── docs/                      # Documentation
│   └── design.md              # Current system design
│
└── plans/                     # Feature planning and tracking
```

## Layer Responsibilities

### 1. `proto/` - API Contract Layer

**Purpose:** Define all service communication protocols. This is the **single source of truth** for APIs.

**Rules:**

- Only `.proto` files allowed
- Organize by `domain/version/` (e.g., `auth/v1/`)
- No Rust code or business concepts
- Changes here trigger regeneration in `libs/rpc-proto`

### 2. `libs/rpc-proto/` - Boundary Layer

**Purpose:** Generate and export gRPC interfaces from proto files.

**FORBIDDEN:**

- ❌ Business logic
- ❌ Helpers / mappers / validators
- ❌ Any non-generated code beyond re-exports

### 3. `libs/infra-config/` - Configuration Layer

**Purpose:** Provide generic configuration loading and environment detection.

**Allowed:**

- `load_config()` function for loading TOML + env vars
- `CommonConfig` struct for shared config fields
- `AppEnv` enum for runtime environment detection

**FORBIDDEN:**

- ❌ Business logic
- ❌ Domain-specific configuration structs
- ❌ Service-specific dependencies

### 4. `libs/infra-telemetry/` - Observability Layer

**Purpose:** Provide metrics, tracing, and observability primitives.

**Allowed:**

- Prometheus metrics initialization and rendering
- gRPC metrics middleware (Tower layer)
- HTTP metrics endpoint server
- Standard metric labels and names

**FORBIDDEN:**

- ❌ Business logic
- ❌ Domain-specific metrics definitions
- ❌ Service-specific dependencies

### 5. `apps/api-gateway/` - HTTP Gateway

**Purpose:** External HTTP API entry point.

**Responsibilities:**

- HTTP ↔ gRPC protocol translation
- Authentication / authorization
- Rate limiting
- Request routing

**FORBIDDEN:**

- ❌ Core business logic
- ❌ Direct database access (delegate to gRPC services)

### 6. `apps/*-service/` - Domain Services

**Purpose:** Implement business logic for specific domains.

**Each service:**

- Is independently deployable
- Owns its domain logic
- Exposes gRPC interface only
- May own its database/migrations

## Dependency Rules

```
apps/*        ───▶ libs/rpc-proto
apps/*        ───▶ libs/infra-config
apps/*        ───▶ libs/infra-telemetry
libs/*        ───▶ proto/
```

**Reverse dependencies are FORBIDDEN.**

## Architecture Iron Laws

1. **Proto is contract, not implementation**
2. **rpc-proto only describes boundaries, no behavior**
3. **Gateway doesn't write business logic, services don't handle HTTP**
4. **libs remain atomic: infra-config for config, infra-telemetry for metrics**
5. **Every app must be independently startable and deployable**

## Port Allocation

Ports follow the fixed policy in
[ADR-0015](../docs/decisions/0015-allocate-service-ports-with-fixed-policy.md).
See `docs/design.md §5` for the authoritative tables. Summary:

| Service       | Container port | Host port (compose) | Protocol |
| ------------- | -------------- | ------------------- | -------- |
| api-gateway   | 8080           | 18000               | HTTP     |
| user-service  | 50051          | (not published)     | gRPC     |
| zitadel       | 8080           | 18080               | HTTP     |

Metrics ports follow `60000 + (service_port % 1000)`:
api-gateway `60080`, user-service `60051`.

## Code Generation

Proto files are compiled during `cargo build` of `libs/rpc-proto`:

- Generated code goes to `OUT_DIR` (not committed)
- Use `tonic::include_proto!()` macro to include

## Environment Variables

Each service supports configuration via environment variables (double-underscore
nesting separator). Prefixes:

- **api-gateway:** `APP__*` (e.g. `APP__APP__HOST`, `APP__APP__PORT`, `APP__APP__USER_SERVICE_URL`, `APP__ZITADEL__AUTHORITY`, `APP__ZITADEL__KEY_FILE`)
- **user-service:** `USER_SERVICE__*` (e.g. `USER_SERVICE__HOST`, `USER_SERVICE__PORT`, `USER_SERVICE__ZITADEL_AUTHORITY`, `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN`)

See `docs/configuration.md` for the full list.
