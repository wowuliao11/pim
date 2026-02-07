use config::ConfigError;
use infra_config::{load_config, CommonConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub host: String,
    pub port: u16,
    pub metrics_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            common: CommonConfig::default(),
            host: "127.0.0.1".to_string(),
            port: 50052,
            metrics_port: 60052,
        }
    }
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("USER_SERVICE", "config.toml")
}
