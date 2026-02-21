use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::ontology;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, OntologyConceptsTemplate, OntologyGraphTemplate, OntologyDataTemplate, OntologyDetailTemplate};

pub async fn concepts(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let ctx = PageContext::build(&session, &pool, "/ontology").await?;
    let entity_types = ontology::find_entity_type_summaries(&pool).await?;
    let relation_types = ontology::find_relation_type_summaries(&pool).await?;

    let tmpl = OntologyConceptsTemplate { ctx, entity_types, relation_types };
    render(tmpl)
}

pub async fn graph(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let ctx = PageContext::build(&session, &pool, "/ontology").await?;

    let tmpl = OntologyGraphTemplate { ctx };
    render(tmpl)
}

pub async fn graph_data(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let data = ontology::find_graph_data(&pool).await?;

    Ok(HttpResponse::Ok().json(data))
}

pub async fn schema_data(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let data = ontology::find_schema_graph_data(&pool).await?;

    Ok(HttpResponse::Ok().json(data))
}

pub async fn data(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let ctx = PageContext::build(&session, &pool, "/ontology").await?;

    let tmpl = OntologyDataTemplate { ctx };
    render(tmpl)
}

pub async fn data_detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let entity_id = path.into_inner();
    let ctx = PageContext::build(&session, &pool, "/ontology").await?;

    let entity = ontology::find_entity_detail(&pool, entity_id).await?
        .ok_or(AppError::NotFound)?;

    let tmpl = OntologyDetailTemplate { ctx, entity };
    render(tmpl)
}
