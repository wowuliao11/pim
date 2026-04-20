//! `.env.local` upsert sink.
//!
//! Line-oriented parser that preserves comments, blank lines, and any
//! keys we don't own. For each `(key, value)` pair we either replace the
//! existing `KEY=...` line in place or append to the end. Writes atomic
//! (temp + rename) and mode `0600` on Unix. If the file does not yet
//! exist we create it.
//!
//! Drift behavior (plan 001 §Phase D): if a key already exists with a
//! different value and `--sync` is not set, we *warn* via `tracing`
//! instead of overwriting. The caller passes `sync=true` to force.

use std::path::{Path, PathBuf};

use tracing::warn;

use super::{atomic_write, SinkTarget};

#[derive(Debug, thiserror::Error)]
pub enum EnvFileError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub enum EnvFileOutcome {
    Written { path: PathBuf, changed: usize },
    Stdout(String),
    NoEntries,
}

/// Upsert `entries` into the file at `path`. Returns the number of lines
/// actually changed (useful for reporting; zero means "no-op").
pub fn upsert(entries: &[(&str, &str)], path: &Path, sync: bool) -> Result<EnvFileOutcome, EnvFileError> {
    if entries.is_empty() {
        return Ok(EnvFileOutcome::NoEntries);
    }

    match SinkTarget::from_path(path) {
        SinkTarget::Stdout(tag) => {
            println!("---BEGIN {}---", tag);
            for (k, v) in entries {
                println!("{}={}", k, v);
            }
            println!("---END {}---", tag);
            Ok(EnvFileOutcome::Stdout(tag.to_string()))
        }
        SinkTarget::File(fp) => {
            let existing = if fp.exists() {
                std::fs::read_to_string(fp).map_err(|source| EnvFileError::Io {
                    path: fp.to_path_buf(),
                    source,
                })?
            } else {
                String::new()
            };
            let (rendered, changed) = apply(&existing, entries, sync, fp);
            atomic_write(fp, rendered.as_bytes()).map_err(|source| EnvFileError::Io {
                path: fp.to_path_buf(),
                source,
            })?;
            Ok(EnvFileOutcome::Written {
                path: fp.to_path_buf(),
                changed,
            })
        }
    }
}

fn apply(existing: &str, entries: &[(&str, &str)], sync: bool, fp: &Path) -> (String, usize) {
    let mut lines: Vec<String> = existing.lines().map(|s| s.to_string()).collect();
    let mut changed = 0usize;

    for (key, value) in entries {
        let needle = format!("{}=", key);
        let mut found = false;
        for line in lines.iter_mut() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with(&needle) {
                let current = trimmed[needle.len()..].to_string();
                if current == *value {
                    found = true;
                    break;
                }
                if sync {
                    *line = format!("{}={}", key, value);
                    changed += 1;
                } else {
                    warn!(
                        path = %fp.display(),
                        key,
                        "env file already has {}={} (existing differs); skipping (use --sync to overwrite)",
                        key,
                        current,
                    );
                }
                found = true;
                break;
            }
        }
        if !found {
            lines.push(format!("{}={}", key, value));
            changed += 1;
        }
    }

    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    (out, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn appends_to_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.local");
        let out = upsert(&[("FOO", "1"), ("BAR", "two")], &path, false).unwrap();
        match out {
            EnvFileOutcome::Written { changed, .. } => assert_eq!(changed, 2),
            _ => panic!("expected Written"),
        }
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("FOO=1"));
        assert!(body.contains("BAR=two"));
    }

    #[test]
    fn preserves_unrelated_lines_and_comments() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.local");
        std::fs::write(&path, "# comment\nKEEP=ok\nOTHER=value\n").unwrap();

        upsert(&[("NEW", "v")], &path, false).unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("# comment"));
        assert!(body.contains("KEEP=ok"));
        assert!(body.contains("OTHER=value"));
        assert!(body.contains("NEW=v"));
    }

    #[test]
    fn matching_value_is_noop() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.local");
        std::fs::write(&path, "FOO=1\n").unwrap();

        let out = upsert(&[("FOO", "1")], &path, false).unwrap();
        match out {
            EnvFileOutcome::Written { changed, .. } => assert_eq!(changed, 0),
            _ => panic!("expected Written"),
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "FOO=1\n");
    }

    #[test]
    fn drift_without_sync_does_not_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.local");
        std::fs::write(&path, "FOO=old\n").unwrap();

        let out = upsert(&[("FOO", "new")], &path, false).unwrap();
        match out {
            EnvFileOutcome::Written { changed, .. } => assert_eq!(changed, 0),
            _ => panic!("expected Written"),
        }
        assert!(std::fs::read_to_string(&path).unwrap().contains("FOO=old"));
    }

    #[test]
    fn drift_with_sync_overwrites_in_place() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.local");
        std::fs::write(&path, "BEFORE=y\nFOO=old\nAFTER=z\n").unwrap();

        let out = upsert(&[("FOO", "new")], &path, true).unwrap();
        match out {
            EnvFileOutcome::Written { changed, .. } => assert_eq!(changed, 1),
            _ => panic!("expected Written"),
        }
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("BEFORE=y"));
        assert!(body.contains("FOO=new"));
        assert!(!body.contains("FOO=old"));
        assert!(body.contains("AFTER=z"));
    }

    #[test]
    fn empty_entries_is_no_op() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env.local");
        let out = upsert(&[], &path, false).unwrap();
        assert!(matches!(out, EnvFileOutcome::NoEntries));
        assert!(!path.exists());
    }

    #[test]
    fn stdout_sentinel_skips_disk() {
        let out = upsert(&[("X", "y")], Path::new("stdout:env"), false).unwrap();
        match out {
            EnvFileOutcome::Stdout(tag) => assert_eq!(tag, "env"),
            _ => panic!("expected Stdout"),
        }
    }
}
