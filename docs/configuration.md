# Configuration Management

## Overview

All services in this monorepo use a shared configuration loading mechanism provided by `libs/infra-config`. The configuration system follows a **composition pattern** where:

- **`libs/infra-config`** provides reusable loading logic (`load_config`) and common configuration fields (`CommonConfig`)
- **Each service** defines its own `Settings` struct and includes common fields via `#[serde(flatten)]`
- **`config.toml`** files live in each service's directory (not tracked in git)
- **`config.example.toml`** templates are tracked in git for reference

## Configuration Architecture

### Common Configuration (`libs/infra-config`)

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
use infra_config::{load_config, CommonConfig};
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

| Field Path                       | Environment Variable                              | Example Value                         |
| -------------------------------- | ------------------------------------------------- | ------------------------------------- |
| `app_env`                        | `APP__APP_ENV`                                    | `prod`                                |
| `log_level`                      | `APP__LOG_LEVEL`                                  | `debug`                               |
| `app.host`                       | `APP__APP__HOST`                                  | `0.0.0.0`                             |
| `app.port`                       | `APP__APP__PORT`                                  | `8080`                                |
| `zitadel.authority`              | `APP__ZITADEL__AUTHORITY`                         | `https://my-instance.zitadel.cloud`   |
| `zitadel.key_file`              | `APP__ZITADEL__KEY_FILE`                          | `./keys/api-gateway.json`             |
| `zitadel_authority`              | `USER_SERVICE__ZITADEL_AUTHORITY`                 | `https://my-instance.zitadel.cloud`   |
| `zitadel_service_account_token` | `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN`     | `pat-xxx`                             |

### Service-Specific Prefixes

| Service      | Prefix         | Config File Path                |
| ------------ | -------------- | ------------------------------- |
| api-gateway  | `APP`          | `apps/api-gateway/config.toml`  |
| user-service | `USER_SERVICE` | `apps/user-service/config.toml` |

## Configuration File Workflow

### For Developers

1. **Clone the repository** and navigate to a service directory
2. **Copy the template**: `cp config.example.toml config.toml`
3. **Customize** `config.toml` with your local settings (Zitadel credentials, database URLs, etc.)
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
metrics_port = 60080
name = "api-gateway"

[db]
url = "postgres://localhost/pim"

[zitadel]
authority = "https://my-instance.zitadel.cloud"
key_file = "zitadel-key.json"
```

### user-service (`apps/user-service/config.example.toml`)

```toml
# Common fields
app_env = "dev"
log_level = "info"

# Service-specific fields
host = "127.0.0.1"
port = 50051
metrics_port = 60051
zitadel_authority = "https://my-instance.zitadel.cloud"
zitadel_service_account_token = "your-service-account-pat"
```

## Zitadel Configuration

### API Gateway (Token Introspection)

The API Gateway validates incoming Bearer tokens by calling Zitadel's Token Introspection endpoint. This requires an **API application** in Zitadel with JWT Profile authentication and a downloaded JSON key file:

| Setting     | Description                              | Env Var                    |
| ----------- | ---------------------------------------- | -------------------------- |
| `authority` | Zitadel instance URL                     | `APP__ZITADEL__AUTHORITY`  |
| `key_file`  | Path to Zitadel API app JSON key file    | `APP__ZITADEL__KEY_FILE`   |

### User Service (Management API Proxy)

The user-service proxies user CRUD operations to Zitadel's Management REST API v2. This requires a **service account** with a Personal Access Token (PAT):

| Setting                      | Description                          | Env Var                                           |
| ---------------------------- | ------------------------------------ | ------------------------------------------------- |
| `zitadel_authority`          | Zitadel instance URL                 | `USER_SERVICE__ZITADEL_AUTHORITY`                 |
| `zitadel_service_account_token` | Service account PAT               | `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN`     |

## Usage in Services

### Step 1: Define Settings Struct with Flattened Common Config

```rust
use infra_config::{load_config, CommonConfig};
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
```

## libs/infra-config and libs/infra-telemetry

The library layer is now split into atomic crates:

### `libs/infra-config`

Provides configuration loading utilities:

- `load_config(prefix, config_path)` - Load configuration from TOML + env vars
- `CommonConfig` - Shared configuration fields across services
- `AppEnv` - Runtime environment detection enum

### `libs/infra-telemetry`

Provides metrics and observability primitives with feature flags:

- `prometheus`: Prometheus metrics exporter and core telemetry
- `grpc`: gRPC metrics layer (requires `prometheus`)
- `http`: HTTP metrics endpoint server (requires `prometheus`)

### Enabling Features in Your Service

Add features to your `Cargo.toml`:

```toml
[dependencies]
infra-config = { path = "../../libs/infra-config" }
infra-telemetry = { path = "../../libs/infra-telemetry", features = ["prometheus", "grpc", "http"] }
```

**Note**: By default, no features are enabled (`default = []`). Services must explicitly opt-in.

## Production Deployment Checklist

- [ ] Copy `config.example.toml` to `config.toml` for each service
- [ ] Set all required environment variables with service-specific prefixes
- [ ] Configure Zitadel credentials (API app JSON key file for gateway, service account PAT for user-service)
- [ ] Validate database URLs and credentials
- [ ] Set `APP_ENV=production` (or via `<SERVICE>__APP_ENV=production`)
- [ ] Do not commit `config.toml` files containing secrets to version control

## Troubleshooting

### Configuration not loading from environment variables

1. Check prefix matches your service: `APP` or `USER_SERVICE`
2. Use `__` (double underscore) for nested fields: `APP__ZITADEL__AUTHORITY`, not `APP_ZITADEL_AUTHORITY`
3. Use `__` for flattened common fields: `USER_SERVICE__APP_ENV`, `USER_SERVICE__LOG_LEVEL`
4. Verify environment variables are exported: `printenv | grep APP__`

### Service fails to start with "Failed to load configuration"

1. Check if `config.toml` exists (it's optional, but helpful for local dev)
2. Validate TOML syntax if using config files
3. Ensure `Default` trait provides valid fallback values
4. Check service logs for detailed deserialization errors

### Gateway fails with "Failed to load Zitadel key file" or "Failed to build Zitadel introspection config"

1. Verify `zitadel.key_file` path points to a valid JSON key file downloaded from Zitadel console
2. Check the key file contains valid JSON with fields: `type`, `keyId`, `key`, `appId`, `clientId`
3. Verify `zitadel.authority` points to a valid Zitadel instance
4. Check that the Zitadel instance is reachable from the gateway
5. Ensure the Zitadel instance's OIDC discovery endpoint (`/.well-known/openid-configuration`) is accessible

### Config values not overriding as expected

Priority order (highest to lowest):

1. **Environment variables** (e.g., `APP__LOG_LEVEL=debug`)
2. **`config.toml`** file (optional)
3. **Default trait** implementation

Example: If `config.toml` sets `log_level = "info"` but `APP__LOG_LEVEL=debug` is set in the environment, the service will use `debug`.
