use actix_session::Session;
use actix_web::HttpResponse;
use rand::Rng;

/// Get the CSRF token from the session, or generate a new one.
pub fn get_or_create_token(session: &Session) -> String {
    if let Ok(Some(token)) = session.get::<String>("csrf_token") {
        return token;
    }
    let token = generate_token();
    let _ = session.insert("csrf_token", &token);
    token
}

/// Validate the submitted CSRF token against the session token.
/// Returns Ok(()) if valid, or an HTTP 403 response if invalid.
pub fn validate_csrf(session: &Session, submitted: &str) -> Result<(), HttpResponse> {
    let stored = session
        .get::<String>("csrf_token")
        .unwrap_or(None)
        .unwrap_or_default();
    if stored.is_empty() || !constant_time_eq(&stored, submitted) {
        return Err(HttpResponse::Forbidden().body("Invalid or missing CSRF token"));
    }
    Ok(())
}

/// Generate a random 32-byte hex token.
fn generate_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    hex::encode(bytes)
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
