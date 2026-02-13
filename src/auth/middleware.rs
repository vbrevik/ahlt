use actix_session::SessionExt;
use actix_web::{
    Error, HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};

/// Middleware function that checks for an authenticated session.
/// Redirects to /login if no session found.
pub async fn require_auth(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let session = req.get_session();
    let has_user = session.get::<i64>("user_id").unwrap_or(None).is_some();

    if !has_user {
        let response = HttpResponse::SeeOther()
            .insert_header(("Location", "/login"))
            .finish();
        return Ok(req.into_response(response).map_into_right_body());
    }

    next.call(req).await.map(|res| res.map_into_left_body())
}
