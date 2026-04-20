//! Per-service `config.toml` renderer.
//!
//! For each target under `[outputs.service_configs]`, we load the existing
//! file (or its `.example.toml` sibling) as a `toml::Value`, mutate the
//! zitadel-related keys in place, and atomically write the result back.
//! Unknown keys are preserved so service-specific settings (ports, DB
//! URLs, ...) survive repeated bootstrap runs.
//!
//! Field mapping is service-specific and documented in plan 001 §Phase D:
//!
//! - `api-gateway`: sets `[zitadel].authority`, `[zitadel].key_file`,
//!   `[zitadel].project_id`, `[zitadel].api_app_id`.
//! - `user-service`: sets top-level `zitadel_authority`,
//!   `zitadel_project_id`, `zitadel_sa_user_id`. The PAT
//!   (`zitadel_service_account_token`) is intentionally NOT written here —
//!   it is sourced out-of-band from the env file.
//!
//! Any other service name is treated as generic: `[zitadel]` table with
//! `authority`, `project_id`, `api_app_id`, `sa_user_id`. This keeps the
//! sink forward-compatible with new services without another code change.

use std::path::{Path, PathBuf};

use toml::Value;

use super::{atomic_write, SinkTarget};

/// Non-secret identifiers produced by the ensure-op pipeline that are safe
/// to persist into per-service `config.toml` files.
#[derive(Debug, Clone)]
pub struct ServiceConfigInputs<'a> {
    pub authority: &'a str,
    pub project_id: &'a str,
    pub api_app_id: &'a str,
    pub sa_user_id: &'a str,
    /// Filesystem path to the JWT key file, as written by `jwt_key::write`.
    /// Stored verbatim under `[zitadel].key_file` for api-gateway.
    pub jwt_key_path: &'a Path,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceConfigError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("toml parse error on {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("toml serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Render every target under `[outputs.service_configs]`. Returns the list
/// of targets touched (file paths or stdout tags) for reporting.
pub fn render_all(
    service_configs: &std::collections::HashMap<String, PathBuf>,
    inputs: &ServiceConfigInputs<'_>,
) -> Result<Vec<String>, ServiceConfigError> {
    let mut touched = Vec::with_capacity(service_configs.len());
    let mut names: Vec<&String> = service_configs.keys().collect();
    names.sort();
    for name in names {
        let path = &service_configs[name];
        let label = render_one(name, path, inputs)?;
        touched.push(label);
    }
    Ok(touched)
}

fn render_one(service_name: &str, path: &Path, inputs: &ServiceConfigInputs<'_>) -> Result<String, ServiceConfigError> {
    let target = SinkTarget::from_path(path);
    let mut root = load_or_seed(service_name, path)?;

    apply_fields(service_name, &mut root, inputs);

    let rendered = toml::to_string_pretty(&root)?;

    match target {
        SinkTarget::Stdout(tag) => {
            println!("---BEGIN {} ({})---", tag, service_name);
            print!("{}", rendered);
            println!("---END {} ({})---", tag, service_name);
            Ok(format!("stdout:{}", tag))
        }
        SinkTarget::File(fp) => {
            atomic_write(fp, rendered.as_bytes()).map_err(|source| ServiceConfigError::Io {
                path: fp.to_path_buf(),
                source,
            })?;
            Ok(fp.display().to_string())
        }
    }
}

fn load_or_seed(service_name: &str, path: &Path) -> Result<Value, ServiceConfigError> {
    if path.exists() {
        let raw = std::fs::read_to_string(path).map_err(|source| ServiceConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        return toml::from_str(&raw).map_err(|source| ServiceConfigError::Parse {
            path: path.to_path_buf(),
            source,
        });
    }

    let example = example_sibling(path);
    if example.exists() {
        let raw = std::fs::read_to_string(&example).map_err(|source| ServiceConfigError::Io {
            path: example.clone(),
            source,
        })?;
        return toml::from_str(&raw).map_err(|source| ServiceConfigError::Parse { path: example, source });
    }

    let _ = service_name;
    Ok(Value::Table(toml::map::Map::new()))
}

fn example_sibling(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("config.toml");
    let example_name = match file.strip_suffix(".toml") {
        Some(stem) => format!("{}.example.toml", stem),
        None => format!("{}.example", file),
    };
    parent.join(example_name)
}

fn apply_fields(service_name: &str, root: &mut Value, inputs: &ServiceConfigInputs<'_>) {
    match service_name {
        "api-gateway" => {
            let zitadel = ensure_table(root, "zitadel");
            set_str(zitadel, "authority", inputs.authority);
            set_str(zitadel, "key_file", &inputs.jwt_key_path.display().to_string());
            set_str(zitadel, "project_id", inputs.project_id);
            set_str(zitadel, "api_app_id", inputs.api_app_id);
        }
        "user-service" => {
            let t = ensure_top_table(root);
            set_str(t, "zitadel_authority", inputs.authority);
            set_str(t, "zitadel_project_id", inputs.project_id);
            set_str(t, "zitadel_sa_user_id", inputs.sa_user_id);
        }
        _ => {
            let zitadel = ensure_table(root, "zitadel");
            set_str(zitadel, "authority", inputs.authority);
            set_str(zitadel, "project_id", inputs.project_id);
            set_str(zitadel, "api_app_id", inputs.api_app_id);
            set_str(zitadel, "sa_user_id", inputs.sa_user_id);
        }
    }
}

fn ensure_top_table(root: &mut Value) -> &mut toml::map::Map<String, Value> {
    if !root.is_table() {
        *root = Value::Table(toml::map::Map::new());
    }
    root.as_table_mut().expect("just ensured table")
}

fn ensure_table<'a>(root: &'a mut Value, key: &str) -> &'a mut toml::map::Map<String, Value> {
    let top = ensure_top_table(root);
    let entry = top
        .entry(key.to_string())
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if !entry.is_table() {
        *entry = Value::Table(toml::map::Map::new());
    }
    entry.as_table_mut().expect("just ensured table")
}

fn set_str(table: &mut toml::map::Map<String, Value>, key: &str, value: &str) {
    table.insert(key.to_string(), Value::String(value.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn inputs<'a>(jwt_key_path: &'a Path) -> ServiceConfigInputs<'a> {
        ServiceConfigInputs {
            authority: "http://pim.localhost:18080",
            project_id: "proj-123",
            api_app_id: "app-456",
            sa_user_id: "sa-789",
            jwt_key_path,
        }
    }

    #[test]
    fn api_gateway_preserves_existing_fields() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("config.toml");
        std::fs::write(
            &target,
            r#"app_env = "dev"
log_level = "info"

[app]
host = "127.0.0.1"
port = 8080

[zitadel]
authority = "OLD"
key_file = "old.json"
"#,
        )
        .unwrap();

        let key_path = dir.path().join("zitadel-key.json");
        let mut svc = HashMap::new();
        svc.insert("api-gateway".to_string(), target.clone());

        render_all(&svc, &inputs(&key_path)).unwrap();

        let out = std::fs::read_to_string(&target).unwrap();
        assert!(out.contains("app_env = \"dev\""), "preserved top-level");
        assert!(out.contains("host = \"127.0.0.1\""), "preserved [app]");
        assert!(out.contains("authority = \"http://pim.localhost:18080\""));
        assert!(out.contains("project_id = \"proj-123\""));
        assert!(out.contains("api_app_id = \"app-456\""));
        assert!(
            out.contains(&format!("key_file = \"{}\"", key_path.display())),
            "key_file overwritten",
        );
    }

    #[test]
    fn user_service_writes_top_level_fields_and_skips_token() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("user.toml");
        std::fs::write(
            &target,
            r#"app_env = "dev"
host = "127.0.0.1"
port = 50051
zitadel_service_account_token = "DO-NOT-TOUCH"
"#,
        )
        .unwrap();

        let key_path = dir.path().join("zitadel-key.json");
        let mut svc = HashMap::new();
        svc.insert("user-service".to_string(), target.clone());
        render_all(&svc, &inputs(&key_path)).unwrap();

        let out = std::fs::read_to_string(&target).unwrap();
        assert!(out.contains("zitadel_authority = \"http://pim.localhost:18080\""));
        assert!(out.contains("zitadel_project_id = \"proj-123\""));
        assert!(out.contains("zitadel_sa_user_id = \"sa-789\""));
        assert!(
            out.contains("zitadel_service_account_token = \"DO-NOT-TOUCH\""),
            "token field must be preserved, not overwritten",
        );
    }

    #[test]
    fn missing_file_seeds_from_example_sibling() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("config.toml");
        let example = dir.path().join("config.example.toml");
        std::fs::write(
            &example,
            r#"app_env = "dev"

[zitadel]
authority = "https://my-instance.zitadel.cloud"
key_file = "zitadel-key.json"
"#,
        )
        .unwrap();

        let key_path = dir.path().join("zitadel-key.json");
        let mut svc = HashMap::new();
        svc.insert("api-gateway".to_string(), target.clone());
        render_all(&svc, &inputs(&key_path)).unwrap();

        assert!(target.exists());
        let out = std::fs::read_to_string(&target).unwrap();
        assert!(out.contains("authority = \"http://pim.localhost:18080\""));
        assert!(out.contains("app_env = \"dev\""), "example app_env carried over");
    }

    #[test]
    fn stdout_sentinel_does_not_touch_disk() {
        let dir = TempDir::new().unwrap();
        let key_path = dir.path().join("zitadel-key.json");
        let mut svc = HashMap::new();
        svc.insert("api-gateway".to_string(), PathBuf::from("stdout:api-gateway-config"));
        let touched = render_all(&svc, &inputs(&key_path)).unwrap();
        assert_eq!(touched, vec!["stdout:api-gateway-config".to_string()]);
    }

    #[test]
    fn generic_service_falls_back_to_zitadel_table() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("config.toml");
        let key_path = dir.path().join("zitadel-key.json");
        let mut svc = HashMap::new();
        svc.insert("future-svc".to_string(), target.clone());
        render_all(&svc, &inputs(&key_path)).unwrap();

        let out = std::fs::read_to_string(&target).unwrap();
        assert!(out.contains("[zitadel]"));
        assert!(out.contains("sa_user_id = \"sa-789\""));
    }
}
