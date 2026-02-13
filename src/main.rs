use actix_files;
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, middleware, web};

mod auth;
mod db;
mod errors;
mod handlers;
mod models;
mod templates_structs;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Ensure data directory exists
    std::fs::create_dir_all("data").expect("Failed to create data directory");

    // Initialize database
    let pool = db::init_pool("data/app.db");
    db::run_migrations(&pool);

    // Seed ontology (relation types, roles, permissions, admin user) if empty
    let admin_hash = auth::password::hash_password("admin123")
        .expect("Failed to hash default password");
    db::seed_ontology(&pool, &admin_hash);

    // Session encryption key
    // TODO: In production, load from environment variable for persistent sessions across restarts
    let secret_key = Key::generate();

    log::info!("Starting server at http://127.0.0.1:8080");

    HttpServer::new(move || {
        let session_mw = SessionMiddleware::builder(
            CookieSessionStore::default(),
            secret_key.clone(),
        )
        .cookie_secure(false)
        .cookie_http_only(true)
        .build();

        App::new()
            .wrap(session_mw)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(pool.clone()))
            // Static files
            .service(actix_files::Files::new("/static", "./static"))
            // Public routes
            .route("/login", web::get().to(handlers::auth_handlers::login_page))
            .route("/login", web::post().to(handlers::auth_handlers::login_submit))
            // Root redirect
            .route("/", web::get().to(|| async {
                actix_web::HttpResponse::SeeOther()
                    .insert_header(("Location", "/dashboard"))
                    .finish()
            }))
            // Protected routes
            .service(
                web::scope("")
                    .wrap(actix_web::middleware::from_fn(auth::middleware::require_auth))
                    .route("/dashboard", web::get().to(handlers::dashboard::index))
                    .route("/logout", web::post().to(handlers::auth_handlers::logout))
                    // User CRUD — /users/new BEFORE /users/{id} to avoid routing conflict
                    .route("/users", web::get().to(handlers::user_handlers::list))
                    .route("/users/new", web::get().to(handlers::user_handlers::new_form))
                    .route("/users", web::post().to(handlers::user_handlers::create))
                    .route("/users/{id}/edit", web::get().to(handlers::user_handlers::edit_form))
                    .route("/users/{id}", web::post().to(handlers::user_handlers::update))
                    .route("/users/{id}/delete", web::post().to(handlers::user_handlers::delete))
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
