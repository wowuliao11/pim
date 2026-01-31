use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 50051,
            jwt_secret: "your-secret-key-change-in-production".to_string(),
            jwt_expiration_hours: 24,
        }
    }
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    // Try to load .env file
    let _ = dotenvy::from_filename(".env").ok();

    Config::builder()
        .add_source(config::Config::try_from(&Settings::default())?)
        .add_source(File::with_name("config/auth-service").required(false))
        .add_source(
            Environment::with_prefix("AUTH_SERVICE")
                .prefix_separator("_")
                .separator("_"),
        )
        .build()?
        .try_deserialize()
}
