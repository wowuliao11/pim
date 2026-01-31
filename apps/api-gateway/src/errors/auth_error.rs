use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing or invalid authorization header")]
    MissingOrInvalidAuthorizationHeader,

    #[error("Invalid token")]
    InvalidToken,
}
