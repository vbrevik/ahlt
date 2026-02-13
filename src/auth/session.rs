use actix_session::Session;
use actix_web::HttpResponse;

/// Wrapper around permission codes with a `has()` method for use in Askama templates.
#[derive(Debug, Clone, Default)]
pub struct Permissions(pub Vec<String>);

impl Permissions {
    pub fn has(&self, code: &str) -> bool {
        self.0.iter().any(|p| p == code)
    }
}

pub fn get_user_id(session: &Session) -> Option<i64> {
    session.get::<i64>("user_id").unwrap_or(None)
}

pub fn get_username(session: &Session) -> String {
    session.get::<String>("username").unwrap_or(None).unwrap_or_default()
}

pub fn get_permissions(session: &Session) -> Permissions {
    let codes = session
        .get::<String>("permissions")
        .unwrap_or(None)
        .map(|csv| csv.split(',').filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    Permissions(codes)
}

pub fn take_flash(session: &Session) -> Option<String> {
    let flash = session.get::<String>("flash").unwrap_or(None);
    if flash.is_some() {
        session.remove("flash");
    }
    flash
}

/// Check permission; returns Err(HttpResponse redirect) if denied.
pub fn require_permission(session: &Session, code: &str) -> Result<(), HttpResponse> {
    if get_permissions(session).has(code) {
        Ok(())
    } else {
        let _ = session.insert("flash", "Access denied: insufficient permissions");
        Err(HttpResponse::SeeOther()
            .insert_header(("Location", "/dashboard"))
            .finish())
    }
}
