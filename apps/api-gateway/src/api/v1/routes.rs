use std::sync::Arc;

use actix_web::web;
use infra_auth::JwtManager;

use super::handlers;
use crate::middlewares::JwtAuth;

pub fn configure(jwt_manager: Arc<JwtManager>) -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("/auth")
                .route("/login", web::post().to(handlers::auth::login))
                .route("/register", web::post().to(handlers::auth::register)),
        )
        .service(
            web::scope("/users")
                .wrap(JwtAuth::new(jwt_manager))
                .route("/me", web::get().to(handlers::user::get_current_user))
                .route("", web::get().to(handlers::user::list_users))
                .route("/{id}", web::get().to(handlers::user::get_user)),
        );
    }
}
