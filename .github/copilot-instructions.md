# Copilot Instructions - Project Architecture Guide

> This document defines the folder responsibilities and architectural constraints for the PIM monorepo.

## Directory Structure

```
pim/
├── proto/                     # API Contract Layer (Source of Truth)
│   ├── auth/v1/               # Auth domain gRPC definitions
│   └── user/v1/               # User domain gRPC definitions
│
├── libs/                      # Library Layer (Reusable crates)
│   ├── rpc-proto/             # Boundary Layer - Generated gRPC code only
│   └── common/                # Infrastructure Layer - Cross-cutting concerns
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

### 3. `libs/common/` - Infrastructure Layer

**Purpose:** Cross-cutting, business-agnostic utilities.

**Allowed:**

- Error types (thiserror / anyhow)
- Configuration loading
- Tracing / logging setup
- Environment utilities

**FORBIDDEN:**

- ❌ Domain models (User, Order, etc.)
- ❌ Dependencies on specific services

### 4. `apps/api-gateway/` - HTTP Gateway

**Purpose:** External HTTP API entry point.

**Responsibilities:**

- HTTP ↔ gRPC protocol translation
- Authentication / authorization
- Rate limiting
- Request routing

**FORBIDDEN:**

- ❌ Core business logic
- ❌ Direct database access (delegate to gRPC services)

### 5. `apps/*-service/` - Domain Services

**Purpose:** Implement business logic for specific domains.

**Each service:**

- Is independently deployable
- Owns its domain logic
- Exposes gRPC interface only
- May own its database/migrations

## Dependency Rules

```
apps/*        ───▶ libs/rpc-proto
apps/*        ───▶ libs/common
libs/*        ───▶ proto/
```

**Reverse dependencies are FORBIDDEN.**

## Architecture Iron Laws

1. **Proto is contract, not implementation**
2. **rpc-proto only describes boundaries, no behavior**
3. **Gateway doesn't write business logic, services don't handle HTTP**
4. **common only contains cross-domain capabilities**
5. **Every app must be independently startable and deployable**

## Port Allocation

| Service      | Default Port | Protocol |
| ------------ | ------------ | -------- |
| api-gateway  | 8080         | HTTP     |
| auth-service | 50051        | gRPC     |
| user-service | 50052        | gRPC     |

## Code Generation

Proto files are compiled during `cargo build` of `libs/rpc-proto`:

- Generated code goes to `OUT_DIR` (not committed)
- Use `tonic::include_proto!()` macro to include

## Environment Variables

Each service supports configuration via environment variables:

- **api-gateway:** `APP_APP_HOST`, `APP_APP_PORT`, `APP_JWT_SECRET`
- **auth-service:** `AUTH_SERVICE_HOST`, `AUTH_SERVICE_PORT`, `AUTH_SERVICE_JWT_SECRET`
- **user-service:** `USER_SERVICE_HOST`, `USER_SERVICE_PORT`
