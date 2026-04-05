use actix_web::HttpResponse;

use infra_auth::IntrospectedUser;

use crate::api::v1::dto::{ApiResponse, UserInfoResponse};

/// GET /api/v1/auth/userinfo
/// Returns the authenticated user's info from the Zitadel introspection response.
/// Requires a valid Bearer token.
pub async fn userinfo(user: IntrospectedUser) -> HttpResponse {
    let response = UserInfoResponse {
        user_id: user.user_id,
        username: user.username,
        name: user.name,
        email: user.email,
        email_verified: user.email_verified,
    };

    HttpResponse::Ok().json(ApiResponse::new(response))
}
