use infra_config::CommonConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub app: AppSettings,
    pub db: DbSettings,
    pub jwt: JwtSettings,
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
pub struct JwtSettings {
    pub secret: String,
    pub expiration_hours: i64,
}

impl Default for JwtSettings {
    fn default() -> Self {
        Self {
            secret: "your-secret-key-change-in-production".to_string(),
            expiration_hours: 24,
        }
    }
}
