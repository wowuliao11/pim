use config::{Config, ConfigError, Environment, File};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Generic configuration loader for services
///
/// Loads configuration from multiple sources with priority (highest to lowest):
/// 1. Environment variables with the given prefix (using `__` as nesting separator)
/// 2. Optional TOML configuration files
/// 3. Default values in the Settings struct
///
/// # Examples
///
/// ```no_run
/// use serde::Deserialize;
/// use common::config::load_config;
///
/// #[derive(Deserialize, Default)]
/// struct Settings {
///     host: String,
///     port: u16,
/// }
///
/// let settings: Settings = load_config(
///     "APP",
///     &["config/default", "config/local"],
/// ).unwrap();
/// ```
pub fn load_config<T>(prefix: &str, config_files: &[&str]) -> Result<T, ConfigError>
where
    T: DeserializeOwned + Default + Serialize,
{
    let mut builder = Config::builder()
        // Start with default values from the struct's Default impl
        .add_source(Config::try_from(&T::default())?);

    // Add optional TOML configuration files in order
    for file_path in config_files {
        builder = builder.add_source(File::with_name(file_path).required(false));
    }

    // Override with environment variables
    // Using `__` as separator for nested fields (Rust ecosystem convention)
    // e.g., APP__JWT__SECRET for nested jwt.secret field
    builder = builder.add_source(
        Environment::with_prefix(prefix)
            .prefix_separator("_")
            .separator("__"),
    );

    builder.build()?.try_deserialize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Default, PartialEq)]
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
        let settings: TestSettings = load_config("NONEXISTENT_PREFIX", &[]).unwrap();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 8080);
    }

    #[test]
    fn test_load_with_nonexistent_files() {
        // Should not fail when config files don't exist (required=false)
        let settings: TestSettings =
            load_config("NONEXISTENT_PREFIX", &["config/nonexistent"]).unwrap();
        assert_eq!(settings.host, "127.0.0.1");
    }
}
