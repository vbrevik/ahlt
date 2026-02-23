use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::setting;
use crate::audit;
use crate::auth::csrf;
use crate::auth::session::{get_user_id, require_permission};
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, SettingsTemplate};

/// Decode a URL-encoded string (form data): `+` → space, `%HH` → byte.
fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i+1..i+3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Parse URL-encoded form body into key-value pairs.
fn parse_form_body(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((url_decode(k), url_decode(v)))
        })
        .collect()
}

pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let ctx = PageContext::build(&session, &pool, "/settings").await?;
    let settings = setting::find_all(&pool).await?;

    let tmpl = SettingsTemplate { ctx, settings };
    render(tmpl)
}

fn get_field<'a>(params: &'a [(String, String)], key: &str) -> &'a str {
    params.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

pub async fn save(
    pool: web::Data<PgPool>,
    session: Session,
    body: String,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let params = parse_form_body(&body);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let current_user_id = get_user_id(&session).unwrap_or(0);

    // Each setting is submitted as setting_<id>=<value>
    let mut changed = Vec::new();
    for (key, value) in &params {
        if let Some(id_str) = key.strip_prefix("setting_") {
            if let Ok(id) = id_str.parse::<i64>() {
                setting::update_value(&pool, id, value.trim()).await?;
                changed.push(id);
            }
        }
    }

    if !changed.is_empty() {
        let details = serde_json::json!({
            "setting_ids": changed,
            "count": changed.len(),
            "summary": format!("Updated {} setting(s)", changed.len())
        });
        let _ = audit::log(&pool, current_user_id, "settings.update", "setting", 0, details).await;
    }

    let _ = session.insert("flash", "Settings saved successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/settings"))
        .finish())
}
