use thiserror::Error;

/// Error taxonomy for Zitadel Management API calls.
///
/// Status → variant mapping (ADR-0016 §5):
/// 400 → `BadRequest`, 401 → `Unauthenticated`, 403 → `PermissionDenied`,
/// 404 → `NotFound`, 409 → `AlreadyExists`, 412 → `FailedPrecondition`,
/// 429 → `ResourceExhausted`, 5xx → `Internal`, others → `Unknown`.
///
/// Transport failures (DNS, TLS, connect) map to `Transport`. Deserialization
/// failures are distinguished from API errors to help the caller route bugs
/// (bad struct) vs environmental issues.
#[derive(Debug, Error)]
pub enum ZitadelError {
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    #[error("bad request (400): {0}")]
    BadRequest(String),

    #[error("unauthenticated (401): {0}")]
    Unauthenticated(String),

    #[error("permission denied (403): {0}")]
    PermissionDenied(String),

    #[error("not found (404): {0}")]
    NotFound(String),

    #[error("already exists (409): {0}")]
    AlreadyExists(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("failed precondition (412): {0}")]
    FailedPrecondition(String),

    #[error("resource exhausted (429): {0}")]
    ResourceExhausted(String),

    #[error("internal server error ({status}): {body}")]
    Internal { status: u16, body: String },

    #[error("unknown status {status}: {body}")]
    Unknown { status: u16, body: String },

    #[error("response deserialization failed: {0}")]
    Deserialize(String),
}

impl ZitadelError {
    /// Map a non-2xx HTTP status + body into the appropriate variant.
    ///
    /// Centralized so every endpoint method gets the same semantics; callers
    /// must NOT demote 409 to a warning here — that decision lives at the
    /// ensure-op layer (ADR-0017).
    pub fn from_status(status: u16, body: String) -> Self {
        match status {
            400 => Self::BadRequest(body),
            401 => Self::Unauthenticated(body),
            403 => Self::PermissionDenied(body),
            404 => Self::NotFound(body),
            409 => Self::AlreadyExists(body),
            412 => Self::FailedPrecondition(body),
            429 => Self::ResourceExhausted(body),
            500..=599 => Self::Internal { status, body },
            _ => Self::Unknown { status, body },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_400_to_bad_request() {
        let e = ZitadelError::from_status(400, "bad".into());
        assert!(matches!(e, ZitadelError::BadRequest(ref s) if s == "bad"));
    }

    #[test]
    fn maps_401_to_unauthenticated() {
        let e = ZitadelError::from_status(401, "no".into());
        assert!(matches!(e, ZitadelError::Unauthenticated(_)));
    }

    #[test]
    fn maps_403_to_permission_denied() {
        assert!(matches!(
            ZitadelError::from_status(403, "x".into()),
            ZitadelError::PermissionDenied(_)
        ));
    }

    #[test]
    fn maps_404_to_not_found() {
        assert!(matches!(
            ZitadelError::from_status(404, "x".into()),
            ZitadelError::NotFound(_)
        ));
    }

    #[test]
    fn maps_409_to_already_exists() {
        assert!(matches!(
            ZitadelError::from_status(409, "x".into()),
            ZitadelError::AlreadyExists(_)
        ));
    }

    #[test]
    fn maps_412_to_failed_precondition() {
        assert!(matches!(
            ZitadelError::from_status(412, "x".into()),
            ZitadelError::FailedPrecondition(_)
        ));
    }

    #[test]
    fn maps_429_to_resource_exhausted() {
        assert!(matches!(
            ZitadelError::from_status(429, "x".into()),
            ZitadelError::ResourceExhausted(_)
        ));
    }

    #[test]
    fn maps_500_range_to_internal() {
        for s in [500u16, 502, 503, 504, 599] {
            let e = ZitadelError::from_status(s, "x".into());
            assert!(matches!(e, ZitadelError::Internal { status, .. } if status == s));
        }
    }

    #[test]
    fn maps_other_to_unknown() {
        let e = ZitadelError::from_status(418, "teapot".into());
        assert!(matches!(e, ZitadelError::Unknown { status: 418, .. }));
    }
}
