use std::sync::Arc;

use actix_web::{web, App, HttpServer};
use api_gateway::config::load_app_config;
use api_gateway::middlewares::{HttpMetrics, RequestId, RequestLogging};
use api_gateway::router::configure_routes;
use infra_auth::JwtManager;
use infra_telemetry as telemetry;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    telemetry::init_tracing("api_gateway=info,actix_web=info,common=info");

    // Load configuration
    let config =
        load_app_config().map_err(|e| std::io::Error::other(format!("Failed to load configuration: {}", e)))?;
    let bind_address = config.bind_address();

    // Initialize Prometheus metrics recorder
    // Env is sourced from infra-config (not read inside the library)
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

    tracing::info!("Starting {} server at http://{}", config.app_name(), bind_address);

    // wrap config in web::Data once (internally Arc), then clone cheaply in closure
    let jwt_manager = Arc::new(JwtManager::new(
        config.jwt_secret().to_owned(),
        config.jwt_expiration_hours(),
    ));
    let jwt_data = web::Data::from(jwt_manager.clone());
    let config_data = web::Data::new(config);

    HttpServer::new(move || {
        App::new()
            .app_data(config_data.clone())
            .app_data(jwt_data.clone())
            .wrap(HttpMetrics)
            .wrap(RequestLogging)
            .wrap(RequestId)
            .configure(configure_routes(jwt_manager.clone()))
    })
    .bind(&bind_address)?
    .run()
    .await
}
