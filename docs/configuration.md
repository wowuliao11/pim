# Configuration Management

## Overview

All services in this monorepo use a shared configuration loading mechanism provided by `libs/common`. Each service maintains its own `Settings` struct but delegates the loading logic to a generic, reusable loader.

## Configuration Sources (Priority from highest to lowest)

1. **Environment variables** (highest priority)
2. **TOML configuration files** (optional)
3. **Default values** from Rust `Default` trait (lowest priority)

## Environment Variable Naming Convention

We use **double underscores (`__`)** as the nesting separator for structured configuration fields. This aligns with Rust ecosystem conventions and avoids conflicts with snake_case field names.

### Examples

| Field Path (nested)  | Environment Variable     | Example Value                    |
| -------------------- | ------------------------ | -------------------------------- |
| `app.host`           | `APP__APP__HOST`         | `0.0.0.0`                        |
| `app.port`           | `APP__APP__PORT`         | `8080`                           |
| `db.url`             | `APP__DB__URL`           | `postgres://localhost/mydb`      |
| `jwt.secret`         | `APP__JWT__SECRET`       | `my-secret-key`                  |
| `jwt_expiration_hours` | `AUTH_SERVICE__JWT_EXPIRATION_HOURS` | `48`               |

### Service-Specific Prefixes

| Service        | Prefix         | Example                          |
| -------------- | -------------- | -------------------------------- |
| api-gateway    | `APP`          | `APP__APP__HOST`                 |
| auth-service   | `AUTH_SERVICE` | `AUTH_SERVICE__JWT_SECRET`       |
| user-service   | `USER_SERVICE` | `USER_SERVICE__PORT`             |

## TOML Configuration Files

Each service can load optional TOML files. File locations are relative to the service's working directory (typically the repository root).

### api-gateway

- `config/default.toml` (base configuration)
- `config/local.toml` (local overrides for development)

### auth-service

- `config/auth-service.toml`

### user-service

- `config/user-service.toml`

### Example TOML Structure

```toml
# config/default.toml (api-gateway)
[app]
host = "0.0.0.0"
port = 8080
name = "api-gateway"

[db]
url = "postgres://localhost/pim"

[jwt]
secret = "change-me-in-production"
expiration_hours = 24
```

## Usage in Services

### Step 1: Define Settings Struct

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    // ... other fields
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}
```

### Step 2: Load Configuration

```rust
use common::config::load_config;
use config::ConfigError;

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("APP", &["config/default", "config/local"])
}
```

### Step 3: Use in Main

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = load_settings()?;
    println!("Starting on {}:{}", settings.host, settings.port);
    Ok(())
}
```

## Migration Notes

### Removed Features

- ❌ **`.env` file support**: No longer loaded automatically. Use environment variables directly or TOML files.
- ❌ **Single underscore separator**: Old `AUTH_SERVICE_JWT_SECRET` syntax no longer works for nested fields.

### Breaking Changes

If you previously used:
```bash
export AUTH_SERVICE_JWT_SECRET="my-secret"
```

You now need:
```bash
export AUTH_SERVICE__JWT_SECRET="my-secret"
```

For flat (non-nested) fields, the old syntax still works:
```bash
export AUTH_SERVICE__HOST="0.0.0.0"  # Recommended
export AUTH_SERVICE_HOST="0.0.0.0"   # May still work if field is not nested
```

## Production Deployment Checklist

- [ ] Set all required environment variables with service-specific prefixes
- [ ] Ensure JWT secrets are rotated from default values
- [ ] Validate database URLs and credentials
- [ ] Set `APP_ENV=production` for production environment detection
- [ ] Do not commit `.toml` files containing secrets to version control

## Troubleshooting

### Configuration not loading from environment variables

1. Check prefix: `APP`, `AUTH_SERVICE`, or `USER_SERVICE`?
2. Use `__` for nested fields: `APP__JWT__SECRET`, not `APP_JWT_SECRET`
3. Verify environment variables are exported: `printenv | grep APP__`

### Service fails to start with "Failed to load configuration"

1. Check if required config files exist (or mark them `.required(false)` in the loader)
2. Validate TOML syntax if using config files
3. Ensure `Default` trait provides valid fallback values
4. Check service logs for detailed deserialization errors

### Config values not overriding as expected

Priority order:
1. Environment variables (highest)
2. TOML files (middle)
3. Default trait (lowest)

Example: If `config/local.toml` sets `port = 9000` but `APP__APP__PORT=8080` is set in the environment, the service will use `8080`.
