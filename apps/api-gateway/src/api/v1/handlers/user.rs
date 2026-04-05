use actix_web::{web, HttpResponse};

use infra_auth::IntrospectedUser;

use crate::api::v1::dto::{ApiResponse, UserResponse, UsersListResponse};
use crate::errors::{AppError, UserError};

/// GET /api/v1/users
/// List all users (requires authentication)
pub async fn list_users(_user: IntrospectedUser) -> Result<HttpResponse, AppError> {
    // TODO: Proxy to user-service gRPC or Zitadel Management API
    let users = vec![UserResponse {
        id: "placeholder".to_string(),
        email: "placeholder@example.com".to_string(),
        name: "Placeholder User".to_string(),
        created_at: chrono::Utc::now(),
    }];

    let response = UsersListResponse {
        total: users.len(),
        users,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::new(response)))
}

/// GET /api/v1/users/{id}
/// Get user by ID (requires authentication)
pub async fn get_user(_user: IntrospectedUser, path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    // TODO: Proxy to user-service gRPC or Zitadel Management API
    if user_id == "0" {
        return Err(UserError::NotFound { user_id }.into());
    }

    let response = UserResponse {
        id: user_id,
        email: "user@example.com".to_string(),
        name: "Example User".to_string(),
        created_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::new(response)))
}

/// GET /api/v1/users/me
/// Get current authenticated user
pub async fn get_current_user(user: IntrospectedUser) -> HttpResponse {
    let response = UserResponse {
        id: user.user_id,
        email: user.email.unwrap_or_default(),
        name: user.name.unwrap_or_default(),
        created_at: chrono::Utc::now(),
    };

    HttpResponse::Ok().json(ApiResponse::new(response))
}
