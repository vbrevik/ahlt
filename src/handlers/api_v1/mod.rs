pub mod entities;
pub mod users;

use actix_web::web;

/// Configure API v1 routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/entities")
            .route("", web::get().to(entities::list))
            .route("", web::post().to(entities::create))
            .route("/{id}", web::get().to(entities::read))
            .route("/{id}", web::put().to(entities::update))
            .route("/{id}", web::delete().to(entities::delete))
    );
    cfg.service(
        web::scope("/users")
            .route("", web::get().to(users::list))
            .route("", web::post().to(users::create))
            .route("/{id}", web::get().to(users::read))
            .route("/{id}", web::put().to(users::update))
            .route("/{id}", web::delete().to(users::delete))
    );
    cfg.route("/user/theme", web::post().to(users::update_theme));
}
