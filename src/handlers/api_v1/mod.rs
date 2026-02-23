pub mod entities;
pub mod users;

use actix_web::{
    web, Error, HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};

/// CSRF protection for REST API mutation endpoints.
///
/// Rejects POST/PUT/DELETE requests that don't have Content-Type: application/json.
/// Browsers cannot send cross-origin JSON with cookies via simple form POST —
/// the Content-Type check acts as a CSRF guard without requiring tokens.
/// GET requests are exempt (read-only, no state changes).
async fn require_json_content_type(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let method = req.method().clone();

    if method == actix_web::http::Method::POST
        || method == actix_web::http::Method::PUT
        || method == actix_web::http::Method::DELETE
    {
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("application/json") {
            let body = serde_json::json!({
                "error": "Content-Type must be application/json for mutation requests"
            });
            let response = HttpResponse::BadRequest().json(body);
            return Ok(req.into_response(response).map_into_right_body());
        }
    }

    next.call(req).await.map(|res| res.map_into_left_body())
}

/// Configure API v1 routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/entities")
            .wrap(actix_web::middleware::from_fn(require_json_content_type))
            .route("", web::get().to(entities::list))
            .route("", web::post().to(entities::create))
            .route("/{id}", web::get().to(entities::read))
            .route("/{id}", web::put().to(entities::update))
            .route("/{id}", web::delete().to(entities::delete))
    );
    cfg.service(
        web::scope("/users")
            .wrap(actix_web::middleware::from_fn(require_json_content_type))
            .route("", web::get().to(users::list))
            .route("", web::post().to(users::create))
            .route("/{id}", web::get().to(users::read))
            .route("/{id}", web::put().to(users::update))
            .route("/{id}", web::delete().to(users::delete))
    );
    cfg.service(
        web::scope("/user")
            .wrap(actix_web::middleware::from_fn(require_json_content_type))
            .route("/theme", web::post().to(users::update_theme))
    );
}
