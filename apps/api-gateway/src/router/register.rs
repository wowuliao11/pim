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
