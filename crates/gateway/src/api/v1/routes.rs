use actix_web::web;

use super::handlers;

/// Configure v1 API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // Auth routes (public)
        .service(
            web::scope("/auth")
                .route("/login", web::post().to(handlers::auth::login))
                .route("/register", web::post().to(handlers::auth::register)),
        )
        // User routes (protected - add middleware in handlers)
        .service(
            web::scope("/users")
                .route("", web::get().to(handlers::user::list_users))
                .route("/{id}", web::get().to(handlers::user::get_user))
                .route("/me", web::get().to(handlers::user::get_current_user)),
        );
}
