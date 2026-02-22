/// Shared validation and ToR boundary check helpers for meeting handlers.
///
/// This module centralizes:
/// - ToR boundary validation (critical security check)
/// - Meeting ownership verification
/// - Date validation
/// - Common error handling patterns

use sqlx::PgPool;
use crate::errors::AppError;
use crate::models::meeting;

/// Validates that a meeting belongs to the requested ToR.
///
/// This is a critical security check: prevents users with permission on ToR A
/// from modifying meetings belonging to ToR B.
///
/// # Errors
/// - `AppError::NotFound` if meeting not found or doesn't belong to tor_id
pub async fn validate_meeting_tor_ownership(
    pool: &PgPool,
    meeting_id: i64,
    tor_id: i64,
) -> Result<(), AppError> {
    let meeting = meeting::find_by_id(pool, meeting_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if meeting.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    Ok(())
}

/// Validates that a date is in the future (useful for meeting scheduling).
///
/// # Arguments
/// * `date_str` - Date string in format "YYYY-MM-DD"
///
/// # Errors
/// - `AppError::PermissionDenied` if date is in the past or today
pub fn validate_future_date(date_str: &str) -> Result<(), AppError> {
    let parsed_date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| AppError::PermissionDenied("Invalid date format, expected YYYY-MM-DD".to_string()))?;

    let today = chrono::Local::now().naive_local().date();
    if parsed_date <= today {
        return Err(AppError::PermissionDenied(
            "Cannot confirm meetings in the past".to_string(),
        ));
    }

    Ok(())
}

/// Parses and validates a date string.
///
/// # Errors
/// - `AppError::PermissionDenied` if date format is invalid
pub fn parse_and_validate_date(date_str: &str) -> Result<chrono::NaiveDate, AppError> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| AppError::PermissionDenied("Invalid date format, expected YYYY-MM-DD".to_string()))
}
