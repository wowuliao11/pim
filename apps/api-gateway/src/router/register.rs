use std::sync::Arc;

use actix_web::web;
use infra_auth::JwtManager;

use crate::api;

pub fn configure_routes(jwt_manager: Arc<JwtManager>) -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.route("/health", web::get().to(health_check))
            .service(web::scope("/api/v1").configure(api::v1::configure(jwt_manager)));
    }
}

async fn health_check() -> &'static str {
    "OK"
}
