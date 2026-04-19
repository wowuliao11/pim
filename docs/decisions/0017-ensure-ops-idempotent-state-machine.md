# ADR-0017: Model ensure-ops as a four-state machine over natural keys

- **Status:** Proposed
- **Date:** 2026-04-19
- **Deciders:** PIM maintainers

## Context

ADR-0005 commits `pim-bootstrap` to a short, verbal idempotency
contract:

> Missing → create. Present and matches spec → skip. Present but
> attributes drift → skip by default, update when `--sync` is passed.
> Present but conflicting in a way `--sync` cannot reconcile → error.
> Secrets are minted once and only rotated when `--rotate-keys` is
> passed.

That contract is correct but informal. Phase 3 of the tool
(`tools/pim-bootstrap/src/main.rs:57`) still logs the plan and returns
before calling Zitadel; the actual ensure-ops must land next. Before
that code is written, we need one unambiguous answer to:

1. What are the states, and what makes one state transition to another?
2. What does "matches spec" mean precisely, per object type, when the
   server stores fields the config does not know about (IDs, timestamps,
   internal enums) and the config stores fields the server does not
   echo back?
3. When a decision depends on secret material (JWT key, PAT,
   client_secret), how does the state machine interact with
   `--rotate-keys` and with the one-shot nature of those values?
4. What exit code does `pim-bootstrap bootstrap` produce in each
   terminal state, and what does `pim-bootstrap diff` emit in each?

Without a codified answer, two developers implementing different
ensure-ops will make subtly different decisions (e.g. one treats a
missing `description` field as drift, the other does not), and the
"idempotent" guarantee degrades to "idempotent if you squint". The
server's response model (Zitadel Management API error codes, list
pagination, name-uniqueness semantics) is captured in ADR-0016 and is
assumed here; this ADR covers the tool-side policy that consumes those
server responses.

Relevant constraints already decided:

- **Natural keys only.** ADR-0005 locks in "lookup by human-meaningful
  key": `project.name`, `api_app.name`, `service_account.username`,
  `role.key`. Objects that cannot be uniquely identified by such a key
  are out of scope for this tool.
- **Three output layers.** ADR-0012 splits provisioned values into
  Layer A (non-secret TOML), Layer B (JWT key JSON), Layer C (env
  file). The state machine must know which layer each transition
  writes to, because rotation rules differ per layer.
- **Dev/prod schema parity.** ADR-0013 requires the same state machine
  to run against `bootstrap/dev.toml` and `bootstrap/prod.example.toml`.
  The only difference is sink destinations (file vs `stdout:<tag>`),
  not state transitions.
- **CLI surface is frozen for now.** `tools/pim-bootstrap/src/cli.rs:54`
  exposes `bootstrap --sync --rotate-keys --dry-run --env`. These flags
  are the inputs to the state machine; adding new ones requires an
  amending ADR.

## Decision

**Every ensure-op is a four-state machine keyed by a single natural
key per object type. States are terminal in one run; they are
re-evaluated from scratch on the next run.** Transitions, exit codes,
and sink writes are fixed per `(state, flag combination)` pair, and
the matrix is the same for every object type `pim-bootstrap` manages.

### States

| State          | Definition                                                                                                    |
|----------------|---------------------------------------------------------------------------------------------------------------|
| `Missing`      | Zitadel has no object with this natural key after paginating through all results.                              |
| `Match`        | Zitadel has exactly one object with this natural key, and every spec-declared field equals the server value.  |
| `Drift`        | Zitadel has exactly one object with this natural key, but at least one spec-declared field differs.            |
| `Conflict`     | Zitadel refused a transition the state machine believed was safe (e.g. name collision across a scope we don't model). Always terminal-error. |

`Match` is the only state that requires a positive check of every
spec-declared field. Server-managed fields (IDs, create timestamps,
`details.sequence`) are ignored. Fields the config marks optional with
`#[serde(default)]` are ignored when absent in the spec regardless of
the server value — the spec is the authority for what we care about.

### Transitions

The matrix below applies uniformly to project / api-app / service-
account / project-role / user-grant ensure-ops. "Write" means a call
to the Zitadel Management API that changes state; "Emit" means writing
to the appropriate output sink (per ADR-0012).

| State      | `--sync` | `--rotate-keys` | Action                                                                                                     | Exit |
|------------|----------|-----------------|------------------------------------------------------------------------------------------------------------|------|
| Missing    | any      | any             | **Create** object. On create, **emit** any one-shot secrets (JWT key, client_secret, PAT) to their sinks.   | 0    |
| Match      | any      | `false`         | **Skip**. Emit nothing. Log `object=<kind> key=<value> action=skip reason=match`.                           | 0    |
| Match      | any      | `true`          | For objects with rotatable secrets (api-app JWT key, service-account key): **rotate** the secret, **emit** the new one to the sink. For objects without rotatable secrets (project, role, user-grant): skip. | 0    |
| Drift      | `false`  | any             | **Skip**. Log `object=<kind> key=<value> action=skip reason=drift fields=<…>`. Exit 0 — drift is observable but not fatal (mirrors ADR-0013's dev/prod drift policy). | 0    |
| Drift      | `true`   | any             | **Update** spec-declared fields. For fields the Management API does not accept in an update call, the transition escalates to `Conflict`. | 0    |
| Conflict   | any      | any             | **Abort the run.** Do not touch any subsequent ensure-op in the batch. Leave Zitadel untouched for this object. | non-zero |

"Skip" is the default for every non-missing case. `--sync` opts into
writes; `--rotate-keys` opts into secret rotation. This means the
zero-flag run is observation-only after the first bootstrap, which is
the property that makes CI and re-runs safe.

### One-shot secret rule

Zitadel returns private key material / client secrets exactly once, at
creation time. The state machine treats those outputs as
**write-once-per-create** to Layer B and Layer C sinks:

- On `Missing → Create`: emit the freshly-minted secret to the sink,
  overwriting whatever is there. This is the only transition that
  writes secrets in the default run.
- On `Match` or `Drift` with `--rotate-keys`: call the rotation
  endpoint (delete old key + create new key, or equivalent), emit the
  new value to the sink, discard the old.
- No other transition writes secrets. This is the property
  `--rotate-keys` guards.

For prod, Layer B/C sinks are `stdout:<tag>` sentinels (ADR-0012); the
state machine's output contract is unchanged — it emits to the sink;
what that sink does with the bytes is the operator's problem.

### Batch semantics

`pim-bootstrap bootstrap` runs ensure-ops in a fixed topological order
dictated by Zitadel's object graph (project → api-app → service-
account → project-roles → user-grants). The batch is **not
transactional** because Zitadel has no cross-resource transaction
primitive. Instead:

- Transitions are **append-only** within a run: a later ensure-op may
  read IDs minted by an earlier one, but never mutates the earlier
  one.
- On `Conflict`, the batch aborts. Ensure-ops that ran before the
  conflict keep their writes; ensure-ops after it do nothing. This is
  acceptable because every ensure-op is itself idempotent — the next
  run resumes from the partial state.

### `dry_run`

`bootstrap --dry-run` runs every state classification (list Zitadel
objects, compare to spec) but emits no writes and no sink emissions.
The output is the action list the non-dry-run path *would* execute.
This is what CI invokes on prod configs per ADR-0013.

### `diff` subcommand

`pim-bootstrap diff --config …` is a read-only sibling: it reports
every object's state and the diff fields when state is `Drift`. It
never writes. Output is machine-parseable (planned: JSON); exit is
always 0 unless the tool itself errors, per ADR-0013's "drift
observable, not fatal" policy.

### What the tool refuses to do

- Creating objects without a natural key in the spec. The spec is the
  source of truth; auto-generated names are out of scope.
- Deleting objects that are not in the spec. This is not drift
  reconciliation; we are not a garbage collector. Operators delete by
  hand.
- Updating objects whose spec drift requires destroying and recreating
  the object (e.g. changing `api_app.auth_method`). That is a
  `Conflict` and must be resolved in Zitadel by hand, then re-run.

## Consequences

**Positive:**

- "Idempotent" becomes a checkable property, not a vibe. Each
  transition is a single cell in the matrix above; a new ensure-op
  cannot invent new behavior without amending this ADR.
- Re-running `pim-bootstrap bootstrap` without flags is exactly a
  health check after first-run. No writes, no surprises, safe in CI.
- `--sync` is a surgical tool, not a hammer: it updates declared
  fields, not every server field. Drift in fields the tool doesn't
  care about stays ignored.
- `--rotate-keys` is cleanly separated from `--sync`. Rotating a
  secret is never a side effect of a field update, and vice versa.
- The matrix is the same for every object type, so one bug in the
  state classifier fixes every ensure-op.

**Negative / accepted trade-offs:**

- "Drift → skip" by default means a prod tenant can accumulate
  declarative-vs-reality drift indefinitely. ADR-0013 accepts this;
  the mitigation is `pim-bootstrap diff` run by an operator, not CI
  enforcement.
- `Conflict` aborts the batch rather than rolling back, so a partial
  state can persist between runs. This is the price of Zitadel having
  no cross-resource transaction; idempotency makes the next run clean
  it up.
- Fields the API does not echo on read (e.g. the API-app client
  secret) cannot be classified as `Match` vs `Drift` from the server
  response alone. For those fields, we treat `Match` conservatively:
  if the natural key matches and all server-echoed fields match, state
  is `Match`, regardless of whether the local sink has the secret on
  disk. Recovering a lost secret is a `--rotate-keys` operation, not a
  state-machine problem.

**Locked in:**

- Four states, no more. Adding a fifth (e.g. "Quarantined") requires
  a new ADR.
- `--sync` and `--rotate-keys` are the only write-enabling flags.
  Additional flags are adjustments to the matrix and require amending
  this ADR.
- Natural-key lookup is the only object-identity strategy. Storing
  server-minted IDs in the repo to short-circuit lookup is out of
  scope (violates ADR-0005).

**Follow-up:**

- Implement the state classifier and transition dispatcher in Phase 3
  (`tools/pim-bootstrap/src/orchestrator/` or similar). The shape is
  a trait per ensure-op that exposes `list()`, `matches(&spec)`,
  `create()`, `update()`, `rotate()` and a generic driver that walks
  the matrix.
- Unit-test the transition matrix exhaustively with a fake Zitadel
  client (one test per `(state, sync, rotate_keys)` cell, 16 cells
  per object type).
- Revisit once Zitadel exposes bulk transactional endpoints; the
  batch semantics above could tighten to "all-or-nothing".

## Alternatives considered

### Option A — Three-state machine (Missing / Present / Error)

Rejected. Collapses `Match` and `Drift` into one state, which defeats
the purpose of `--sync`: the tool cannot tell the operator what it
*would* change without actually changing it. `diff` becomes less
informative, and "idempotent" becomes "idempotent except when the spec
changed, then who knows".

### Option B — Last-write-wins reconciliation (Terraform-style)

Rejected. Matches ADR-0005's rejection of Terraform: we do not want
unattended drift reconciliation to be the default, because it makes
legitimate manual ops actions (break-glass users, emergency IdPs) into
bugs that CI silently reverts. We want drift *reported*, not
*reconciled*.

### Option C — Per-object-type bespoke state machines

Rejected. Every ensure-op would grow its own notion of "matches" and
"drift", which is how the "idempotent if you squint" failure mode
starts. The four-state matrix is uniform because the policy is
uniform; object-type specifics live in the `matches(&spec)` predicate
each trait implementation provides, not in the state graph.

### Option D — Treat secrets as first-class state

Considered: add a fifth state `SecretRotationPending` tracking whether
the sink has the current secret. Rejected as over-engineering. The
sink is a one-way channel (we write to it, we don't read back). The
one-shot secret rule above gives the same guarantee — mint on create,
rotate on `--rotate-keys` — with no extra state.

## Implementation notes

<!-- sketch -->

```rust
/// One trait per resource kind. The generic driver walks the matrix.
trait EnsureOp {
    type Spec;
    type Server;

    fn natural_key(spec: &Self::Spec) -> &str;

    /// Paginate until exhaustion. Deduplicate by natural key.
    async fn list(&self, client: &Client) -> Result<Vec<Self::Server>>;

    /// Compare spec-declared fields only. Return the diff, empty when
    /// state is Match.
    fn diff(spec: &Self::Spec, server: &Self::Server) -> Vec<FieldDiff>;

    async fn create(&self, client: &Client, spec: &Self::Spec) -> Result<Created>;
    async fn update(&self, client: &Client, server: &Self::Server, spec: &Self::Spec) -> Result<()>;

    /// Only implemented for resources with rotatable secrets. Default
    /// panics; a `supports_rotation()` method gates dispatch.
    async fn rotate(&self, client: &Client, server: &Self::Server) -> Result<Rotated> {
        unimplemented!("rotation not supported for this resource")
    }
    fn supports_rotation(&self) -> bool { false }
}

/// The matrix above, one `match` block, called once per ensure-op.
async fn drive<E: EnsureOp>(op: &E, spec: &E::Spec, flags: Flags, client: &Client, sinks: &Sinks) -> Transition {
    let server = lookup(op, client, spec).await?;
    let state = classify(spec, server.as_ref());

    match (state, flags.sync, flags.rotate_keys) {
        (State::Missing, _, _) => { let created = op.create(client, spec).await?; emit_create(sinks, created); Transition::Created }
        (State::Match, _, false) => Transition::Skip,
        (State::Match, _, true) if op.supports_rotation() => { let r = op.rotate(client, server.as_ref().unwrap()).await?; emit_rotation(sinks, r); Transition::Rotated }
        (State::Match, _, true) => Transition::Skip,
        (State::Drift, false, _) => { log_drift(...); Transition::Skip }
        (State::Drift, true, _) => { op.update(client, server.as_ref().unwrap(), spec).await?; Transition::Updated }
        (State::Conflict, _, _) => Transition::Conflict,
    }
}
```

## References

- Source code:
  - `tools/pim-bootstrap/src/cli.rs:54-79` — `Bootstrap` subcommand
    flags that feed the state machine
  - `tools/pim-bootstrap/src/config.rs:66-91` — natural-key fields
    (`ProjectSpec.name`, `ApiAppSpec.name`, `ServiceAccountSpec.username`,
    `RoleSpec.key`)
  - `tools/pim-bootstrap/src/config.rs:105-110` — `OutputSinks` shape
    consumed by secret-emission transitions
  - `tools/pim-bootstrap/src/main.rs:57` — current stub that this ADR
    unblocks
- Related ADRs:
  - ADR-0005 (the idempotency contract this ADR formalises)
  - ADR-0012 (Layer A/B/C sinks the transitions write to)
  - ADR-0013 (dev/prod parity, drift-observable-not-fatal posture)
  - ADR-0016 (Zitadel Management API wire protocol; supplies the
    error-code taxonomy consumed by the `Conflict` classifier)
