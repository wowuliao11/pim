# JWT Profile Introspection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace api-gateway's Basic Auth token introspection with Private Key JWT Profile authentication, using a downloaded JSON key file from Zitadel.

**Architecture:** The `zitadel` crate (v5) already provides `IntrospectionConfigBuilder::with_jwt_profile(Application)` alongside the current `with_basic_auth()`. We swap the auth method, change the config from `client_id`/`client_secret` to a `key_file` path pointing to the Zitadel-downloaded JSON key, and update `infra-auth` to re-export the `Application` type.

**Tech Stack:** Rust, `zitadel` crate v5 (features: `actix`, `oidc`, `credentials`), `serde`, `config` crate

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Modify | `libs/infra-auth/src/lib.rs` | Add re-export of `zitadel::credentials::Application` |
| Modify | `apps/api-gateway/src/config/settings.rs` | Replace `client_id`/`client_secret` with `key_file` in `ZitadelSettings` |
| Modify | `apps/api-gateway/src/config/app_config.rs` | Replace accessor methods to expose `key_file` path |
| Modify | `apps/api-gateway/src/main.rs` | Switch from `with_basic_auth` to `with_jwt_profile` |
| Modify | `apps/api-gateway/config.example.toml` | Update `[zitadel]` section |
| Modify | `docs/configuration.md` | Update config documentation |
| Modify | `docs/design.md` | Update authentication description |

---

### Task 1: Update `infra-auth` to re-export `Application`

**Files:**
- Modify: `libs/infra-auth/src/lib.rs`

- [ ] **Step 1: Add Application re-export**

Replace the contents of `libs/infra-auth/src/lib.rs` with:

```rust
//! Infrastructure authentication library — Zitadel OIDC integration
//!
//! Provides re-exports from the `zitadel` crate for actix-web Token Introspection.
//! The API Gateway uses `IntrospectedUser` as an actix extractor to validate
//! Bearer tokens against Zitadel's introspection endpoint.

// Re-export the actix introspection types that consumers need
pub use zitadel::actix::introspection::{IntrospectedUser, IntrospectionConfig, IntrospectionConfigBuilder};

// Re-export Application credential type for JWT Profile authentication
pub use zitadel::credentials::Application;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p infra-auth`
Expected: compiles with no errors (warnings OK)

- [ ] **Step 3: Commit**

```bash
git add libs/infra-auth/src/lib.rs
git commit -m "feat(infra-auth): re-export Application for JWT Profile auth"
```

---

### Task 2: Update `ZitadelSettings` config struct

**Files:**
- Modify: `apps/api-gateway/src/config/settings.rs`

- [ ] **Step 1: Replace `client_id`/`client_secret` with `key_file`**

In `apps/api-gateway/src/config/settings.rs`, replace the `ZitadelSettings` struct and its impls:

```rust
#[derive(Deserialize, Serialize, Clone)]
pub struct ZitadelSettings {
    /// Zitadel instance URL, e.g. "https://my-instance.zitadel.cloud"
    pub authority: String,
    /// Path to the Zitadel API application JSON key file (downloaded from Zitadel console)
    /// The file contains: type, keyId, key (RSA private key), appId, clientId
    pub key_file: String,
}

// Manual Debug impl: show key_file path but not its contents
impl fmt::Debug for ZitadelSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZitadelSettings")
            .field("authority", &self.authority)
            .field("key_file", &self.key_file)
            .finish()
    }
}

impl Default for ZitadelSettings {
    fn default() -> Self {
        Self {
            authority: "https://localhost.zitadel.cloud".to_string(),
            key_file: "zitadel-key.json".to_string(),
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p api-gateway`
Expected: compile errors in `app_config.rs` and `main.rs` referencing removed fields — this is expected and will be fixed in the next tasks.

- [ ] **Step 3: Commit**

```bash
git add apps/api-gateway/src/config/settings.rs
git commit -m "refactor(api-gateway): replace client_id/client_secret with key_file in ZitadelSettings"
```

---

### Task 3: Update `AppConfig` accessor methods

**Files:**
- Modify: `apps/api-gateway/src/config/app_config.rs`

- [ ] **Step 1: Replace accessor methods**

In `apps/api-gateway/src/config/app_config.rs`, replace the three Zitadel accessor methods:

Remove:
```rust
    pub fn zitadel_client_id(&self) -> &str {
        &self.settings.zitadel.client_id
    }

    pub fn zitadel_client_secret(&self) -> &str {
        &self.settings.zitadel.client_secret
    }
```

Add:
```rust
    pub fn zitadel_key_file(&self) -> &str {
        &self.settings.zitadel.key_file
    }
```

Keep `zitadel_authority()` unchanged.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p api-gateway`
Expected: compile errors only in `main.rs` — this is expected and will be fixed next.

- [ ] **Step 3: Commit**

```bash
git add apps/api-gateway/src/config/app_config.rs
git commit -m "refactor(api-gateway): replace client accessor methods with key_file accessor"
```

---

### Task 4: Update `main.rs` to use JWT Profile

**Files:**
- Modify: `apps/api-gateway/src/main.rs`

- [ ] **Step 1: Switch introspection config builder**

In `apps/api-gateway/src/main.rs`, add the `Application` import and replace the introspection config construction.

Change the import line:
```rust
use infra_auth::IntrospectionConfigBuilder;
```
to:
```rust
use infra_auth::{Application, IntrospectionConfigBuilder};
```

Replace lines 36-41 (the introspection config block):
```rust
    // Build Zitadel introspection config (fetches OIDC discovery document)
    let introspection_config = IntrospectionConfigBuilder::new(config.zitadel_authority())
        .with_basic_auth(config.zitadel_client_id(), config.zitadel_client_secret())
        .build()
        .await
        .map_err(|e| std::io::Error::other(format!("Failed to build Zitadel introspection config: {}", e)))?;
```

with:
```rust
    // Load Zitadel application key file for JWT Profile authentication
    let application = Application::load_from_file(config.zitadel_key_file())
        .map_err(|e| std::io::Error::other(format!("Failed to load Zitadel key file '{}': {}", config.zitadel_key_file(), e)))?;

    // Build Zitadel introspection config (fetches OIDC discovery document)
    let introspection_config = IntrospectionConfigBuilder::new(config.zitadel_authority())
        .with_jwt_profile(application)
        .build()
        .await
        .map_err(|e| std::io::Error::other(format!("Failed to build Zitadel introspection config: {}", e)))?;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p api-gateway`
Expected: compiles successfully with no errors

- [ ] **Step 3: Verify the full workspace compiles**

Run: `cargo check --workspace`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add apps/api-gateway/src/main.rs
git commit -m "feat(api-gateway): switch from Basic Auth to JWT Profile for token introspection"
```

---

### Task 5: Update config example and documentation

**Files:**
- Modify: `apps/api-gateway/config.example.toml`
- Modify: `docs/configuration.md`
- Modify: `docs/design.md`

- [ ] **Step 1: Update config.example.toml**

Replace the `[zitadel]` section in `apps/api-gateway/config.example.toml`:

```toml
[zitadel]
# IMPORTANT: Replace these with your actual Zitadel settings!
# Create an API application in your Zitadel project with JWT Profile authentication.
# Download the JSON key file from the Zitadel console and place it in a secure location.
authority = "https://my-instance.zitadel.cloud"
key_file = "zitadel-key.json"
```

- [ ] **Step 2: Update docs/configuration.md**

In `docs/configuration.md`, update these sections:

1. The environment variable table (lines 67-69): replace `client_id`/`client_secret` rows with:

| Field Path         | Environment Variable         | Example Value                       |
| ------------------ | ---------------------------- | ----------------------------------- |
| `zitadel.authority` | `APP__ZITADEL__AUTHORITY`   | `https://my-instance.zitadel.cloud` |
| `zitadel.key_file` | `APP__ZITADEL__KEY_FILE`    | `./keys/api-gateway.json`           |

2. The TOML example (lines 114-118): replace with updated `[zitadel]` block.

3. The Zitadel Configuration section (lines 136-145): update heading and table:

> The API Gateway validates incoming Bearer tokens by calling Zitadel's Token Introspection endpoint. This requires an **API application** in Zitadel with JWT Profile authentication and a downloaded JSON key file:
>
> | Setting     | Description                              | Env Var                    |
> |-------------|------------------------------------------|----------------------------|
> | `authority` | Zitadel instance URL                     | `APP__ZITADEL__AUTHORITY`  |
> | `key_file`  | Path to Zitadel API app JSON key file    | `APP__ZITADEL__KEY_FILE`   |

4. The troubleshooting section (lines 265-270): update error guidance to mention key file instead of client_id/client_secret.

5. The production checklist (line 243): update to mention key file.

- [ ] **Step 3: Update docs/design.md**

In `docs/design.md`, update:

1. Line 54: change description to mention JWT Profile instead of Basic Auth:
   > **API Gateway** receives Bearer tokens and validates them via Zitadel's Token Introspection endpoint using JWT Profile authentication (via the `zitadel` crate's `IntrospectedUser` actix extractor)

2. Lines 157-159: replace the environment variable examples:
   > - `APP__ZITADEL__KEY_FILE=./keys/api-gateway.json` → `zitadel.key_file`

   Remove the `CLIENT_ID` and `CLIENT_SECRET` lines.

- [ ] **Step 4: Commit**

```bash
git add apps/api-gateway/config.example.toml docs/configuration.md docs/design.md
git commit -m "docs: update configuration for JWT Profile introspection"
```

---

## Verification

After all tasks are complete:

1. `cargo check --workspace` — must compile
2. `cargo clippy --workspace` — no new warnings
3. Place a valid Zitadel JSON key file at the configured path and start the gateway to verify connectivity

## Summary of Removals

- `ZitadelSettings.client_id` field — removed
- `ZitadelSettings.client_secret` field — removed
- `AppConfig::zitadel_client_id()` method — removed
- `AppConfig::zitadel_client_secret()` method — removed
- `with_basic_auth()` call in `main.rs` — replaced with `with_jwt_profile()`
- All `client_secret` redaction logic in `Debug` impl — no longer needed (key file path is not sensitive)
