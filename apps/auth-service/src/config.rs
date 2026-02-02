use common::config::{load_config, CommonConfig};
use config::ConfigError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub host: String,
    pub port: u16,
    pub metrics_port: u16,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            common: CommonConfig::default(),
            host: "127.0.0.1".to_string(),
            port: 50051,
            metrics_port: 60051,
            jwt_secret: "your-secret-key-change-in-production".to_string(),
            jwt_expiration_hours: 24,
        }
    }
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("AUTH_SERVICE", "config.toml")
}
