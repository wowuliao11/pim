use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

/// Common configuration fields shared across services
///
/// Use `#[serde(flatten)]` in your service-specific config struct to include these fields.
///
/// # Example
///
/// ```no_run
/// use serde::{Deserialize, Serialize};
/// use infra_config::CommonConfig;
///
/// #[derive(Debug, Deserialize, Serialize)]
/// struct AppConfig {
///     #[serde(flatten)]
///     pub common: CommonConfig,
///
///     // Service-specific fields
///     pub grpc_port: u16,
/// }
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommonConfig {
    /// Application environment: dev, staging, prod, test
    #[serde(default = "default_app_env")]
    pub app_env: String,

    /// Log level: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Optional database URL (common for many services)
    #[serde(default)]
    pub database_url: Option<String>,
}

fn default_app_env() -> String {
    "dev".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for CommonConfig {
    fn default() -> Self {
        Self {
            app_env: default_app_env(),
            log_level: default_log_level(),
            database_url: None,
        }
    }
}

/// Generic configuration loader for services
///
/// Loads configuration from multiple sources with priority (highest to lowest):
/// 1. Environment variables with the given prefix (using `__` as nesting separator)
/// 2. Optional TOML configuration file at the specified path
/// 3. Default values from the struct's Default impl
///
/// # Arguments
///
/// * `prefix` - Environment variable prefix (e.g., "APP", "AUTH_SERVICE", "USER_SERVICE")
/// * `config_path` - Path to TOML config file (e.g., "config.toml"), can be empty string to skip file loading
///
/// # Examples
///
/// ```no_run
/// use serde::{Deserialize, Serialize};
/// use infra_config::{load_config, CommonConfig};
///
/// #[derive(Debug, Deserialize, Serialize, Default)]
/// struct AppConfig {
///     #[serde(flatten)]
///     pub common: CommonConfig,
///
///     pub grpc_port: u16,
/// }
///
/// let config: AppConfig = load_config("AUTH_SERVICE", "config.toml").unwrap();
/// ```
pub fn load_config<T>(prefix: &str, config_path: &str) -> Result<T, ConfigError>
where
    T: serde::de::DeserializeOwned + Default + serde::Serialize,
{
    let mut builder = Config::builder()
        // Start with default values from the struct's Default impl
        .add_source(Config::try_from(&T::default())?);

    // Add optional TOML configuration file
    if !config_path.is_empty() {
        builder = builder.add_source(File::with_name(config_path).required(false));
    }

    // Override with environment variables
    // Using `__` as separator for nested fields (Rust ecosystem convention)
    // e.g., AUTH_SERVICE__DATABASE_URL for database_url field
    builder = builder.add_source(Environment::with_prefix(prefix).prefix_separator("_").separator("__"));

    builder.build()?.try_deserialize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestSettings {
        host: String,
        port: u16,
    }

    impl Default for TestSettings {
        fn default() -> Self {
            Self {
                host: "127.0.0.1".to_string(),
                port: 8080,
            }
        }
    }

    #[test]
    fn test_load_with_defaults_only() {
        // Should load defaults when no files or env vars present
        let settings: TestSettings = load_config("NONEXISTENT_PREFIX", "").unwrap();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 8080);
    }

    #[test]
    fn test_load_with_nonexistent_files() {
        // Should not fail when config files don't exist (required=false)
        let settings: TestSettings = load_config("NONEXISTENT_PREFIX", "config/nonexistent").unwrap();
        assert_eq!(settings.host, "127.0.0.1");
    }
}
