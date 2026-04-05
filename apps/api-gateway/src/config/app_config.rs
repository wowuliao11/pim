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

    pub fn user_service_url(&self) -> &str {
        &self.settings.app.user_service_url
    }

    pub fn zitadel_authority(&self) -> &str {
        &self.settings.zitadel.authority
    }

    pub fn zitadel_key_file(&self) -> &str {
        &self.settings.zitadel.key_file
    }
}

pub fn load_app_config() -> Result<AppConfig, config::ConfigError> {
    let settings = load_settings()?;
    Ok(AppConfig::new(settings))
}
