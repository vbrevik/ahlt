use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, middleware, web};

use ahlt::{audit, auth, db, handlers, warnings};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Determine environment and data directory
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    let data_dir = format!("data/{}", app_env);
    log::info!("Environment: {} | Data directory: {}", app_env, data_dir);

    // Ensure data directory exists
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    // Initialize database
    let db_path = format!("{}/app.db", data_dir);
    let pool = db::init_pool(&db_path);
    db::run_migrations(&pool);

    // Seed data based on environment
    let admin_hash = auth::password::hash_password("admin123")
        .expect("Failed to hash default password");
    match app_env.as_str() {
        "staging" => db::seed_staging(&pool, &admin_hash),
        _ => db::seed_ontology(&pool, &admin_hash),
    }

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

    // Server binding configuration
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let cookie_secure = std::env::var("COOKIE_SECURE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    log::info!("Starting server at http://{}:{}", host, port);
    if cookie_secure {
        log::info!("Cookie secure flag enabled (requires HTTPS)");
    }

    let bind_addr = format!("{}:{}", host, port);

    // Login rate limiter (per-IP, in-memory)
    let rate_limiter = auth::rate_limit::RateLimiter::new();

    // WebSocket connection map for real-time warning notifications
    let conn_map = handlers::warning_handlers::ws::new_connection_map();

    // Spawn background scheduler for warning generators and cleanup
    warnings::scheduler::spawn_scheduler(pool.clone(), conn_map.clone(), data_dir.clone());

    HttpServer::new(move || {
        let session_mw = SessionMiddleware::builder(
            CookieSessionStore::default(),
            secret_key.clone(),
        )
        .cookie_secure(cookie_secure)
        .cookie_http_only(true)
        .build();

        App::new()
            .wrap(session_mw)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(conn_map.clone()))
            .app_data(web::Data::new(rate_limiter.clone()))
            // Static files
            .service(actix_files::Files::new("/static", "./static"))
            // WebSocket route (before auth middleware scope)
            .route("/ws/notifications", web::get().to(handlers::warning_handlers::ws::ws_connect))
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
                    // Data Manager — import/export API + admin page
                    .route("/data-manager", web::get().to(handlers::data_handlers::data_manager_page))
                    .route("/api/data/import", web::post().to(handlers::data_handlers::import_data))
                    .route("/api/data/export", web::get().to(handlers::data_handlers::export_data))
                    .route("/api/data/schema", web::get().to(handlers::data_handlers::schema))
                    // API v1 — REST endpoints for external integrations
                    .service(web::scope("/api/v1").configure(handlers::api_v1::configure))
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
                    // Role Builder — specific routes BEFORE /roles/{id}
                    .route("/roles/builder", web::get().to(handlers::role_builder_handlers::wizard_form))
                    .route("/roles/builder/preview", web::post().to(handlers::role_builder_handlers::preview_menu))
                    .route("/roles/builder/create", web::post().to(handlers::role_builder_handlers::create_role))
                    .route("/roles/builder/update", web::post().to(handlers::role_builder_handlers::update_role))
                    .route("/roles/builder/{id}/edit", web::get().to(handlers::role_builder_handlers::edit_form))
                    .route("/roles/{id}/delete", web::post().to(handlers::role_handlers::delete))
                    // Governance map — before parameterized /tor/{id} routes
                    .route("/governance/map", web::get().to(handlers::governance_handlers::governance_map))
                    .route("/api/governance/graph", web::get().to(handlers::governance_handlers::governance_graph_api))
                    // Workflow builder — BEFORE /workflow to avoid path conflict
                    .route("/workflow/builder", web::get().to(handlers::workflow_builder_handlers::list))
                    .route("/workflow/builder/{scope}", web::get().to(handlers::workflow_builder_handlers::detail))
                    .route("/workflow/builder/{scope}/statuses", web::post().to(handlers::workflow_builder_handlers::create_status))
                    .route("/workflow/builder/{scope}/statuses/{id}/update", web::post().to(handlers::workflow_builder_handlers::update_status))
                    .route("/workflow/builder/{scope}/statuses/{id}/delete", web::post().to(handlers::workflow_builder_handlers::delete_status))
                    .route("/workflow/builder/{scope}/transitions", web::post().to(handlers::workflow_builder_handlers::create_transition))
                    .route("/workflow/builder/{scope}/transitions/{id}/update", web::post().to(handlers::workflow_builder_handlers::update_transition))
                    .route("/workflow/builder/{scope}/transitions/{id}/delete", web::post().to(handlers::workflow_builder_handlers::delete_transition))
                    // Workflow index
                    .route("/workflow", web::get().to(handlers::workflow_handlers::index))
                    // ToR CRUD — /tor/new and /tor/outlook BEFORE /tor/{id}
                    .route("/tor", web::get().to(handlers::tor_handlers::list))
                    .route("/tor/new", web::get().to(handlers::tor_handlers::new_form))
                    .route("/tor/outlook", web::get().to(handlers::tor_handlers::outlook))
                    .route("/api/tor/calendar", web::get().to(handlers::tor_handlers::calendar_api))
                    .route("/api/tor/{id}/meetings/confirm-calendar", web::post().to(handlers::meeting_handlers::confirm_calendar))
                    .route("/tor", web::post().to(handlers::tor_handlers::create))
                    .route("/tor/{id}", web::get().to(handlers::tor_handlers::detail))
                    .route("/tor/{id}/edit", web::get().to(handlers::tor_handlers::edit_form))
                    .route("/tor/{id}", web::post().to(handlers::tor_handlers::update))
                    .route("/tor/{id}/delete", web::post().to(handlers::tor_handlers::delete))
                    // ToR member management
                    .route("/tor/{id}/members", web::post().to(handlers::tor_handlers::manage_members))
                    // ToR protocol management
                    .route("/tor/{id}/protocol", web::post().to(handlers::tor_handlers::add_step))
                    .route("/tor/{id}/protocol/{step_id}/delete", web::post().to(handlers::tor_handlers::delete_step))
                    .route("/tor/{id}/protocol/{step_id}/move", web::post().to(handlers::tor_handlers::move_step))
                    // ToR dependency management
                    .route("/tor/{id}/dependencies", web::post().to(handlers::tor_handlers::handle_add_dependency))
                    .route("/tor/{id}/dependencies/{relation_id}/delete", web::post().to(handlers::tor_handlers::handle_remove_dependency))
                    // Presentation template management
                    .route("/tor/{id}/templates", web::get().to(handlers::tor_handlers::list_templates))
                    .route("/tor/{id}/templates", web::post().to(handlers::tor_handlers::create_template))
                    .route("/tor/{id}/templates/{template_id}/delete", web::post().to(handlers::tor_handlers::delete_template))
                    .route("/tor/{id}/templates/{template_id}/slides", web::post().to(handlers::tor_handlers::handle_add_slide))
                    .route("/tor/{id}/templates/{template_id}/slides/{slide_id}/delete", web::post().to(handlers::tor_handlers::handle_delete_slide))
                    .route("/tor/{id}/templates/{template_id}/slides/{slide_id}/move", web::post().to(handlers::tor_handlers::handle_move_slide))
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
                    // Workflow queue
                    .route("/tor/{id}/workflow/queue", web::get().to(handlers::queue_handlers::view_queue))
                    .route("/tor/{id}/workflow/queue/schedule-form", web::get().to(handlers::queue_handlers::schedule_form))
                    .route("/tor/{id}/proposals/{proposal_id}/ready-for-agenda", web::post().to(handlers::queue_handlers::mark_ready))
                    .route("/tor/{id}/proposals/{proposal_id}/unqueue", web::post().to(handlers::queue_handlers::unqueue_proposal))
                    .route("/tor/{id}/workflow/queue/schedule", web::post().to(handlers::queue_handlers::bulk_schedule))
                    // Agenda points — /new BEFORE /{agenda_id}
                    .route("/tor/{id}/workflow/agenda/new", web::get().to(handlers::agenda_handlers::new_form))
                    .route("/tor/{id}/workflow/agenda", web::post().to(handlers::agenda_handlers::create))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}", web::get().to(handlers::agenda_handlers::detail))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/edit", web::get().to(handlers::agenda_handlers::edit_form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}", web::post().to(handlers::agenda_handlers::update))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/transition", web::post().to(handlers::agenda_handlers::transition))
                    // COAs — /new BEFORE /{coa_id}
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/new", web::get().to(handlers::coa_handlers::new_form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa", web::post().to(handlers::coa_handlers::create))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/edit", web::get().to(handlers::coa_handlers::edit_form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}", web::post().to(handlers::coa_handlers::update))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/delete", web::post().to(handlers::coa_handlers::delete))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections", web::post().to(handlers::coa_handlers::add_section))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}", web::post().to(handlers::coa_handlers::update_section))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}/delete", web::post().to(handlers::coa_handlers::delete_section))
                    // Opinions + Decisions
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/input", web::get().to(handlers::opinion_handlers::form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/input", web::post().to(handlers::opinion_handlers::submit))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/decide", web::get().to(handlers::opinion_handlers::decision_form))
                    .route("/tor/{id}/workflow/agenda/{agenda_id}/decide", web::post().to(handlers::opinion_handlers::record_decision))
                    // Minutes management
                    .route("/minutes/generate", web::post().to(handlers::minutes_handlers::generate_minutes))
                    .route("/minutes/{id}", web::get().to(handlers::minutes_handlers::view_minutes))
                    .route("/minutes/{id}/sections/{section_id}", web::post().to(handlers::minutes_handlers::update_section))
                    .route("/minutes/{id}/status", web::post().to(handlers::minutes_handlers::update_minutes_status))
                    // Meeting management — confirm BEFORE {mid} to avoid path param conflict
                    .route("/meetings", web::get().to(handlers::meeting_handlers::list))
                    .route("/tor/{id}/meetings/confirm", web::post().to(handlers::meeting_handlers::confirm))
                    .route("/tor/{id}/meetings/{mid}", web::get().to(handlers::meeting_handlers::detail))
                    .route("/tor/{id}/meetings/{mid}/transition", web::post().to(handlers::meeting_handlers::transition))
                    .route("/tor/{id}/meetings/{mid}/agenda/assign", web::post().to(handlers::meeting_handlers::assign_agenda))
                    .route("/tor/{id}/meetings/{mid}/agenda/remove", web::post().to(handlers::meeting_handlers::remove_agenda))
                    .route("/tor/{id}/meetings/{mid}/minutes/generate", web::post().to(handlers::meeting_handlers::generate_minutes))
                    // Warnings — /warnings before /warnings/{id}
                    .route("/warnings", web::get().to(handlers::warning_handlers::list::list))
                    .route("/warnings/{id}", web::get().to(handlers::warning_handlers::detail::detail))
                    .route("/warnings/{id}/delete", web::post().to(handlers::warning_handlers::actions::mark_deleted))
                    .route("/warnings/{id}/forward", web::post().to(handlers::warning_handlers::actions::forward))
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
    .bind(&bind_addr)?
    .run()
    .await
}
