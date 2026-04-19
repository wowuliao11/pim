# ADR-0015: Allocate service ports with a fixed, dev/prod-parity policy

- **Status:** Accepted
- **Date:** 2026-04-19
- **Deciders:** PIM maintainers

## Context

Port allocation had drifted across the repository. Four independent sources
of truth disagreed on basic facts:

- `apps/user-service/src/config.rs:40-43` and
  `apps/user-service/config.toml:19-21` defaulted to gRPC port **50052**.
- `apps/api-gateway/src/config/settings.rs:37-39` defaulted its
  `user_service_url` to `http://127.0.0.1:50051`.
- `compose.yml:103-106` published `user-service` on the host as
  `50051:50051`.
- `docs/design.md:86`, `README.md:35-36`, and
  `.github/copilot-instructions.md:136-138` all documented the user-service
  gRPC port as `:50051`, and `copilot-instructions.md` additionally listed
  a long-removed `auth-service` on `:50051` (ADR-0006 deleted that service).

On top of that, `compose.yml` double-bound the host port `18080`:
`zitadel-api` exposed `18080:8080` (`compose.yml:51-52`, load-bearing per
ADR-0005 — `authority = "http://pim.localhost:18080"`), and the stub
`pim-api-gateway` service under the `app` profile also exposed `18080:8080`
(`compose.yml:86-87`). Bringing both up would fail on bind.

The root cause is that port numbers are an **operational invariant** (see
`docs/decisions/README.md` §"When to write an ADR") and were never
centrally declared. Each surface independently made a plausible local
choice, and the choices diverged.

Constraints that shape the policy:

- ADR-0005 locks Zitadel to `pim.localhost:18080` on the host. That slot is
  not negotiable.
- ADR-0013 requires the dev and prod stacks to run the same binaries with
  the same shape of configuration. That means **container-internal** ports
  must be identical in dev and prod; only host-side publishing is allowed
  to differ.
- ADR-0011 already separates metrics onto their own port (`metrics_port`)
  so that Prometheus scraping does not share a listener with business
  traffic. That convention stays.
- ADR-0012 splits config by sensitivity. Port numbers are Layer A
  (non-secret), so they belong in `config.example.toml` and in code
  defaults.

## Decision

Adopt a single, fixed port policy covering container-internal listeners,
host-published ports, and metrics ports. The policy is documented here
and mirrored in `docs/design.md` §"Port Allocation".

### Container-internal listeners (identical in dev and prod)

| Service        | Port    | Protocol |
| -------------- | ------- | -------- |
| `api-gateway`  | `8080`  | HTTP     |
| `user-service` | `50051` | gRPC     |
| `zitadel`      | `8080`  | HTTP     |

Rationale: `8080` is the Actix/HTTP convention; `50051` is the canonical
gRPC port used in the tonic ecosystem and documentation. Making these
identical across dev and prod means a config file authored for prod runs
unchanged against the dev compose stack.

### Host-published ports (dev compose only)

| Service        | Host port | Container port | Rationale                          |
| -------------- | --------- | -------------- | ---------------------------------- |
| `zitadel`      | `18080`   | `8080`         | Locked by ADR-0005                 |
| `api-gateway`  | `18000`   | `8080`         | `1xxxx` = local public HTTP ingress |
| `user-service` | —         | —              | **Not published.** Access via compose network only (`pim-user-service:50051`). |

Rationale: The `1xxxx` range is reserved here for "local developer-facing
HTTP entrypoints" — Zitadel on `18080`, our gateway on `18000`. Internal
gRPC services stay on the compose network. Not publishing `user-service`
to the host follows the principle of least exposure and matches how it
would be deployed in prod (internal only, reached by the gateway).

### Metrics ports

| Service        | Metrics port |
| -------------- | ------------ |
| `api-gateway`  | `60080`      |
| `user-service` | `60051`      |

Convention: `60000 + <service port % 1000>`. Keeps metrics ports distinct
per service and trivially derivable from the service port. Existing
gateway value `60080` already follows the pattern; user-service moves
from `60052` to `60051` to stay consistent now that its service port is
`50051`.

### Cross-service addressing

- In the compose network: `api-gateway` reaches user-service via the
  service DNS name — `http://pim-user-service:50051`.
- For local `cargo run` (outside compose): the same URL form applies —
  `http://127.0.0.1:50051` — since the container-internal port and the
  local-run port are intentionally the same.

The `user_service_url` default in gateway code is kept as
`http://127.0.0.1:50051` (the `cargo run` case). The compose file
overrides it via environment variable to the service-DNS form. This is
how ADR-0013 parity is preserved: code defaults target the host-less
developer loop; compose/prod override via env.

## Consequences

**Positive:**

- One place (this ADR, mirrored in `docs/design.md`) is the source of
  truth for ports. Drift between `compose.yml`, TOML defaults, Rust
  defaults, and docs becomes a lint-level concern instead of a design
  question.
- `podman compose --profile app up -d` no longer double-binds `18080`.
  Zitadel and the gateway can run simultaneously.
- `user-service` defaults in code, config, compose, and docs all agree
  on `50051`.
- `user-service` is not exposed to the host in dev, matching its prod
  posture. Developers who need to poke it directly use `podman exec` or
  `grpcurl` through the compose network.

**Negative / accepted trade-offs:**

- Developers who had previously been talking to `localhost:50052` (the
  old user-service default) or `localhost:50051` (the old publish) need
  to update their scripts. The breakage is contained to a single repo
  and a single service.
- The `1xxxx` host range convention is a local convention, not an
  industry standard. Contributors need to read this ADR (or the mirror
  in `docs/design.md`) to know why `18000` is not `8080`.

**Locked in:**

- `pim.localhost:18080` as the Zitadel dev authority (already locked by
  ADR-0005).
- `api-gateway` container-internal listener on `8080`.
- `user-service` container-internal listener on `50051`.
- Metrics port convention (`60000 + service_port % 1000`).
- `user-service` is compose-network-only in dev; no host publish.

**Follow-up:**

- None required. The policy is applied in the same PR that records this
  ADR.

## Alternatives considered

### Option A — Keep `user-service` on `50052`, fix everything else to `50052`

Rejected. It would require changing the gateway default, the compose
publish, `docs/design.md`, `README.md`, and
`.github/copilot-instructions.md`, plus onboarding tutorials. The only
surface currently using `50052` is the `user-service` crate itself;
aligning to `50051` changes one crate's defaults and is consistent with
the tonic ecosystem's default port.

### Option B — Let the gateway share host port `8080`

Rejected. `8080` on the host clashes with a wide range of common local
tooling (other dev stacks, Zitadel running in dev mode outside compose,
etc.). Keeping host-side entrypoints in the `1xxxx` range makes the PIM
stack coexist with other projects without collisions and groups "my PIM
endpoints" under a visually distinct prefix.

### Option C — Publish `user-service` to the host on `50051`

Rejected for the default path. Publishing an internal gRPC service to
the host is useful during ad-hoc debugging but is not how it runs in
prod, which violates ADR-0013 dev-prod parity. Developers who need
direct access can either:

- run `user-service` outside compose via `cargo run -p user-service`
  (talks on `127.0.0.1:50051`), or
- use `podman compose exec pim-api-gateway grpcurl …` against the
  internal DNS name.

## Implementation notes

This ADR is applied in the same commit that records it. The concrete
surface touched:

- `compose.yml`
  - `pim-api-gateway` host publish changed from `18080:8080` to
    `18000:8080`.
  - `pim-user-service` no longer publishes to the host.
  - `pim-api-gateway` gains
    `APP__APP__USER_SERVICE_URL=http://pim-user-service:50051` so the
    gateway targets the service-DNS form on the compose network.
- `apps/user-service/src/config.rs` — default `port` changes `50052 →
  50051`, default `metrics_port` changes `60052 → 60051`.
- `apps/user-service/config.example.toml`,
  `apps/user-service/config.toml` — same values updated.
- `apps/api-gateway/src/config/settings.rs` — `default_user_service_url`
  unchanged (`http://127.0.0.1:50051`), but the comment is clarified.
- `docs/design.md` — adds the port policy table and points at this ADR.
- `docs/configuration.md` — TOML examples updated.
- `README.md` — quick-start port references updated and the stale
  `auth-service` line removed.
- `.github/copilot-instructions.md` — port table updated, stale
  `auth-service` row removed (ADR-0006).

## References

- Source code:
  - `compose.yml:28-110` — Zitadel, gateway, user-service compose wiring
  - `apps/user-service/src/config.rs:36-47` — user-service defaults
  - `apps/api-gateway/src/config/settings.rs:29-50` — gateway defaults
  - `apps/user-service/src/main.rs:370-407` — user-service bind
  - `apps/api-gateway/src/main.rs:16-72` — gateway bind + user-service client
- Related ADRs:
  - ADR-0005 (local Zitadel on `18080`, dev stack shape)
  - ADR-0006 (auth-service removed — stale port row cleanup)
  - ADR-0011 (metrics on a dedicated port)
  - ADR-0012 (config split by sensitivity — port is Layer A)
  - ADR-0013 (dev-prod parity via the same declarative config)
