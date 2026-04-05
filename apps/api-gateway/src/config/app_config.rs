use super::load_settings;
use super::Settings;
use config;

/// Application configuration wrapper
#[derive(Clone)]
pub struct AppConfig {
    pub settings: Settings,
}

impl AppConfig {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.settings.app.host, self.settings.app.port)
    }

    pub fn app_name(&self) -> &str {
        &self.settings.app.name
    }

    pub fn zitadel_authority(&self) -> &str {
        &self.settings.zitadel.authority
    }

    pub fn zitadel_client_id(&self) -> &str {
        &self.settings.zitadel.client_id
    }

    pub fn zitadel_client_secret(&self) -> &str {
        &self.settings.zitadel.client_secret
    }
}

pub fn load_app_config() -> Result<AppConfig, config::ConfigError> {
    let settings = load_settings()?;
    Ok(AppConfig::new(settings))
}
