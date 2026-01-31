use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::AUTHORIZATION,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AuthError};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub exp: i64,    // Expiration time
    pub iat: i64,    // Issued at
    #[serde(default)]
    pub roles: Vec<String>, // User roles
}

/// Authenticated user data extracted from JWT
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub roles: Vec<String>,
}

/// JWT Authentication middleware
/// Validates JWT tokens and extracts user information
pub struct JwtAuth {
    secret: String,
}

impl JwtAuth {
    pub fn new(secret: impl Into<String>) -> Self {
        Self { secret: secret.into() }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = JwtAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtAuthMiddleware {
            service: Rc::new(service),
            secret: self.secret.clone(),
        }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
    secret: String,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let secret = self.secret.clone();

        Box::pin(async move {
            // Extract Authorization header
            let auth_header = req.headers().get(AUTHORIZATION).and_then(|v| v.to_str().ok());

            let token = match auth_header {
                Some(header) if header.starts_with("Bearer ") => &header[7..],
                _ => {
                    return Err(AppError::from(AuthError::MissingOrInvalidAuthorizationHeader).into());
                }
            };

            // Validate JWT token
            let token_data = decode::<Claims>(
                token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &Validation::default(),
            )
            .map_err(|_| AppError::from(AuthError::InvalidToken))?;

            // Store authenticated user in request extensions
            req.extensions_mut().insert(AuthenticatedUser {
                user_id: token_data.claims.sub.clone(),
                roles: token_data.claims.roles.clone(),
            });

            service.call(req).await
        })
    }
}

/// Helper function to generate JWT token
pub fn generate_token(
    user_id: &str,
    roles: Vec<String>,
    secret: &str,
    expiration_hours: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let now = chrono::Utc::now().timestamp();
    let exp = now + (expiration_hours * 3600);

    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        iat: now,
        roles,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}
