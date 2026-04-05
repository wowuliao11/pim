use rpc_proto::user::v1::user_service_server::{UserService, UserServiceServer};
use rpc_proto::user::v1::{
    DeleteUserRequest, DeleteUserResponse, GetCurrentUserRequest, GetCurrentUserResponse, GetUserRequest,
    GetUserResponse, ListUsersRequest, ListUsersResponse, UpdateUserRequest, UpdateUserResponse, User,
};
use tonic::{transport::Server, Request, Response, Status};

use infra_telemetry as telemetry;

mod config;

use config::load_settings;

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
        Self {
            http_client: reqwest::Client::new(),
            zitadel_authority,
            service_account_token,
        }
    }

    /// Extract a User from Zitadel v2 user JSON response.
    fn user_from_json(body: &serde_json::Value) -> User {
        let user_id = body["userId"].as_str().unwrap_or_default().to_string();

        let given_name = body["human"]["profile"]["givenName"].as_str().unwrap_or_default();
        let family_name = body["human"]["profile"]["familyName"].as_str().unwrap_or_default();
        let name = format!("{} {}", given_name, family_name).trim().to_string();

        let email = body["human"]["email"]["email"].as_str().unwrap_or_default().to_string();

        let created_at = body["details"]["creationDate"].as_str().unwrap_or_default().to_string();
        let updated_at = body["details"]["changeDate"].as_str().unwrap_or_default().to_string();

        User {
            id: user_id,
            email,
            name,
            created_at,
            updated_at,
        }
    }

    /// Map reqwest errors to gRPC Status.
    fn map_reqwest_err(e: reqwest::Error) -> Status {
        tracing::error!(error = %e, "Zitadel API request failed");
        Status::internal(format!("Zitadel API error: {}", e))
    }

    /// Map JSON parse errors to gRPC Status.
    fn map_json_err(e: reqwest::Error) -> Status {
        tracing::error!(error = %e, "Failed to parse Zitadel API response");
        Status::internal(format!("Zitadel response parse error: {}", e))
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("User ID is required"));
        }

        let url = format!("{}/v2/users/{}", self.zitadel_authority, req.id);
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
            return Err(Status::internal(format!("Zitadel API returned {}", status)));
        }

        let body: serde_json::Value = response.json().await.map_err(Self::map_json_err)?;
        let user = Self::user_from_json(&body["user"]);

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
            return Err(Status::internal(format!("Zitadel API returned {}", status)));
        }

        let resp_body: serde_json::Value = response.json().await.map_err(Self::map_json_err)?;

        let users: Vec<User> = resp_body["result"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(Self::user_from_json)
            .collect();

        let total = resp_body["details"]["totalResult"].as_i64().unwrap_or(0) as i32;

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

        // Reuse get_user logic
        let url = format!("{}/v2/users/{}", self.zitadel_authority, req.user_id);
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
            return Err(Status::internal(format!("Zitadel API returned {}", status)));
        }

        let body: serde_json::Value = response.json().await.map_err(Self::map_json_err)?;
        let user = Self::user_from_json(&body["user"]);

        Ok(Response::new(GetCurrentUserResponse { user: Some(user) }))
    }

    async fn update_user(&self, request: Request<UpdateUserRequest>) -> Result<Response<UpdateUserResponse>, Status> {
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("User ID is required"));
        }

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
                return Err(Status::internal(format!("Zitadel API returned {}", status)));
            }
        }

        // Fetch updated user
        let get_url = format!("{}/v2/users/{}", self.zitadel_authority, req.id);
        let get_response = self
            .http_client
            .get(&get_url)
            .bearer_auth(&self.service_account_token)
            .send()
            .await
            .map_err(Self::map_reqwest_err)?;

        let body: serde_json::Value = get_response.json().await.map_err(Self::map_json_err)?;
        let user = Self::user_from_json(&body["user"]);

        Ok(Response::new(UpdateUserResponse { user: Some(user) }))
    }

    async fn delete_user(&self, request: Request<DeleteUserRequest>) -> Result<Response<DeleteUserResponse>, Status> {
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("User ID is required"));
        }

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
            return Err(Status::internal(format!("Zitadel API returned {}", status)));
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
