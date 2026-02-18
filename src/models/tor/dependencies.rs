use rusqlite::{Connection, params};

/// A dependency relationship between two ToRs.
#[derive(Debug, Clone)]
pub struct TorDependency {
    pub relation_id: i64,
    pub relation_type: String,        // "feeds_into" or "escalates_to"
    pub other_tor_id: i64,
    pub other_tor_name: String,
    pub other_tor_label: String,
    pub output_types: String,
    pub description: String,
    pub is_blocking: bool,
}

/// Find ToRs that feed into or escalate to this ToR (upstream dependencies).
pub fn find_upstream(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorDependency>> {
    let mut stmt = conn.prepare(
        "SELECT r.id AS relation_id, rt.name AS relation_type, \
                e.id AS other_tor_id, e.name AS other_tor_name, e.label AS other_tor_label, \
                COALESCE(rp_ot.value, '') AS output_types, \
                COALESCE(rp_desc.value, '') AS description, \
                COALESCE(rp_block.value, 'false') AS is_blocking \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         JOIN entities e ON r.source_id = e.id \
         LEFT JOIN relation_properties rp_ot ON r.id = rp_ot.relation_id AND rp_ot.key = 'output_types' \
         LEFT JOIN relation_properties rp_desc ON r.id = rp_desc.relation_id AND rp_desc.key = 'description' \
         LEFT JOIN relation_properties rp_block ON r.id = rp_block.relation_id AND rp_block.key = 'is_blocking' \
         WHERE r.target_id = ?1 \
           AND rt.name IN ('feeds_into', 'escalates_to') \
         ORDER BY rt.name, e.label",
    )?;

    let deps = stmt
        .query_map(params![tor_id], |row| {
            Ok(TorDependency {
                relation_id: row.get("relation_id")?,
                relation_type: row.get("relation_type")?,
                other_tor_id: row.get("other_tor_id")?,
                other_tor_name: row.get("other_tor_name")?,
                other_tor_label: row.get("other_tor_label")?,
                output_types: row.get("output_types")?,
                description: row.get("description")?,
                is_blocking: row.get::<_, String>("is_blocking")? == "true",
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(deps)
}

/// Find ToRs that this ToR feeds into or escalates to (downstream dependencies).
pub fn find_downstream(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorDependency>> {
    let mut stmt = conn.prepare(
        "SELECT r.id AS relation_id, rt.name AS relation_type, \
                e.id AS other_tor_id, e.name AS other_tor_name, e.label AS other_tor_label, \
                COALESCE(rp_ot.value, '') AS output_types, \
                COALESCE(rp_desc.value, '') AS description, \
                COALESCE(rp_block.value, 'false') AS is_blocking \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         JOIN entities e ON r.target_id = e.id \
         LEFT JOIN relation_properties rp_ot ON r.id = rp_ot.relation_id AND rp_ot.key = 'output_types' \
         LEFT JOIN relation_properties rp_desc ON r.id = rp_desc.relation_id AND rp_desc.key = 'description' \
         LEFT JOIN relation_properties rp_block ON r.id = rp_block.relation_id AND rp_block.key = 'is_blocking' \
         WHERE r.source_id = ?1 \
           AND rt.name IN ('feeds_into', 'escalates_to') \
         ORDER BY rt.name, e.label",
    )?;

    let deps = stmt
        .query_map(params![tor_id], |row| {
            Ok(TorDependency {
                relation_id: row.get("relation_id")?,
                relation_type: row.get("relation_type")?,
                other_tor_id: row.get("other_tor_id")?,
                other_tor_name: row.get("other_tor_name")?,
                other_tor_label: row.get("other_tor_label")?,
                output_types: row.get("output_types")?,
                description: row.get("description")?,
                is_blocking: row.get::<_, String>("is_blocking")? == "true",
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(deps)
}

/// Add a dependency between two ToRs.
pub fn add_dependency(
    conn: &Connection,
    source_tor_id: i64,
    target_tor_id: i64,
    relation_type_name: &str,
    output_types: &str,
    description: &str,
    is_blocking: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1), ?2, ?3)",
        params![relation_type_name, source_tor_id, target_tor_id],
    )?;
    let relation_id = conn.last_insert_rowid();

    if !output_types.is_empty() {
        conn.execute(
            "INSERT INTO relation_properties (relation_id, key, value) VALUES (?1, 'output_types', ?2)",
            params![relation_id, output_types],
        )?;
    }
    if !description.is_empty() {
        conn.execute(
            "INSERT INTO relation_properties (relation_id, key, value) VALUES (?1, 'description', ?2)",
            params![relation_id, description],
        )?;
    }
    if is_blocking {
        conn.execute(
            "INSERT INTO relation_properties (relation_id, key, value) VALUES (?1, 'is_blocking', 'true')",
            params![relation_id],
        )?;
    }

    Ok(())
}

/// Remove a dependency relation.
pub fn remove_dependency(conn: &Connection, relation_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM relations WHERE id = ?1", params![relation_id])?;
    Ok(())
}

/// Find all other ToRs (for dependency selection dropdown).
pub fn find_other_tors(conn: &Connection, exclude_tor_id: i64) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, label FROM entities \
         WHERE entity_type = 'tor' AND id != ?1 AND is_active = 1 \
         ORDER BY label",
    )?;
    let tors = stmt
        .query_map(params![exclude_tor_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tors)
}
