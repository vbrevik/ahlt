use serde::Deserialize;
use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use crate::models::{user, role};
use crate::models::table_filter::{FilterTree, SortSpec};
use crate::models::table_filter::columns as col_resolver;
use crate::models::user::filter as uf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserListTemplate};

#[derive(Deserialize)]
pub struct ListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
}

pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let ctx = PageContext::build(&session, &pool, "/users").await?;
    let user_id = get_user_id(&session).unwrap_or(0);

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);

    // Parse filter
    let filter = query.filter.as_deref()
        .and_then(|s| FilterTree::from_json(s).ok())
        .unwrap_or_default();
    let filter_active = !filter.is_empty();
    let filter_json = filter.to_json();

    // Parse sort
    let sort = SortSpec::from_params(query.sort.as_deref(), query.dir.as_deref());

    // Resolve columns
    let all_cols = uf::default_columns();
    let columns = col_resolver::resolve_columns("users", user_id, &pool, &all_cols).await;

    // Fetch roles for filter builder dropdown
    let roles = role::queries::find_all_list_items(&pool).await?;
    let available_roles: Vec<(String, String)> = roles.iter()
        .map(|r| (r.name.clone(), r.label.clone()))
        .collect();

    let fields_json = uf::fields_json(&available_roles);

    let user_page = user::find_paginated(&pool, page, per_page, &filter, &sort).await?;

    let tmpl = UserListTemplate {
        ctx,
        user_page,
        filter_json,
        filter_active,
        sort_column: sort.column.clone(),
        sort_dir: sort.dir_str().to_string(),
        columns,
        available_roles,
        fields_json,
    };
    render(tmpl)
}
