use common::config::load_config;
use config::ConfigError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub metrics_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 50052,
            metrics_port: 60052,
        }
    }
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("USER_SERVICE", &["config/user-service"])
}
