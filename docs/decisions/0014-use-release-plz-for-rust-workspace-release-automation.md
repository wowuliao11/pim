# ADR-0014: Use release-plz for Rust workspace release automation

- **Status:** Accepted
- **Date:** 2026-04-18
- **Deciders:** Jimmy Bao
- **Supersedes:** none (supersedes Plan 005's provisional adoption of release-please)

## Context

PIM is a Cargo workspace laid out per ADR-0009 with a shared `[workspace.package]` block. Every member crate declares `version.workspace = true` so that workspace-wide version bumps touch exactly one file (`Cargo.toml:6`). This is the Cargo-official pattern for multi-crate workspaces since Rust 1.64 (Cargo Book §Workspaces), and `cargo publish` transparently rewrites inherited fields to concrete values in the uploaded manifest (originals preserved in `Cargo.toml.orig`). The inheritance is the right design for this repo.

Plan 005 provisionally adopted `googleapis/release-please-action@v4` in manifest mode to automate releases. Once wired up, every run on `main` failed with:

```
cargo-workspace (wowuliao11/pim): package manifest at libs/infra-auth/Cargo.toml has an invalid [package.version]
```

The `cargo-workspace` strategy inside release-please parses each member `Cargo.toml` directly and does not resolve `version.workspace = true` inheritance. This is tracked upstream at `googleapis/release-please#2111` (open since 2023-11, priority `p3`, no scheduled fix) and reinforced by related open issues #2517, #2558, #2748, which collectively show the Rust integration is second-class inside a Google-monorepo-first tool. Removing inheritance to appease release-please would regress the workspace design for every other Cargo-native tool.

The repository is a private application monorepo, not a public crate set. Releases need to produce a tag and a GitHub Release for a single workspace-wide version; crates.io publishing is an explicit non-goal for the foreseeable future. ADR-0009 already commits us to an `apps/` + `libs/` + `tools/` + `rpc-proto` layout in which `apps/api-gateway` is the externally-visible deliverable, so a single workspace tag anchored to that crate is the right release unit.

## Decision

Adopt [`release-plz`](https://github.com/release-plz/release-plz) (Apache-2.0/MIT, Rust-native, actively maintained) as the sole release-automation tool. release-plz compares the local `Cargo.toml` against crates.io, letting Cargo itself resolve inheritance rather than re-implementing that logic — so the failure mode above is structurally impossible.

Configuration lives at repo root `release-plz.toml`:

- `[workspace] publish = false` — do not publish to crates.io (`release-plz.toml:6`).
- `[workspace] changelog_update = true` — keep per-crate `CHANGELOG.md` in sync (`release-plz.toml:9`).
- `[workspace] git_tag_enable = false`, `git_release_enable = false`, `semver_check = false` — disable per-crate tags, per-crate GitHub Releases, and semver checks (which require crates.io) for the default case (`release-plz.toml:13-17`).
- `[[package]] api-gateway` is designated the **workspace representative**: it alone sets `git_tag_enable = true`, `git_release_enable = true`, `git_tag_name = "v{{ version }}"`, `git_release_name = "v{{ version }}"` (`release-plz.toml:24-29`). A single canonical `v{version}` tag and one GitHub Release per workspace bump, no per-crate prefix.

Two GitHub Actions jobs live in `.github/workflows/release-plz.yml`, both triggered on `push` to `main`:

- `release-plz-pr` runs `release-plz/action@v0.5` with `command: release-pr` to open or update the Release PR (`.github/workflows/release-plz.yml:38-62`).
- `release-plz-release` runs the same action with `command: release` to create the tag and GitHub Release once the Release PR merges (`.github/workflows/release-plz.yml:16-35`).

Permissions are scoped: `contents: write` for both jobs, plus `pull-requests: write` for the PR job. A top-level `concurrency: release-plz-${{ github.ref }}` group prevents overlapping runs, and a nested `concurrency: release-plz-pr` group serializes PR updates so concurrent pushes cannot clobber each other (`.github/workflows/release-plz.yml:9-11, 46-48`). Auth uses `GITHUB_TOKEN` only; no `CARGO_REGISTRY_TOKEN` is configured because `publish = false`.

Branch protection on `main` is unchanged (`Rustfmt`, `Clippy`, `Test`, `Buf (Proto)`, `Cargo Deny`, `Conventional Commit`). release-plz runs on push, not on PRs, so it is not a required check.

## Consequences

### Positive

- `version.workspace = true` remains the single source of truth for workspace version. No source layout changes forced on us by the release tool.
- Release PR flow is familiar (GitHub-native PR titled `chore: release`) with Keep-a-Changelog-style `CHANGELOG.md` diffs per crate, so humans can reason about what each release ships before merging.
- One tag per workspace release (`v0.1.1`, `v0.1.2`, …) keeps the tag namespace readable and matches how the repo is consumed externally — callers care about the api-gateway version, not the infra-auth version.
- Enabling crates.io publishing later is a config flip: set `publish = true` per package and add `CARGO_REGISTRY_TOKEN`. No architectural rework.

### Negative

- release-plz generates a Release PR on every push that touches release-worthy commits. This is working-as-intended but introduces Release-PR noise on busy branches. Mitigation: close without merging if the content is undesired; cadence is self-throttling because multiple commits fold into one PR.
- release-plz does not run semver checks (`semver_check = false`) because they require crates.io. For private app code this is acceptable; if the repo ever exports a stable library crate, that crate will need `publish = true` plus semver checks re-enabled.

### Locked-in

- `api-gateway` is the workspace tag/release representative. If the repo ever ships a second user-facing deliverable that needs its own release cadence, `release-plz.toml` must grow a second `[[package]]` block and the tag-naming convention must absorb the ambiguity (prefix, suffix, or multi-tag).
- `publish = false` is the default. Any crate that wants to go to crates.io must explicitly override, and any such override implies re-enabling semver checks for that crate.

### Follow-up

- None required. Plan 007 tasks are complete on disk (release-please artifacts deleted, release-plz wired in, first workspace tag `v0.1.1` cut).

## Alternatives considered

### Option A — Keep release-please and remove workspace inheritance

Rewrite every member `Cargo.toml` with a concrete `version = "…"` to satisfy `cargo-workspace`'s non-inheriting parser. **Rejected:** regresses the Cargo-official workspace pattern for every Cargo-native tool in the ecosystem just to accommodate one tool that is second-class for Rust. `cargo publish` already handles inheritance correctly; the problem is release-please, not Cargo.

### Option B — Keep release-please and wait for upstream fix

Issue #2111 has been open since 2023-11 at priority `p3` with related Rust-integration issues (#2517, #2558, #2748) stacking up. **Rejected:** no credible signal of a fix; `main` stays red indefinitely; the cost of waiting compounds with every release-worthy commit that should have been tagged.

### Option C — Hand-rolled release scripts (bash + `cargo set-version` + `gh release create`)

Full control and zero new dependencies. **Rejected:** reinvents a well-maintained Rust-native tool. The marginal cost of adopting release-plz is one config file plus one workflow file; the marginal cost of a hand-rolled tool is ongoing maintenance, changelog templating, Release-PR orchestration, and conventional-commit parsing. Not worth the wheel-reinvention.

### Option D — cargo-release

Rust-native CLI for cutting releases from a developer workstation. **Rejected:** requires a human to run it locally and push tags, which couples release cadence to whoever happens to be at a keyboard. release-plz's GitHub-Actions-driven Release PR flow is strictly more agent-friendly and removes the human-in-the-loop requirement from the happy path (humans still review and merge the Release PR, which is the right checkpoint).

## Implementation notes

- Action pin: `release-plz/action@v0.5`. Upgrade by bumping the tag; no config schema churn expected across minor versions per the action changelog.
- Conventional Commit parsing is automatic; historical pre-policy commits are ignored gracefully by release-plz.
- The migration landed in PR #5 (`83982b8 ci: migrate release automation to release-plz`), with follow-ups `a1a91cb` (single workspace tag via api-gateway representative) and `3fa1732` (rename `tag_name` to `git_tag_name` after a release-plz schema change).
- The first workspace release under this system was `v0.1.1` (`Cargo.toml:6`), cut after the migration merged.

## References

- `release-plz.toml:6` — `publish = false`, the non-goal lock for crates.io.
- `release-plz.toml:13-17` — per-crate tag/release suppression.
- `release-plz.toml:24-29` — `api-gateway` as the workspace tag representative.
- `.github/workflows/release-plz.yml:1-62` — two-job release workflow with concurrency groups.
- `Cargo.toml:6` — `[workspace.package] version = "0.1.1"`, the single source of version truth.
- `apps/api-gateway/Cargo.toml:3`, `libs/infra-auth/Cargo.toml:3` — `version.workspace = true` inheritance examples.
- Upstream blocker: [`googleapis/release-please#2111`](https://github.com/googleapis/release-please/issues/2111).
- Cargo Book §Workspaces — `workspace.package` inheritance semantics and publish-time rewriting.
- Originated from: `plans/007-release-plz-migration.md` at commit 3ee9cc8; migration landed in commit 83982b8.
- Related: ADR-0009 (workspace layout).
