use super::Settings;
use common::config::load_config;
use config::ConfigError;

/// Load settings from configuration files and environment variables
/// Priority (highest to lowest):
/// 1. Environment variables (prefix: APP_, using __ as nesting separator)
///    e.g., APP__APP__HOST, APP__DB__URL, APP__JWT__SECRET
/// 2. config/local.toml (optional, for local development)
/// 3. config/default.toml (optional, base configuration)
/// 4. Default values in Settings struct
pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("APP", &["config/default", "config/local"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_settings() {
        let settings = load_settings();
        assert!(settings.is_ok());
    }
}
