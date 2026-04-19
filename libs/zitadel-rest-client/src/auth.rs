use std::path::Path;

use crate::error::ZitadelError;

/// Admin credential material used by `ZitadelClient` when issuing Management API calls.
///
/// Per ADR-0016 §4 there are exactly two variants. Dev exclusively exercises
/// `Pat`; `JwtProfile` is kept compiling so prod rollout is a configuration
/// change, not a code change.
#[derive(Clone)]
pub enum AdminCredential {
    Pat(String),
    JwtProfile { key_json: Vec<u8>, audience: String },
}

impl AdminCredential {
    /// Read a PAT from the given environment variable.
    ///
    /// Returns `ZitadelError::Unauthenticated` with a diagnostic message when
    /// the variable is missing or empty, because an empty PAT is never a
    /// transport problem.
    pub fn from_env_pat(var_name: &str) -> Result<Self, ZitadelError> {
        let value = std::env::var(var_name)
            .map_err(|_| ZitadelError::Unauthenticated(format!("environment variable `{var_name}` is not set")))?;
        if value.trim().is_empty() {
            return Err(ZitadelError::Unauthenticated(format!(
                "environment variable `{var_name}` is empty"
            )));
        }
        Ok(Self::Pat(value))
    }

    /// Load a JWT Profile key JSON from disk. Audience is typically the
    /// Zitadel issuer URL.
    pub fn from_jwt_key_path(path: &Path, audience: impl Into<String>) -> Result<Self, ZitadelError> {
        let bytes = std::fs::read(path)
            .map_err(|e| ZitadelError::Unauthenticated(format!("failed to read JWT key `{}`: {e}", path.display())))?;
        Ok(Self::JwtProfile {
            key_json: bytes,
            audience: audience.into(),
        })
    }
}

impl std::fmt::Debug for AdminCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pat(_) => f.write_str("AdminCredential::Pat(<redacted>)"),
            Self::JwtProfile { audience, .. } => f
                .debug_struct("AdminCredential::JwtProfile")
                .field("audience", audience)
                .field("key_json", &"<redacted>")
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_pat_reads_value() {
        let var = "ZITADEL_TEST_PAT_ENV_abc123";
        std::env::set_var(var, "tok-xyz");
        let c = AdminCredential::from_env_pat(var).unwrap();
        match c {
            AdminCredential::Pat(v) => assert_eq!(v, "tok-xyz"),
            _ => panic!("expected Pat"),
        }
        std::env::remove_var(var);
    }

    #[test]
    fn from_env_pat_rejects_missing() {
        let err = AdminCredential::from_env_pat("ZITADEL_TEST_PAT_ENV_missing_xxx").unwrap_err();
        assert!(matches!(err, ZitadelError::Unauthenticated(_)));
    }

    #[test]
    fn from_env_pat_rejects_empty() {
        let var = "ZITADEL_TEST_PAT_ENV_empty_xxx";
        std::env::set_var(var, "   ");
        let err = AdminCredential::from_env_pat(var).unwrap_err();
        std::env::remove_var(var);
        assert!(matches!(err, ZitadelError::Unauthenticated(_)));
    }

    #[test]
    fn debug_redacts_pat() {
        let c = AdminCredential::Pat("super-secret".into());
        let s = format!("{c:?}");
        assert!(!s.contains("super-secret"), "PAT leaked in Debug: {s}");
    }
}
