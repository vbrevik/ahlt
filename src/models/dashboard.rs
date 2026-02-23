use sqlx::PgPool;
use crate::models::tor;

// Re-export UserTorMembership as the canonical type for dashboard consumers
pub use crate::models::tor::types::UserTorMembership;

// ---------- Types ----------

/// A compact upcoming meeting for the dashboard.
#[derive(Debug, Clone, Default)]
pub struct UpcomingMeeting {
    pub tor_id: i64,
    pub tor_label: String,
    pub date: String,           // YYYY-MM-DD
    pub start_time: String,     // HH:MM
    pub duration_minutes: i64,
    pub location: String,
    pub meeting_id: Option<i64>,
}

/// Aggregated pending items for the dashboard "needs attention" panel.
#[derive(Debug, Clone, Default)]
pub struct PendingItems {
    pub unread_warnings: Vec<PendingWarning>,
    pub pending_proposals: Vec<PendingProposal>,
    pub open_suggestions: Vec<PendingSuggestion>,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct PendingWarning {
    pub warning_id: i64,
    pub receipt_id: i64,
    pub severity: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct PendingProposal {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub tor_label: String,
    pub submitted_by: String,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct PendingSuggestion {
    pub id: i64,
    pub description_preview: String,
    pub tor_label: String,
    pub submitted_by: String,
}

// ---------- Queries ----------

/// Find all ToRs where the given user fills a position.
/// Delegates to the canonical implementation in tor::queries.
pub async fn find_user_tors(pool: &PgPool, user_id: i64) -> Vec<UserTorMembership> {
    tor::find_user_tors(pool, user_id).await
}

/// Find upcoming meetings (next N days) for ToRs the user belongs to.
/// Uses the calendar computation engine, then filters to user's ToRs.
pub async fn find_upcoming_meetings(pool: &PgPool, user_id: i64, days: i64) -> Vec<UpcomingMeeting> {
    use chrono::{Local, Duration};
    use crate::models::tor::calendar;

    let today = Local::now().date_naive();
    let end = today + Duration::days(days);

    // Get user's ToR IDs via shared query
    let tor_ids = tor::find_tor_ids_for_user(pool, user_id).await;

    if tor_ids.is_empty() {
        return Vec::new();
    }

    // Compute all meetings in range, then filter to user's ToRs
    let all_events = match calendar::compute_meetings(pool, today, end).await {
        Ok(events) => events,
        Err(_) => return Vec::new(),
    };
    let mut meetings: Vec<UpcomingMeeting> = all_events
        .into_iter()
        .filter(|e| tor_ids.contains(&e.tor_id))
        .map(|e| UpcomingMeeting {
            tor_id: e.tor_id,
            tor_label: e.tor_label,
            date: e.date,
            start_time: e.start_time,
            duration_minutes: e.duration_minutes,
            location: e.location,
            meeting_id: e.meeting_id,
        })
        .collect();

    // Sort by date then time, limit to 8
    meetings.sort_by(|a, b| (&a.date, &a.start_time).cmp(&(&b.date, &b.start_time)));
    meetings.truncate(8);
    meetings
}

/// Find pending items that need the user's attention.
pub async fn find_pending_items(pool: &PgPool, user_id: i64) -> PendingItems {
    // Pre-fetch user's ToR IDs once, shared by proposals and suggestions queries
    let tor_ids = tor::find_tor_ids_for_user(pool, user_id).await;

    let unread_warnings = find_unread_warnings(pool, user_id).await;
    let pending_proposals = find_pending_proposals_for_tors(pool, &tor_ids).await;
    let open_suggestions = find_open_suggestions_for_tors(pool, &tor_ids).await;

    PendingItems {
        unread_warnings,
        pending_proposals,
        open_suggestions,
    }
}

/// Top 5 unread warnings for the user.
async fn find_unread_warnings(pool: &PgPool, user_id: i64) -> Vec<PendingWarning> {
    sqlx::query_as::<_, PendingWarning>(
        "SELECT w.id AS warning_id, receipt.id AS receipt_id, \
                COALESCE(p_sev.value, 'info') AS severity, \
                COALESCE(p_msg.value, '') AS message, \
                w.created_at::TEXT AS created_at \
         FROM entities receipt \
         JOIN relations r_warn ON receipt.id = r_warn.source_id \
         JOIN entities w ON r_warn.target_id = w.id \
         JOIN relations r_user ON receipt.id = r_user.source_id \
         LEFT JOIN entity_properties p_status ON receipt.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_sev ON w.id = p_sev.entity_id AND p_sev.key = 'severity' \
         LEFT JOIN entity_properties p_msg ON w.id = p_msg.entity_id AND p_msg.key = 'message' \
         WHERE receipt.entity_type = 'warning_receipt' \
           AND w.entity_type = 'warning' \
           AND r_warn.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'for_warning') \
           AND r_user.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'for_user') \
           AND r_user.target_id = $1 \
           AND COALESCE(p_status.value, 'unread') = 'unread' \
         ORDER BY w.created_at DESC \
         LIMIT 5"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Pending proposals (submitted or under_review) across given ToR IDs.
async fn find_pending_proposals_for_tors(pool: &PgPool, tor_ids: &[i64]) -> Vec<PendingProposal> {
    if tor_ids.is_empty() {
        return Vec::new();
    }

    // Build dynamic IN clause for tor IDs
    let placeholders: Vec<String> = tor_ids.iter().enumerate()
        .map(|(i, _)| format!("${}", i + 1))
        .collect();
    let in_clause = placeholders.join(", ");

    let sql = format!(
        "SELECT p.id, p.label AS title, \
                COALESCE(p_status.value, '') AS status, \
                COALESCE(tor.label, '') AS tor_label, \
                COALESCE(p_sub.value, '') AS submitted_by \
         FROM entities p \
         LEFT JOIN entity_properties p_status ON p.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN relations r_tor ON p.id = r_tor.source_id \
            AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'submitted_to') \
         LEFT JOIN entities tor ON r_tor.target_id = tor.id \
         LEFT JOIN entity_properties p_sub ON p.id = p_sub.entity_id AND p_sub.key = 'submitted_by_name' \
         WHERE p.entity_type = 'proposal' \
           AND COALESCE(p_status.value, '') IN ('submitted', 'under_review') \
           AND tor.id IN ({in_clause}) \
         ORDER BY p.created_at DESC \
         LIMIT 5"
    );

    let mut query = sqlx::query_as::<_, PendingProposal>(&sql);
    for id in tor_ids {
        query = query.bind(*id);
    }
    query.fetch_all(pool).await.unwrap_or_default()
}

/// Open suggestions across given ToR IDs.
async fn find_open_suggestions_for_tors(pool: &PgPool, tor_ids: &[i64]) -> Vec<PendingSuggestion> {
    if tor_ids.is_empty() {
        return Vec::new();
    }

    // Build dynamic IN clause for tor IDs
    let placeholders: Vec<String> = tor_ids.iter().enumerate()
        .map(|(i, _)| format!("${}", i + 1))
        .collect();
    let in_clause = placeholders.join(", ");

    let sql = format!(
        "SELECT s.id, \
                COALESCE(LEFT(p_desc.value, 80), s.label) AS description_preview, \
                COALESCE(tor.label, '') AS tor_label, \
                COALESCE(p_sub.value, '') AS submitted_by \
         FROM entities s \
         LEFT JOIN entity_properties p_status ON s.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_desc ON s.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN relations r_tor ON s.id = r_tor.source_id \
            AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'suggested_to') \
         LEFT JOIN entities tor ON r_tor.target_id = tor.id \
         LEFT JOIN entity_properties p_sub ON s.id = p_sub.entity_id AND p_sub.key = 'submitted_by_name' \
         WHERE s.entity_type = 'suggestion' \
           AND COALESCE(p_status.value, '') = 'open' \
           AND tor.id IN ({in_clause}) \
         ORDER BY s.created_at DESC \
         LIMIT 5"
    );

    let mut query = sqlx::query_as::<_, PendingSuggestion>(&sql);
    for id in tor_ids {
        query = query.bind(*id);
    }
    query.fetch_all(pool).await.unwrap_or_default()
}
