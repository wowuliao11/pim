use actix_web::{web, HttpResponse};
use chrono::{DateTime, TimeZone, Utc};

use infra_auth::IntrospectedUser;
use rpc_proto::user::v1::user_service_client::UserServiceClient;
use rpc_proto::user::v1::{GetCurrentUserRequest, GetUserRequest, ListUsersRequest};
use tonic::transport::Channel;

use crate::api::v1::dto::{ApiResponse, UserResponse, UsersListResponse};
use crate::errors::{AppError, UserError};

/// Shared gRPC client handle, wrapped in actix `Data`.
pub type UserGrpcClient = web::Data<UserServiceClient<Channel>>;

/// Convert a proto `prost_types::Timestamp` to `chrono::DateTime<Utc>`.
fn timestamp_to_datetime(ts: Option<prost_types::Timestamp>) -> DateTime<Utc> {
    ts.and_then(|t| Utc.timestamp_opt(t.seconds, t.nanos as u32).single())
        .unwrap_or_default()
}

/// Map a proto `User` to the gateway DTO `UserResponse`.
fn proto_user_to_dto(u: rpc_proto::user::v1::User) -> UserResponse {
    UserResponse {
        id: u.id,
        email: u.email,
        name: u.name,
        created_at: timestamp_to_datetime(u.created_at),
    }
}

/// GET /api/v1/users
/// List all users (requires authentication)
pub async fn list_users(_user: IntrospectedUser, client: UserGrpcClient) -> Result<HttpResponse, AppError> {
    let request = tonic::Request::new(ListUsersRequest { page: 1, page_size: 20 });

    let response = client
        .get_ref()
        .clone()
        .list_users(request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "gRPC list_users failed");
            AppError::Internal(anyhow::anyhow!("Failed to fetch users"))
        })?
        .into_inner();

    let users: Vec<UserResponse> = response.users.into_iter().map(proto_user_to_dto).collect();
    let total = response.total as usize;

    Ok(HttpResponse::Ok().json(ApiResponse::new(UsersListResponse { users, total })))
}

/// GET /api/v1/users/{id}
/// Get user by ID (requires authentication)
pub async fn get_user(
    _user: IntrospectedUser,
    path: web::Path<String>,
    client: UserGrpcClient,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    let request = tonic::Request::new(GetUserRequest { id: user_id.clone() });

    let response = client
        .get_ref()
        .clone()
        .get_user(request)
        .await
        .map_err(|e| match e.code() {
            tonic::Code::NotFound => AppError::User(UserError::NotFound { user_id }),
            _ => {
                tracing::error!(error = %e, "gRPC get_user failed");
                AppError::Internal(anyhow::anyhow!("Failed to fetch user"))
            }
        })?
        .into_inner();

    let user = response
        .user
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User response was empty")))?;

    Ok(HttpResponse::Ok().json(ApiResponse::new(proto_user_to_dto(user))))
}

/// GET /api/v1/users/me
/// Get current authenticated user
pub async fn get_current_user(user: IntrospectedUser, client: UserGrpcClient) -> Result<HttpResponse, AppError> {
    let request = tonic::Request::new(GetCurrentUserRequest {
        user_id: user.user_id.clone(),
    });

    let response = client
        .get_ref()
        .clone()
        .get_current_user(request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "gRPC get_current_user failed");
            AppError::Internal(anyhow::anyhow!("Failed to fetch current user"))
        })?
        .into_inner();

    let proto_user = response
        .user
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User response was empty")))?;

    Ok(HttpResponse::Ok().json(ApiResponse::new(proto_user_to_dto(proto_user))))
}
