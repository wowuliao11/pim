use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing or invalid authorization header")]
    MissingOrInvalidAuthorizationHeader,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Token introspection failed")]
    IntrospectionFailed,
}
