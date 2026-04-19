# ADR-0009: Adopt `apps/` + `libs/` + `proto/` monorepo layout with tonic codegen-only crate

- **Status:** Accepted
- **Date:** 2026-01 (restructure landed early in project history; layout
  has since been extended with `tools/`)
- **Deciders:** PIM maintainers
- **Supersedes:** the original `crates/{gateway,common,grpc,pim-core}`
  flat layout

## Context

The repository started as a flat `crates/*` workspace with four members:
`gateway` (Actix HTTP server with inline business logic), `common` (a
single `Env` enum), `grpc` (broken — missing `Cargo.toml`), and
`pim-core` (broken — missing `src/lib.rs`). Two of four crates did not
even compile, `common` held almost nothing, and all domain logic lived
inside the gateway binary.

We needed a layout that could grow to the intended "1 × HTTP gateway +
N × gRPC services" shape without every service re-inventing its own
proto codegen, config loader, or tracing init. We also needed a single
home for `.proto` files so that contract changes were visible in one
place instead of scattered across service crates.

## Decision

Adopt a three-bucket workspace layout, with each bucket having a single
well-defined role:

```
pim/
├── Cargo.toml             # workspace = ["libs/*", "apps/*", "tools/*"]
├── proto/                 # Protobuf IDL — SINGLE SOURCE OF TRUTH
│   └── user/v1/user.proto
├── libs/                  # shared, non-binary crates
│   ├── rpc-proto/         # tonic-generated code only, zero business logic
│   ├── infra-config/
│   ├── infra-auth/
│   └── infra-telemetry/
├── apps/                  # deployable binaries
│   ├── api-gateway/       # Actix HTTP → gRPC orchestrator
│   └── user-service/      # Tonic gRPC server
└── tools/                 # operator / dev-time CLIs
    └── pim-bootstrap/
```

Rules that make this layout load-bearing rather than cosmetic:

- **`proto/` is the single source of truth** for every wire contract.
  Service crates do not own their `.proto` files.
- **`libs/rpc-proto/` is the only crate that runs `tonic-build`.** It
  generates into `OUT_DIR` (not committed) and re-exports the generated
  modules. Every other crate consumes these types; nobody else calls
  `tonic-build`.
- **Dependency direction is one-way:** `apps/*` may depend on
  `libs/*`; `libs/*` must not depend on `apps/*`; `libs/*` crates may
  depend on each other but should minimise it.
- **`libs/*` crates are infrastructure, not domain.** They hold
  cross-cutting concerns (auth, config, telemetry, wire types). Domain
  logic lives in `apps/*`.
- **`apps/*` crates are deployable.** Each produces a binary; each owns
  its own config schema, migrations, and deployment concerns.
- **`tools/*` holds CLIs that are neither long-running services nor
  shared libraries** (e.g. `pim-bootstrap` for setting up local
  Zitadel).

## Consequences

**Positive:**

- Contract changes are visible in one directory (`proto/`) in PRs.
- Adding a new service means creating `apps/new-service/` and letting
  it depend on `libs/rpc-proto` — no duplicated codegen setup.
- `libs/infra-*` crates centralise the "every service needs this"
  machinery: config loading, tracing init, Zitadel introspection,
  Prometheus recorder. Services become thin.
- The layout is self-documenting: `apps/` names map to deployable
  units, `libs/infra-*` names describe what cross-cutting concern each
  owns.

**Negative / accepted trade-offs:**

- More directories than a flat `crates/*` layout. For a repo with two
  apps this is overhead; it pays off at three or more.
- The dependency-direction rule (`libs/` must not depend on `apps/`)
  is enforced by convention and code review, not by a build check.
  Violations would show up as circular dependency errors in `cargo
  build`, which is a late warning.
- `proto/` lives outside both `libs/` and `apps/`. This is
  intentional — it is neither a Rust crate nor a deployable — but it
  does mean tooling has to special-case the directory.

**Locked in:**

- Workspace members are **exactly** `libs/*`, `apps/*`, `tools/*`. New
  code goes in one of these three. Do not add top-level crates outside
  this pattern.
- A new service is always `apps/<name>-service/` (gRPC) or
  `apps/<name>-gateway/` (HTTP); a new shared concern is always
  `libs/infra-<concern>/` or `libs/<concern>/`.
- `tonic-build` runs only in `libs/rpc-proto/build.rs`. If a crate
  needs codegen for some other IDL, it must be a new ADR.

## Alternatives considered

### Option A — Keep flat `crates/*` layout

Rejected. Two of the original four crates were broken and the layout
gave no signal about which crate was deployable versus shared. Growing
to "gateway + N services" in `crates/*` would have produced
`crates/gateway/`, `crates/auth-service/`, `crates/user-service/`,
`crates/common/`, etc. — a flat list where humans and agents have to
read each crate's `Cargo.toml` to know its role. `apps/` vs `libs/`
makes role visible in the path.

### Option B — Per-service proto ownership

Rejected. If `apps/user-service/proto/` owned the user contract and
`apps/api-gateway/` had to import it, gateway-service dependency
direction would invert ("gateway depends on service internals"). It
would also scatter contract evolution across multiple directories. A
central `proto/` tree keeps contracts reviewable in isolation.

### Option C — Committed generated Rust code

Rejected. Generating into `OUT_DIR` rather than committing keeps
`rpc-proto` small and avoids the "regenerate and diff" noise in PRs.
The cost — every clean build regenerates — is negligible for the
current contract size. If regeneration becomes slow, we can revisit
(`buf generate` with committed output is a viable future path).

### Option D — One crate per domain aggregating both proto and service

Rejected at the time; partially revisited. The argument for this
layout is "user domain is one thing, keep it in one crate". The
argument against is that a gRPC service and the gateway both need the
generated types — forcing the gateway to depend on a service crate
couples them. The `rpc-proto` codegen-only crate breaks this coupling
cleanly. See also ADR-0006 (why we do not have a separate
`auth-service` crate today).

## Implementation notes

- Workspace config: `Cargo.toml:1-2` — `members = ["libs/*", "apps/*",
  "tools/*"]`.
- Codegen: `libs/rpc-proto/build.rs`, `libs/rpc-proto/src/lib.rs`.
- Current services: `apps/api-gateway/`, `apps/user-service/`.
- Current infra crates: `libs/infra-auth/`, `libs/infra-config/`,
  `libs/infra-telemetry/`.
- Current tools: `tools/pim-bootstrap/`.
- Note: the original plan listed `libs/common`; this was later split
  into `libs/infra-config`, `libs/infra-telemetry`, and
  `libs/infra-auth` as cross-cutting concerns became distinct enough
  to separate. The `libs/<concern>` naming survived; the monolithic
  `common` did not.

## References

- Source code: paths above.
- External: [Rust Cargo workspaces
  documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html),
  [tonic-build
  docs](https://docs.rs/tonic-build/latest/tonic_build/).
- Originated from: `plans/002-monorepo-restructure.plan.md` at the
  workspace-restructure commits.
- Related: ADR-0005 (bootstrap tool lives in `tools/`),
  ADR-0006 (why `auth-service` was removed rather than created as an
  app).
