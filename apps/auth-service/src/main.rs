use rpc_proto::auth::v1::auth_service_server::{AuthService, AuthServiceServer};
use rpc_proto::auth::v1::{
    LoginRequest, LoginResponse, RefreshTokenRequest, RefreshTokenResponse, RegisterRequest, RegisterResponse,
    ValidateTokenRequest, ValidateTokenResponse,
};
use tonic::{transport::Server, Request, Response, Status};

use infra_telemetry as telemetry;

mod config;

use config::load_settings;
use infra_auth::JwtManager;

/// Auth service implementation
pub struct AuthServiceImpl {
    jwt_manager: JwtManager,
}

impl AuthServiceImpl {
    pub fn new(jwt_secret: String, expiration_hours: i64) -> Self {
        Self {
            jwt_manager: JwtManager::new(jwt_secret, expiration_hours),
        }
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(&self, request: Request<LoginRequest>) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        // Validate request
        if req.email.is_empty() || req.password.is_empty() {
            return Err(Status::invalid_argument("Email and password are required"));
        }

        // TODO: Validate credentials against database
        // For now, accept any non-empty credentials
        let user_id = format!("user_{}", uuid::Uuid::new_v4());

        // Generate JWT token
        let token = self
            .jwt_manager
            .generate_token(&user_id, vec!["user".to_string()])
            .map_err(|e| Status::internal(format!("Failed to generate token: {}", e)))?;

        let response = LoginResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_manager.expiration_hours() * 3600,
            user_id,
        };

        Ok(Response::new(response))
    }

    async fn register(&self, request: Request<RegisterRequest>) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();

        // Validate request
        if req.email.is_empty() || req.password.is_empty() || req.name.is_empty() {
            return Err(Status::invalid_argument("All fields are required"));
        }

        if !req.email.contains('@') {
            return Err(Status::invalid_argument("Invalid email format"));
        }

        if req.password.len() < 8 {
            return Err(Status::invalid_argument("Password must be at least 8 characters"));
        }

        // TODO: Create user in database
        let user_id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        let response = RegisterResponse {
            id: user_id,
            email: req.email,
            name: req.name,
            created_at,
        };

        Ok(Response::new(response))
    }

    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<ValidateTokenResponse>, Status> {
        let req = request.into_inner();

        match self.jwt_manager.validate_token(&req.token) {
            Ok(claims) => {
                let response = ValidateTokenResponse {
                    valid: true,
                    user_id: claims.sub,
                    roles: claims.roles,
                };
                Ok(Response::new(response))
            }
            Err(_) => {
                let response = ValidateTokenResponse {
                    valid: false,
                    user_id: String::new(),
                    roles: vec![],
                };
                Ok(Response::new(response))
            }
        }
    }

    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> Result<Response<RefreshTokenResponse>, Status> {
        let req = request.into_inner();

        // Validate existing token
        let claims = self
            .jwt_manager
            .validate_token(&req.token)
            .map_err(|_| Status::unauthenticated("Invalid token"))?;

        // Generate new token
        let new_token = self
            .jwt_manager
            .generate_token(&claims.sub, claims.roles)
            .map_err(|e| Status::internal(format!("Failed to generate token: {}", e)))?;

        let response = RefreshTokenResponse {
            access_token: new_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_manager.expiration_hours() * 3600,
        };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    telemetry::init_tracing("auth_service=info,common=info");

    // Load configuration
    let settings = load_settings()?;
    let addr = format!("{}:{}", settings.host, settings.port).parse()?;

    // Initialize Prometheus metrics recorder
    // Env is sourced from infra-config (not read inside the library)
    match telemetry::install_prometheus(
        telemetry::PrometheusOptions::new(env!("CARGO_PKG_NAME")).env(settings.common.app_env.to_string()),
    ) {
        Ok(handle) => {
            // Start metrics HTTP server (management plane)
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

    let auth_service = AuthServiceImpl::new(settings.jwt_secret, settings.jwt_expiration_hours);

    Server::builder()
        .layer(telemetry::GrpcMetricsLayer)
        .add_service(AuthServiceServer::new(auth_service))
        .serve(addr)
        .await?;

    Ok(())
}
