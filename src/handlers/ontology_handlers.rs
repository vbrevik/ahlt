use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::ontology;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, OntologyConceptsTemplate, OntologyGraphTemplate, OntologyDataTemplate, OntologyDetailTemplate};

pub async fn concepts(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/ontology")?;
    let entity_types = ontology::find_entity_type_summaries(&conn)?;
    let relation_types = ontology::find_relation_type_summaries(&conn)?;

    let tmpl = OntologyConceptsTemplate { ctx, entity_types, relation_types };
    render(tmpl)
}

pub async fn graph(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/ontology")?;

    let tmpl = OntologyGraphTemplate { ctx };
    render(tmpl)
}

pub async fn graph_data(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let data = ontology::find_graph_data(&conn)?;

    Ok(HttpResponse::Ok().json(data))
}

pub async fn schema_data(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let data = ontology::find_schema_graph_data(&conn)?;

    Ok(HttpResponse::Ok().json(data))
}

pub async fn data(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/ontology")?;

    let tmpl = OntologyDataTemplate { ctx };
    render(tmpl)
}

pub async fn data_detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let entity_id = path.into_inner();
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/ontology")?;

    let entity = ontology::find_entity_detail(&conn, entity_id)?
        .ok_or(AppError::NotFound)?;

    let tmpl = OntologyDetailTemplate { ctx, entity };
    render(tmpl)
}
