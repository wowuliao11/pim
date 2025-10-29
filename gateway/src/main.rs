use actix_web::{get, web, App, HttpServer, Responder};

#[get("/{name}")]
async fn hi(name: web::Path<String>) -> impl Responder {
    format!("Hi {name}!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(hi))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
