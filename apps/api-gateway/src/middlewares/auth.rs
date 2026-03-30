use std::{
    future::{ready, Ready},
    rc::Rc,
    sync::Arc,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::AUTHORIZATION,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use infra_auth::JwtManager;

use crate::errors::{AppError, AuthError};

/// Authenticated user data extracted from JWT
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub roles: Vec<String>,
}

/// JWT Authentication middleware
/// Validates JWT tokens and extracts user information
pub struct JwtAuth {
    jwt_manager: Arc<JwtManager>,
}

impl JwtAuth {
    pub fn new(jwt_manager: Arc<JwtManager>) -> Self {
        Self { jwt_manager }
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
            jwt_manager: self.jwt_manager.clone(),
        }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
    jwt_manager: Arc<JwtManager>,
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
        let jwt_manager = self.jwt_manager.clone();

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
            let claims = jwt_manager
                .validate_token(token)
                .map_err(|_| AppError::from(AuthError::InvalidToken))?;

            // Store authenticated user in request extensions
            req.extensions_mut().insert(AuthenticatedUser {
                user_id: claims.sub,
                roles: claims.roles,
            });

            service.call(req).await
        })
    }
}
