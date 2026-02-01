use super::load_settings;
use super::Settings;
use config;

/// Application configuration wrapper
/// Provides convenient methods for accessing configuration values
#[derive(Clone)]
pub struct AppConfig {
    pub settings: Settings,
}

impl AppConfig {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Get the bind address for the HTTP server
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.settings.app.host, self.settings.app.port)
    }

    /// Get the application name
    pub fn app_name(&self) -> &str {
        &self.settings.app.name
    }

    /// Get JWT secret
    pub fn jwt_secret(&self) -> &str {
        &self.settings.jwt.secret
    }

    /// Get JWT expiration in hours
    pub fn jwt_expiration_hours(&self) -> i64 {
        self.settings.jwt.expiration_hours
    }
}

/// Load application configuration from environment and config files
pub fn load_app_config() -> Result<AppConfig, config::ConfigError> {
    let settings = load_settings()?;
    Ok(AppConfig::new(settings))
}
