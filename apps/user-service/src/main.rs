use rpc_proto::user::v1::user_service_server::{UserService, UserServiceServer};
use rpc_proto::user::v1::{
    DeleteUserRequest, DeleteUserResponse, GetCurrentUserRequest, GetCurrentUserResponse, GetUserRequest,
    GetUserResponse, ListUsersRequest, ListUsersResponse, UpdateUserRequest, UpdateUserResponse, User,
};
use tonic::{transport::Server, Request, Response, Status};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use common::telemetry;

mod config;

use config::load_settings;

/// User service implementation
#[derive(Default)]
pub struct UserServiceImpl;

impl UserServiceImpl {
    pub fn new() -> Self {
        Self
    }

    /// Create a mock user for demonstration
    fn mock_user(id: &str) -> User {
        let now = chrono::Utc::now().to_rfc3339();
        User {
            id: id.to_string(),
            email: format!("user{}@example.com", id),
            name: format!("User {}", id),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("User ID is required"));
        }

        // TODO: Query user from database
        if req.id == "0" {
            return Err(Status::not_found("User not found"));
        }

        let user = Self::mock_user(&req.id);
        let response = GetUserResponse { user: Some(user) };

        Ok(Response::new(response))
    }

    async fn list_users(&self, request: Request<ListUsersRequest>) -> Result<Response<ListUsersResponse>, Status> {
        let req = request.into_inner();

        let page = if req.page <= 0 { 1 } else { req.page };
        let page_size = if req.page_size <= 0 || req.page_size > 100 {
            20
        } else {
            req.page_size
        };

        // TODO: Query users from database with pagination
        let users = vec![Self::mock_user("1"), Self::mock_user("2"), Self::mock_user("3")];

        let response = ListUsersResponse {
            users,
            total: 3,
            page,
            page_size,
        };

        Ok(Response::new(response))
    }

    async fn get_current_user(
        &self,
        request: Request<GetCurrentUserRequest>,
    ) -> Result<Response<GetCurrentUserResponse>, Status> {
        // In real implementation, extract user ID from request metadata (set by gateway)
        let _req = request.into_inner();

        // TODO: Get user ID from authentication context
        let user = Self::mock_user("current");
        let response = GetCurrentUserResponse { user: Some(user) };

        Ok(Response::new(response))
    }

    async fn update_user(&self, request: Request<UpdateUserRequest>) -> Result<Response<UpdateUserResponse>, Status> {
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("User ID is required"));
        }

        // TODO: Update user in database
        let now = chrono::Utc::now().to_rfc3339();
        let user = User {
            id: req.id,
            email: req.email.unwrap_or_else(|| "user@example.com".to_string()),
            name: req.name.unwrap_or_else(|| "Updated User".to_string()),
            created_at: now.clone(),
            updated_at: now,
        };

        let response = UpdateUserResponse { user: Some(user) };

        Ok(Response::new(response))
    }

    async fn delete_user(&self, request: Request<DeleteUserRequest>) -> Result<Response<DeleteUserResponse>, Status> {
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("User ID is required"));
        }

        // TODO: Delete user from database
        let response = DeleteUserResponse { success: true };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "user_service=info,common=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let settings = load_settings()?;
    let addr = format!("{}:{}", settings.host, settings.port).parse()?;

    // Initialize Prometheus metrics recorder
    if let Err(err) = telemetry::init("user-service") {
        tracing::warn!(error = %err, "failed to initialize metrics");
    }

    // Start metrics HTTP server (management plane)
    let metrics_host = settings.host.clone();
    let metrics_port = settings.metrics_port;
    tokio::spawn(async move {
        if let Err(err) = telemetry::serve_metrics_http(&metrics_host, metrics_port).await {
            tracing::warn!(error = %err, "metrics server stopped");
        }
    });

    tracing::info!("Starting user-service on {}", addr);

    let user_service = UserServiceImpl::new();

    Server::builder()
        .layer(telemetry::GrpcMetricsLayer)
        .add_service(UserServiceServer::new(user_service))
        .serve(addr)
        .await?;

    Ok(())
}
