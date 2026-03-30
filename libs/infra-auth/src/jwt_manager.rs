use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

use crate::claims::Claims;
use crate::error::JwtError;

/// Manages JWT token generation and validation
///
/// Holds the signing secret and expiration policy.
/// Intended to be constructed once at service startup and shared (e.g. via `Arc` or `web::Data`).
pub struct JwtManager {
    secret: String,
    expiration_hours: i64,
}

impl JwtManager {
    /// Create a new manager with the given secret and token lifetime.
    pub fn new(secret: impl Into<String>, expiration_hours: i64) -> Self {
        Self {
            secret: secret.into(),
            expiration_hours,
        }
    }

    /// Token lifetime in hours (exposed for response metadata).
    pub fn expiration_hours(&self) -> i64 {
        self.expiration_hours
    }

    /// Generate a signed JWT for the given user and roles.
    pub fn generate_token(&self, user_id: &str, roles: Vec<String>) -> Result<String, JwtError> {
        let now = chrono::Utc::now().timestamp();
        let exp = now + (self.expiration_hours * 3600);

        let claims = Claims {
            sub: user_id.to_string(),
            exp,
            iat: now,
            roles,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(JwtError::Encode)
    }

    /// Validate a token and return the decoded claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(JwtError::Decode)?;

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> JwtManager {
        JwtManager::new("test-secret-key-that-is-long-enough", 24)
    }

    #[test]
    fn roundtrip_token() {
        let mgr = manager();
        let token = mgr
            .generate_token("user-123", vec!["admin".to_string()])
            .expect("generate");
        let claims = mgr.validate_token(&token).expect("validate");
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.roles, vec!["admin"]);
    }

    #[test]
    fn invalid_token_fails() {
        let mgr = manager();
        let result = mgr.validate_token("not-a-real-token");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_secret_fails() {
        let mgr = manager();
        let token = mgr.generate_token("user-1", vec![]).expect("generate");

        let other = JwtManager::new("different-secret-key-also-long", 24);
        assert!(other.validate_token(&token).is_err());
    }
}
