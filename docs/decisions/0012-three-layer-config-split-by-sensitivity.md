# ADR-0012: Split runtime configuration into three layers by sensitivity

- **Status:** Accepted
- **Date:** 2026-04
- **Deciders:** PIM maintainers

## Context

PIM services receive three categorically different kinds of configuration
at startup:

1. **Non-secret operational config** — Zitadel project IDs, API app
   client IDs, service listen ports, OIDC authority URLs, log level.
   Safe to commit for review when rendered as examples; unsafe to
   commit when populated with a specific tenant's IDs because those
   IDs are the audience in JWT validation.
2. **Asymmetric private key material** — the API app's JWT Profile
   private key, used by the gateway when it calls Zitadel
   introspection (see ADR-0004). A structured JSON document
   (`{keyId, key, userId, type}`), too large for an env var, and
   rotation is coarse-grained (mint a new key, re-deploy).
3. **Symmetric secrets** — short strings like a Zitadel admin PAT,
   a database password, a webhook HMAC. Naturally fit env vars, read
   at process startup, rotated as a unit.

Lumping these together in a single file (one `.env`, or one TOML with
secrets inline) has two failure modes we've seen before:

- Non-secret IDs get quietly treated as secret, which blocks people
  from committing example configs to the repo and reviewing them.
- Secret material gets quietly treated as non-secret, which is how
  private keys leak into git history.

ADR-0005 creates a single tool (`pim-bootstrap`) that provisions all
three kinds of values at once. That tool needs to write each kind to
the right destination without the operator having to sort them by
hand.

## Decision

Runtime configuration is split into three layers, each with its own
format, its own location, and its own handling rule:

**Layer A — Service `config.toml` (non-secret operational config).**
Each service reads `config.toml` from its working directory. These
files are gitignored as real artifacts but committed as
`config.example.toml` siblings so the shape of the config is visible
in review. `pim-bootstrap` writes the real file per service, filling
in IDs provisioned at bootstrap time.

**Layer B — `zitadel-key.json` (asymmetric private key material).**
The API app's JWT Profile key lives in its own JSON file at a
well-known path. Gitignored. Never written to an env var. Rotated only
when `pim-bootstrap bootstrap --rotate-keys` is passed.

**Layer C — `.env.local` (symmetric secrets).** Short string secrets
(Zitadel admin PAT, Zitadel master key, database passwords) land in
a single env file. Gitignored. `pim-bootstrap` upserts into it;
`compose.yml` references it via `env_file:`; local shells source it
ad hoc.

In prod, the layout stays the same but the destinations shift: Layer
A files land in a reviewable deploy artifact path (e.g.
`deploy/prod/api-gateway.config.toml`), while Layers B and C are
emitted to stdout via `stdout:<tag>` sentinels so the operator pipes
them into Vault, AWS Secrets Manager, sealed secrets, or whatever the
target environment uses. The bootstrap tool never writes prod secrets
to disk.

This three-way split is encoded in the `OutputSinks` shape in the
bootstrap config (`tools/pim-bootstrap/src/config.rs:105-110`):
`service_configs` (map of service → path), `jwt_key_path` (single
path), `env_file_path` (single path).

## Consequences

**Positive:**

- Each kind of value has a single canonical destination. No ambiguity
  about "where does the PAT go" vs "where does the client ID go".
- `.example.toml` files in git stay small and reviewable because they
  contain only Layer A shape — no placeholder secrets to scrub.
- Rotation policies are per-layer. Rotating a JWT key is a different
  operation from rotating a PAT, and the tool reflects that.
- Prod is free to adopt any secret manager without changing the
  service code: stdout sentinels keep Layers B and C off disk.

**Negative / accepted trade-offs:**

- Three files instead of one. New developers must learn the layering.
  Mitigated by docstrings on `OutputSinks` and example configs
  committed to `bootstrap/`.
- The `stdout:<tag>` prod convention is a tool-local DSL, not a
  standard. Operators need to know to pipe stdout to their secret
  manager.
- Service `config.toml` files duplicate a small amount of information
  (e.g. Zitadel authority URL) across services. Accepted as the cost
  of having self-contained service configs.

**Locked in:**

- `.env.local` is the one-and-only symmetric-secret file name. Tools
  and compose refer to it by name.
- `zitadel-key.json` lives at the repo root for dev. Any future
  additional asymmetric key gets its own file at a sibling path, not
  inlined into this one.

## Alternatives considered

### Option A — Single `.env` for everything

Rejected. Mixes non-secret IDs with private keys. Encourages people to
commit example `.env` files with placeholder secrets, which is the
exact failure mode we've seen leak material on other projects.

### Option B — One TOML per service with secrets inline

Rejected. Blocks committing any version of the real file for review,
even with placeholders. Forces `config.example.toml` to be hand-
maintained and drift from the real shape.

### Option C — A secret manager from day one (Vault / sops-age)

Rejected for the dev stack. Adds a hard dependency on a secret store
for every developer to run `cargo test`. Prod deployments can layer a
secret manager on top by reading the `stdout:<tag>` sentinels; dev
stays on plain files.

### Option D — Environment variables only (12-factor strict)

Rejected. Fine for Layer C. Awkward for Layer A (long nested keys
become `APP__ZITADEL__AUTHORITY`, hard to review). Impossible for
Layer B (JWT profile JSON doesn't fit cleanly in an env var).

## References

- Source code:
  - `tools/pim-bootstrap/src/config.rs:93-110` — `OutputSinks`
    docstring calling out the three layers
  - `tools/pim-bootstrap/src/config.rs:105-122` — `BootstrapConfig`
    wiring
  - `bootstrap/dev.toml:34-46` — dev `[outputs]` section pointing
    each layer at its destination
  - `bootstrap/prod.example.toml:34-45` — prod `[outputs]` section
    using `stdout:<tag>` sentinels for Layers B and C
  - `.gitignore:53-64` — gitignore entries that enforce the split
    (real `config.toml`, `zitadel-key.json`, `.env.local` excluded;
    `.example.toml` siblings allowed through)
  - `compose.yml:104-110` — `env_file: .env.local` wiring for Layer C
- Originated from: `plans/006-dev-bootstrap.md` at commit `3ee9cc8`,
  decision D10.
- Related: ADR-0005 (the tool that writes to all three layers),
  ADR-0013 (how the same layering serves dev and prod).
