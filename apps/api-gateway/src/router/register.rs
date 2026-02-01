use actix_web::{web, HttpResponse};

use common::telemetry;

use crate::api;

/// Configure all application routes
/// This is the central place for route registration
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Health check endpoint
        .route("/health", web::get().to(health_check))
    // Prometheus metrics
    .route("/metrics", web::get().to(metrics))
        // API v1 routes
        .service(web::scope("/api/v1").configure(api::v1::configure));
}

/// Health check handler
async fn health_check() -> &'static str {
    "OK"
}

async fn metrics() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(telemetry::render())
}
