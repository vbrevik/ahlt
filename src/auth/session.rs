use actix_session::Session;
use crate::errors::AppError;

/// Wrapper around permission codes with a `has()` method for use in Askama templates.
#[derive(Debug, Clone, Default)]
pub struct Permissions(pub Vec<String>);

impl Permissions {
    pub fn has(&self, code: &str) -> bool {
        self.0.iter().any(|p| p == code)
    }

    pub fn from_csv(csv: &str) -> Self {
        let codes = csv
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Permissions(codes)
    }
}

pub fn get_user_id(session: &Session) -> Option<i64> {
    session.get::<i64>("user_id").unwrap_or(None)
}

pub fn get_username(session: &Session) -> Result<String, String> {
    match session.get::<String>("username") {
        Ok(Some(username)) => Ok(username),
        Ok(None) => Err("No username in session".to_string()),
        Err(e) => Err(format!("Session error: {}", e)),
    }
}

pub fn get_permissions(session: &Session) -> Result<Permissions, String> {
    match session.get::<String>("permissions") {
        Ok(Some(csv)) => Ok(Permissions::from_csv(&csv)),
        Ok(None) => Err("No permissions in session".to_string()),
        Err(e) => Err(format!("Session error: {}", e)),
    }
}

pub fn take_flash(session: &Session) -> Option<String> {
    let flash = session.get::<String>("flash").unwrap_or(None);
    if flash.is_some() {
        session.remove("flash");
    }
    flash
}

/// Check permission; returns Err(AppError) if denied.
pub fn require_permission(session: &Session, code: &str) -> Result<(), AppError> {
    let permissions = get_permissions(session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;

    if permissions.has(code) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(code.to_string()))
    }
}
