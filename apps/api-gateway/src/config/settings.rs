use infra_config::CommonConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub app: AppSettings,
    pub db: DbSettings,
    pub zitadel: ZitadelSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
    pub metrics_port: u16,
    pub name: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            metrics_port: 60080,
            name: env!("CARGO_PKG_NAME").to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DbSettings {
    pub url: String,
}

impl Default for DbSettings {
    fn default() -> Self {
        Self {
            url: "postgres://localhost/pim".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZitadelSettings {
    /// Zitadel instance URL, e.g. "https://my-instance.zitadel.cloud"
    pub authority: String,
    /// API application client ID (for token introspection)
    pub client_id: String,
    /// API application client secret (for token introspection)
    pub client_secret: String,
}

impl Default for ZitadelSettings {
    fn default() -> Self {
        Self {
            authority: "https://localhost.zitadel.cloud".to_string(),
            client_id: "change-me".to_string(),
            client_secret: "change-me".to_string(),
        }
    }
}
