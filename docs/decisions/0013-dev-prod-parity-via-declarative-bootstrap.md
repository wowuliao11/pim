# ADR-0013: Use one declarative config shape for dev and prod, with drift as a manual ops tool

- **Status:** Accepted
- **Date:** 2026-04
- **Deciders:** PIM maintainers

## Context

ADR-0005 introduces `pim-bootstrap` as the one tool that provisions
Zitadel for PIM. That leaves two follow-on questions this ADR answers:

1. Does dev run against the same Zitadel offering as prod?
2. What is the relationship between the declarative config and the
   live tenant over time — do we enforce no drift in CI, or not?

The naive "no drift" answer is to run `pim-bootstrap diff` in CI and
fail the build on any delta between the repo's declarative config and
the live prod tenant. That sounds like parity but produces a
pathological loop: every manual ops action (create a break-glass user,
add a temporary IdP while debugging a federation issue, rotate a
compromised secret ahead of the next scheduled rotation) breaks CI
until someone reverse-engineers the change back into the repo. The
incentive to bypass the tool entirely becomes overwhelming.

The naive "no enforcement" answer is to let dev and prod drift freely.
That surrenders the entire reason we wrote the tool.

We want something in between: one declarative source that is the
*origin* of prod state, with drift *observable* but not enforced.

## Decision

**Dev targets a self-hosted Zitadel; prod targets Zitadel Cloud. Both
are driven by `pim-bootstrap` against configs with the same shape.**

`bootstrap/dev.toml` and `bootstrap/prod.example.toml` share the
`BootstrapConfig` schema (`tools/pim-bootstrap/src/config.rs:113-122`).
The differences between them are exactly the differences between the
two environments and nothing else:

| Axis             | Dev (`dev.toml`)              | Prod (`prod.example.toml`)          |
|------------------|-------------------------------|-------------------------------------|
| Target           | Self-hosted via `compose.yml` | Zitadel Cloud                        |
| Authority        | `http://pim.localhost:18080`  | `https://<tenant>.zitadel.cloud`     |
| Admin auth       | PAT (`admin_auth = "pat"`)    | JWT Profile (`admin_auth = "jwt_profile"`) |
| Output: configs  | `apps/<svc>/config.toml`      | `deploy/prod/<svc>.config.toml`      |
| Output: key      | `zitadel-key.json`            | `stdout:jwt_key` (→ secret manager)  |
| Output: env file | `.env.local`                  | `stdout:pat` (→ secret manager)      |

Everything else — project name, API app name, roles, service account
username — is identical. That identity is the parity contract.

**Drift is detected by `pim-bootstrap diff`, run manually. It is not
wired into CI.** Operators invoke it (e.g. via a `just prod-diff`
recipe) when they want to know the answer. The output is informational,
not a gate.

This is a deliberate asymmetry:

- The *forward* direction (repo → Zitadel) is the enforced contract:
  CI runs `pim-bootstrap bootstrap --dry-run` on prod configs to catch
  structural errors, and deployments invoke the non-dry-run path.
- The *reverse* direction (Zitadel → repo) is not enforced. Manual
  ops actions in the Zitadel console are legitimate; drift reporting
  exists so operators choose when to reconcile.

## Consequences

**Positive:**

- Dev and prod share the same tool, same schema, same object graph.
  A bug reproducible against `bootstrap/dev.toml` reproduces against
  `bootstrap/prod.example.toml`.
- Emergency ops actions are not blocked by CI. Break-glass is a
  first-class operation, not a workaround.
- The cost of switching between dev and prod is configuration, not
  code.

**Negative / accepted trade-offs:**

- Drift is real. Over weeks, prod can accumulate hand-edits the repo
  doesn't know about. The expectation (not yet enforced by anything
  other than this ADR) is that operators run `pim-bootstrap diff`
  before and after any manual ops action, and reconcile promptly.
- Zitadel Cloud outages affect prod auth. We accept this under
  ADR-0003 and do not mitigate it here.
- Two destinations for real configs (`apps/<svc>/config.toml` in dev
  vs `deploy/prod/<svc>.config.toml` in prod) means any tool that
  reads these paths has to know the environment. Accepted.

**Locked in:**

- Dev Zitadel is self-hosted. We do not have individual developers
  connecting to a shared dev Zitadel Cloud tenant; reset-the-world
  is a local operation.
- Prod Zitadel is Zitadel Cloud. Switching to self-hosted prod later
  is possible but requires a new ADR documenting the operational
  model (backups, upgrades, HA).

**Follow-up:**

- A `just prod-diff` recipe wrapping `pim-bootstrap diff
  --config bootstrap/prod.toml` once the full `diff` implementation
  lands (tracked in the shortened `plans/006-dev-bootstrap.md`).

## Alternatives considered

### Option A — Dev connects to a shared dev Zitadel Cloud tenant

Rejected. Cross-developer collisions on shared state (user A's seed
step clobbers user B's test data), no offline development, and a
monthly bill. The self-hosted dev instance costs a compose stack; the
cost of shared cloud dev is friction every day.

### Option B — `pim-bootstrap diff` enforced in CI

Rejected. Incentive failure: turns every legitimate manual ops action
into a CI outage. Teams that try this either accumulate bypass
mechanisms (skip-CI labels, emergency merges) or stop doing ops
actions through the console entirely even when that is the right
tool. We want drift *visible*, not *fatal*.

### Option C — Prod self-hosted on day one

Rejected for PIM's current scale. Self-hosting Zitadel in prod means
owning HA, backups, upgrades, TLS rotation, and patching. Zitadel
Cloud takes all of that. When PIM has an operational reason to
self-host prod (data sovereignty, cost crossover, SLA control), that
becomes its own ADR.

### Option D — Per-developer prod-shaped tenants

Rejected. Every developer with a personal Zitadel Cloud tenant is a
licensing and secret-hygiene liability. The local compose stack
covers the same need with zero external dependencies.

## References

- Source code:
  - `bootstrap/dev.toml` — dev config, self-hosted target
  - `bootstrap/prod.example.toml` — prod config shape, Zitadel Cloud
    target
  - `tools/pim-bootstrap/src/config.rs:113-122` — `BootstrapConfig`
    schema shared by both
  - `tools/pim-bootstrap/src/cli.rs:98-103` — `Diff` subcommand
    (read-only)
- Originated from: `plans/006-dev-bootstrap.md` at commit `3ee9cc8`.
  Absorbs decisions D6 (dev/prod same binary, seed refuses prod),
  D8 (prod targets Zitadel Cloud), D9 (drift reporting is manual,
  not in CI).
- Related: ADR-0003 (Zitadel as IdP), ADR-0005 (the bootstrap tool),
  ADR-0012 (three-layer config; same layering, different
  destinations per environment).
