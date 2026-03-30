use thiserror::Error;

/// JWT operation errors
#[derive(Debug, Error)]
pub enum JwtError {
    #[error("failed to encode token: {0}")]
    Encode(#[source] jsonwebtoken::errors::Error),

    #[error("failed to decode token: {0}")]
    Decode(#[source] jsonwebtoken::errors::Error),
}
