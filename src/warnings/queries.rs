use sqlx::PgPool;

/// Count of unread warnings for a specific user.
pub async fn count_unread(pool: &PgPool, user_id: i64) -> i64 {
    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*)
         FROM entities receipt
         JOIN entity_properties st ON st.entity_id = receipt.id AND st.key = 'status'
         JOIN relations r_user ON r_user.source_id = receipt.id
         JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
         WHERE receipt.entity_type = 'warning_receipt'
           AND st.value = 'unread'
           AND r_user.target_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await;
    result.map(|r| r.0).unwrap_or(0)
}

/// A warning for display in list view.
#[derive(Debug, Clone)]
pub struct WarningListItem {
    pub warning_id: i64,
    pub receipt_id: i64,
    pub severity: String,
    pub category: String,
    pub message: String,
    pub status: String,
    pub status_at: String,
    pub created_at: String,
}

/// Page of warnings for a user.
pub struct WarningPage {
    pub items: Vec<WarningListItem>,
    pub page: i64,
    pub per_page: i64,
    pub total_count: i64,
    pub total_pages: i64,
}

/// Find paginated warnings for a user with optional filters.
pub async fn find_for_user(
    pool: &PgPool,
    user_id: i64,
    page: i64,
    per_page: i64,
    category_filter: Option<&str>,
    severity_filter: Option<&str>,
    show_read: bool,
    show_deleted: bool,
) -> Result<WarningPage, sqlx::Error> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let base_from = "\
        FROM entities receipt \
        JOIN entity_properties rst ON rst.entity_id = receipt.id AND rst.key = 'status' \
        JOIN entity_properties rsa ON rsa.entity_id = receipt.id AND rsa.key = 'status_at' \
        JOIN relations r_user ON r_user.source_id = receipt.id \
        JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user' \
        JOIN relations r_warn ON r_warn.source_id = receipt.id \
        JOIN entities rt_warn ON rt_warn.id = r_warn.relation_type_id AND rt_warn.name = 'for_warning' \
        JOIN entities w ON w.id = r_warn.target_id \
        JOIN entity_properties wsev ON wsev.entity_id = w.id AND wsev.key = 'severity' \
        JOIN entity_properties wcat ON wcat.entity_id = w.id AND wcat.key = 'category' \
        JOIN entity_properties wmsg ON wmsg.entity_id = w.id AND wmsg.key = 'message' \
        WHERE receipt.entity_type = 'warning_receipt' \
          AND r_user.target_id = $1";

    // Build dynamic filter clauses and track parameter index
    let mut filters = Vec::new();
    let mut param_idx = 2u32; // $1 is user_id

    // We'll collect filter values to bind later
    let mut filter_values: Vec<String> = Vec::new();

    // Status filters
    if !show_read {
        filters.push(format!("rst.value != ${}", param_idx));
        filter_values.push("read".to_string());
        param_idx += 1;
    }
    if !show_deleted {
        filters.push(format!("rst.value != ${}", param_idx));
        filter_values.push("deleted".to_string());
        param_idx += 1;
    }

    // Also hide resolved by default
    filters.push(format!("rst.value != ${}", param_idx));
    filter_values.push("resolved".to_string());
    param_idx += 1;

    let category_val = category_filter.filter(|c| c != &"all").map(|c| c.to_string());
    if let Some(ref cat) = category_val {
        filters.push(format!("wcat.value = ${}", param_idx));
        filter_values.push(cat.clone());
        param_idx += 1;
    }

    let severity_val = severity_filter.filter(|s| s != &"all").map(|s| s.to_string());
    if let Some(ref sev) = severity_val {
        filters.push(format!("wsev.value = ${}", param_idx));
        filter_values.push(sev.clone());
        param_idx += 1;
    }

    let filter_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" AND {}", filters.join(" AND "))
    };

    // Count query
    let count_sql = format!("SELECT COUNT(*) {}{}", base_from, filter_clause);
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql).bind(user_id);
    for val in &filter_values {
        count_query = count_query.bind(val);
    }
    let (total_count,) = count_query.fetch_one(pool).await?;
    let total_pages = ((total_count as f64) / (per_page as f64)).ceil().max(1.0) as i64;

    // Results query
    let limit_param = param_idx;
    let offset_param = param_idx + 1;
    let select_sql = format!(
        "SELECT w.id as warning_id, receipt.id as receipt_id, \
                wsev.value as severity, wcat.value as category, \
                wmsg.value as message, rst.value as status, \
                rsa.value as status_at, w.created_at::TEXT \
         {} {} \
         ORDER BY CASE rst.value WHEN 'unread' THEN 0 ELSE 1 END, w.created_at DESC \
         LIMIT ${} OFFSET ${}",
        base_from, filter_clause,
        limit_param, offset_param,
    );

    let mut select_query = sqlx::query_as::<_, (i64, i64, String, String, String, String, String, String)>(&select_sql)
        .bind(user_id);
    for val in &filter_values {
        select_query = select_query.bind(val);
    }
    select_query = select_query.bind(per_page).bind(offset);

    let rows = select_query.fetch_all(pool).await?;
    let items = rows
        .into_iter()
        .map(|r| WarningListItem {
            warning_id: r.0,
            receipt_id: r.1,
            severity: r.2,
            category: r.3,
            message: r.4,
            status: r.5,
            status_at: r.6,
            created_at: r.7,
        })
        .collect();

    Ok(WarningPage { items, page, per_page, total_count, total_pages })
}

/// Warning detail for the detail page.
#[derive(Debug, Clone)]
pub struct WarningDetail {
    pub id: i64,
    pub severity: String,
    pub category: String,
    pub message: String,
    pub source_action: String,
    pub details: String,
    pub status: String,
    pub scope: String,
    pub created_at: String,
}

/// Recipient with status info.
#[derive(Debug, Clone)]
pub struct WarningRecipient {
    pub user_id: i64,
    pub username: String,
    pub user_label: String,
    pub receipt_id: i64,
    pub status: String,
    pub status_at: String,
}

/// Event in the timeline.
#[derive(Debug, Clone)]
pub struct WarningTimelineEvent {
    pub action: String,
    pub actor_user_id: i64,
    pub actor_username: String,
    pub created_at: String,
    pub note: String,
}

/// Get full warning detail by warning entity ID.
pub async fn get_warning_detail(pool: &PgPool, warning_id: i64) -> Result<Option<WarningDetail>, sqlx::Error> {
    let row: Option<(i64, String, String, String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT e.id, e.created_at::TEXT,
                COALESCE(psev.value, '') as severity,
                COALESCE(pcat.value, '') as category,
                COALESCE(pmsg.value, '') as message,
                COALESCE(psa.value, '') as source_action,
                COALESCE(pdet.value, '') as details,
                COALESCE(pst.value, '') as status,
                COALESCE(psc.value, '') as scope
         FROM entities e
         LEFT JOIN entity_properties psev ON e.id = psev.entity_id AND psev.key = 'severity'
         LEFT JOIN entity_properties pcat ON e.id = pcat.entity_id AND pcat.key = 'category'
         LEFT JOIN entity_properties pmsg ON e.id = pmsg.entity_id AND pmsg.key = 'message'
         LEFT JOIN entity_properties psa ON e.id = psa.entity_id AND psa.key = 'source_action'
         LEFT JOIN entity_properties pdet ON e.id = pdet.entity_id AND pdet.key = 'details'
         LEFT JOIN entity_properties pst ON e.id = pst.entity_id AND pst.key = 'status'
         LEFT JOIN entity_properties psc ON e.id = psc.entity_id AND psc.key = 'scope'
         WHERE e.id = $1 AND e.entity_type = 'warning'",
    )
    .bind(warning_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| WarningDetail {
        id: r.0,
        created_at: r.1,
        severity: r.2,
        category: r.3,
        message: r.4,
        source_action: r.5,
        details: r.6,
        status: r.7,
        scope: r.8,
    }))
}

/// Get all recipients for a warning with their receipt status.
pub async fn get_recipients(pool: &PgPool, warning_id: i64) -> Result<Vec<WarningRecipient>, sqlx::Error> {
    let rows: Vec<(i64, String, String, i64, String, String)> = sqlx::query_as(
        "SELECT u.id as user_id, u.name as username, u.label as user_label,
                receipt.id as receipt_id,
                COALESCE(rst.value, 'unread') as status,
                COALESCE(rsa.value, '') as status_at
         FROM entities receipt
         JOIN relations r_warn ON r_warn.source_id = receipt.id
         JOIN entities rt_warn ON rt_warn.id = r_warn.relation_type_id AND rt_warn.name = 'for_warning'
         JOIN relations r_user ON r_user.source_id = receipt.id
         JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
         JOIN entities u ON u.id = r_user.target_id
         LEFT JOIN entity_properties rst ON rst.entity_id = receipt.id AND rst.key = 'status'
         LEFT JOIN entity_properties rsa ON rsa.entity_id = receipt.id AND rsa.key = 'status_at'
         WHERE r_warn.target_id = $1 AND receipt.entity_type = 'warning_receipt'
         ORDER BY u.name",
    )
    .bind(warning_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WarningRecipient {
            user_id: r.0,
            username: r.1,
            user_label: r.2,
            receipt_id: r.3,
            status: r.4,
            status_at: r.5,
        })
        .collect())
}

/// Get event timeline for a receipt.
pub async fn get_receipt_timeline(pool: &PgPool, receipt_id: i64) -> Result<Vec<WarningTimelineEvent>, sqlx::Error> {
    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT COALESCE(pa.value, '') as action,
                COALESCE(pau.value, '0') as actor_user_id,
                COALESCE(u.name, 'system') as actor_username,
                evt.created_at::TEXT,
                COALESCE(pn.value, '') as note
         FROM entities evt
         JOIN relations r ON r.source_id = evt.id
         JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'on_receipt'
         LEFT JOIN entity_properties pa ON pa.entity_id = evt.id AND pa.key = 'action'
         LEFT JOIN entity_properties pau ON pau.entity_id = evt.id AND pau.key = 'actor_user_id'
         LEFT JOIN entities u ON u.id = CAST(pau.value AS BIGINT) AND u.entity_type = 'user'
         LEFT JOIN entity_properties pn ON pn.entity_id = evt.id AND pn.key = 'note'
         WHERE evt.entity_type = 'warning_event' AND r.target_id = $1
         ORDER BY evt.created_at ASC",
    )
    .bind(receipt_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WarningTimelineEvent {
            action: r.0,
            actor_user_id: r.1.parse().unwrap_or(0),
            actor_username: r.2,
            created_at: r.3,
            note: r.4,
        })
        .collect())
}

/// Find a receipt for a specific user and warning.
pub async fn find_receipt_for_user(pool: &PgPool, warning_id: i64, user_id: i64) -> Result<Option<i64>, sqlx::Error> {
    let receipt_name = format!("wr.{}.{}", warning_id, user_id);
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'warning_receipt' AND name = $1",
    )
    .bind(receipt_name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}
