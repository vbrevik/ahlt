use sqlx::PgPool;

// ---------- Types ----------

/// A ToR the current user fills a position in.
#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct UserTor {
    pub tor_id: i64,
    pub tor_name: String,
    pub tor_label: String,
    pub position_label: String,
}

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
/// Chain: user --(fills_position)--> tor_function --(belongs_to_tor)--> tor
pub async fn find_user_tors(pool: &PgPool, user_id: i64) -> Vec<UserTor> {
    sqlx::query_as::<_, UserTor>(
        "SELECT DISTINCT tor.id AS tor_id, tor.name AS tor_name, tor.label AS tor_label, \
                f.label AS position_label \
         FROM entities tor \
         JOIN relations r_tor ON tor.id = r_tor.target_id \
         JOIN entities f ON r_tor.source_id = f.id \
         JOIN relations r_fills ON f.id = r_fills.target_id \
         WHERE tor.entity_type = 'tor' \
           AND f.entity_type = 'tor_function' \
           AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND r_fills.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND r_fills.source_id = $1 \
         ORDER BY tor.label"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Find upcoming meetings (next N days) for ToRs the user belongs to.
/// Uses the calendar computation engine, then filters to user's ToRs.
pub async fn find_upcoming_meetings(pool: &PgPool, user_id: i64, days: i64) -> Vec<UpcomingMeeting> {
    use chrono::{Local, Duration};
    use crate::models::tor::calendar;

    let today = Local::now().date_naive();
    let end = today + Duration::days(days);

    // Get user's ToR IDs
    let user_tors = find_user_tors(pool, user_id).await;
    let tor_ids: Vec<i64> = user_tors.iter().map(|t| t.tor_id).collect();

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
    let unread_warnings = find_unread_warnings(pool, user_id).await;
    let pending_proposals = find_pending_proposals(pool, user_id).await;
    let open_suggestions = find_open_suggestions(pool, user_id).await;

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

/// Pending proposals (submitted or under_review) across user's ToRs.
async fn find_pending_proposals(pool: &PgPool, user_id: i64) -> Vec<PendingProposal> {
    sqlx::query_as::<_, PendingProposal>(
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
           AND tor.id IN ( \
               SELECT DISTINCT tor2.id \
               FROM entities tor2 \
               JOIN relations r_bt ON tor2.id = r_bt.target_id \
               JOIN entities f ON r_bt.source_id = f.id \
               JOIN relations r_fp ON f.id = r_fp.target_id \
               WHERE tor2.entity_type = 'tor' \
                 AND f.entity_type = 'tor_function' \
                 AND r_bt.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                 AND r_fp.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
                 AND r_fp.source_id = $1 \
           ) \
         ORDER BY p.created_at DESC \
         LIMIT 5"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Open suggestions across user's ToRs.
async fn find_open_suggestions(pool: &PgPool, user_id: i64) -> Vec<PendingSuggestion> {
    sqlx::query_as::<_, PendingSuggestion>(
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
           AND tor.id IN ( \
               SELECT DISTINCT tor2.id \
               FROM entities tor2 \
               JOIN relations r_bt ON tor2.id = r_bt.target_id \
               JOIN entities f ON r_bt.source_id = f.id \
               JOIN relations r_fp ON f.id = r_fp.target_id \
               WHERE tor2.entity_type = 'tor' \
                 AND f.entity_type = 'tor_function' \
                 AND r_bt.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                 AND r_fp.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
                 AND r_fp.source_id = $1 \
           ) \
         ORDER BY s.created_at DESC \
         LIMIT 5"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}
