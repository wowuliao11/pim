# Plan: Monorepo Restructure (Gateway + gRPC Architecture)

## Context / Goal

Transform the current `crates/*` layout into a production-ready Rust monorepo following the "1 × HTTP Gateway + N × gRPC Services" architecture pattern:

- `proto/` as the **single source of truth** for all API contracts
- `libs/rpc-proto` for tonic-generated interface code (no business logic)
- `libs/common` for cross-cutting infrastructure (error, config, tracing)
- `apps/api-gateway` as the HTTP entry point (Actix-web, HTTP↔gRPC orchestration)
- `apps/*-service` as independently deployable gRPC microservices

### Key Decisions (User-Confirmed)

1. **Proto codegen**: generate into `OUT_DIR` (not committed)
2. **Domain mapping**: derive from existing code (`user`, `auth`), extensible for future domains
3. **DB/ORM placement**: each `apps/*-service` owns its migrations; `libs/common` provides shared DB pool/config utilities

---

## Current State Analysis

| Current Path      | Status            | Content                                      |
| ----------------- | ----------------- | -------------------------------------------- |
| `crates/gateway`  | Actix HTTP server | routes, handlers, middleware, config, errors |
| `crates/common`   | minimal lib       | `Env` enum only                              |
| `crates/grpc`     | broken            | missing `Cargo.toml`                         |
| `crates/pim-core` | broken            | missing `src/lib.rs`, unresolved deps        |

### Identified Domains (from gateway handlers)

- **auth** — login / register
- **user** — list / get / me

---

## Target Directory Structure

```
pim/
├── Cargo.toml               # workspace: members = ["libs/*", "apps/*"]
├── proto/
│   ├── auth/v1/auth.proto
│   └── user/v1/user.proto
├── libs/
│   ├── rpc-proto/           # tonic codegen only
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   └── src/lib.rs
│   └── common/              # error, config, tracing
│       ├── Cargo.toml
│       └── src/lib.rs
├── apps/
│   ├── api-gateway/         # Actix HTTP gateway
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── auth-service/        # gRPC server (auth domain)
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── user-service/        # gRPC server (user domain)
│       ├── Cargo.toml
│       └── src/main.rs
├── docs/
│   └── design.md            # updated after each phase stabilizes
├── plans/
│   └── 002-monorepo-restructure.plan.md  # this file
└── .github/
    └── copilot-instructions.md  # folder responsibility notes
```

---

## Phased Implementation

### Phase 1: Workspace Stabilization

**Goal**: Make `cargo build` pass with the new directory skeleton.

**Tasks**:

1. Remove broken `crates/grpc` and `crates/pim-core` directories
2. Create `libs/` and `apps/` directories
3. Create minimal `libs/common` (migrate from `crates/common`)
4. Create minimal `libs/rpc-proto` skeleton (empty, no proto yet)
5. Create `apps/api-gateway` skeleton (copy from `crates/gateway`)
6. Update root `Cargo.toml` to `members = ["libs/*", "apps/*"]`
7. Add missing workspace dependencies (`async-trait`, `tonic`, `prost`)
8. Verify `cargo build` succeeds

**Acceptance Criteria**:

- [x] `cargo build` passes
- [x] `cargo run -p api-gateway` starts HTTP server on configured port

---

### Phase 2: Proto & RPC-Proto Setup

**Goal**: Establish proto SSoT and working tonic codegen.

**Tasks**:

1. Create `proto/auth/v1/auth.proto` (Login, Register RPCs)
2. Create `proto/user/v1/user.proto` (GetUser, ListUsers, GetMe RPCs)
3. Implement `libs/rpc-proto/build.rs` with `tonic-build`
4. Re-export generated modules in `libs/rpc-proto/src/lib.rs`
5. Verify `cargo build -p rpc-proto` generates code into `OUT_DIR`

**Acceptance Criteria**:

- [x] `cargo build -p rpc-proto` succeeds
- [x] Generated Rust types are usable from other crates

---

### Phase 3: Infrastructure Consolidation (libs/common)

**Goal**: Move cross-cutting concerns out of gateway into `libs/common`.

**Tasks**:

1. Migrate error types to `libs/common/src/error.rs`
2. Migrate config loading to `libs/common/src/config.rs`
3. Migrate tracing init to `libs/common/src/tracing.rs`
4. Update `apps/api-gateway` to depend on `libs/common`
5. Remove duplicated code from gateway

**Acceptance Criteria**:

- [x] `apps/api-gateway` compiles with `libs/common` dependency
- [x] No duplicate error/config/tracing code in gateway

---

### Phase 4: gRPC Services Implementation

**Goal**: Create independently deployable gRPC microservices.

**Tasks**:

1. Create `apps/auth-service` with tonic server implementing `Auth` service
2. Create `apps/user-service` with tonic server implementing `User` service
3. Each service depends on `libs/rpc-proto` + `libs/common`
4. Add DB pool setup per service (placeholder for migrations)

**Acceptance Criteria**:

- [x] `cargo run -p auth-service` starts gRPC server
- [x] `cargo run -p user-service` starts gRPC server
- [ ] Services respond to gRPC requests (grpcurl / grpcui test)

---

### Phase 5: Gateway Integration

**Goal**: Gateway calls gRPC services instead of inline business logic.

**Tasks**:

1. Add tonic client dependencies to `apps/api-gateway`
2. Replace inline auth handlers with gRPC client calls to `auth-service`
3. Replace inline user handlers with gRPC client calls to `user-service`
4. Keep HTTP-specific concerns (middleware, routing, error mapping) in gateway

**Acceptance Criteria**:

- [ ] HTTP requests to gateway are forwarded to gRPC services
- [ ] End-to-end flow works (HTTP → Gateway → gRPC → Response)

---

### Phase 6: Documentation & Enforcement

**Goal**: Document architecture and enforce boundaries.

**Tasks**:

1. Update `docs/design.md` with final architecture
2. Create `.github/copilot-instructions.md` with folder responsibilities
3. Remove legacy `crates/` directory completely
4. Add CI check for dependency direction (optional)

**Acceptance Criteria**:

- [x] `docs/design.md` reflects actual system
- [x] `.github/copilot-instructions.md` exists with clear guidance
- [x] No files remain under `crates/`

---

## Status Tracking

| Phase   | Status      | Notes                                      |
| ------- | ----------- | ------------------------------------------ |
| Phase 1 | ✅ Complete | Workspace compiles with libs/_ and apps/_  |
| Phase 2 | ✅ Complete | Proto files and rpc-proto codegen working  |
| Phase 3 | ✅ Complete | (Merged with Phase 4)                      |
| Phase 4 | ✅ Complete | auth-service and user-service implemented  |
| Phase 5 | Not Started | Gateway→gRPC integration pending           |
| Phase 6 | ✅ Complete | copilot-instructions.md and design.md done |

---

## Risks & Mitigations

| Risk                                  | Mitigation                                           |
| ------------------------------------- | ---------------------------------------------------- |
| Actix + Tonic runtime conflicts       | Both use Tokio; test early in Phase 4                |
| Proto breaking changes                | Add buf lint/breaking checks in future               |
| Large migration disrupts ongoing work | Phase 1 keeps gateway functional throughout          |
| Missing domain logic during refactor  | Current handlers are stubs; minimal logic to migrate |

---

## Open Items

- [ ] Confirm gRPC port allocation strategy (per-service unique port vs service discovery)
- [ ] Decide on health check / readiness probe pattern for services
- [ ] Plan database migration tooling (sqlx-cli, sea-orm-cli, etc.)
