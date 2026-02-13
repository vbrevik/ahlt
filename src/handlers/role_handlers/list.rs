use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::role;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, RoleListTemplate};

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let conn = pool.get()?;

    let ctx = PageContext::build(&session, &conn, "/roles")?;
    let roles = role::find_all_list_items(&conn)?;

    let tmpl = RoleListTemplate { ctx, roles };
    render(tmpl)
}
