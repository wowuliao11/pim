use std::fmt;

use config::ConfigError;
use infra_config::{load_config, CommonConfig};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub host: String,
    pub port: u16,
    pub metrics_port: u16,

    /// Zitadel instance URL, e.g. "https://my-instance.zitadel.cloud"
    pub zitadel_authority: String,
    /// Zitadel service account personal access token (PAT)
    pub zitadel_service_account_token: String,
}

// Manual Debug impl: mask service_account_token to prevent leaking credentials in logs
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("common", &self.common)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("metrics_port", &self.metrics_port)
            .field("zitadel_authority", &self.zitadel_authority)
            .field("zitadel_service_account_token", &"[REDACTED]")
            .finish()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            common: CommonConfig::default(),
            host: "127.0.0.1".to_string(),
            port: 50052,
            metrics_port: 60052,
            zitadel_authority: "https://localhost.zitadel.cloud".to_string(),
            zitadel_service_account_token: "change-me".to_string(),
        }
    }
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    load_config("USER_SERVICE", "config.toml")
}
