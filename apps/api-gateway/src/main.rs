use actix_web::{web, App, HttpServer};
use api_gateway::config::load_app_config;
use api_gateway::middlewares::{HttpMetrics, RequestId, RequestLogging};
use api_gateway::router::configure_routes;
use infra_telemetry as telemetry;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_gateway=info,actix_web=info,common=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config =
        load_app_config().map_err(|e| std::io::Error::other(format!("Failed to load configuration: {}", e)))?;
    let bind_address = config.bind_address();

    // Initialize Prometheus metrics recorder
    // Env is sourced from infra-config (not read inside the library)
    match telemetry::install_prometheus(
        telemetry::PrometheusOptions::new(env!("CARGO_PKG_NAME")).env(config.settings.common.app_env.clone()),
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
    let config_data = web::Data::new(config);

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            // Add shared application state
            .app_data(config_data.clone())
            // Add middlewares
            .wrap(HttpMetrics)
            .wrap(RequestLogging)
            .wrap(RequestId)
            // Configure routes
            .configure(configure_routes)
    })
    .bind(&bind_address)?
    .run()
    .await
}
