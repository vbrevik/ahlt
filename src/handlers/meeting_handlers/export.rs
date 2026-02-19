use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::minutes;
use crate::auth::session::require_permission;
use crate::errors::AppError;

/// GET /meetings/{id}/export — Return print-friendly HTML export of approved minutes
pub async fn export_minutes_html(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.view")?;

    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    // Fetch minutes
    let min = minutes::find_by_id(&conn, minutes_id)?
        .ok_or(AppError::NotFound)?;

    // Only allow export of approved minutes
    if min.status != "approved" {
        return Err(AppError::PermissionDenied("Can only export approved minutes".to_string()));
    }

    // Fetch sections
    let sections = minutes::find_sections(&conn, minutes_id)?;

    // Build HTML content
    let sections_html = sections
        .into_iter()
        .map(|s| {
            let icon = match s.section_type.as_str() {
                "attendance" => "👥",
                "protocol" => "📋",
                "agenda_items" => "📝",
                "decisions" => "✅",
                "action_items" => "🎯",
                _ => "📄",
            };
            
            format!(
                r#"<section class="minutes-section">
                    <h2>{} {}</h2>
                    <div class="section-content">{}</div>
                </section>"#,
                icon,
                s.label,
                s.content.replace("\n", "<br>")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Meeting Minutes — {}</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, system-ui, 'Segoe UI', Roboto, sans-serif;
            line-height: 1.6;
            color: #333;
            background: #fff;
        }}
        .page {{
            max-width: 900px;
            margin: 0 auto;
            padding: 2rem;
        }}
        header {{
            border-bottom: 3px solid #333;
            padding-bottom: 1.5rem;
            margin-bottom: 2rem;
        }}
        h1 {{
            font-size: 1.75rem;
            margin-bottom: 0.5rem;
        }}
        .meta {{
            display: grid;
            grid-template-columns: 1fr 1fr 1fr;
            gap: 1.5rem;
            margin-top: 1rem;
            font-size: 0.9rem;
            color: #666;
        }}
        .meta-item label {{
            font-weight: 600;
            color: #333;
            display: block;
            margin-bottom: 0.25rem;
        }}
        .minutes-section {{
            margin-bottom: 2rem;
            page-break-inside: avoid;
        }}
        h2 {{
            font-size: 1.25rem;
            margin-bottom: 0.75rem;
            color: #1c1917;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }}
        .section-content {{
            padding-left: 1rem;
            border-left: 3px solid #ddd;
            line-height: 1.8;
        }}
        .section-content br {{
            margin-bottom: 0.5rem;
        }}
        footer {{
            margin-top: 3rem;
            padding-top: 1.5rem;
            border-top: 1px solid #ddd;
            font-size: 0.85rem;
            color: #999;
            text-align: center;
        }}
        @media print {{
            body {{
                background: none;
                padding: 0;
            }}
            .page {{
                max-width: none;
                padding: 0;
            }}
            header, .minutes-section {{
                page-break-inside: avoid;
            }}
            h1, h2 {{
                page-break-after: avoid;
            }}
        }}
    </style>
</head>
<body>
    <div class="page">
        <header>
            <h1>Meeting Minutes</h1>
            <div class="meta">
                <div class="meta-item">
                    <label>Meeting</label>
                    <span>{}</span>
                </div>
                <div class="meta-item">
                    <label>Generated</label>
                    <span>{}</span>
                </div>
                <div class="meta-item">
                    <label>Status</label>
                    <span style="font-weight: 600; color: #15803d;">Approved</span>
                </div>
            </div>
        </header>
        
        <main>
            {}
        </main>
        
        <footer>
            <p>This is an approved record. Print this page to PDF for permanent archival.</p>
        </footer>
    </div>
</body>
</html>"#,
        min.label,
        min.meeting_name,
        min.generated_date,
        sections_html
    );

    // Audit log the export
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "minutes_id": minutes_id,
        "minutes_label": min.label,
        "format": "html",
        "summary": "Minutes exported to HTML"
    });
    let _ = crate::audit::log(&conn, current_user_id, "minutes.exported", "minutes", minutes_id, details);

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header((
            "Content-Disposition",
            format!("inline; filename=\"minutes-{}.html\"", minutes_id),
        ))
        .body(html))
}
