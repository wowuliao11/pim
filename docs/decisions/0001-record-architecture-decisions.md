# ADR-0001: Record architecture decisions as ADRs

- **Status:** Accepted
- **Date:** 2026-04-18
- **Deciders:** PIM maintainers

## Context

PIM has accumulated significant architectural intent in `plans/*.md` files
(8 files, ~2930 lines). Inspection on 2026-04-18 found that these files
conflate four distinct artifacts:

1. Decision rationale (why option X over Y)
2. Implementation task checklists (`- [ ] Step 1: ...`)
3. Illustrative code samples (70+ Rust code blocks in `plans/003`, 54 in
   `plans/006`)
4. Status tracking (Phase complete tables)

After implementation completes, the rationale (1) remains valuable but
gets buried under (2)/(3)/(4), which are either stale (the code in plans
doesn't track the real source) or worthless (completed checklists). The
result is duplicated truth across `plans/`, source code, and
`docs/design.md`, with no clear answer to "where do I look to understand
why this exists?".

`AGENTS.md` §3.2 already states "Plans are mutable. They evolve as we
learn. They are NOT the final documentation." but did not provide a place
for the decision rationale to live after a plan completes. So plans
linger.

The industry-standard answer to this problem is the **Architecture
Decision Record** (ADR), popularised by Michael Nygard (2011) and now
endorsed by AWS, Azure, Red Hat, GitHub, and the
[adr.github.io](https://adr.github.io/) community. An ADR is a single
short document recording one decision, its context, and its consequences,
with no implementation code.

## Decision

PIM adopts ADRs as the durable home for architectural decision rationale.
ADRs live under `docs/decisions/`, one decision per file, named
`NNNN-imperative-verb-phrase.md`. The template is MADR-derived (see
`0000-template.md`).

`plans/` is reduced to a short-lived workspace for in-flight multi-PR
work. When a plan phase completes, its decisions are extracted into
ADRs, the corresponding plan section is deleted, and `docs/design.md` is
updated to reflect the new accepted state. When a plan completes
entirely, the file is deleted (git history preserves it).

Eventually `plans/` may disappear entirely if no work in flight requires
it. ADRs become the only long-lived design-intent artifact.

## Consequences

**Positive:**

- One location to learn *why* the system is built the way it is.
- Decision documents stay short (one screen) and therefore actually get
  read at PR time.
- Source code stays the only home for implementation detail; no more
  drift between plan code blocks and real code.
- Clear lifecycle: Proposed → Accepted → (optionally) Superseded.
- Compatible with off-the-shelf tooling (adr-tools CLI, Decision
  Guardian-style PR-time surfacing — see ADR-0002).

**Negative / accepted trade-offs:**

- One-time migration cost: extract ADRs from existing completed plans
  before deleting them.
- Slight overhead per architecturally-significant change: write the ADR
  in the same PR as the implementation. Mitigated by the template and a
  one-screen size budget.
- Loss of inline narrative ("we tried X, then realised Y, so switched to
  Z") in some legacy plans. Where that history matters, the originating
  plan SHA is recorded in the ADR's References section so `git show` can
  recover it.

**Locked in:**

- File naming convention (`NNNN-imperative-verb-phrase.md`).
- MADR-derived template structure (Status, Context, Decision,
  Consequences, Alternatives).
- Append-mostly mutability (per [Henderson's ADR
  guide](https://github.com/joelparkerhenderson/architecture-decision-record):
  pure immutability is dogma; date-stamped amendments work better in
  practice).

**Follow-up:**

- ADR-0002 picks the discoverability tooling (Decision Guardian-style PR
  bot vs. plain repo grep vs. adr-tools CLI).
- Migrate completed plans (`plans/002`, `plans/003`, `plans/006` Phase
  1-2, `plans/enterprise-logger-error.plan.md`) into ADRs in the rollout
  PRs that follow this one.
- Update `AGENTS.md` §3 to make ADR creation a process rule.

## Alternatives considered

### Option A — Keep `plans/` as long-form docs, archive completed ones to `plans/archive/`

Rejected. Archive directories become graveyards: contributors don't
trust them as truth and don't bother reading them, but also don't dare
delete them. The 1000-line completed plan stays in the working tree
either way, still visible, still confusing. Doesn't disincentivise the
next plan from also being 1000 lines.

### Option B — Inline decision rationale into `docs/design.md`

Rejected. `docs/design.md` describes *current state* ("the gateway uses
Zitadel introspection"), not *why current state was chosen over
alternatives*. Mixing the two bloats `design.md` and obscures both. ADRs
are the right separation: design.md is the snapshot, ADRs are the log of
how we got here.

### Option C — Use GitHub Issues / Discussions for decisions

Rejected. Issues are ephemeral, hard to link from source code, vendor-
locked to GitHub, and not part of the repo working tree. Decisions need
to live next to the code they govern, in version control, reviewable
through PRs.

### Option D — Do nothing, accept the status quo

Rejected. The status quo already produced the problem this ADR
addresses (1000-line completed plans, source-of-truth ambiguity, dead
code blocks). Inertia is not a strategy.

## References

- [adr.github.io](https://adr.github.io/) — community ADR resources.
- [MADR](https://adr.github.io/madr/) — the template family.
- [Documenting Architecture Decisions, Michael Nygard
  (2011)](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions.html)
  — the foundational blog post.
- [Joel Parker Henderson's ADR
  guide](https://github.com/joelparkerhenderson/architecture-decision-record)
  — 15.6k-star reference repo.
- `AGENTS.md` — process rules updated alongside ADR adoption.
