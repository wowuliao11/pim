//! JWT key sink — persists the base64 JSON blob Zitadel returned exactly
//! once from `POST /users/{id}/keys` into `outputs.jwt_key_path`.
//!
//! Invariants (plan 001 §Phase D, ADR-0017 §Service account):
//!
//! - The blob is base64-encoded JSON. We decode before writing so the
//!   on-disk file is a normal Zitadel machine-key JSON document that
//!   libraries like `zitadel-rs` can consume directly.
//! - The write is atomic (temp + rename) and mode `0600` on Unix.
//! - If the target file already exists we refuse to overwrite unless
//!   `rotate_keys` is true — the caller enforces this by only staging a
//!   new blob when a rotation happened, but we double-check here so a
//!   stale `ctx.jwt_key_blob` from a previous run cannot clobber a valid
//!   key.

use std::path::{Path, PathBuf};

use base64::Engine as _;

use super::{atomic_write, SinkTarget};

#[derive(Debug, thiserror::Error)]
pub enum JwtKeyError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("base64 decode of jwt key blob failed: {0}")]
    Decode(#[from] base64::DecodeError),
    #[error(
        "jwt key file {path} already exists; use --rotate-keys to overwrite \
         (ADR-0017 §Service account one-shot rule)"
    )]
    ExistsWithoutRotate { path: PathBuf },
}

#[derive(Debug)]
pub enum JwtKeyOutcome {
    Written(PathBuf),
    Stdout(String),
    Skipped,
}

/// Persist the JWT key blob from `PipelineContext.jwt_key_blob` to the
/// configured path. Returns `Skipped` when no blob was staged (e.g. a
/// steady-state Apply run where nothing rotated).
pub fn write(blob: Option<&str>, path: &Path, rotate_keys: bool) -> Result<JwtKeyOutcome, JwtKeyError> {
    let Some(blob) = blob else {
        return Ok(JwtKeyOutcome::Skipped);
    };

    let decoded = base64::engine::general_purpose::STANDARD.decode(blob.trim())?;

    match SinkTarget::from_path(path) {
        SinkTarget::Stdout(tag) => {
            println!("---BEGIN {}---", tag);
            std::io::Write::write_all(&mut std::io::stdout(), &decoded).map_err(|source| JwtKeyError::Io {
                path: PathBuf::from(format!("stdout:{}", tag)),
                source,
            })?;
            println!();
            println!("---END {}---", tag);
            Ok(JwtKeyOutcome::Stdout(tag.to_string()))
        }
        SinkTarget::File(fp) => {
            if fp.exists() && !rotate_keys {
                return Err(JwtKeyError::ExistsWithoutRotate { path: fp.to_path_buf() });
            }
            atomic_write(fp, &decoded).map_err(|source| JwtKeyError::Io {
                path: fp.to_path_buf(),
                source,
            })?;
            Ok(JwtKeyOutcome::Written(fp.to_path_buf()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn encode(raw: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
    }

    #[test]
    fn missing_blob_is_skip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("zitadel-key.json");
        let out = write(None, &path, false).unwrap();
        assert!(matches!(out, JwtKeyOutcome::Skipped));
        assert!(!path.exists());
    }

    #[test]
    fn new_file_writes_decoded_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("zitadel-key.json");
        let blob = encode(r#"{"type":"serviceaccount","keyId":"abc"}"#);
        let out = write(Some(&blob), &path, false).unwrap();
        assert!(matches!(out, JwtKeyOutcome::Written(_)));

        let got = std::fs::read_to_string(&path).unwrap();
        assert_eq!(got, r#"{"type":"serviceaccount","keyId":"abc"}"#);
    }

    #[test]
    fn existing_file_without_rotate_is_refused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("zitadel-key.json");
        std::fs::write(&path, "OLD").unwrap();

        let blob = encode(r#"{"keyId":"new"}"#);
        let err = write(Some(&blob), &path, false).unwrap_err();
        assert!(matches!(err, JwtKeyError::ExistsWithoutRotate { .. }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "OLD");
    }

    #[test]
    fn existing_file_with_rotate_is_overwritten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("zitadel-key.json");
        std::fs::write(&path, "OLD").unwrap();

        let blob = encode(r#"{"keyId":"new"}"#);
        write(Some(&blob), &path, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"keyId":"new"}"#,);
    }

    #[test]
    fn invalid_base64_surfaces_as_decode_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("zitadel-key.json");
        let err = write(Some("!!!not-base64!!!"), &path, false).unwrap_err();
        assert!(matches!(err, JwtKeyError::Decode(_)));
    }

    #[test]
    fn stdout_sentinel_does_not_touch_disk() {
        let dir = TempDir::new().unwrap();
        let ghost = dir.path().join("never-created.json");
        let _ = ghost;
        let blob = encode(r#"{"keyId":"x"}"#);
        let out = write(Some(&blob), Path::new("stdout:jwt-key"), false).unwrap();
        match out {
            JwtKeyOutcome::Stdout(tag) => assert_eq!(tag, "jwt-key"),
            _ => panic!("expected stdout outcome"),
        }
    }
}
