use rusqlite::{Connection, params};

/// Count of unread warnings for a specific user.
pub fn count_unread(conn: &Connection, user_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*)
         FROM entities receipt
         JOIN entity_properties st ON st.entity_id = receipt.id AND st.key = 'status'
         JOIN relations r_user ON r_user.source_id = receipt.id
         JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
         WHERE receipt.entity_type = 'warning_receipt'
           AND st.value = 'unread'
           AND r_user.target_id = ?1",
        params![user_id],
        |row| row.get(0),
    ).unwrap_or(0)
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
pub fn find_for_user(
    conn: &Connection,
    user_id: i64,
    page: i64,
    per_page: i64,
    category_filter: Option<&str>,
    severity_filter: Option<&str>,
    show_read: bool,
    show_deleted: bool,
) -> rusqlite::Result<WarningPage> {
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
          AND r_user.target_id = ?1";

    let mut filters = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params_vec.push(Box::new(user_id));

    // Status filters
    if !show_read {
        filters.push(format!("rst.value != ?{}", params_vec.len() + 1));
        params_vec.push(Box::new("read".to_string()));
    }
    if !show_deleted {
        filters.push(format!("rst.value != ?{}", params_vec.len() + 1));
        params_vec.push(Box::new("deleted".to_string()));
    }

    // Also hide resolved by default
    filters.push(format!("rst.value != ?{}", params_vec.len() + 1));
    params_vec.push(Box::new("resolved".to_string()));

    if let Some(cat) = category_filter.filter(|c| c != &"all") {
        filters.push(format!("wcat.value = ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(cat.to_string()));
    }

    if let Some(sev) = severity_filter.filter(|s| s != &"all") {
        filters.push(format!("wsev.value = ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(sev.to_string()));
    }

    let filter_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" AND {}", filters.join(" AND "))
    };

    // Count
    let count_sql = format!("SELECT COUNT(*) {}{}", base_from, filter_clause);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    let total_count: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;
    let total_pages = ((total_count as f64) / (per_page as f64)).ceil().max(1.0) as i64;

    // Results
    let select_sql = format!(
        "SELECT w.id as warning_id, receipt.id as receipt_id, \
                wsev.value as severity, wcat.value as category, \
                wmsg.value as message, rst.value as status, \
                rsa.value as status_at, w.created_at \
         {} {} \
         ORDER BY CASE rst.value WHEN 'unread' THEN 0 ELSE 1 END, w.created_at DESC \
         LIMIT ?{} OFFSET ?{}",
        base_from, filter_clause,
        params_vec.len() + 1, params_vec.len() + 2,
    );

    params_vec.push(Box::new(per_page));
    params_vec.push(Box::new(offset));
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&select_sql)?;
    let items = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(WarningListItem {
            warning_id: row.get("warning_id")?,
            receipt_id: row.get("receipt_id")?,
            severity: row.get("severity")?,
            category: row.get("category")?,
            message: row.get("message")?,
            status: row.get("status")?,
            status_at: row.get("status_at")?,
            created_at: row.get("created_at")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

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
pub fn get_warning_detail(conn: &Connection, warning_id: i64) -> rusqlite::Result<Option<WarningDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.created_at,
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
         WHERE e.id = ?1 AND e.entity_type = 'warning'"
    )?;
    let mut rows = stmt.query_map(params![warning_id], |row| {
        Ok(WarningDetail {
            id: row.get("id")?,
            severity: row.get("severity")?,
            category: row.get("category")?,
            message: row.get("message")?,
            source_action: row.get("source_action")?,
            details: row.get("details")?,
            status: row.get("status")?,
            scope: row.get("scope")?,
            created_at: row.get("created_at")?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Get all recipients for a warning with their receipt status.
pub fn get_recipients(conn: &Connection, warning_id: i64) -> rusqlite::Result<Vec<WarningRecipient>> {
    let mut stmt = conn.prepare(
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
         WHERE r_warn.target_id = ?1 AND receipt.entity_type = 'warning_receipt'
         ORDER BY u.name"
    )?;
    let rows = stmt.query_map(params![warning_id], |row| {
        Ok(WarningRecipient {
            user_id: row.get("user_id")?,
            username: row.get("username")?,
            user_label: row.get("user_label")?,
            receipt_id: row.get("receipt_id")?,
            status: row.get("status")?,
            status_at: row.get("status_at")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get event timeline for a receipt.
pub fn get_receipt_timeline(conn: &Connection, receipt_id: i64) -> rusqlite::Result<Vec<WarningTimelineEvent>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(pa.value, '') as action,
                COALESCE(pau.value, '0') as actor_user_id,
                COALESCE(u.name, 'system') as actor_username,
                evt.created_at,
                COALESCE(pn.value, '') as note
         FROM entities evt
         JOIN relations r ON r.source_id = evt.id
         JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'on_receipt'
         LEFT JOIN entity_properties pa ON pa.entity_id = evt.id AND pa.key = 'action'
         LEFT JOIN entity_properties pau ON pau.entity_id = evt.id AND pau.key = 'actor_user_id'
         LEFT JOIN entities u ON u.id = CAST(pau.value AS INTEGER) AND u.entity_type = 'user'
         LEFT JOIN entity_properties pn ON pn.entity_id = evt.id AND pn.key = 'note'
         WHERE evt.entity_type = 'warning_event' AND r.target_id = ?1
         ORDER BY evt.created_at ASC"
    )?;
    let rows = stmt.query_map(params![receipt_id], |row| {
        Ok(WarningTimelineEvent {
            action: row.get("action")?,
            actor_user_id: row.get::<_, String>("actor_user_id")?.parse().unwrap_or(0),
            actor_username: row.get("actor_username")?,
            created_at: row.get("created_at")?,
            note: row.get("note")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Find a receipt for a specific user and warning.
pub fn find_receipt_for_user(conn: &Connection, warning_id: i64, user_id: i64) -> rusqlite::Result<Option<i64>> {
    let receipt_name = format!("wr.{}.{}", warning_id, user_id);
    let mut stmt = conn.prepare(
        "SELECT id FROM entities WHERE entity_type = 'warning_receipt' AND name = ?1"
    )?;
    let mut rows = stmt.query_map(params![receipt_name], |row| row.get::<_, i64>(0))?;
    match rows.next() {
        Some(id) => Ok(Some(id?)),
        None => Ok(None),
    }
}
