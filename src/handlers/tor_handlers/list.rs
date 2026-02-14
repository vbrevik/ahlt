use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::tor;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, TorListTemplate};

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/tor")?;
    let tors = tor::find_all_list_items(&conn)?;

    let tmpl = TorListTemplate { ctx, tors };
    render(tmpl)
}
