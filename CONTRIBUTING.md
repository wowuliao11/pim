# Contributing to PIM

Welcome. This document is the operational handbook for contributing code to PIM.
It complements two other authoritative documents:

- [`AGENTS.md`](./AGENTS.md) — process constitution (planning, documentation lifecycle, language policy).
- [`docs/design.md`](./docs/design.md) — current accepted system architecture.

If anything here conflicts with `AGENTS.md`, `AGENTS.md` wins.

---

## 1. Branching Model: Trunk-Based Development

PIM follows **Trunk-Based Development (TBD)**. The single source of truth is the
`main` branch. All work flows through short-lived branches that merge back to
`main` quickly via squash merges.

### 1.1 Core rules

- **One trunk:** `main` is always releasable. Direct pushes are blocked by
  branch protection.
- **Short-lived branches:** Open a branch, ship a PR, merge, delete — ideally
  within **2 working days**. If a branch lives longer, rebase against `main`
  daily and explain the delay in the PR description.
- **Small PRs:** Aim for **< 400 lines of diff** (excluding generated files,
  lockfiles, and snapshots). Larger PRs require justification in the
  description and a reviewer's explicit acceptance.
- **Squash merge only:** History on `main` is linear. The squash commit message
  becomes the canonical Conventional Commit for that change.
- **Hide unfinished work:** Land code behind feature flags
  (`libs/infra-config::features`) rather than holding it on a long-lived
  branch.

### 1.2 Branch naming

Use a typed prefix that mirrors the Conventional Commit type, then a short
kebab-case slug:

| Prefix      | Use for                                             | Example                            |
| ----------- | --------------------------------------------------- | ---------------------------------- |
| `feat/`     | New user-visible behavior or new modules            | `feat/user-service-list-endpoint`  |
| `fix/`      | Bug fixes that change runtime behavior              | `fix/jwt-clock-skew`               |
| `refactor/` | Internal restructuring with no behavior change      | `refactor/extract-config-loader`   |
| `docs/`     | Documentation-only changes                          | `docs/tbd-workflow`                |
| `ci/`       | Workflows, release tooling, repo configuration      | `ci/fix-release-plz-tag-name`      |
| `chore/`    | Dependency bumps, version pins, housekeeping        | `chore/bump-tonic`                 |
| `test/`     | Test-only additions or restructuring                | `test/api-gateway-config-defaults` |

Dependabot branches (`dependabot/...`) are exempt from this convention.

### 1.3 Commit messages

- **PR title** must be a valid [Conventional Commit](https://www.conventionalcommits.org/)
  (`type(scope): subject`). This is enforced by
  `.github/workflows/pr-title.yml`.
- Individual commits inside a PR are not validated (they are squashed away).
  Keep them readable for review, but the only message that lasts is the squash
  commit.
- Breaking changes go in the footer: `BREAKING CHANGE: <description>`.

---

## 2. Workflow

### 2.1 Standard contribution loop

1. `git checkout main && git pull --ff-only`
2. `git checkout -b <prefix>/<slug>`
3. Make changes. Stage explicitly (`git add <path>`) — never `git add .`.
4. Commit. Push: `git push -u origin <branch>`.
5. Open a PR with `gh pr create` (or via the web UI). Fill in the PR template.
6. Wait for the 6 required checks to go green:
   `Rustfmt`, `Clippy`, `Test`, `Buf (Proto)`, `Cargo Deny`, `Conventional Commit`.
7. Request review if the change is non-trivial. Self-approval is disabled.
8. Squash-merge via `gh pr merge <num> --squash --delete-branch`.
9. `git checkout main && git pull --ff-only`. Delete local branch.

### 2.2 When you must write a plan first

A plan in `/plans/NNN-*.md` is required only when **at least one** of the
following is true (see `AGENTS.md §3.1` for the authoritative rule):

- The change spans more than one PR or more than ~2 working days.
- It introduces or modifies cross-cutting architectural patterns
  (auth, transport, observability, release, build).
- It will touch ≥ 3 crates or ≥ 1 public API surface in `libs/`.
- It will require a coordinated migration (data shape, config shape, proto
  contract, CI gate).

Single-PR changes that fit comfortably under the size and scope rules above
**do not** require a plan. Document them in the PR description instead.

### 2.3 Updating documentation

- `docs/design.md` is updated when a stabilized change alters the architecture
  described there. Update it in the same PR as the change, not after.
- `AGENTS.md` is the process constitution. Edits require explicit human
  authorization.
- `CONTRIBUTING.md` (this file) tracks workflow conventions. Update it when
  team practice changes.

---

## 3. Feature Flags

Long-lived branches are not allowed. Unfinished work lands on `main` behind a
runtime flag.

```rust
use infra_config::features;

if features::is_enabled("new_user_search") {
    new_path().await
} else {
    legacy_path().await
}
```

Flags are read from environment variables of the form
`APP_FEATURE_<UPPERCASE_NAME>=true`. See `libs/infra-config/src/features.rs`
for the full contract.

Naming:

- Use snake_case identifiers in code (`new_user_search`).
- The corresponding env var is `APP_FEATURE_NEW_USER_SEARCH`.
- Document each flag in the PR that introduces it: name, owner, removal
  criteria.

A feature flag is a debt instrument. Schedule its removal in the same PR that
introduces it (link a follow-up issue if removal cannot happen immediately).

---

## 4. Local Development

- Toolchain is pinned in `rust-toolchain.toml` (currently `1.90.0`).
  Run `rustup show` to install it on first checkout.
- Standard checks before pushing:

  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-features --no-fail-fast
  ```

- Proto changes additionally require `buf lint` and `buf breaking` (run by CI).
- Dependency advisories are gated by `cargo deny check` (run by CI; see
  `deny.toml` for the active ignore list).

---

## 5. Release

Releases are fully automated by `release-plz` (see
`.github/workflows/release-plz.yml` and `release-plz.toml`). Contributors
should not create tags manually. Workspace version is single-sourced through
`api-gateway` as the representative crate (Model C); see `docs/design.md §7`.

---

## 6. Code Review Expectations

- A green CI run is necessary but not sufficient.
- Reviewers should verify: scope discipline, test coverage for the change,
  documentation updates, and adherence to the workflow rules above.
- Reviewers may request that an oversized PR be split before approving.
- Author addresses feedback in additional commits on the same branch
  (squashed at merge).

---

## 7. Reporting Problems

- Bugs / feature requests: open a GitHub issue with reproduction steps and
  environment details.
- Security issues: do not open a public issue; contact the maintainers
  directly.
