# Configuration Management

## Overview

All services in this monorepo use a shared configuration loading mechanism provided by `libs/common`. The configuration system follows a **composition pattern** where:

- **`libs/common`** provides reusable loading logic (`load_config`) and common configuration fields (`CommonConfig`)
- **Each service** defines its own `Settings` struct and includes common fields via `#[serde(flatten)]`
- **`config.toml`** files live in each service's directory (not tracked in git)
- **`config.example.toml`** templates are tracked in git for reference

## Configuration Architecture

### Common Configuration (`libs/common`)

The `CommonConfig` struct contains fields shared across all services:

```rust
pub struct CommonConfig {
    pub app_env: String,         // dev, staging, prod, test
    pub log_level: String,        // trace, debug, info, warn, error
    pub database_url: Option<String>,  // Optional database URL
}
```

### Service-Specific Configuration

Each service defines its own `Settings` struct and flattens the common config:

```rust
use common::config::{load_config, CommonConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    // Service-specific fields
    pub host: String,
    pub port: u16,
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("MY_SERVICE", "config.toml")
}
```

## Configuration Sources (Priority from highest to lowest)

1. **Environment variables** (highest priority, per-service prefix)
2. **`config.toml`** file in service directory (optional)
3. **Default values** from Rust `Default` trait (lowest priority)

## Environment Variable Naming Convention

We use **double underscores (`__`)** as the nesting separator for structured configuration fields.

### Examples

| Field Path             | Environment Variable                 | Example Value   |
| ---------------------- | ------------------------------------ | --------------- |
| `app_env`              | `APP__APP_ENV`                       | `prod`          |
| `log_level`            | `APP__LOG_LEVEL`                     | `debug`         |
| `app.host`             | `APP__APP__HOST`                     | `0.0.0.0`       |
| `app.port`             | `APP__APP__PORT`                     | `8080`          |
| `jwt_secret`           | `AUTH_SERVICE__JWT_SECRET`           | `my-secret-key` |
| `jwt_expiration_hours` | `AUTH_SERVICE__JWT_EXPIRATION_HOURS` | `48`            |

### Service-Specific Prefixes

| Service      | Prefix         | Config File Path                |
| ------------ | -------------- | ------------------------------- |
| api-gateway  | `APP`          | `apps/api-gateway/config.toml`  |
| auth-service | `AUTH_SERVICE` | `apps/auth-service/config.toml` |
| user-service | `USER_SERVICE` | `apps/user-service/config.toml` |

## Configuration File Workflow

### For Developers

1. **Clone the repository** and navigate to a service directory
2. **Copy the template**: `cp config.example.toml config.toml`
3. **Customize** `config.toml` with your local settings (database URLs, secrets, etc.)
4. **Never commit** `config.toml` (it's in `.gitignore`)

### For Production/CI/CD

- Use **environment variables** to override configuration (recommended for secrets)
- Or mount `config.toml` via ConfigMaps/Secrets in Kubernetes
- Or generate `config.toml` during deployment from secure storage

## Example Configuration Files

### api-gateway (`apps/api-gateway/config.example.toml`)

```toml
# Common fields
app_env = "dev"
log_level = "info"

# Service-specific fields
[app]
host = "127.0.0.1"
port = 8080
name = "api-gateway"

[db]
url = "postgres://localhost/pim"

[jwt]
secret = "your-secret-key-change-in-production"
expiration_hours = 24
```

### auth-service (`apps/auth-service/config.example.toml`)

```toml
# Common fields
app_env = "dev"
log_level = "info"

# Service-specific fields
host = "127.0.0.1"
port = 50051
metrics_port = 60051
jwt_secret = "your-secret-key-change-in-production"
jwt_expiration_hours = 24
```

## Usage in Services

### Step 1: Define Settings Struct with Flattened Common Config

```rust
use common::config::{load_config, CommonConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub host: String,
    pub port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            common: CommonConfig::default(),
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}
```

### Step 2: Load Configuration with Service-Specific Prefix

```rust
use config::ConfigError;

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("MY_SERVICE", "config.toml")
}
```

}

````

### Step 3: Use in Main

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = load_settings()?;
    println!("Starting on {}:{}", settings.host, settings.port);
    // Access common fields
    println!("Environment: {}", settings.common.app_env);
    println!("Log level: {}", settings.common.log_level);
    Ok(())
}
````

## libs/common Feature Flags

The `common` library now uses feature flags to enable optional functionality:

- `config_mod`: Configuration loading support (requires `config` and `serde` crates)
- `telemetry_prometheus`: Prometheus metrics exporter and core telemetry
- `telemetry_grpc`: gRPC metrics layer (requires `telemetry_prometheus`)
- `telemetry_http`: HTTP metrics endpoint server (requires `telemetry_prometheus`)

### Enabling Features in Your Service

Add features to your `Cargo.toml`:

```toml
[dependencies]
common = { path = "../../libs/common", features = ["config_mod", "telemetry_prometheus", "telemetry_grpc"] }
```

**Note**: By default, no features are enabled (`default = []`). Services must explicitly opt-in.

## Migration from Old Configuration System

### Key Changes

1. **Compositional config**: Use `#[serde(flatten)]` to include `CommonConfig` instead of duplicating fields
2. **Per-service prefix**: `load_config` now accepts a custom prefix (e.g., `"AUTH_SERVICE"`) instead of hardcoded `"APP"`
3. **Single config file**: Changed from `load_config("APP", &["config/default", "config/local"])` to `load_config("AUTH_SERVICE", "config.toml")`
4. **Config location**: Files moved from `config/service-name.toml` to `apps/service-name/config.toml`
5. **Git workflow**: Only `.example.toml` files are tracked; real `config.toml` is ignored

### Breaking Changes

If you previously used:

```bash
export AUTH_SERVICE_JWT_SECRET="my-secret"  # Old: single underscore
```

You now need:

```bash
export AUTH_SERVICE__JWT_SECRET="my-secret"  # New: double underscore
```

For nested fields (e.g., `app.host`), double underscores are required:

```bash
export APP__APP__HOST="0.0.0.0"  # Correct
export APP_APP_HOST="0.0.0.0"    # Incorrect
```

## Production Deployment Checklist

- [ ] Copy `config.example.toml` to `config.toml` for each service
- [ ] Set all required environment variables with service-specific prefixes
- [ ] Ensure JWT secrets are rotated from default values
- [ ] Validate database URLs and credentials
- [ ] Set `APP_ENV=production` (or via `<SERVICE>__APP_ENV=production`)
- [ ] Do not commit `config.toml` files containing secrets to version control

## Troubleshooting

### Configuration not loading from environment variables

1. Check prefix matches your service: `APP`, `AUTH_SERVICE`, or `USER_SERVICE`
2. Use `__` (double underscore) for nested fields: `APP__JWT__SECRET`, not `APP_JWT_SECRET`
3. Use `__` for flattened common fields: `AUTH_SERVICE__APP_ENV`, `AUTH_SERVICE__LOG_LEVEL`
4. Verify environment variables are exported: `printenv | grep AUTH_SERVICE__`

### Service fails to start with "Failed to load configuration"

1. Check if `config.toml` exists (it's optional, but helpful for local dev)
2. Validate TOML syntax if using config files
3. Ensure `Default` trait provides valid fallback values
4. Check service logs for detailed deserialization errors

### Config values not overriding as expected

Priority order (highest to lowest):

1. **Environment variables** (e.g., `APP__LOG_LEVEL=debug`)
2. **`config.toml`** file (optional)
3. **Default trait** implementation

Example: If `config.toml` sets `log_level = "info"` but `APP__LOG_LEVEL=debug` is set in the environment, the service will use `debug`.
