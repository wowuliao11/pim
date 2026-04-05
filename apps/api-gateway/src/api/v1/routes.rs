use actix_web::web;

use super::handlers;

pub fn configure() -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.service(web::scope("/auth").route("/userinfo", web::get().to(handlers::auth::userinfo)))
            .service(
                web::scope("/users")
                    .route("/me", web::get().to(handlers::user::get_current_user))
                    .route("", web::get().to(handlers::user::list_users))
                    .route("/{id}", web::get().to(handlers::user::get_user)),
            );
    }
}
