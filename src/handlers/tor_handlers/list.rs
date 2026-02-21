use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::tor;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, TorListTemplate};

pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let ctx = PageContext::build(&session, &pool, "/tor").await?;
    let tors = tor::find_all_list_items(&pool).await?;

    let tmpl = TorListTemplate { ctx, tors };
    render(tmpl)
}
