use std::fmt;

use infra_config::CommonConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Default)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub app: AppSettings,
    pub zitadel: ZitadelSettings,
}

// Manual Debug impl to avoid leaking secrets
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("common", &self.common)
            .field("app", &self.app)
            .field("zitadel", &self.zitadel)
            .finish()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
    pub metrics_port: u16,
    pub name: String,
    /// gRPC endpoint of the user-service, e.g. "http://127.0.0.1:50051"
    #[serde(default = "default_user_service_url")]
    pub user_service_url: String,
}

fn default_user_service_url() -> String {
    "http://127.0.0.1:50051".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            metrics_port: 60080,
            name: env!("CARGO_PKG_NAME").to_string(),
            user_service_url: default_user_service_url(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ZitadelSettings {
    /// Zitadel instance URL, e.g. "https://my-instance.zitadel.cloud"
    pub authority: String,
    /// Path to the Zitadel API application JSON key file (downloaded from Zitadel console)
    /// The file contains: type, keyId, key (RSA private key), appId, clientId
    pub key_file: String,
}

// Manual Debug impl for ZitadelSettings
impl fmt::Debug for ZitadelSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZitadelSettings")
            .field("authority", &self.authority)
            .field("key_file", &self.key_file)
            .finish()
    }
}

impl Default for ZitadelSettings {
    fn default() -> Self {
        Self {
            authority: "https://localhost.zitadel.cloud".to_string(),
            key_file: "zitadel-key.json".to_string(),
        }
    }
}
