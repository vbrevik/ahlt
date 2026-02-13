use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;

use crate::db::DbPool;
use crate::models::ontology;
use crate::auth::session::require_permission;
use crate::templates_structs::{PageContext, OntologyConceptsTemplate, OntologyGraphTemplate, OntologyDataTemplate, OntologyDetailTemplate};

pub async fn concepts(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/ontology");
    let entity_types = ontology::find_entity_type_summaries(&conn).unwrap_or_default();
    let relation_types = ontology::find_relation_type_summaries(&conn).unwrap_or_default();

    let tmpl = OntologyConceptsTemplate { ctx, entity_types, relation_types };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn graph(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/ontology");

    let tmpl = OntologyGraphTemplate { ctx };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn graph_data(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Database error"})),
    };

    match ontology::find_graph_data(&conn) {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(_) => HttpResponse::InternalServerError().json(serde_json::json!({"error": "Query error"})),
    }
}

pub async fn schema_data(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Database error"})),
    };

    match ontology::find_schema_graph_data(&conn) {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(_) => HttpResponse::InternalServerError().json(serde_json::json!({"error": "Query error"})),
    }
}

pub async fn data(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/ontology");

    let tmpl = OntologyDataTemplate { ctx };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn data_detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let entity_id = path.into_inner();
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/ontology");

    match ontology::find_entity_detail(&conn, entity_id) {
        Ok(Some(entity)) => {
            let tmpl = OntologyDetailTemplate { ctx, entity };
            match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            }
        }
        _ => HttpResponse::NotFound().body("Entity not found"),
    }
}
