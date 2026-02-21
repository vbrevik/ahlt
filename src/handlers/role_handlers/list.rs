use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::{role, user};
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, RoleAssignmentTemplate};

#[derive(serde::Deserialize)]
pub struct AssignmentQuery {
    pub role_id: Option<i64>,
    pub tab: Option<String>,
}

pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<AssignmentQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;

    let ctx = PageContext::build(&session, &pool, "/roles").await?;
    let roles = role::find_all_list_items(&pool).await?;

    let active_tab = query.tab.clone().unwrap_or_else(|| "by-role".to_string());

    // Select first role by default
    let selected_role_id = query.role_id.unwrap_or_else(|| {
        roles.first().map(|r| r.id).unwrap_or(0)
    });

    let members = if selected_role_id > 0 {
        role::find_users_by_role(&pool, selected_role_id).await?
    } else {
        vec![]
    };

    let available_users = if selected_role_id > 0 {
        role::find_users_not_in_role(&pool, selected_role_id).await?
    } else {
        vec![]
    };

    let users_with_roles = user::find_all_with_roles(&pool).await?;

    let tmpl = RoleAssignmentTemplate {
        ctx,
        roles,
        selected_role_id,
        members,
        available_users,
        users_with_roles,
        active_tab,
    };
    render(tmpl)
}
