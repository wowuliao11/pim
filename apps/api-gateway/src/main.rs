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
