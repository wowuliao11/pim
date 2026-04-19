# ADR-0005: Bootstrap local Zitadel with a declarative `pim-bootstrap` CLI

- **Status:** Accepted (partially implemented — see "Implementation notes")
- **Date:** 2026-04
- **Deciders:** PIM maintainers

## Context

ADR-0003 commits PIM to Zitadel as its Identity Provider, with both a
self-hosted dev instance and a Zitadel Cloud prod instance as supported
targets. That leaves a concrete operational problem: every developer and
every environment needs the same Zitadel objects provisioned the same
way — a project, an API app with a JWT key, a service account the
user-service uses to call the Management API, and a fixed set of
project roles (`admin`, `member`). Without automation, each developer
clicks through the Zitadel console, copies secrets by hand, and
"works-on-my-machine" drift is inevitable.

Zitadel's own tooling covers the first-boot case: `zitadel
start-from-init --steps <file>` applies a one-shot configuration to an
empty database. It is not a general-purpose provisioner — it runs only
when the Postgres schema is empty, and its surface is limited to
`FirstInstance.*` fields. Anything declared outside that surface
(projects, API apps, roles, service accounts) must be created via the
Management API.

Constraints:

- The same declarative source must cover dev and prod; we do not want
  two divergent provisioning paths.
- Running the tool twice must be safe (developers reset their stack
  often; CI reruns happen).
- Dev secrets land on the local filesystem; prod secrets must be
  pipeable to an external secret manager without ever touching disk.
- Rootless podman on macOS is the common dev environment. This rules
  out anything that needs access to the Docker socket from inside a
  container (see Option C below).

## Decision

A workspace crate at `tools/pim-bootstrap/` provides a single binary
that provisions a Zitadel tenant from a TOML file.

The tool's scope is deliberately narrow:

1. **`bootstrap --config <file>`** — ensure the declared project, API
   app, service account, and roles exist in Zitadel. Idempotent:
   already-present objects are left alone unless `--sync` is passed;
   secrets are minted once and only rotated when `--rotate-keys` is
   passed.
2. **`seed --config <file>`** — create dev human users and role
   assignments. Refuses to run against a prod config.
3. **`diff --config <file>`** — read-only drift report between the
   declarative config and the live tenant. Not wired into CI (see
   ADR-0013 for why).

The first-boot admin identity (`IAM_OWNER`) is dispensed by Zitadel
itself via `FirstInstance` in `bootstrap/steps.yaml`, using the
`Machine.Pat` path. The minted PAT lands on a named volume
(`zitadel-bootstrap`) and is read back by `pim-bootstrap` over an env
var (`ZITADEL_ADMIN_PAT`). Prod flips this: the admin identity is a
pre-provisioned service-account JSON key (`admin_auth = "jwt_profile"`)
that the deployment pipeline mounts from its secret store.

The local Zitadel itself runs via `compose.yml` at the repo root,
derived from Zitadel's official compose pack with four deliberate
deviations documented inline in that file:

- Zitadel's Login service is disabled (no PIM frontend yet relies on
  it).
- No reverse proxy in the dev stack — Zitadel publishes port 18080 on
  the host directly.
- External domain is `pim.localhost` (RFC 6761 loopback; no
  `/etc/hosts` edit required).
- The Zitadel master key is generated once on first `just dev-up` and
  persisted to `.env.local`.

### Idempotency contract

For every object type the tool manages, the ensure-op uses a natural
key (project name, app name, service-account username, role key) to
look up existing state before writing:

- **Missing** → create, record the ID into the appropriate output
  sink.
- **Present and matches spec** → skip. Exit status unaffected.
- **Present but attributes drift** → skip by default; update when
  `--sync` is passed. Diffs logged regardless.
- **Present but conflicting in a way `--sync` cannot reconcile** →
  error with a non-zero exit code and leave the tenant untouched.

Secret material (JWT keys, PATs) is minted at most once per run and
only when the underlying object is newly created; `--rotate-keys`
explicitly overrides this.

## Consequences

**Positive:**

- One source of truth for tenant shape. Dev resets and prod
  deployments apply the same declarative config.
- Safe to rerun. CI can call `pim-bootstrap bootstrap` on every run
  without trashing existing state.
- Secrets split by sensitivity from the start (see ADR-0012) instead
  of retrofitted later.
- Rust-native: the workspace gets one more crate, not a new runtime.

**Negative / accepted trade-offs:**

- We own a CLI. Every new Zitadel primitive PIM needs (IdPs, actions,
  event hooks, …) is code we write rather than a click in the
  console.
- Zitadel Management API coverage in the `zitadel` crate is
  incomplete; some ensure-ops may need hand-rolled gRPC or HTTP
  calls.
- Idempotency is our responsibility. Bugs in the lookup-by-natural-
  key step can produce duplicates.

**Locked in:**

- Natural-key lookup as the idempotency contract. Objects that cannot
  be uniquely identified by a human-meaningful key (e.g. Zitadel
  doesn't expose a stable external ID for some resources) are not
  managed by this tool.
- Dev admin identity is a PAT on a named volume. Rotating the PAT is
  done by resetting the dev stack (`just dev-reset`), not in place.
- Prod admin identity is a service-account JWT profile key supplied
  by the deployment pipeline.

**Follow-up:**

- Phase 3 of the legacy `plans/006-dev-bootstrap.md` (full ensure-ops,
  role assignments, diff reporting) is not yet implemented. Current
  `main.rs` parses the config, logs the plan, and exits.

## Alternatives considered

### Option A — Manual console setup documented in a runbook

Rejected. Works once, drifts forever. Every new developer repeats
every step and copies secrets into their shell. Prod and dev inevitably
diverge.

### Option B — Extend `FirstInstance` in `steps.yaml` to cover everything

Rejected. `FirstInstance` only runs when the Postgres schema is empty,
so it cannot be the ongoing reconciliation path. Its schema also does
not cover all the objects we need (API apps with JWT keys, role
assignments for seed users). It stays in scope for exactly what it is
designed for: minting the first admin identity.

### Option C — Terraform with the `zitadel/zitadel` provider

Rejected for PIM's current scale. Terraform is a strong option for a
larger ops footprint, but it introduces a second language, a second
dependency manager, and state-file concerns (remote backend, locking)
that PIM does not need yet. Revisit if we grow beyond one Zitadel
tenant or pick up more SaaS dependencies worth managing declaratively.

### Option D — Shell + `curl` + `zitadel-tools`

Rejected. Bash scripts calling the Management API over curl are
reachable but painful: no type checking on the JSON payloads, no
structured error handling, and idempotency becomes `if grep …` chains
that are easy to get wrong.

### Option E — Keep Traefik in front of Zitadel

Rejected for the dev stack. Rootless podman's socket is owned by the
VM user (UID 502 on macOS), and Traefik's docker provider cannot read
it from inside the container. Publishing `zitadel-api` on port 18080
directly is simpler and removes a failure mode. When a second HTTP
service enters the dev stack, Caddy with a static config is the
expected successor.

## Implementation notes

Scope of what has landed:

- `tools/pim-bootstrap/src/{cli,config,lib,main}.rs` — CLI surface,
  TOML schema, binary entry point. `main.rs:53-82` currently logs the
  plan and exits without calling Zitadel; ensure-ops land later.
- `bootstrap/dev.toml`, `bootstrap/prod.example.toml` —
  declarative configs for the two target environments.
- `bootstrap/seed.dev.toml` — dev-only human users.
- `bootstrap/steps.yaml` — `FirstInstance.Machine.Pat` minting for the
  admin identity.
- `compose.yml` — local Zitadel + Postgres, four documented
  deviations from the upstream pack.

Pending in a future change (tracked in the shortened
`plans/006-dev-bootstrap.md`): ensure-ops that actually call the
Zitadel Management API, the `diff` read path, and the `just dev-up` /
`just dev-reset` recipes that wrap the tool.

## References

- Source code:
  - `tools/pim-bootstrap/src/cli.rs:52-103` — subcommand surface
  - `tools/pim-bootstrap/src/config.rs:92-122` — `OutputSinks` split
    by sensitivity
  - `tools/pim-bootstrap/src/config.rs:177-206` — `BootstrapConfig`
    load + admin-auth validation
  - `tools/pim-bootstrap/src/main.rs:25-92` — dispatch, dev-only seed
    guard
  - `bootstrap/dev.toml` — the dev declarative config
  - `bootstrap/steps.yaml:17-35` — `FirstInstance.Machine.Pat` shape
    (verified against `zitadel/zitadel@v4.13.0 cmd/setup/03.go`)
  - `compose.yml:1-60` — Zitadel + Postgres stack and its deviations
- External:
  - [Zitadel `FirstInstance` docs](https://zitadel.com/docs/self-hosting/manage/configure)
  - [Zitadel official compose pack](https://github.com/zitadel/zitadel/tree/main/deploy/compose)
  - RFC 6761 §6.3 (`localhost` name reservation)
- Originated from: `plans/006-dev-bootstrap.md` at commit `3ee9cc8`.
  Absorbs decisions D1, D2, D4, D6, D11, D12 (which superseded D3),
  D14, D15, D16, and the Idempotency Contract section.
- Related: ADR-0003 (why Zitadel at all), ADR-0004 (how services
  validate tokens), ADR-0007 (user-service as Management API proxy),
  ADR-0009 (why `tools/pim-bootstrap/` sits where it does),
  ADR-0012 (three-layer config split by sensitivity), ADR-0013
  (dev-prod parity via the same declarative config).
