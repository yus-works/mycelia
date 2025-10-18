use actix_files::Files;
use actix_web::{get, middleware::Logger, web, App, HttpServer, Responder};

#[get("/hello")]
async fn greet() -> impl Responder {
    format!("Hello!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(|| {
        App::new()
            .service(greet)
            .service(Files::new("/pages", "static/pages/").show_files_listing())
            .service(Files::new("/", "static/").index_file("index.html"))
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
