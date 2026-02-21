use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::tor;
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
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let data = tor::find_graph_data(&pool).await?;
    Ok(HttpResponse::Ok().json(data))
}
