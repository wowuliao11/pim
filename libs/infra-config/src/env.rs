use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Application environment configuration
///
/// Determines runtime behavior, logging format, and default log levels.
/// Load from APP_ENV environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppEnv {
    #[default]
    Development,
    Staging,
    Production,
}

// Custom deserializer to accept both full names and abbreviations
// e.g., "development" or "dev", "production" or "prod", "staging" or "stage"
impl<'de> Deserialize<'de> for AppEnv {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<AppEnv>().map_err(serde::de::Error::custom)
    }
}

impl AppEnv {
    /// Load from APP_ENV environment variable
    ///
    /// Falls back to Development if not set or invalid.
    pub fn from_env() -> Self {
        std::env::var("APP_ENV")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::Development)
    }

    /// Check if running in production
    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    /// Check if running in development
    pub fn is_development(&self) -> bool {
        matches!(self, Self::Development)
    }

    /// Check if running in staging
    pub fn is_staging(&self) -> bool {
        matches!(self, Self::Staging)
    }
}

impl FromStr for AppEnv {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(format!("Invalid environment: {s}")),
        }
    }
}

impl fmt::Display for AppEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Staging => write!(f, "staging"),
            Self::Production => write!(f, "production"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!("development".parse::<AppEnv>().unwrap(), AppEnv::Development);
        assert_eq!("dev".parse::<AppEnv>().unwrap(), AppEnv::Development);
        assert_eq!("staging".parse::<AppEnv>().unwrap(), AppEnv::Staging);
        assert_eq!("stage".parse::<AppEnv>().unwrap(), AppEnv::Staging);
        assert_eq!("production".parse::<AppEnv>().unwrap(), AppEnv::Production);
        assert_eq!("prod".parse::<AppEnv>().unwrap(), AppEnv::Production);
        assert!("invalid".parse::<AppEnv>().is_err());
        assert!("whatever".parse::<AppEnv>().is_err());
    }

    #[test]
    fn test_default() {
        assert_eq!(AppEnv::default(), AppEnv::Development);
    }

    #[test]
    fn test_is_methods() {
        assert!(AppEnv::Development.is_development());
        assert!(!AppEnv::Development.is_production());
        assert!(!AppEnv::Development.is_staging());

        assert!(AppEnv::Production.is_production());
        assert!(!AppEnv::Production.is_development());
    }

    #[test]
    fn test_display() {
        assert_eq!(AppEnv::Development.to_string(), "development");
        assert_eq!(AppEnv::Staging.to_string(), "staging");
        assert_eq!(AppEnv::Production.to_string(), "production");
    }
}
