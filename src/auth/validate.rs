/// Validate a username: 2-50 chars, alphanumeric and underscore only.
pub fn validate_username(username: &str) -> Option<String> {
    let trimmed = username.trim();
    if trimmed.is_empty() {
        return Some("Username is required".to_string());
    }
    if trimmed.len() < 2 {
        return Some("Username must be at least 2 characters".to_string());
    }
    if trimmed.len() > 50 {
        return Some("Username must be at most 50 characters".to_string());
    }
    if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Some("Username may only contain letters, numbers, and underscores".to_string());
    }
    None
}

/// Validate an email: must contain '@' and '.', max 254 chars.
pub fn validate_email(email: &str) -> Option<String> {
    let trimmed = email.trim();
    if trimmed.is_empty() {
        return Some("Email is required".to_string());
    }
    if trimmed.len() > 254 {
        return Some("Email must be at most 254 characters".to_string());
    }
    if !trimmed.contains('@') || !trimmed.contains('.') {
        return Some("Email must be a valid address (contain '@' and '.')".to_string());
    }
    None
}

/// Validate a password: min 8 chars on create.
pub fn validate_password(password: &str) -> Option<String> {
    if password.is_empty() {
        return Some("Password is required".to_string());
    }
    if password.len() < 8 {
        return Some("Password must be at least 8 characters".to_string());
    }
    None
}

/// Validate a required text field with a max length.
pub fn validate_required(value: &str, field_name: &str, max_len: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(format!("{field_name} is required"));
    }
    if trimmed.len() > max_len {
        return Some(format!("{field_name} must be at most {max_len} characters"));
    }
    None
}

/// Validate an optional text field with a max length (empty is OK).
pub fn validate_optional(value: &str, field_name: &str, max_len: usize) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.is_empty() && trimmed.len() > max_len {
        return Some(format!("{field_name} must be at most {max_len} characters"));
    }
    None
}
