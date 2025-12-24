use actix_web::{web, App, HttpServer};
use gateway::config::load_app_config;
use gateway::middlewares::RequestId;
use gateway::router::configure_routes;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gateway=debug,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = load_app_config();
    let bind_address = config.bind_address();

    tracing::info!(
        "Starting {} server at http://{}",
        config.app_name(),
        bind_address
    );

    // wrap config in web::Data once (internally Arc), then clone cheaply in closure
    let config_data = web::Data::new(config);

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            // Add shared application state
            .app_data(config_data.clone())
            // Add middlewares
            .wrap(RequestId)
            .wrap(tracing_actix_web::TracingLogger::default())
            // Configure routes
            .configure(configure_routes)
    })
    .bind(&bind_address)?
    .run()
    .await
}
