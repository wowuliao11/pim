# ADR-0002: Surface relevant ADRs at PR time via lightweight CI check

- **Status:** Proposed
- **Date:** 2026-04-18
- **Deciders:** PIM maintainers

## Context

ADR-0001 establishes ADRs as the home for decision rationale, but writing
ADRs only pays off if contributors **actually see them at the moment they
might violate one**. Without surfacing, ADRs degrade into the same
read-only graveyard that long-form `plans/` became.

The industry pattern for this is "Decision Guardian"-style tooling
(see [DecispherHQ/decision-guardian](https://github.com/DecispherHQ/decision-guardian),
referenced from [adr.github.io](https://adr.github.io/)): on every PR,
the tool diffs touched files, looks up which ADRs cover those files, and
posts a PR comment listing the relevant ADRs. The contributor reads
context **before** merging, not after they've broken something.

The full Decision Guardian product is overkill for a single-team Rust
monorepo: it's designed for large orgs with cross-cutting compliance
needs, requires its own service to host, and adds an external dependency
for a problem solvable in ~50 lines of CI script.

What we need is the **same UX** (ADR comment on PR) with the **simplest
possible mechanism**.

## Decision

PIM uses a lightweight in-repo GitHub Action that, on every PR
open/synchronize event:

1. Computes the list of changed files via `git diff --name-only`.
2. For each ADR under `docs/decisions/*.md`, parses its **References**
   section for any file paths.
3. If any changed file matches a path referenced by an ADR, the action
   posts (or updates) a single PR comment listing the matched ADRs
   with title and one-line summary.
4. Posts no comment when no ADR matches; never blocks the merge.

This is advisory, not gating. It nudges; it does not enforce. Enforcement
remains a human responsibility during code review.

ADR file conventions to make this work:

- The **References** section MUST list the canonical source paths the
  ADR governs (e.g. `apps/api-gateway/src/middlewares/`,
  `libs/infra-auth/`).
- Path references use the form `path/` (directory) or
  `path/to/file.rs:NN` (file with optional line). Glob patterns
  (`apps/*/src/main.rs`) are allowed.
- The first H1 line is the ADR title and gets surfaced verbatim.

The action lives at `.github/workflows/adr-check.yml` and is implemented
as a small inline Bash/Python script — no marketplace action, no
external service.

## Consequences

**Positive:**

- Contributors see relevant ADRs in PR comments without having to know
  to look. Solves the discoverability problem ADR-0001 leaves open.
- Zero external dependencies; the script lives in the repo and is
  reviewable like any other code.
- The "References must list governed paths" convention forces ADR
  authors to think concretely about scope ("which code does this
  decision actually govern?"), which improves ADR quality.
- Works for both human and agent contributors equally — both submit
  PRs, both see the comment.

**Negative / accepted trade-offs:**

- Path-based matching is coarse: an ADR governing `auth` will match any
  PR touching `apps/api-gateway/src/middlewares/auth.rs` even if the
  change is purely cosmetic. We accept this — false positives in an
  advisory comment cost almost nothing; false negatives (missing a
  relevant ADR) are the real risk.
- ADR authors must remember to update **References** when source paths
  move during refactors. This is the one maintenance burden ADRs
  introduce. Mitigated by: the next refactor's PR will fail to surface
  the ADR, prompting the fix.
- Implementation is deferred — this ADR records the **decision**;
  building the workflow is follow-up work tracked separately.

**Locked in:**

- ADR References section becomes load-bearing: contributors and tools
  rely on it being accurate.
- GitHub Actions as the surfacing mechanism (we assume PRs happen on
  GitHub).

**Follow-up:**

- Implement `.github/workflows/adr-check.yml` after at least 5 ADRs
  exist with non-empty References sections (otherwise the action has
  nothing to surface). The migration from `plans/` (ADR-0001 follow-up)
  produces those ADRs.
- Consider adding a `just adr-new <slug>` recipe or `adr-tools` CLI
  install once ADR creation frequency justifies it.

## Alternatives considered

### Option A — Adopt Decision Guardian as-is

Rejected. External service adds operational dependency. Designed for
multi-team orgs with compliance needs PIM doesn't have. The matching
logic we need is ~50 lines; importing a product to do it is over-
engineered.

### Option B — `adr-tools` (npryce) CLI only, no PR surfacing

Rejected as the primary mechanism. `adr-tools` helps **create** ADRs but
does nothing to surface them at PR time. Discoverability is the harder
problem; creation friction is already low. We may still adopt the CLI
later as a creation convenience (see Follow-up), but not as the answer
to ADR-0002's question.

### Option C — Pre-commit hook that warns locally

Rejected as the primary mechanism. Pre-commit hooks are skipped (`-n`),
not run by all contributors, and invisible to reviewers. PR-time
surfacing is reviewable: the comment is part of the PR record. A
pre-commit hook can be added later as an additional nudge but is not
load-bearing.

### Option D — Manual: rely on PR template asking "which ADRs apply?"

Rejected. Manual processes degrade. An automated nudge that costs
nothing per PR is strictly better than asking humans to remember.

## Implementation notes

```yaml
<!-- sketch -->
# .github/workflows/adr-check.yml
name: ADR Check
on: pull_request
jobs:
  surface-adrs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }
      - name: Find relevant ADRs
        run: scripts/adr-check.sh "${{ github.event.pull_request.base.sha }}" "${{ github.sha }}"
      - name: Post comment
        if: steps.find.outputs.matched != ''
        uses: actions/github-script@v7
        # ... post or update single sticky comment
```

The matching script is plain shell + `rg`; it greps each ADR's
References section for any line matching a path from the PR diff.

## References

- Source code: `.github/workflows/adr-check.yml` (to be created),
  `scripts/adr-check.sh` (to be created)
- External: [DecispherHQ/decision-guardian](https://github.com/DecispherHQ/decision-guardian)
  (the inspiration we are deliberately not using)
- Related: ADR-0001 (establishes the ADR practice this tooling supports)
