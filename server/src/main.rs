use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_web_httpauth::middleware::HttpAuthentication;
use sqlx::sqlite::SqlitePoolOptions; 

mod db;
mod middleware;
mod models;
mod routes;

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("DEBUG: Server is starting...");
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret   = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let allowed_origin = std::env::var("ALLOWED_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let base_url = std::env::var("BASE_URL")
    .unwrap_or_else(|_| format!("http://localhost:{port}"));
    // Thay đổi cách tạo Pool cho SQLite
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to SQLite");

    // Run pending migrations automatically on startup
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    println!("Server running at http://0.0.0.0:{port}");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin(&allowed_origin)
            .allowed_methods(vec!["GET", "POST", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
            ]);

        // JWT bearer middleware
        let auth = HttpAuthentication::bearer(middleware::auth::validator);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(base_url.clone()))
            .app_data(web::Data::new(jwt_secret.clone()))
            // Public routes
            .service(health)
            .service(routes::auth::register)
            .service(routes::auth::login)
            .service(routes::url::shorten)
            .service(routes::url::get_stats)
            .service(routes::url::redirect)
    })
    .bind(format!("0.0.0.0:{port}"))?
    .run()
    .await
}