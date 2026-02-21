use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::{tor, graph_sync::{self, GraphPool}};
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, GovernanceMapTemplate};

pub async fn governance_map(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let ctx = PageContext::build(&session, &pool, "/governance/map").await?;
    let tors = tor::find_all_tors(&pool).await?;
    let dependencies = tor::find_all_dependencies(&pool).await?;

    let tmpl = GovernanceMapTemplate {
        ctx,
        tors,
        dependencies,
    };
    render(tmpl)
}

pub async fn governance_graph_api(
    pool: web::Data<PgPool>,
    graph: web::Data<GraphPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    // Try Neo4j first for graph data, fall back to Postgres
    if let Some(g) = graph.get_ref() {
        if let Some((nodes, edges)) = graph_sync::queries::governance_graph(g).await {
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "nodes": nodes,
                "edges": edges,
            })));
        }
    }

    let data = tor::find_graph_data(&pool).await?;
    Ok(HttpResponse::Ok().json(data))
}
