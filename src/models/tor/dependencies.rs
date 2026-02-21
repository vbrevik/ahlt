use sqlx::PgPool;
use serde::Serialize;

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

/// Helper struct for raw DB rows before converting is_blocking from String to bool.
#[derive(Debug, sqlx::FromRow)]
struct TorDependencyRow {
    relation_id: i64,
    relation_type: String,
    other_tor_id: i64,
    other_tor_name: String,
    other_tor_label: String,
    output_types: String,
    description: String,
    is_blocking: String,
}

impl From<TorDependencyRow> for TorDependency {
    fn from(row: TorDependencyRow) -> Self {
        TorDependency {
            relation_id: row.relation_id,
            relation_type: row.relation_type,
            other_tor_id: row.other_tor_id,
            other_tor_name: row.other_tor_name,
            other_tor_label: row.other_tor_label,
            output_types: row.output_types,
            description: row.description,
            is_blocking: row.is_blocking == "true",
        }
    }
}

/// Find ToRs that feed into or escalate to this ToR (upstream dependencies).
pub async fn find_upstream(pool: &PgPool, tor_id: i64) -> Result<Vec<TorDependency>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TorDependencyRow>(
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
         WHERE r.target_id = $1 \
           AND rt.name IN ('feeds_into', 'escalates_to') \
         ORDER BY rt.name, e.label",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(TorDependency::from).collect())
}

/// Find ToRs that this ToR feeds into or escalates to (downstream dependencies).
pub async fn find_downstream(pool: &PgPool, tor_id: i64) -> Result<Vec<TorDependency>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TorDependencyRow>(
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
         WHERE r.source_id = $1 \
           AND rt.name IN ('feeds_into', 'escalates_to') \
         ORDER BY rt.name, e.label",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(TorDependency::from).collect())
}

/// Add a dependency between two ToRs.
pub async fn add_dependency(
    pool: &PgPool,
    source_tor_id: i64,
    target_tor_id: i64,
    relation_type_name: &str,
    output_types: &str,
    description: &str,
    is_blocking: bool,
) -> Result<(), sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $1), $2, $3) \
         RETURNING id",
    )
    .bind(relation_type_name)
    .bind(source_tor_id)
    .bind(target_tor_id)
    .fetch_one(pool)
    .await?;
    let relation_id = row.0;

    if !output_types.is_empty() {
        sqlx::query(
            "INSERT INTO relation_properties (relation_id, key, value) VALUES ($1, 'output_types', $2)",
        )
        .bind(relation_id)
        .bind(output_types)
        .execute(pool)
        .await?;
    }
    if !description.is_empty() {
        sqlx::query(
            "INSERT INTO relation_properties (relation_id, key, value) VALUES ($1, 'description', $2)",
        )
        .bind(relation_id)
        .bind(description)
        .execute(pool)
        .await?;
    }
    if is_blocking {
        sqlx::query(
            "INSERT INTO relation_properties (relation_id, key, value) VALUES ($1, 'is_blocking', 'true')",
        )
        .bind(relation_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Remove a dependency relation.
pub async fn remove_dependency(pool: &PgPool, relation_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM relations WHERE id = $1")
        .bind(relation_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// A dependency entry for the governance map.
#[derive(Debug, Clone)]
pub struct GovernanceMapEntry {
    pub source_tor_id: i64,
    pub source_tor_label: String,
    pub target_tor_id: i64,
    pub target_tor_label: String,
    pub relation_type: String,  // "feeds_into" or "escalates_to"
    pub is_blocking: bool,
}

/// Helper struct for raw DB rows.
#[derive(Debug, sqlx::FromRow)]
struct GovernanceMapEntryRow {
    source_id: i64,
    source_label: String,
    target_id: i64,
    target_label: String,
    relation_type: String,
    is_blocking: String,
}

impl From<GovernanceMapEntryRow> for GovernanceMapEntry {
    fn from(row: GovernanceMapEntryRow) -> Self {
        GovernanceMapEntry {
            source_tor_id: row.source_id,
            source_tor_label: row.source_label,
            target_tor_id: row.target_id,
            target_tor_label: row.target_label,
            relation_type: row.relation_type,
            is_blocking: row.is_blocking == "true",
        }
    }
}

/// Find all inter-ToR dependencies for the governance map.
pub async fn find_all_dependencies(pool: &PgPool) -> Result<Vec<GovernanceMapEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, GovernanceMapEntryRow>(
        "SELECT src.id AS source_id, src.label AS source_label, \
                tgt.id AS target_id, tgt.label AS target_label, \
                rt.name AS relation_type, \
                COALESCE(rp.value, 'false') AS is_blocking \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         LEFT JOIN relation_properties rp ON r.id = rp.relation_id AND rp.key = 'is_blocking' \
         WHERE rt.name IN ('feeds_into', 'escalates_to') \
           AND src.entity_type = 'tor' \
           AND tgt.entity_type = 'tor' \
         ORDER BY src.label, tgt.label",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(GovernanceMapEntry::from).collect())
}

/// Find all active ToRs (for the map's row/column headers).
pub async fn find_all_tors(pool: &PgPool) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
    let tors: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, name, label FROM entities WHERE entity_type = 'tor' AND is_active = true ORDER BY label",
    )
    .fetch_all(pool)
    .await?;
    Ok(tors)
}

/// Find all other ToRs (for dependency selection dropdown).
pub async fn find_other_tors(pool: &PgPool, exclude_tor_id: i64) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
    let tors: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, name, label FROM entities \
         WHERE entity_type = 'tor' AND id != $1 AND is_active = true \
         ORDER BY label",
    )
    .bind(exclude_tor_id)
    .fetch_all(pool)
    .await?;
    Ok(tors)
}

// --- Governance graph (DAG visualization) ---

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct GraphNode {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub cadence: String,
    pub cadence_day: String,
    pub cadence_time: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    pub relation_type: String,
    pub is_blocking: bool,
    pub output_types: String,
}

/// Helper struct for raw DB rows.
#[derive(Debug, sqlx::FromRow)]
struct GraphEdgeRow {
    source_id: i64,
    target_id: i64,
    relation_type: String,
    is_blocking: String,
    output_types: String,
}

impl From<GraphEdgeRow> for GraphEdge {
    fn from(row: GraphEdgeRow) -> Self {
        GraphEdge {
            source: row.source_id,
            target: row.target_id,
            relation_type: row.relation_type,
            is_blocking: row.is_blocking == "true",
            output_types: row.output_types,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GovernanceGraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Build the full graph data for the governance map DAG visualization.
pub async fn find_graph_data(pool: &PgPool) -> Result<GovernanceGraphData, sqlx::Error> {
    // Nodes: all active ToRs with cadence properties
    let nodes = sqlx::query_as::<_, GraphNode>(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_cad.value, '') AS cadence, \
                COALESCE(p_day.value, '') AS cadence_day, \
                COALESCE(p_time.value, '') AS cadence_time, \
                COALESCE(p_status.value, 'active') AS status \
         FROM entities e \
         LEFT JOIN entity_properties p_cad ON e.id = p_cad.entity_id AND p_cad.key = 'meeting_cadence' \
         LEFT JOIN entity_properties p_day ON e.id = p_day.entity_id AND p_day.key = 'cadence_day' \
         LEFT JOIN entity_properties p_time ON e.id = p_time.entity_id AND p_time.key = 'cadence_time' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         WHERE e.entity_type = 'tor' AND e.is_active = true \
         ORDER BY e.label",
    )
    .fetch_all(pool)
    .await?;

    // Edges: all inter-ToR dependency relations
    let edge_rows = sqlx::query_as::<_, GraphEdgeRow>(
        "SELECT r.source_id, r.target_id, rt.name AS relation_type, \
                COALESCE(rp_block.value, 'false') AS is_blocking, \
                COALESCE(rp_ot.value, '') AS output_types \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         LEFT JOIN relation_properties rp_block ON r.id = rp_block.relation_id AND rp_block.key = 'is_blocking' \
         LEFT JOIN relation_properties rp_ot ON r.id = rp_ot.relation_id AND rp_ot.key = 'output_types' \
         WHERE rt.name IN ('feeds_into', 'escalates_to') \
           AND src.entity_type = 'tor' \
           AND tgt.entity_type = 'tor' \
         ORDER BY src.label, tgt.label",
    )
    .fetch_all(pool)
    .await?;

    let edges: Vec<GraphEdge> = edge_rows.into_iter().map(GraphEdge::from).collect();

    Ok(GovernanceGraphData { nodes, edges })
}
