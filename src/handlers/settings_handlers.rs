use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;

use crate::db::DbPool;
use crate::models::setting;
use crate::auth::csrf;
use crate::auth::session::require_permission;
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

    let ctx = PageContext::build(&session, &conn, "/settings");
    let settings = setting::find_all(&conn).unwrap_or_default();

    let tmpl = SettingsTemplate { ctx, settings };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

fn get_field<'a>(params: &'a [(String, String)], key: &str) -> &'a str {
    params.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

pub async fn save(
    pool: web::Data<DbPool>,
    session: Session,
    body: String,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "settings.manage") {
        return resp;
    }

    let params = parse_form_body(&body);
    if let Err(resp) = csrf::validate_csrf(&session, get_field(&params, "csrf_token")) {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    // Each setting is submitted as setting_<id>=<value>
    for (key, value) in &params {
        if let Some(id_str) = key.strip_prefix("setting_") {
            if let Ok(id) = id_str.parse::<i64>() {
                let _ = setting::update_value(&conn, id, value.trim());
            }
        }
    }

    let _ = session.insert("flash", "Settings saved successfully");
    HttpResponse::SeeOther()
        .insert_header(("Location", "/settings"))
        .finish()
}
