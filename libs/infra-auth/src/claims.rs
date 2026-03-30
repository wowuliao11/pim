use serde::{Deserialize, Serialize};

/// JWT claims shared across all services
///
/// This is the single source of truth for the JWT token payload.
/// Both the HTTP gateway (for token validation in middleware) and
/// the auth gRPC service (for token generation) use this struct.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Expiration time (UTC timestamp)
    pub exp: i64,
    /// Issued at (UTC timestamp)
    pub iat: i64,
    /// User roles
    #[serde(default)]
    pub roles: Vec<String>,
}
