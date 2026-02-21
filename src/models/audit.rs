use sqlx::PgPool;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub action: String,
    pub target_type: String,
    pub target_id: i64,
    pub summary: String,
    pub created_at: String,
}

pub struct AuditEntryPage {
    pub entries: Vec<AuditEntry>,
    pub page: i64,
    pub per_page: i64,
    pub total_count: i64,
    pub total_pages: i64,
}

/// SQL for audit entry display: entity + properties + username via JOIN
const SELECT_AUDIT_DISPLAY: &str = "\
    SELECT e.id, \
           COALESCE(p_user_id.value, '0')::BIGINT AS user_id, \
           COALESCE(u.name, 'unknown') AS username, \
           COALESCE(p_action.value, '') AS action, \
           COALESCE(p_target_type.value, '') AS target_type, \
           COALESCE(p_target_id.value, '0')::BIGINT AS target_id, \
           COALESCE(p_summary.value, '') AS summary, \
           e.created_at::TEXT AS created_at \
    FROM entities e \
    LEFT JOIN entity_properties p_user_id ON e.id = p_user_id.entity_id AND p_user_id.key = 'user_id' \
    LEFT JOIN entity_properties p_action ON e.id = p_action.entity_id AND p_action.key = 'action' \
    LEFT JOIN entity_properties p_target_type ON e.id = p_target_type.entity_id AND p_target_type.key = 'target_type' \
    LEFT JOIN entity_properties p_target_id ON e.id = p_target_id.entity_id AND p_target_id.key = 'target_id' \
    LEFT JOIN entity_properties p_summary ON e.id = p_summary.entity_id AND p_summary.key = 'summary' \
    LEFT JOIN entities u ON CAST(p_user_id.value AS BIGINT) = u.id AND u.entity_type = 'user' \
    WHERE e.entity_type = 'audit_entry'";

/// Find audit entries with pagination and optional filters
pub async fn find_paginated(
    pool: &PgPool,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    action_filter: Option<&str>,
    target_type_filter: Option<&str>,
) -> Result<AuditEntryPage, sqlx::Error> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Build filter clauses with sequential $N parameters
    let mut filters = Vec::new();
    let mut param_index: usize = 0;
    // We'll collect the string values to bind later
    let mut string_params: Vec<String> = Vec::new();

    if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
        let pattern = format!("%{}%", q.trim());
        param_index += 1;
        let p1 = param_index;
        param_index += 1;
        let p2 = param_index;
        filters.push(format!("(u.name LIKE ${} OR p_summary.value LIKE ${})", p1, p2));
        string_params.push(pattern.clone());
        string_params.push(pattern);
    }

    if let Some(action) = action_filter.filter(|a| a != &"all") {
        param_index += 1;
        filters.push(format!("p_action.value LIKE ${}", param_index));
        string_params.push(format!("{}%", action));
    }

    if let Some(target) = target_type_filter.filter(|t| t != &"all") {
        param_index += 1;
        filters.push(format!("p_target_type.value = ${}", param_index));
        string_params.push(target.to_string());
    }

    let filter_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" AND {}", filters.join(" AND "))
    };

    // Get total count
    let count_sql = format!(
        "SELECT COUNT(*) FROM entities e \
         LEFT JOIN entity_properties p_user_id ON e.id = p_user_id.entity_id AND p_user_id.key = 'user_id' \
         LEFT JOIN entity_properties p_action ON e.id = p_action.entity_id AND p_action.key = 'action' \
         LEFT JOIN entity_properties p_target_type ON e.id = p_target_type.entity_id AND p_target_type.key = 'target_type' \
         LEFT JOIN entity_properties p_summary ON e.id = p_summary.entity_id AND p_summary.key = 'summary' \
         LEFT JOIN entities u ON CAST(p_user_id.value AS BIGINT) = u.id AND u.entity_type = 'user' \
         WHERE e.entity_type = 'audit_entry'{}",
        filter_clause
    );

    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    for p in &string_params {
        count_query = count_query.bind(p);
    }
    let (total_count,) = count_query.fetch_one(pool).await?;
    let total_pages = (total_count as f64 / per_page as f64).ceil() as i64;

    // Get paginated results
    let limit_param = param_index + 1;
    let offset_param = param_index + 2;
    let sql = format!(
        "{}{} ORDER BY e.created_at DESC LIMIT ${} OFFSET ${}",
        SELECT_AUDIT_DISPLAY,
        filter_clause,
        limit_param,
        offset_param
    );

    let mut data_query = sqlx::query_as::<_, AuditEntry>(&sql);
    for p in &string_params {
        data_query = data_query.bind(p);
    }
    data_query = data_query.bind(per_page);
    data_query = data_query.bind(offset);
    let entries = data_query.fetch_all(pool).await?;

    Ok(AuditEntryPage {
        entries,
        page,
        per_page,
        total_count,
        total_pages,
    })
}

/// Fetch the N most recent audit entries (for dashboard activity feed).
pub async fn find_recent(pool: &PgPool, limit: i64) -> Result<Vec<AuditEntry>, sqlx::Error> {
    let sql = format!(
        "{} ORDER BY e.created_at DESC LIMIT $1",
        SELECT_AUDIT_DISPLAY
    );
    let entries = sqlx::query_as::<_, AuditEntry>(&sql)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(entries)
}

/// Create an audit entry in the database
pub async fn create(
    pool: &PgPool,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    summary: &str,
) -> Result<i64, sqlx::Error> {
    // Insert audit_entry entity with RETURNING id
    let (entry_id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label) VALUES ('audit_entry', $1, $2) RETURNING id"
    )
    .bind(action)
    .bind(summary)
    .fetch_one(pool)
    .await?;

    // Set properties
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'user_id', $2)"
    )
    .bind(entry_id)
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'action', $2)"
    )
    .bind(entry_id)
    .bind(action)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'target_type', $2)"
    )
    .bind(entry_id)
    .bind(target_type)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'target_id', $2)"
    )
    .bind(entry_id)
    .bind(target_id.to_string())
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'summary', $2)"
    )
    .bind(entry_id)
    .bind(summary)
    .execute(pool)
    .await?;

    Ok(entry_id)
}
