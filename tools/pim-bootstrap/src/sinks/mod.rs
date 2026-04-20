//! Output sinks — the second half of every Apply run.
//!
//! After ensure-ops resolve IDs and emit key material into the shared
//! [`PipelineContext`](crate::ops::PipelineContext), the sink layer persists
//! those values to the filesystem paths declared under `[outputs]` in
//! `bootstrap/*.toml`. Split into three units by sensitivity class
//! (ADR-0012 three-layer config, plan 001 §Phase D):
//!
//! - [`service_config`] — non-secret IDs into per-service `config.toml`.
//! - [`jwt_key`] — API JWT signing key (base64 JSON) into `zitadel-key.json`.
//! - [`env_file`] — symmetric PAT upserts into `.env.local`.
//!
//! All paths support the `stdout:<tag>` sentinel so prod runs can pipe
//! material into an external secret manager without ever touching disk.

use std::path::Path;

pub mod env_file;
pub mod jwt_key;
pub mod service_config;

/// Split a configured output path into either a filesystem target or a
/// `stdout:<tag>` sentinel. Sentinels are the escape hatch for prod runs
/// that must not persist secrets to disk (plan 001 §Phase D, ADR-0012).
pub enum SinkTarget<'a> {
    File(&'a Path),
    Stdout(&'a str),
}

impl<'a> SinkTarget<'a> {
    pub fn from_path(path: &'a Path) -> Self {
        if let Some(s) = path.to_str() {
            if let Some(tag) = s.strip_prefix("stdout:") {
                return SinkTarget::Stdout(tag);
            }
        }
        SinkTarget::File(path)
    }
}

/// Write `bytes` to `path` atomically: create a sibling temp file, fsync,
/// then `rename`. Also applies mode `0600` on Unix (best-effort).
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "output path has no file name"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));
    // Avoid collisions if a previous crash left a stale tmp behind.
    let mut counter: u32 = 0;
    while tmp.exists() {
        counter += 1;
        tmp = parent.join(format!(".{}.tmp.{}", file_name.to_string_lossy(), counter));
    }

    {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&tmp, perms)?;
    }

    std::fs::rename(&tmp, path)?;
    Ok(())
}
