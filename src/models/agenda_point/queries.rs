use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;

/// Find all agenda points for a given ToR via the `belongs_to_tor` relation.
pub fn find_all_for_tor(conn: &Connection, tor_id: i64) -> Result<Vec<AgendaPointListItem>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'scheduled') AS status, \
                COALESCE(p_sched.value, '') AS scheduled_date, \
                COALESCE(p_type.value, 'informative') AS item_type, \
                COALESCE(p_tor.value, '0') AS tor_id \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'belongs_to_tor' \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_sched \
             ON e.id = p_sched.entity_id AND p_sched.key = 'scheduled_date' \
         LEFT JOIN entity_properties p_type \
             ON e.id = p_type.entity_id AND p_type.key = 'item_type' \
         LEFT JOIN entity_properties p_tor \
             ON e.id = p_tor.entity_id AND p_tor.key = 'tor_id' \
         WHERE e.entity_type = 'agenda_point' AND r.target_id = ?1 \
         ORDER BY scheduled_date ASC",
    )?;

    let items = stmt
        .query_map(params![tor_id], |row| {
            let tor_id_str: String = row.get("tor_id")?;
            let tor_id: i64 = tor_id_str.parse().unwrap_or(0);

            Ok(AgendaPointListItem {
                id: row.get("id")?,
                title: row.get("title")?,
                description: row.get("description")?,
                status: row.get("status")?,
                scheduled_date: row.get("scheduled_date")?,
                item_type: row.get("item_type")?,
                tor_id,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Find all agenda points across all ToRs (or filtered to ToRs a user fills a position in).
///
/// `user_id = None`  → returns every agenda point across all ToRs.
/// `user_id = Some(id)` → returns only agenda points for ToRs the user fills a position in.
pub fn find_all_cross_tor(conn: &Connection, user_id: Option<i64>) -> Result<Vec<CrossTorAgendaItem>, AppError> {
    let base_sql = "SELECT tor.id AS tor_id, tor.label AS tor_name, e.id, \
                           COALESCE(p_title.value, '') AS title, \
                           COALESCE(p_desc.value, '') AS description, \
                           COALESCE(p_status.value, 'scheduled') AS status, \
                           COALESCE(p_sched.value, '') AS scheduled_date, \
                           COALESCE(p_type.value, 'informative') AS item_type \
                    FROM entities e \
                    JOIN relations r ON e.id = r.source_id \
                    JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'belongs_to_tor' \
                    JOIN entities tor ON tor.id = r.target_id AND tor.entity_type = 'tor' \
                    LEFT JOIN entity_properties p_title \
                        ON e.id = p_title.entity_id AND p_title.key = 'title' \
                    LEFT JOIN entity_properties p_desc \
                        ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
                    LEFT JOIN entity_properties p_status \
                        ON e.id = p_status.entity_id AND p_status.key = 'status' \
                    LEFT JOIN entity_properties p_sched \
                        ON e.id = p_sched.entity_id AND p_sched.key = 'scheduled_date' \
                    LEFT JOIN entity_properties p_type \
                        ON e.id = p_type.entity_id AND p_type.key = 'item_type' \
                    WHERE e.entity_type = 'agenda_point'";

    let row_to_item = |row: &rusqlite::Row<'_>| {
        Ok(CrossTorAgendaItem {
            tor_id: row.get("tor_id")?,
            tor_name: row.get("tor_name")?,
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: row.get("status")?,
            scheduled_date: row.get("scheduled_date")?,
            item_type: row.get("item_type")?,
        })
    };

    let items = if let Some(uid) = user_id {
        let sql = format!(
            "{} AND EXISTS (\
                SELECT 1 FROM relations r_fills \
                JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
                WHERE r_fills.source_id = ?1 \
                  AND r_tor.target_id = tor.id \
                  AND r_fills.relation_type_id = (\
                      SELECT id FROM entities \
                      WHERE entity_type = 'relation_type' AND name = 'fills_position') \
                  AND r_tor.relation_type_id = (\
                      SELECT id FROM entities \
                      WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')\
            ) ORDER BY tor.label ASC, scheduled_date ASC",
            base_sql
        );
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map(params![uid], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let sql = format!("{} ORDER BY tor.label ASC, scheduled_date ASC", base_sql);
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map([], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(items)
}

/// Find a single agenda point by its entity id.
pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<AgendaPointDetail>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'scheduled') AS status, \
                COALESCE(p_type.value, 'informative') AS item_type, \
                COALESCE(p_tor.value, '0') AS tor_id, \
                COALESCE(p_creator.value, '0') AS created_by, \
                COALESCE(p_created.value, '') AS created_date, \
                COALESCE(p_sched.value, '') AS scheduled_date, \
                COALESCE(p_time.value, '0') AS time_allocation_minutes, \
                COALESCE(p_presenter.value, '') AS presenter, \
                COALESCE(p_priority.value, 'normal') AS priority, \
                COALESCE(p_preread.value, '') AS pre_read_url \
         FROM entities e \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_type \
             ON e.id = p_type.entity_id AND p_type.key = 'item_type' \
         LEFT JOIN entity_properties p_tor \
             ON e.id = p_tor.entity_id AND p_tor.key = 'tor_id' \
         LEFT JOIN entity_properties p_creator \
             ON e.id = p_creator.entity_id AND p_creator.key = 'created_by' \
         LEFT JOIN entity_properties p_created \
             ON e.id = p_created.entity_id AND p_created.key = 'created_date' \
         LEFT JOIN entity_properties p_sched \
             ON e.id = p_sched.entity_id AND p_sched.key = 'scheduled_date' \
         LEFT JOIN entity_properties p_time \
             ON e.id = p_time.entity_id AND p_time.key = 'time_allocation_minutes' \
         LEFT JOIN entity_properties p_presenter \
             ON e.id = p_presenter.entity_id AND p_presenter.key = 'presenter' \
         LEFT JOIN entity_properties p_priority \
             ON e.id = p_priority.entity_id AND p_priority.key = 'priority' \
         LEFT JOIN entity_properties p_preread \
             ON e.id = p_preread.entity_id AND p_preread.key = 'pre_read_url' \
         WHERE e.id = ?1 AND e.entity_type = 'agenda_point'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        let tor_id_str: String = row.get("tor_id")?;
        let tor_id: i64 = tor_id_str.parse().unwrap_or(0);
        let created_by_str: String = row.get("created_by")?;
        let created_by: i64 = created_by_str.parse().unwrap_or(0);
        let time_str: String = row.get("time_allocation_minutes")?;
        let time_allocation_minutes: i32 = time_str.parse().unwrap_or(0);

        Ok(AgendaPointDetail {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: row.get("status")?,
            item_type: row.get("item_type")?,
            tor_id,
            created_by,
            created_date: row.get("created_date")?,
            scheduled_date: row.get("scheduled_date")?,
            time_allocation_minutes,
            coa_ids: Vec::new(),  // Will be populated after query
            presenter: row.get("presenter")?,
            priority: row.get("priority")?,
            pre_read_url: row.get("pre_read_url")?,
        })
    })?;

    let mut detail = match rows.next() {
        Some(row) => row?,
        None => return Ok(None),
    };

    // Fetch related COA IDs via considers_coa relation
    let mut coa_stmt = conn.prepare(
        "SELECT r.target_id \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'considers_coa' \
         WHERE r.source_id = ?1 \
         ORDER BY r.target_id",
    )?;

    let coa_ids = coa_stmt
        .query_map(params![id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    detail.coa_ids = coa_ids;

    Ok(Some(detail))
}

/// Create a new agenda point entity linked to a ToR via `belongs_to_tor`.
/// Returns the new entity id.
pub fn create(
    conn: &Connection,
    tor_id: i64,
    title: &str,
    description: &str,
    item_type: &str,
    scheduled_date: &str,
    time_allocation_minutes: i32,
    created_by_id: i64,
    presenter: &str,
    priority: &str,
    pre_read_url: &str,
) -> Result<i64, AppError> {
    let name = format!("agenda_{}_{}", scheduled_date.replace('-', "_"), tor_id);
    let label = if title.len() > 50 {
        format!("{}...", &title[..50])
    } else {
        title.to_string()
    };

    let agenda_point_id = entity::create(conn, "agenda_point", &name, &label)?;

    entity::set_property(conn, agenda_point_id, "title", title)?;
    entity::set_property(conn, agenda_point_id, "description", description)?;
    entity::set_property(conn, agenda_point_id, "item_type", item_type)?;
    entity::set_property(conn, agenda_point_id, "scheduled_date", scheduled_date)?;
    entity::set_property(conn, agenda_point_id, "time_allocation_minutes", &time_allocation_minutes.to_string())?;
    entity::set_property(conn, agenda_point_id, "created_by", &created_by_id.to_string())?;
    entity::set_property(conn, agenda_point_id, "created_date", &chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string())?;
    entity::set_property(conn, agenda_point_id, "status", "scheduled")?;
    entity::set_property(conn, agenda_point_id, "tor_id", &tor_id.to_string())?;
    if !presenter.is_empty() {
        entity::set_property(conn, agenda_point_id, "presenter", presenter)?;
    }
    if !priority.is_empty() {
        entity::set_property(conn, agenda_point_id, "priority", priority)?;
    }
    if !pre_read_url.is_empty() {
        entity::set_property(conn, agenda_point_id, "pre_read_url", pre_read_url)?;
    }

    relation::create(conn, "belongs_to_tor", agenda_point_id, tor_id)?;

    Ok(agenda_point_id)
}

/// Update basic properties of an agenda point.
pub fn update(
    conn: &Connection,
    agenda_point_id: i64,
    title: &str,
    description: &str,
    item_type: &str,
    scheduled_date: &str,
    time_allocation_minutes: i32,
    presenter: &str,
    priority: &str,
    pre_read_url: &str,
) -> Result<(), AppError> {
    entity::set_property(conn, agenda_point_id, "title", title)?;
    entity::set_property(conn, agenda_point_id, "description", description)?;
    entity::set_property(conn, agenda_point_id, "item_type", item_type)?;
    entity::set_property(conn, agenda_point_id, "scheduled_date", scheduled_date)?;
    entity::set_property(conn, agenda_point_id, "time_allocation_minutes", &time_allocation_minutes.to_string())?;
    entity::set_property(conn, agenda_point_id, "presenter", presenter)?;
    entity::set_property(conn, agenda_point_id, "priority", priority)?;
    entity::set_property(conn, agenda_point_id, "pre_read_url", pre_read_url)?;
    Ok(())
}

