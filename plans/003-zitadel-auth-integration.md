# Zitadel Authentication Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace PIM's placeholder auth system with Zitadel Cloud as the external Identity Provider, using OIDC Token Introspection via the `zitadel` Rust crate's actix-web `IntrospectedUser` extractor.

**Architecture:** Clients (Tauri mobile app + React admin panel) authenticate directly with Zitadel Cloud using Authorization Code + PKCE flow. The API Gateway validates incoming Bearer tokens via Zitadel's Token Introspection endpoint using the `zitadel` crate's built-in actix `IntrospectedUser` extractor — **not** a custom middleware. The `auth-service` gRPC service is removed. The `user-service` becomes a proxy to Zitadel's User Management REST API v2.

**Tech Stack:** `zitadel` crate v5 (features: `actix`, `oidc`, `credentials`), Zitadel Cloud (SaaS), OIDC/OAuth 2.0, `reqwest` for Zitadel REST API calls

---

## Context

### Current State

PIM is a Rust microservices monorepo: "1 x HTTP Gateway (actix-web 4) + N x gRPC Services (tonic 0.12)".

Current auth system (all placeholder/mock):
- `libs/infra-auth/` — `JwtManager` using `jsonwebtoken` crate (HMAC-SHA256)
- `apps/api-gateway/src/middlewares/auth.rs` — Custom `JwtAuth` actix Transform middleware
- `apps/auth-service/` — gRPC service with Login/Register/ValidateToken/RefreshToken (all TODO)
- `apps/user-service/` — gRPC service with user CRUD (all mock data)
- No database, no password hashing, no real credential validation anywhere

### Target State

- **Zitadel Cloud** is the single source of truth for identity, authentication, and authorization
- **API Gateway** validates tokens via the `zitadel` crate's `IntrospectedUser` extractor (OIDC Token Introspection)
- **auth-service is removed** — Zitadel handles login/register/token lifecycle
- **user-service proxies Zitadel** — user queries go through Zitadel's Management REST API v2
- **Clients** (Tauri + React) perform OIDC Authorization Code + PKCE flow directly with Zitadel

### Key API: `zitadel` crate actix integration

The `zitadel` crate provides an `IntrospectedUser` struct that implements actix's `FromRequest` trait. It works as an **extractor**, not a middleware:

```rust
// Protected route — just add IntrospectedUser parameter
async fn protected(user: IntrospectedUser) -> impl Responder {
    format!("Hello {}", user.user_id)
}

// Unprotected route — no IntrospectedUser parameter
async fn public() -> impl Responder {
    "Hello anonymous"
}
```

Setup requires injecting `IntrospectionConfig` into actix app_data:

```rust
let config = IntrospectionConfigBuilder::new("https://instance.zitadel.cloud")
    .with_basic_auth("client_id", "client_secret")
    .build()
    .await
    .unwrap();

HttpServer::new(move || {
    App::new()
        .app_data(config.clone())
        .service(protected)
        .service(public)
})
```

`IntrospectedUser` fields: `user_id: String`, `username: Option<String>`, `name: Option<String>`, `email: Option<String>`, `email_verified: Option<bool>`, `project_roles: Option<HashMap<String, HashMap<String, String>>>`, `metadata: Option<HashMap<String, String>>`, etc.

---

## Phase 1: Gateway Token Introspection (Core)

### Task 1: Add `zitadel` crate to workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `libs/infra-auth/Cargo.toml`

- [ ] **Step 1: Add zitadel to workspace dependencies**

In the workspace root `Cargo.toml`, add to the `[workspace.dependencies]` section:

```toml
# Authentication (Zitadel OIDC)
zitadel = { version = "5", features = ["actix", "oidc", "credentials"] }
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: Update `libs/infra-auth/Cargo.toml`**

Replace the entire file content with:

```toml
[package]
name = "infra-auth"
version.workspace = true
edition.workspace = true

[dependencies]
zitadel.workspace = true
serde.workspace = true
thiserror.workspace = true
```

This removes `jsonwebtoken` and `chrono`, adds `zitadel`.

- [ ] **Step 3: Verify dependency resolution**

Run: `cargo check -p infra-auth 2>&1 | head -20`

Expected: May fail on source code errors (we haven't updated code yet), but dependencies should resolve without version conflicts. If there's a tonic version conflict, verify that the `api` feature is NOT enabled on the zitadel crate.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock libs/infra-auth/Cargo.toml
git commit -m "deps: add zitadel crate, replace jsonwebtoken in infra-auth"
```

---

### Task 2: Rewrite `libs/infra-auth` as Zitadel integration layer

**Files:**
- Remove: `libs/infra-auth/src/jwt_manager.rs`
- Remove: `libs/infra-auth/src/claims.rs`
- Remove: `libs/infra-auth/src/error.rs`
- Modify: `libs/infra-auth/src/lib.rs`

- [ ] **Step 1: Delete old files**

```bash
rm libs/infra-auth/src/jwt_manager.rs
rm libs/infra-auth/src/claims.rs
rm libs/infra-auth/src/error.rs
```

- [ ] **Step 2: Rewrite `libs/infra-auth/src/lib.rs`**

Replace the entire file with:

```rust
//! Infrastructure authentication library — Zitadel OIDC integration
//!
//! Provides re-exports from the `zitadel` crate for actix-web Token Introspection.
//! The API Gateway uses `IntrospectedUser` as an actix extractor to validate
//! Bearer tokens against Zitadel's introspection endpoint.

// Re-export the actix introspection types that consumers need
pub use zitadel::actix::introspection::{
    IntrospectedUser, IntrospectionConfig, IntrospectionConfigBuilder,
};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p infra-auth`

Expected: PASS — the crate should compile with just re-exports.

- [ ] **Step 4: Commit**

```bash
git add -A libs/infra-auth/
git commit -m "refactor(infra-auth): replace JWT manager with Zitadel OIDC re-exports"
```

---

### Task 3: Update Gateway configuration for Zitadel

**Files:**
- Modify: `apps/api-gateway/src/config/settings.rs`
- Modify: `apps/api-gateway/src/config/app_config.rs`

- [ ] **Step 1: Replace `JwtSettings` with `ZitadelSettings` in `settings.rs`**

Replace the entire file `apps/api-gateway/src/config/settings.rs`:

```rust
use infra_config::CommonConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Settings {
    #[serde(flatten)]
    pub common: CommonConfig,

    pub app: AppSettings,
    pub db: DbSettings,
    pub zitadel: ZitadelSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
    pub metrics_port: u16,
    pub name: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            metrics_port: 60080,
            name: env!("CARGO_PKG_NAME").to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DbSettings {
    pub url: String,
}

impl Default for DbSettings {
    fn default() -> Self {
        Self {
            url: "postgres://localhost/pim".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZitadelSettings {
    /// Zitadel instance URL, e.g. "https://my-instance.zitadel.cloud"
    pub authority: String,
    /// API application client ID (for token introspection)
    pub client_id: String,
    /// API application client secret (for token introspection)
    pub client_secret: String,
}

impl Default for ZitadelSettings {
    fn default() -> Self {
        Self {
            authority: "https://localhost.zitadel.cloud".to_string(),
            client_id: "change-me".to_string(),
            client_secret: "change-me".to_string(),
        }
    }
}
```

- [ ] **Step 2: Update `app_config.rs` accessors**

Replace the entire file `apps/api-gateway/src/config/app_config.rs`:

```rust
use super::load_settings;
use super::Settings;
use config;

/// Application configuration wrapper
#[derive(Clone)]
pub struct AppConfig {
    pub settings: Settings,
}

impl AppConfig {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.settings.app.host, self.settings.app.port)
    }

    pub fn app_name(&self) -> &str {
        &self.settings.app.name
    }

    pub fn zitadel_authority(&self) -> &str {
        &self.settings.zitadel.authority
    }

    pub fn zitadel_client_id(&self) -> &str {
        &self.settings.zitadel.client_id
    }

    pub fn zitadel_client_secret(&self) -> &str {
        &self.settings.zitadel.client_secret
    }
}

pub fn load_app_config() -> Result<AppConfig, config::ConfigError> {
    let settings = load_settings()?;
    Ok(AppConfig::new(settings))
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/api-gateway/src/config/
git commit -m "refactor(gateway): replace JWT config with Zitadel settings"
```

---

### Task 4: Replace auth middleware with Zitadel extractor pattern

**Files:**
- Modify: `apps/api-gateway/src/middlewares/mod.rs`
- Modify: `apps/api-gateway/src/middlewares/auth.rs`

- [ ] **Step 1: Simplify `middlewares/auth.rs`**

Replace the entire file `apps/api-gateway/src/middlewares/auth.rs`:

```rust
//! Authentication types for the API Gateway.
//!
//! Token validation is handled by the `zitadel` crate's `IntrospectedUser`
//! extractor (injected directly into handler functions). This module provides
//! a thin `AuthenticatedUser` wrapper that extracts the fields PIM cares about.

use infra_auth::IntrospectedUser;

/// Authenticated user data extracted from Zitadel token introspection.
///
/// Convenience wrapper. Handlers that need full introspection data
/// can use `IntrospectedUser` directly from `infra_auth`.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub roles: Vec<String>,
}

impl From<&IntrospectedUser> for AuthenticatedUser {
    fn from(user: &IntrospectedUser) -> Self {
        // Extract role names from project_roles map
        // Zitadel project_roles: HashMap<role_name, HashMap<org_id, org_name>>
        let roles = user
            .project_roles
            .as_ref()
            .map(|pr| pr.keys().cloned().collect())
            .unwrap_or_default();

        Self {
            user_id: user.user_id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            roles,
        }
    }
}
```

- [ ] **Step 2: Update `middlewares/mod.rs`**

Replace the entire file `apps/api-gateway/src/middlewares/mod.rs`:

```rust
pub mod auth;
pub mod http_metrics;
pub mod request_id;
pub mod request_logging;

pub use auth::AuthenticatedUser;
pub use http_metrics::HttpMetrics;
pub use request_id::RequestId;
pub use request_logging::RequestLogging;
```

Note: `JwtAuth` is no longer exported.

- [ ] **Step 3: Commit**

```bash
git add apps/api-gateway/src/middlewares/
git commit -m "refactor(gateway): replace JwtAuth middleware with AuthenticatedUser wrapper"
```

---

### Task 5: Update route configuration (remove auth routes, use extractor)

**Files:**
- Modify: `apps/api-gateway/src/api/v1/routes.rs`
- Modify: `apps/api-gateway/src/api/v1/handlers/auth.rs`
- Modify: `apps/api-gateway/src/api/v1/handlers/user.rs`
- Modify: `apps/api-gateway/src/api/v1/dto.rs`

- [ ] **Step 1: Rewrite route configuration**

Replace the entire file `apps/api-gateway/src/api/v1/routes.rs`:

```rust
use actix_web::web;

use super::handlers;

pub fn configure() -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("/auth")
                .route("/userinfo", web::get().to(handlers::auth::userinfo)),
        )
        .service(
            web::scope("/users")
                .route("/me", web::get().to(handlers::user::get_current_user))
                .route("", web::get().to(handlers::user::list_users))
                .route("/{id}", web::get().to(handlers::user::get_user)),
        );
    }
}
```

No `jwt_manager` parameter. No `.wrap(JwtAuth::new(...))`. `/auth/login` and `/auth/register` routes removed.

- [ ] **Step 2: Replace auth handlers**

Replace the entire file `apps/api-gateway/src/api/v1/handlers/auth.rs`:

```rust
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
```

- [ ] **Step 3: Update user handlers to use IntrospectedUser extractor**

Replace the entire file `apps/api-gateway/src/api/v1/handlers/user.rs`:

```rust
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
pub async fn get_user(
    _user: IntrospectedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
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
```

- [ ] **Step 4: Update DTOs**

Replace the entire file `apps/api-gateway/src/api/v1/dto.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;

// ============ Auth DTOs ============

#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    pub user_id: String,
    pub username: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
}

// ============ User DTOs ============

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UsersListResponse {
    pub users: Vec<UserResponse>,
    pub total: usize,
}

// ============ Common DTOs ============

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { success: true, data }
    }
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

impl MessageResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
```

Removed: `LoginRequest`, `LoginResponse`, `RegisterRequest`, `RegisterResponse`, `Deserialize` import.
Added: `UserInfoResponse`.

- [ ] **Step 5: Commit**

```bash
git add apps/api-gateway/src/api/
git commit -m "refactor(gateway): replace auth routes with userinfo, use IntrospectedUser extractor"
```

---

### Task 6: Update Gateway main.rs and router

**Files:**
- Modify: `apps/api-gateway/src/main.rs`
- Modify: `apps/api-gateway/src/router/register.rs`
- Modify: `apps/api-gateway/src/errors/auth_error.rs`

- [ ] **Step 1: Rewrite `router/register.rs`**

Replace the entire file `apps/api-gateway/src/router/register.rs`:

```rust
use actix_web::web;

use crate::api;

pub fn configure_routes() -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.route("/health", web::get().to(health_check))
            .service(web::scope("/api/v1").configure(api::v1::configure()));
    }
}

async fn health_check() -> &'static str {
    "OK"
}
```

No `jwt_manager` parameter.

- [ ] **Step 2: Rewrite `main.rs`**

Replace the entire file `apps/api-gateway/src/main.rs`:

```rust
use actix_web::{App, HttpServer};
use api_gateway::config::load_app_config;
use api_gateway::middlewares::{HttpMetrics, RequestId, RequestLogging};
use api_gateway::router::configure_routes;
use infra_auth::IntrospectionConfigBuilder;
use infra_telemetry as telemetry;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    telemetry::init_tracing("api_gateway=info,actix_web=info,common=info");

    // Load configuration
    let config =
        load_app_config().map_err(|e| std::io::Error::other(format!("Failed to load configuration: {}", e)))?;
    let bind_address = config.bind_address();

    // Initialize Prometheus metrics recorder
    match telemetry::install_prometheus(
        telemetry::PrometheusOptions::new(env!("CARGO_PKG_NAME")).env(config.settings.common.app_env.to_string()),
    ) {
        Ok(handle) => {
            let metrics_host = config.settings.app.host.clone();
            let metrics_port = config.settings.app.metrics_port;
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

    // Build Zitadel introspection config (fetches OIDC discovery document)
    let introspection_config = IntrospectionConfigBuilder::new(config.zitadel_authority())
        .with_basic_auth(config.zitadel_client_id(), config.zitadel_client_secret())
        .build()
        .await
        .map_err(|e| std::io::Error::other(format!("Failed to build Zitadel introspection config: {}", e)))?;

    tracing::info!("Starting {} server at http://{}", config.app_name(), bind_address);
    tracing::info!("Zitadel authority: {}", config.zitadel_authority());

    HttpServer::new(move || {
        App::new()
            .app_data(introspection_config.clone())
            .wrap(HttpMetrics)
            .wrap(RequestLogging)
            .wrap(RequestId)
            .configure(configure_routes())
    })
    .bind(&bind_address)?
    .run()
    .await
}
```

Key changes: Removed `JwtManager`, `Arc`, `web::Data`. Added `IntrospectionConfigBuilder` with `with_basic_auth`. `configure_routes()` takes no params.

- [ ] **Step 3: Update error types**

Replace `apps/api-gateway/src/errors/auth_error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing or invalid authorization header")]
    MissingOrInvalidAuthorizationHeader,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Token introspection failed")]
    IntrospectionFailed,
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p api-gateway`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/api-gateway/
git commit -m "refactor(gateway): integrate Zitadel IntrospectionConfig in main, remove JwtManager"
```

---

## Phase 2: Remove auth-service

### Task 7: Remove auth-service and its proto

**Files:**
- Remove: `apps/auth-service/` (entire directory)
- Remove: `proto/auth/` (directory)
- Modify: `libs/rpc-proto/build.rs`
- Modify: `libs/rpc-proto/src/lib.rs`

- [ ] **Step 1: Remove auth-service directory**

```bash
rm -rf apps/auth-service
```

- [ ] **Step 2: Remove auth proto**

```bash
rm -rf proto/auth
```

- [ ] **Step 3: Update `libs/rpc-proto/build.rs`**

Replace the entire file:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../proto";

    println!("cargo:rerun-if-changed={}", proto_root);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["../../proto/user/v1/user.proto"], &[proto_root])?;

    Ok(())
}
```

- [ ] **Step 4: Update `libs/rpc-proto/src/lib.rs`**

Replace the entire file:

```rust
//! RPC Proto - Generated gRPC interfaces
//!
//! This crate re-exports the gRPC generated code from proto files.
//!
//! IMPORTANT: This crate is a "boundary layer" - it only contains:
//! - Generated gRPC client/server types from proto files
//! - Re-exports of those types
//!
//! FORBIDDEN in this crate:
//! - Business logic
//! - Helpers / mappers / validators
//! - Any non-generated code beyond re-exports

/// User service proto definitions
pub mod user {
    pub mod v1 {
        tonic::include_proto!("user.v1");
    }
}
```

- [ ] **Step 5: Verify full workspace compilation**

Run: `cargo check --workspace`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: remove auth-service and auth proto (replaced by Zitadel)"
```

---

## Phase 3: User Service -> Zitadel Proxy

### Task 8: Implement Zitadel Management API proxy in user-service

**Files:**
- Modify: `apps/user-service/Cargo.toml`
- Modify: `apps/user-service/src/config.rs`
- Modify: `apps/user-service/src/main.rs`

- [ ] **Step 1: Add reqwest to `apps/user-service/Cargo.toml`**

Add these dependencies:

```toml
reqwest.workspace = true
serde_json.workspace = true
serde.workspace = true
```

- [ ] **Step 2: Add Zitadel settings to user-service config**

In `apps/user-service/src/config.rs`, add Zitadel-related fields to the Settings struct:

```rust
pub zitadel_authority: String,
pub zitadel_service_account_token: String,
```

With defaults:

```rust
zitadel_authority: "https://localhost.zitadel.cloud".to_string(),
zitadel_service_account_token: "change-me".to_string(),
```

Env vars: `USER_SERVICE__ZITADEL_AUTHORITY`, `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN`

- [ ] **Step 3: Implement Zitadel user API client in `main.rs`**

Update `UserServiceImpl` to hold an `http_client: reqwest::Client`, `zitadel_authority: String`, and `service_account_token: String`.

Replace each mock RPC with a `reqwest` call to Zitadel Management REST API v2. Example for `GetUser`:

```rust
async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<GetUserResponse>, Status> {
    let req = request.into_inner();
    if req.id.is_empty() {
        return Err(Status::invalid_argument("User ID is required"));
    }

    let url = format!("{}/v2/users/{}", self.zitadel_authority, req.id);
    let response = self.http_client
        .get(&url)
        .bearer_auth(&self.service_account_token)
        .send()
        .await
        .map_err(|e| Status::internal(format!("Zitadel API error: {}", e)))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Status::not_found("User not found"));
    }

    let body: serde_json::Value = response.json().await
        .map_err(|e| Status::internal(format!("Parse error: {}", e)))?;

    let user = User {
        id: body["userId"].as_str().unwrap_or_default().to_string(),
        email: body["human"]["email"]["email"].as_str().unwrap_or_default().to_string(),
        name: format!(
            "{} {}",
            body["human"]["profile"]["givenName"].as_str().unwrap_or_default(),
            body["human"]["profile"]["familyName"].as_str().unwrap_or_default()
        ),
        created_at: body["details"]["creationDate"].as_str().unwrap_or_default().to_string(),
        updated_at: body["details"]["changeDate"].as_str().unwrap_or_default().to_string(),
    };

    Ok(Response::new(GetUserResponse { user: Some(user) }))
}
```

Apply similar pattern for `ListUsers`, `GetCurrentUser`, `UpdateUser`, `DeleteUser`.

Note: Zitadel v2 API response format should be verified at implementation time.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p user-service`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/user-service/
git commit -m "feat(user-service): implement Zitadel Management API proxy for user operations"
```

---

## Phase 4: Documentation & Cleanup

### Task 9: Update design documentation and clean up

**Files:**
- Modify: `docs/design.md`
- Modify: `docs/configuration.md`
- Modify: `apps/api-gateway/config.example.toml`

- [ ] **Step 1: Update `docs/design.md`**

Update to reflect:
- Remove `auth-service` from architecture diagram
- Show Zitadel Cloud as external dependency
- Update Layer Responsibilities table (Authentication = Zitadel OIDC)
- Update Domain Services (remove auth-service)
- Update HTTP routes table (remove login/register, add userinfo)
- Update Configuration Management (Zitadel settings)
- Mark auth integration as done in Future Evolution

- [ ] **Step 2: Update `docs/configuration.md`**

Add Zitadel config section:
- `APP__ZITADEL__AUTHORITY`
- `APP__ZITADEL__CLIENT_ID`
- `APP__ZITADEL__CLIENT_SECRET`
- `USER_SERVICE__ZITADEL_AUTHORITY`
- `USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN`

Remove JWT configuration docs.

- [ ] **Step 3: Update config example**

Update `apps/api-gateway/config.example.toml`:

```toml
[app]
host = "127.0.0.1"
port = 8080
metrics_port = 60080

[db]
url = "postgres://localhost/pim"

[zitadel]
authority = "https://my-instance.zitadel.cloud"
client_id = "your-api-app-client-id@your-project"
client_secret = "your-api-app-client-secret"
```

- [ ] **Step 4: Clean up stale code**

Verify no stale references:
- `grep -r "JwtManager" apps/ libs/` should return nothing
- `grep -r "JwtAuth" apps/ libs/` should return nothing
- `grep -r "jsonwebtoken" apps/ libs/` should return nothing

Remove `apps/api-gateway/src/services/` if unused after refactor.

- [ ] **Step 5: Final workspace build**

Run: `cargo build --workspace`

Expected: PASS with no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "docs: update design and configuration for Zitadel auth integration"
```

---

## Acceptance Criteria

### Phase 1
- [ ] API Gateway validates tokens via Zitadel Token Introspection (`IntrospectedUser` extractor)
- [ ] Protected routes (`/api/v1/users/*`, `/api/v1/auth/userinfo`) reject requests without valid Bearer token
- [ ] Unprotected routes (`/health`) work without Bearer token
- [ ] Configuration loads Zitadel settings from environment variables
- [ ] `cargo build -p api-gateway` succeeds with no warnings
- [ ] Existing telemetry/metrics infrastructure is preserved

### Phase 2
- [ ] `auth-service` directory and auth proto are removed
- [ ] `cargo check --workspace` passes cleanly
- [ ] No dangling references to auth proto or auth-service

### Phase 3
- [ ] `GetUser`, `ListUsers`, `GetCurrentUser` return real data from Zitadel
- [ ] `UpdateUser`, `DeleteUser` modify real data in Zitadel
- [ ] Service authenticates to Zitadel API via service account PAT
- [ ] Error mapping from Zitadel API errors to gRPC status codes

### Phase 4
- [ ] `docs/design.md` accurately reflects the new architecture
- [ ] `docs/configuration.md` documents all Zitadel settings
- [ ] No stale auth DTOs, error types, or commented-out code remains
- [ ] `cargo build --workspace` passes with no warnings

---

## Status

| Phase | Status |
|-------|--------|
| Phase 1: Gateway Token Introspection | Complete |
| Phase 2: Remove auth-service | Complete |
| Phase 3: User Service -> Zitadel Proxy | Complete |
| Phase 4: Documentation & Cleanup | Complete |
