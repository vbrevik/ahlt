use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::user;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserFormTemplate};

pub async fn edit_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.edit")?;

    let id = path.into_inner();

    match user::find_display_by_id(&pool, id).await {
        Ok(Some(u)) => {
            let ctx = PageContext::build(&session, &pool, "/users").await?;
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: Some(u),
                errors: vec![],
            };
            render(tmpl)
        }
        _ => Err(AppError::NotFound),
    }
}
