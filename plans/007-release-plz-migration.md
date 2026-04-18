# Plan 007: Migrate Release Automation from release-please to release-plz

**Status:** Draft
**Created:** 2026-04-18
**Owner:** Platform / CI
**Supersedes (partially):** Plan 005 Phase related to release-please setup

---

## 1. Context

After Plan 005 landed release automation via `googleapis/release-please-action@v4` in manifest mode (`release-please-config.json` + `.release-please-manifest.json`), the release workflow on `main` still fails with:

```
cargo-workspace (wowuliao11/pim): package manifest at libs/infra-auth/Cargo.toml has an invalid [package.version]
```

### Root cause

Our workspace follows the Cargo-official best practice of **version inheritance**:

- Root `Cargo.toml` defines `[workspace.package] version = "0.1.0"`.
- Every member crate declares `version.workspace = true`.

release-please's `cargo-workspace` plugin parses member `Cargo.toml` files directly and does not resolve workspace inheritance. This is tracked upstream in [`googleapis/release-please#2111`](https://github.com/googleapis/release-please/issues/2111) (open since 2023-11, priority `p3`, no fix in sight). Related open issues (#2517, #2558, #2748) show the Rust integration is second-class inside a Google-monorepo-first tool.

### Why not "fix" by abandoning inheritance

Per the Cargo Book (§Workspaces, Rust 1.64+), `workspace.package` inheritance is the **recommended** pattern for multi-crate workspaces. `cargo publish` automatically rewrites inherited fields to concrete values in the published `Cargo.toml` (originals preserved in `Cargo.toml.orig`), so there is no downside for crates.io consumers. Removing inheritance to appease one tool would regress the workspace design.

### Why release-plz

[`release-plz`](https://github.com/release-plz/release-plz) (Apache-2.0/MIT, 1.3k★, actively maintained) is Rust-native and:

- Compares local `Cargo.toml` against the version published on crates.io — it never has to resolve workspace inheritance itself because Cargo resolves it for it.
- Ships CHANGELOG generation (git-cliff), GitHub Release creation, crates.io publishing, and Release PR workflow out of the box.
- Self-describes as *"optimized for Rust projects... doesn't need any configuration"*.

It directly solves our failure mode while keeping the workspace layout intact.

---

## 2. Goals & Non-Goals

### Goals

- Replace release-please with release-plz as the single release-automation tool.
- Keep version inheritance (`version.workspace = true`) unchanged.
- Preserve every non-release CI improvement already merged (Conventional Commit PR title check, buf breaking check fix).
- Land the change through PR + branch protection gating, consistent with `AGENTS.md §3`.

### Non-Goals

- Publishing to crates.io (this repo is private/application code; we only need tags + GitHub Releases for now). Publishing remains opt-in per-crate via `publish = false` / `publish = true`.
- Changing the existing `ci.yml` (fmt / clippy / test / buf / cargo-deny) jobs.
- Altering repo merge settings or main branch protection (already configured).
- Touching the unrelated in-flight local changes (`Cargo.toml`, `Cargo.lock`, `plans/006-dev-bootstrap.md`, `bootstrap/*.toml`, `tools/pim-bootstrap/*`).

---

## 3. Phased Delivery

### Phase 1 — Remove release-please artifacts (Status: Pending)

Delete:

- `release-please-config.json`
- `.release-please-manifest.json`
- `.github/workflows/release.yml`

**Acceptance:**
- Files no longer exist on `main` after merge.
- No workflow references the deleted files (grep clean).

### Phase 2 — Introduce release-plz (Status: Pending)

Add:

- `release-plz.toml` at repo root with:
  - `[workspace] changelog_update = true`, `git_release_enable = true`, `publish = false` (default; overridable per package later).
  - `[changelog]` using release-plz defaults (Keep a Changelog + Conventional Commits).
  - No per-package overrides in v1; revisit when a crate needs a different cadence.
- `.github/workflows/release-plz.yml`:
  - Triggers on `push` to `main`.
  - Uses official `MarcoIeni/release-plz-action@v0.5` (pin to latest stable major at implementation time; exact tag chosen during Phase 2 execution after verifying latest release).
  - Two jobs, both using the same action with different `command:` inputs — `release-plz-pr` (opens/updates the Release PR) and `release-plz-release` (creates tags + GitHub Releases when Release PR is merged).
  - Permissions: `contents: write`, `pull-requests: write`.
  - Uses `GITHUB_TOKEN`; no crates.io token needed while `publish = false`.
  - Concurrency group to prevent overlapping runs on `main`.

**Acceptance:**
- Release workflow run on `main` completes with status `success` (no error about `[package.version]`).
- A Release PR is opened by release-plz against `main` (or no-op if no release-worthy commits since seed — both outcomes are green).
- `version.workspace = true` remains in every member crate; root `[workspace.package] version` unchanged.

### Phase 3 — Branch protection check sync (Status: Pending)

Current required checks on `main`: `Rustfmt`, `Clippy`, `Test`, `Buf (Proto)`, `Cargo Deny`, `Conventional Commit`.

release-plz workflow runs on `push` to `main`, not on PRs, so it does **not** need to be added as a required PR check. No branch-protection change required.

**Acceptance:**
- `gh api repos/wowuliao11/pim/branches/main/protection` shows the same six contexts as today.

### Phase 4 — Design doc update (Status: Pending)

Update `/docs/design.md` (CI/CD section) to reflect:
- Release tool: `release-plz` (replaces release-please).
- Rationale: version inheritance incompatibility with release-please.
- Link to this plan.

**Acceptance:**
- `docs/design.md` no longer mentions release-please as the active tool.
- A brief "Release automation" subsection documents the release-plz flow.

---

## 4. Delivery Mechanics

- **Branch:** `ci/release-plz-migration`.
- **Commits (staged carefully — repo has unrelated in-flight local edits that must NOT be included):**
  1. `ci: remove release-please configuration`
  2. `ci: add release-plz workflow and config`
  3. `docs: update design doc for release-plz migration` (Phase 4)
- **PR:** Single PR titled `ci: migrate release automation to release-plz` (Conventional Commits compliant to satisfy the PR title check).
- **Merge:** Squash merge per repo policy. Owner is sole maintainer; required-reviews is `null` by design, so self-merge is permitted.
- **Post-merge verification:** Watch the first `release-plz` run on `main`; confirm success and inspect the Release PR (if any).

---

## 5. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| release-plz action major version drifts between draft and execution | Low | Pin to explicit tag at implementation time; document chosen version in the commit message. |
| release-plz opens a noisy Release PR immediately after merge | Medium | Expected and desirable — it is the tool working correctly. Close without merging if content is undesired. |
| Unrelated local edits get swept into commits | Medium | Use explicit file paths with `git add`, verify `git diff --cached` before each commit, never `git add -A` / `git add .`. |
| Future need to publish to crates.io | Low | Flip `publish = true` per package in `release-plz.toml` and add `CARGO_REGISTRY_TOKEN` secret. No re-architecture needed. |

---

## 6. Out of Scope / Follow-ups

- Publishing any crate to crates.io.
- Re-evaluating conventional commit history warnings (historical commits pre-policy; release-plz ignores unparseable commits gracefully).
- Cross-workspace linked versioning (release-plz supports it via `[workspace] semver_check` and per-package config if ever needed).
