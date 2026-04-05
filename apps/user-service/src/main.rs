use rpc_proto::user::v1::user_service_server::{UserService, UserServiceServer};
use rpc_proto::user::v1::{
    DeleteUserRequest, DeleteUserResponse, GetCurrentUserRequest, GetCurrentUserResponse, GetUserRequest,
    GetUserResponse, ListUsersRequest, ListUsersResponse, UpdateUserRequest, UpdateUserResponse, User,
};
use serde::Deserialize;
use tonic::{transport::Server, Request, Response, Status};

use infra_telemetry as telemetry;

mod config;

use config::load_settings;

/// Parse an RFC 3339 / ISO 8601 datetime string into a `prost_types::Timestamp`.
/// Returns `None` if the string is empty or unparseable.
fn parse_rfc3339(s: &str) -> Option<prost_types::Timestamp> {
    if s.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| prost_types::Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        })
}

// ── Zitadel v2 API response types ──────────────────────────────────────────

/// Wrapper for a single-user GET response (`GET /v2/users/{id}`).
#[derive(Deserialize)]
struct ZitadelUserResponse {
    user: ZitadelUser,
}

/// Wrapper for the list/search POST response (`POST /v2/users`).
#[derive(Deserialize)]
struct ZitadelListUsersResponse {
    #[serde(default)]
    result: Vec<ZitadelUser>,
    #[serde(default)]
    details: ZitadelListDetails,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ZitadelListDetails {
    #[serde(default)]
    total_result: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZitadelUser {
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    human: Option<ZitadelHuman>,
    #[serde(default)]
    details: Option<ZitadelResourceDetails>,
}

#[derive(Deserialize)]
struct ZitadelHuman {
    #[serde(default)]
    profile: Option<ZitadelProfile>,
    #[serde(default)]
    email: Option<ZitadelEmail>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZitadelProfile {
    #[serde(default)]
    given_name: String,
    #[serde(default)]
    family_name: String,
}

#[derive(Deserialize)]
struct ZitadelEmail {
    #[serde(default)]
    email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZitadelResourceDetails {
    #[serde(default)]
    creation_date: String,
    #[serde(default)]
    change_date: String,
}

/// Validate that a user ID looks like a valid Zitadel resource ID.
///
/// Zitadel uses numeric string IDs (e.g. "123456789012345678").
/// Rejecting unexpected formats prevents SSRF via URL path injection.
#[allow(clippy::result_large_err)]
fn validate_user_id(id: &str) -> Result<(), Status> {
    if id.is_empty() {
        return Err(Status::invalid_argument("User ID is required"));
    }
    // Zitadel IDs are numeric strings; reject anything with path separators,
    // whitespace, or non-alphanumeric characters that could manipulate the URL.
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Status::invalid_argument("Invalid user ID format"));
    }
    Ok(())
}

/// User service implementation backed by Zitadel Management REST API v2.
///
/// All user queries and mutations are proxied to the Zitadel instance.
/// Authentication to Zitadel is done via a service account Personal Access Token (PAT).
pub struct UserServiceImpl {
    http_client: reqwest::Client,
    zitadel_authority: String,
    service_account_token: String,
}

impl UserServiceImpl {
    pub fn new(zitadel_authority: String, service_account_token: String) -> Self {
        use std::time::Duration;
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http_client,
            zitadel_authority,
            service_account_token,
        }
    }

    /// Convert a typed Zitadel user into the proto `User`.
    fn zitadel_user_to_proto(zu: &ZitadelUser) -> User {
        let (given_name, family_name) = zu
            .human
            .as_ref()
            .and_then(|h| h.profile.as_ref())
            .map(|p| (p.given_name.as_str(), p.family_name.as_str()))
            .unwrap_or_default();
        let name = format!("{} {}", given_name, family_name).trim().to_string();

        let email = zu
            .human
            .as_ref()
            .and_then(|h| h.email.as_ref())
            .map(|e| e.email.clone())
            .unwrap_or_default();

        let (created_at, updated_at) = zu
            .details
            .as_ref()
            .map(|d| (parse_rfc3339(&d.creation_date), parse_rfc3339(&d.change_date)))
            .unwrap_or_default();

        User {
            id: zu.user_id.clone(),
            email,
            name,
            created_at,
            updated_at,
        }
    }

    /// Map reqwest errors to gRPC Status (generic message — details stay in logs only).
    fn map_reqwest_err(e: reqwest::Error) -> Status {
        tracing::error!(error = %e, "Zitadel API request failed");
        Status::internal("upstream service request failed")
    }

    /// Map JSON parse errors to gRPC Status (generic message — details stay in logs only).
    fn map_json_err(e: reqwest::Error) -> Status {
        tracing::error!(error = %e, "Failed to parse Zitadel API response");
        Status::internal("upstream service returned an invalid response")
    }

    /// Fetch a single user by ID from Zitadel, returning a parsed `User`.
    async fn fetch_user_by_id(&self, user_id: &str) -> Result<User, Status> {
        validate_user_id(user_id)?;

        let url = format!("{}/v2/users/{}", self.zitadel_authority, user_id);
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.service_account_token)
            .send()
            .await
            .map_err(Self::map_reqwest_err)?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Status::not_found("User not found"));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Zitadel API error");
            return Err(Status::internal("upstream service error"));
        }

        let resp: ZitadelUserResponse = response.json().await.map_err(Self::map_json_err)?;
        Ok(Self::zitadel_user_to_proto(&resp.user))
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();
        let user = self.fetch_user_by_id(&req.id).await?;
        Ok(Response::new(GetUserResponse { user: Some(user) }))
    }

    async fn list_users(&self, request: Request<ListUsersRequest>) -> Result<Response<ListUsersResponse>, Status> {
        let req = request.into_inner();

        let page = if req.page <= 0 { 1 } else { req.page };
        let page_size = if req.page_size <= 0 || req.page_size > 100 {
            20
        } else {
            req.page_size
        };

        // Zitadel v2 uses POST /v2/users for listing/searching
        let url = format!("{}/v2/users", self.zitadel_authority);
        let body = serde_json::json!({
            "query": {
                "offset": ((page - 1) * page_size) as u64,
                "limit": page_size as u64,
                "asc": true,
            }
        });

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.service_account_token)
            .json(&body)
            .send()
            .await
            .map_err(Self::map_reqwest_err)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Zitadel list users API error");
            return Err(Status::internal("upstream service error"));
        }

        let resp: ZitadelListUsersResponse = response.json().await.map_err(Self::map_json_err)?;

        let users: Vec<User> = resp.result.iter().map(Self::zitadel_user_to_proto).collect();

        let total = resp.details.total_result as i32;

        Ok(Response::new(ListUsersResponse {
            users,
            total,
            page,
            page_size,
        }))
    }

    async fn get_current_user(
        &self,
        request: Request<GetCurrentUserRequest>,
    ) -> Result<Response<GetCurrentUserResponse>, Status> {
        let req = request.into_inner();

        if req.user_id.is_empty() {
            return Err(Status::invalid_argument(
                "user_id must be provided (set by gateway from token)",
            ));
        }

        let user = self.fetch_user_by_id(&req.user_id).await?;
        Ok(Response::new(GetCurrentUserResponse { user: Some(user) }))
    }

    async fn update_user(&self, request: Request<UpdateUserRequest>) -> Result<Response<UpdateUserResponse>, Status> {
        let req = request.into_inner();
        validate_user_id(&req.id)?;

        // Update profile via Zitadel v2 API
        // PATCH /v2/users/{userId}
        let url = format!("{}/v2/users/{}", self.zitadel_authority, req.id);
        let mut update_body = serde_json::Map::new();

        // Build profile update if name is provided
        if let Some(ref name) = req.name {
            let parts: Vec<&str> = name.splitn(2, ' ').collect();
            let given_name = parts.first().unwrap_or(&"");
            let family_name = if parts.len() > 1 { parts[1] } else { "" };
            update_body.insert(
                "profile".to_string(),
                serde_json::json!({
                    "givenName": given_name,
                    "familyName": family_name,
                }),
            );
        }

        // Build email update if provided
        if let Some(ref email) = req.email {
            update_body.insert(
                "email".to_string(),
                serde_json::json!({
                    "email": email,
                }),
            );
        }

        if !update_body.is_empty() {
            let response = self
                .http_client
                .put(&url)
                .bearer_auth(&self.service_account_token)
                .json(&serde_json::Value::Object(update_body))
                .send()
                .await
                .map_err(Self::map_reqwest_err)?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::error!(status = %status, body = %body, "Zitadel update user API error");
                return Err(Status::internal("upstream service error"));
            }
        }

        // Fetch updated user
        let user = self.fetch_user_by_id(&req.id).await?;
        Ok(Response::new(UpdateUserResponse { user: Some(user) }))
    }

    async fn delete_user(&self, request: Request<DeleteUserRequest>) -> Result<Response<DeleteUserResponse>, Status> {
        let req = request.into_inner();
        validate_user_id(&req.id)?;

        let url = format!("{}/v2/users/{}", self.zitadel_authority, req.id);
        let response = self
            .http_client
            .delete(&url)
            .bearer_auth(&self.service_account_token)
            .send()
            .await
            .map_err(Self::map_reqwest_err)?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Status::not_found("User not found"));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Zitadel delete user API error");
            return Err(Status::internal("upstream service error"));
        }

        Ok(Response::new(DeleteUserResponse { success: true }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init_tracing("user_service=info,common=info");

    let settings = load_settings()?;
    let addr = format!("{}:{}", settings.host, settings.port).parse()?;

    // Initialize Prometheus metrics recorder
    match telemetry::install_prometheus(
        telemetry::PrometheusOptions::new(env!("CARGO_PKG_NAME")).env(settings.common.app_env.to_string()),
    ) {
        Ok(handle) => {
            let metrics_host = settings.host.clone();
            let metrics_port = settings.metrics_port;
            tokio::spawn(async move {
                if let Err(err) = telemetry::serve_metrics_http(&metrics_host, metrics_port, handle).await {
                    tracing::warn!(error = %err, "metrics server stopped");
                }
            });
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to initialize metrics");
        }
    }

    tracing::info!("Starting {} on {}", env!("CARGO_PKG_NAME"), addr);
    tracing::info!("Zitadel authority: {}", settings.zitadel_authority);

    let user_service = UserServiceImpl::new(settings.zitadel_authority, settings.zitadel_service_account_token);

    Server::builder()
        .layer(telemetry::GrpcMetricsLayer)
        .add_service(UserServiceServer::new(user_service))
        .serve(addr)
        .await?;

    Ok(())
}
