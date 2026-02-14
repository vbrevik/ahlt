use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, middleware, web};

mod audit;
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

    // Clean up old audit entries based on retention policy
    {
        let conn = pool.get().expect("Failed to get connection for audit cleanup");
        audit::cleanup_old_entries(&conn);
    }

    // Session encryption key — load from SESSION_KEY env var for persistent sessions across restarts
    let secret_key = match std::env::var("SESSION_KEY") {
        Ok(val) if val.len() >= 64 => {
            log::info!("Using SESSION_KEY from environment");
            Key::from(val.as_bytes())
        }
        Ok(val) => {
            log::warn!("SESSION_KEY too short ({} bytes, need 64+) — generating random key", val.len());
            Key::generate()
        }
        Err(_) => {
            log::warn!("No SESSION_KEY set — generating random key (sessions lost on restart)");
            Key::generate()
        }
    };

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
                    // Role CRUD — /roles/new BEFORE /roles/{id}
                    .route("/roles", web::get().to(handlers::role_handlers::list))
                    .route("/roles/new", web::get().to(handlers::role_handlers::new_form))
                    .route("/roles", web::post().to(handlers::role_handlers::create))
                    .route("/roles/{id}/edit", web::get().to(handlers::role_handlers::edit_form))
                    .route("/roles/{id}", web::post().to(handlers::role_handlers::update))
                    .route("/roles/{id}/delete", web::post().to(handlers::role_handlers::delete))
                    // ToR CRUD — /tor/new BEFORE /tor/{id}
                    .route("/tor", web::get().to(handlers::tor_handlers::list))
                    .route("/tor/new", web::get().to(handlers::tor_handlers::new_form))
                    .route("/tor", web::post().to(handlers::tor_handlers::create))
                    .route("/tor/{id}", web::get().to(handlers::tor_handlers::detail))
                    .route("/tor/{id}/edit", web::get().to(handlers::tor_handlers::edit_form))
                    .route("/tor/{id}", web::post().to(handlers::tor_handlers::update))
                    .route("/tor/{id}/delete", web::post().to(handlers::tor_handlers::delete))
                    // ToR member management
                    .route("/tor/{id}/members", web::post().to(handlers::tor_handlers::manage_members))
                    // Workflow view
                    .route("/tor/{id}/workflow", web::get().to(handlers::workflow_handlers::view))
                    // Suggestion workflow
                    .route("/tor/{id}/suggestions/new", web::get().to(handlers::suggestion_handlers::new_form))
                    .route("/tor/{id}/suggestions", web::post().to(handlers::suggestion_handlers::create))
                    .route("/tor/{id}/suggestions/{suggestion_id}/accept", web::post().to(handlers::suggestion_handlers::accept))
                    .route("/tor/{id}/suggestions/{suggestion_id}/reject", web::post().to(handlers::suggestion_handlers::reject))
                    // Proposal workflow
                    .route("/tor/{id}/proposals/new", web::get().to(handlers::proposal_handlers::new_form))
                    .route("/tor/{id}/proposals", web::post().to(handlers::proposal_handlers::create))
                    .route("/tor/{id}/proposals/{proposal_id}", web::get().to(handlers::proposal_handlers::detail))
                    .route("/tor/{id}/proposals/{proposal_id}/edit", web::get().to(handlers::proposal_handlers::edit_form))
                    .route("/tor/{id}/proposals/{proposal_id}", web::post().to(handlers::proposal_handlers::update))
                    .route("/tor/{id}/proposals/{proposal_id}/submit", web::post().to(handlers::proposal_handlers::submit))
                    .route("/tor/{id}/proposals/{proposal_id}/review", web::post().to(handlers::proposal_handlers::review))
                    .route("/tor/{id}/proposals/{proposal_id}/approve", web::post().to(handlers::proposal_handlers::approve))
                    .route("/tor/{id}/proposals/{proposal_id}/reject", web::post().to(handlers::proposal_handlers::reject))
                    // Opinions + Decisions
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/input", web::get().to(handlers::opinion_handlers::form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/input", web::post().to(handlers::opinion_handlers::submit))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/decide", web::get().to(handlers::opinion_handlers::decision_form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/decide", web::post().to(handlers::opinion_handlers::record_decision))
                    // Account
                    .route("/account", web::get().to(handlers::account_handlers::form))
                    .route("/account", web::post().to(handlers::account_handlers::submit))
                    // Settings
                    .route("/settings", web::get().to(handlers::settings_handlers::list))
                    .route("/settings", web::post().to(handlers::settings_handlers::save))
                    // Menu Builder
                    .route("/menu-builder", web::get().to(handlers::menu_builder_handlers::index))
                    .route("/menu-builder", web::post().to(handlers::menu_builder_handlers::save))
                    // Audit log
                    .route("/audit", web::get().to(handlers::audit_handlers::list))
                    // Ontology explorer — Concepts (schema graph) is the landing page
                    .route("/ontology", web::get().to(handlers::ontology_handlers::graph))
                    .route("/ontology/data", web::get().to(handlers::ontology_handlers::data))
                    .route("/ontology/data/{id}", web::get().to(handlers::ontology_handlers::data_detail))
                    .route("/ontology/reference", web::get().to(handlers::ontology_handlers::concepts))
                    // Ontology JSON APIs
                    .route("/ontology/api/schema", web::get().to(handlers::ontology_handlers::schema_data))
                    .route("/ontology/api/graph", web::get().to(handlers::ontology_handlers::graph_data))
            )
            // Default 404 handler (must be registered last)
            .default_service(web::to(|| async {
                let html = include_str!("../templates/errors/404.html");
                actix_web::HttpResponse::NotFound()
                    .content_type("text/html; charset=utf-8")
                    .body(html)
            }))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
