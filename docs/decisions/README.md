# Architecture Decision Records (ADRs)

This directory is the **decision log** for PIM. It captures every architecturally
significant decision, its context, and its consequences.

## Why ADRs

Plans describe *what we are going to do*. Source code describes *what we did*.
Neither captures **why we chose one design over another**. Without that "why",
future contributors (human or agent) re-litigate settled questions, miss
non-obvious constraints, and drift from the system's intent.

ADRs are the answer. Each ADR is a short, focused document that records one
decision so it survives the people who made it.

PIM previously used long-form `plans/*.md` files that mixed four different
artifacts (decision rationale, task checklists, illustrative code, status
tracking). That format scaled badly: completed plans turned into stale
1000-line documents that duplicated source code and competed with
`docs/design.md` as a source of truth. ADRs replace the **decision** portion of
those plans. Task tracking moves to PR descriptions and (where the work is
genuinely multi-PR) a small remaining `plans/` checklist.

## When to write an ADR

Write an ADR when **any** of the following is true about a change:

- It picks one technology, library, or protocol over alternatives that a
  reasonable person could have chosen instead.
- It establishes or modifies a cross-cutting pattern (auth, transport,
  observability, configuration, release process, build system).
- It changes a public API contract (HTTP route shape, gRPC proto, exported
  Rust API in `libs/`).
- It introduces or removes a workspace crate, service, or external dependency
  that other parts of the system rely on.
- It defines an operational invariant (port numbers, env-var conventions,
  secret-handling rules, file-layout conventions).
- A future contributor would reasonably ask "why was it built this way?" and
  the answer is not obvious from the code alone.

**Do not** write an ADR for: bug fixes, refactors that preserve behaviour,
dependency version bumps without API change, lint or formatting changes,
documentation tweaks, or temporary workarounds.

If you are unsure, prefer to write the ADR. ADRs are cheap; lost context is
expensive.

## Lifecycle

Each ADR has a `Status` field:

- **Proposed** — drafted but not yet accepted. May still change.
- **Accepted** — agreed and in effect. The system is built or being built
  this way.
- **Superseded by ADR-NNNN** — a later ADR replaces this one. The original
  document remains in place for history.
- **Deprecated** — the decision no longer applies, but no replacement was
  needed (e.g. the feature was removed).

ADRs are **append-mostly**, not strictly immutable. Small clarifications and
typo fixes are fine. Substantive change of intent must happen through a new
ADR that supersedes the old one. This matches the practical guidance from
[adr.github.io](https://adr.github.io/) and Joel Parker Henderson's ADR
guide: pure immutability is dogma; "living document with date-stamped
amendments" works better in practice.

## File naming

`NNNN-imperative-verb-phrase.md` — four-digit zero-padded number, kebab case,
present-tense verb first.

Good:

- `0001-record-architecture-decisions.md`
- `0007-validate-tokens-via-zitadel-introspection.md`
- `0012-bootstrap-zitadel-with-rust-tool.md`

Bad:

- `zitadel.md` (not a decision, just a topic)
- `0007-zitadel-auth.md` (no verb, ambiguous)
- `7-auth.md` (no zero-padding, too vague)

The next available number is whatever follows the highest existing ADR in
this directory. Reserve `0000-template.md` for the template.

## Format

Use `0000-template.md` (MADR-derived). Keep ADRs **short** — typically one
screen of prose. Every ADR must answer:

1. **Context** — what forces are at play, what's the current state, what
   constraints apply.
2. **Decision** — what we chose, in one sentence at the top.
3. **Consequences** — what becomes easier, what becomes harder, what's
   locked in, what we explicitly accept.
4. **Alternatives considered** — at least one other option we rejected, with
   why. If there were no alternatives, say so explicitly.

Code blocks are allowed but should be **illustrative sketches** showing API
shape or contract — not full implementations. Mark sketches with
`<!-- sketch -->`. Implementation lives in source code; ADRs link to it.

## Process

1. Open a PR that adds the new ADR file with status `Proposed`.
2. Discussion happens in the PR thread.
3. On merge, the ADR is `Accepted`. The implementation follows in the same
   PR or subsequent PRs.
4. If a later decision changes the answer, write a new ADR that says
   `Supersedes ADR-NNNN`, and edit the old ADR's status to
   `Superseded by ADR-MMMM`.

For agent-authored changes: the agent must propose ADRs proactively whenever
the trigger conditions above are met, **before** making implementation
changes that depend on them.

## Discoverability

Until tooling lands (see ADR-0002), discovery is by `ls docs/decisions/` and
keyword search (`rg <topic> docs/decisions/`). When a PR touches code that
implements an ADR, the PR description should link to the ADR. When code
deviates from an ADR, that's a smell — open a new ADR or amend the existing
one before merging.

## See also

- `AGENTS.md` §3 — process rules and the relationship between ADRs, plans,
  and `docs/design.md`.
- `docs/design.md` — current accepted system design (the **what**, not the
  **why**).
- [adr.github.io](https://adr.github.io/) — community resource and template
  catalogue.
- [MADR](https://adr.github.io/madr/) — the template family this project
  uses.
