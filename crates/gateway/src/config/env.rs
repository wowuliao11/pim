use config::{Config, ConfigError, Environment, File};

use super::Settings;

/// Load settings from configuration files and environment variables
/// Priority (highest to lowest):
/// 1. Environment variables (prefix: APP_)
/// 2. config/local.toml (optional, for local development)
/// 3. config/default.toml (optional, base configuration)
/// 4. Default values in Settings struct
pub fn load_settings() -> Result<Settings, ConfigError> {
    // Try to load .env file (ignore if not found)
    let _ = dotenvy::from_filename(".env").ok();

    Config::builder()
        // Start with default values
        .add_source(config::Config::try_from(&Settings::default())?)
        // Load default config file (optional)
        .add_source(File::with_name("config/default").required(false))
        // Load local config file for development (optional)
        .add_source(File::with_name("config/local").required(false))
        // Override with environment variables (prefix: APP_)
        // e.g., APP_APP_HOST, APP_DB_URL, APP_JWT_SECRET
        .add_source(Environment::with_prefix("APP").prefix_separator("_").separator("_"))
        .build()?
        .try_deserialize()
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
