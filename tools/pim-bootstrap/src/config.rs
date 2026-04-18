//! Declarative configuration schema for `pim-bootstrap`.
//!
//! Two top-level shapes:
//!
//! - [`BootstrapConfig`] — `bootstrap/dev.toml`, `bootstrap/prod.example.toml`.
//!   Describes the tenant-scoped objects (project, API app, service account,
//!   roles) that must exist in Zitadel, plus the sinks where provisioned
//!   values are written back.
//! - [`SeedConfig`] — `bootstrap/seed.dev.toml`. Describes dev human users
//!   and their role assignments. Only consumed by `pim-bootstrap seed`.
//!
//! The split follows plan decision D10: secrets and non-secret config land
//! in different sinks (`OutputSinks`), and prod configs use `stdout:*`
//! sentinels so operators pipe to their own secret manager.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Target environment. Rejected by `seed` when set to `Prod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Dev,
    Prod,
}

/// How `pim-bootstrap` authenticates to the Zitadel Management API.
///
/// Dev defaults to a PAT read from an env var (`Pat`); prod flips to
/// `JwtProfile` backed by a service-account JSON key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminAuthMode {
    Pat,
    JwtProfile,
}

/// API app authentication method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppAuthMethod {
    JwtProfile,
    ClientSecret,
}

/// Zitadel tenant coordinates and admin credential source.
#[derive(Debug, Clone, Deserialize)]
pub struct ZitadelTarget {
    /// Base URL of the Zitadel instance (e.g. `http://pim.localhost:18080`).
    pub authority: String,

    /// Admin credential mode used by `pim-bootstrap` itself.
    pub admin_auth: AdminAuthMode,

    /// Env var that holds the PAT when `admin_auth = "pat"`.
    #[serde(default)]
    pub admin_pat_env_var: Option<String>,

    /// Path to the JWT profile JSON when `admin_auth = "jwt_profile"`.
    #[serde(default)]
    pub admin_key_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSpec {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiAppSpec {
    pub name: String,
    pub auth_method: AppAuthMethod,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceAccountSpec {
    pub username: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// A role declared on the project. Matches Zitadel's project-role shape.
#[derive(Debug, Clone, Deserialize)]
pub struct RoleSpec {
    pub key: String,
    pub display_name: String,
    #[serde(default)]
    pub group: Option<String>,
}

/// Where provisioned values are written back, split by sensitivity per D10.
///
/// - `service_configs`: non-secret values (project ID, app client ID) that
///   land in each service's `config.toml`. Real files are gitignored;
///   `.example.toml` siblings are committed.
/// - `jwt_key_path`: the API app's JWT signing key (private key material).
///   Written once, never overwritten without `--rotate-keys`.
/// - `env_file_path`: symmetric PAT strings, appended/upserted into a
///   single env file referenced by `compose.yml`'s `env_file:` directive.
///
/// For prod, any of these may be a `stdout:<tag>` sentinel so values are
/// emitted to stdout for piping into a secret manager.
#[derive(Debug, Clone, Deserialize)]
pub struct OutputSinks {
    pub service_configs: HashMap<String, PathBuf>,
    pub jwt_key_path: PathBuf,
    pub env_file_path: PathBuf,
}

/// Top-level bootstrap config.
#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapConfig {
    pub env: Environment,
    pub zitadel: ZitadelTarget,
    pub project: ProjectSpec,
    pub api_app: ApiAppSpec,
    pub service_account: ServiceAccountSpec,
    pub roles: Vec<RoleSpec>,
    pub outputs: OutputSinks,
}

/// Declarative seed config. Only consumed by `pim-bootstrap seed` and only
/// when `env = "dev"`.
#[derive(Debug, Clone, Deserialize)]
pub struct SeedConfig {
    pub env: Environment,
    #[serde(default)]
    pub users: Vec<HumanSpec>,
    #[serde(default)]
    pub role_assignments: Vec<RoleAssignmentSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HumanSpec {
    pub username: String,
    pub email: String,
    pub given_name: String,
    pub family_name: String,
    /// Disposable dev password. Documented in `seed.dev.toml` as disposable.
    pub initial_password: String,
    #[serde(default)]
    pub email_verified: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoleAssignmentSpec {
    pub user: String,
    pub roles: Vec<String>,
}

/// Errors surfaced while loading and validating config files.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("bootstrap config declares admin_auth = \"pat\" but no admin_pat_env_var is set")]
    PatEnvVarMissing,

    #[error("bootstrap config declares admin_auth = \"jwt_profile\" but no admin_key_file is set")]
    JwtProfileKeyMissing,
}

impl BootstrapConfig {
    /// Load and validate a bootstrap config from disk.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let parsed: BootstrapConfig = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        parsed.validate()?;
        Ok(parsed)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match self.zitadel.admin_auth {
            AdminAuthMode::Pat => {
                if self.zitadel.admin_pat_env_var.is_none() {
                    return Err(ConfigError::PatEnvVarMissing);
                }
            }
            AdminAuthMode::JwtProfile => {
                if self.zitadel.admin_key_file.is_none() {
                    return Err(ConfigError::JwtProfileKeyMissing);
                }
            }
        }
        Ok(())
    }
}

impl SeedConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_config_parses() {
        let toml_src = r#"
env = "dev"

[zitadel]
authority = "http://pim.localhost:18080"
admin_auth = "pat"
admin_pat_env_var = "ZITADEL_ADMIN_PAT"

[project]
name = "pim"

[api_app]
name = "api-gateway"
auth_method = "jwt_profile"

[service_account]
username = "user-service-sa"

[[roles]]
key = "admin"
display_name = "Administrator"

[[roles]]
key = "member"
display_name = "Member"

[outputs]
jwt_key_path = "zitadel-key.json"
env_file_path = ".env.local"

[outputs.service_configs]
api-gateway = "apps/api-gateway/config.toml"
user-service = "apps/user-service/config.toml"
"#;
        let cfg: BootstrapConfig = toml::from_str(toml_src).expect("parses");
        cfg.validate().expect("validates");
        assert_eq!(cfg.env, Environment::Dev);
        assert_eq!(cfg.roles.len(), 2);
        assert_eq!(cfg.outputs.service_configs.len(), 2);
    }

    #[test]
    fn pat_without_env_var_is_rejected() {
        let toml_src = r#"
env = "dev"

[zitadel]
authority = "http://pim.localhost:18080"
admin_auth = "pat"

[project]
name = "pim"

[api_app]
name = "api-gateway"
auth_method = "jwt_profile"

[service_account]
username = "user-service-sa"

[[roles]]
key = "admin"
display_name = "Administrator"

[outputs]
jwt_key_path = "k.json"
env_file_path = ".env.local"

[outputs.service_configs]
"#;
        let cfg: BootstrapConfig = toml::from_str(toml_src).expect("parses");
        assert!(matches!(cfg.validate(), Err(ConfigError::PatEnvVarMissing)));
    }

    #[test]
    fn seed_config_parses() {
        let toml_src = r#"
env = "dev"

[[users]]
username = "alice"
email = "alice@pim.dev"
given_name = "Alice"
family_name = "Tester"
initial_password = "Alice-Dev-Pass-1"
email_verified = true

[[role_assignments]]
user = "alice"
roles = ["admin"]
"#;
        let cfg: SeedConfig = toml::from_str(toml_src).expect("parses");
        assert_eq!(cfg.users.len(), 1);
        assert_eq!(cfg.role_assignments[0].roles, vec!["admin"]);
    }
}
