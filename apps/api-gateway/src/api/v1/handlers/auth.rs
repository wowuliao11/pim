use actix_web::{web, HttpResponse};

use anyhow::Context;

use crate::api::v1::dto::{ApiResponse, LoginRequest, LoginResponse, RegisterRequest, RegisterResponse};
use crate::config::AppConfig;
use crate::errors::{AppError, ValidationError};
use crate::middlewares::auth::generate_token;

/// POST /api/v1/auth/login
/// Authenticate user and return JWT token
pub async fn login(body: web::Json<LoginRequest>, config: web::Data<AppConfig>) -> Result<HttpResponse, AppError> {
    // TODO: Validate credentials against database
    // This is a placeholder implementation

    if body.email.is_empty() || body.password.is_empty() {
        return Err(ValidationError::MissingCredentials.into());
    }

    // Generate JWT token
    let token = generate_token(
        &body.email, // In real app, use user ID from database
        vec!["user".to_string()],
        config.jwt_secret(),
        config.jwt_expiration_hours(),
    )
    .context("Failed to generate token")?;

    let response = LoginResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: config.jwt_expiration_hours() * 3600,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::new(response)))
}

/// POST /api/v1/auth/register
/// Register a new user
pub async fn register(body: web::Json<RegisterRequest>) -> Result<HttpResponse, AppError> {
    // TODO: Implement actual user registration with database
    // This is a placeholder implementation

    if body.email.is_empty() || body.password.is_empty() || body.name.is_empty() {
        return Err(ValidationError::MissingRegistrationFields.into());
    }

    // Validate email format
    if !body.email.contains('@') {
        return Err(ValidationError::InvalidEmail.into());
    }

    // Validate password strength
    if body.password.len() < 8 {
        return Err(ValidationError::WeakPassword { min_len: 8 }.into());
    }

    let response = RegisterResponse {
        id: uuid::Uuid::new_v4().to_string(),
        email: body.email.clone(),
        name: body.name.clone(),
        created_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Created().json(ApiResponse::new(response)))
}
