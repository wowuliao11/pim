# Plan 005: CI/CD Remaining Issues & Hardening

**Status:** Pending
**Created:** 2025-04-16
**Context:** After initial CI/CD infrastructure setup (buf integration, GitHub Actions, cargo-deny, Dockerfiles, release-please), several issues remain that need follow-up.

---

## Phase 1: Fix Pre-existing Test Failure (Priority: High)

### Problem

`apps/api-gateway/src/config/env.rs:20-23` — `test_load_default_settings` fails because `load_settings()` calls `load_config("APP", "config.toml")` which requires a config file or environment variables to be present.

### Acceptance Criteria

- [ ] Test passes in CI without external config files
- [ ] Test properly validates default settings behavior
- [ ] No regression on other tests

### Approach Options

1. **Fix the test** — provide in-test defaults or use `temp_env` crate to set required env vars
2. **Fix `load_config`** — ensure it returns valid defaults when no config file exists
3. **Mark as `#[ignore]` with TODO** — if it's an integration test that genuinely needs config

---

## Phase 2: Resolve RUSTSEC Advisory Ignores (Priority: Medium)

### Problem

`deny.toml` currently ignores 3 known vulnerabilities in transitive dependencies:

| RUSTSEC ID | Crate | Issue | Root Cause |
|---|---|---|---|
| RUSTSEC-2023-0071 | `rsa 0.9.10` | Marvin Attack timing sidechannel | `openidconnect` → `rsa` |
| RUSTSEC-2026-0098 | `rustls-webpki 0.103.10` | URI name constraint bypass | `rustls` → `rustls-webpki` |
| RUSTSEC-2026-0099 | `rustls-webpki 0.103.10` | Wildcard name constraint bypass | `rustls` → `rustls-webpki` |

### Acceptance Criteria

- [ ] All 3 RUSTSEC ignores removed from `deny.toml`
- [ ] `cargo deny check` passes clean

### Approach

1. Check if `zitadel`/`openidconnect` crates have released updates that pull in patched dependencies
2. If yes: `cargo update -p openidconnect -p zitadel -p rustls-webpki -p rsa`
3. If no: open issues upstream or evaluate alternative crates
4. Monitor via GitHub Dependabot (enable in repo settings)

---

## Phase 3: CI Enhancements (Priority: Low)

### 3a. Unify Dockerfiles with build arg

Both `apps/user-service/Dockerfile` and `apps/api-gateway/Dockerfile` are identical except for the binary name. Consolidate into a single `Dockerfile` with `ARG SERVICE_NAME`.

### 3b. Add `docker-compose.yml` for local development

Document correct Docker build commands:
```yaml
services:
  user-service:
    build:
      context: .
      dockerfile: apps/user-service/Dockerfile
  api-gateway:
    build:
      context: .
      dockerfile: apps/api-gateway/Dockerfile
```

### 3c. Pin Rust toolchain version

Change `rust-toolchain.toml` from `channel = "stable"` to a specific version (e.g., `channel = "1.86.0"`) for reproducible builds.

### 3d. Add `cargo test --no-fail-fast` in CI

Show all test failures at once instead of stopping at the first.

### 3e. Enable Dependabot

Create `.github/dependabot.yml` for automated dependency update PRs (cargo + GitHub Actions).

### Acceptance Criteria

- [ ] Single Dockerfile with build arg working for both services
- [ ] docker-compose.yml for local dev
- [ ] Pinned Rust toolchain version
- [ ] Dependabot enabled
