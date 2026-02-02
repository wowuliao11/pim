use super::Settings;
use common::config::load_config;
use config::ConfigError;

/// Load settings from configuration file and environment variables
/// Priority (highest to lowest):
/// 1. Environment variables (prefix: APP_, using __ as nesting separator)
///    e.g., APP__APP__HOST, APP__DB__URL, APP__JWT__SECRET, APP__APP_ENV, APP__LOG_LEVEL
/// 2. config.toml (optional, for service-specific configuration)
/// 3. Default values in Settings struct
pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("APP", "config.toml")
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
