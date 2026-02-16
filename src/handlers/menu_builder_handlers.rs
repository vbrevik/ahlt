use std::collections::HashSet;
use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::audit;
use crate::auth::csrf;
use crate::auth::session::{get_user_id, require_permission};
use crate::db::DbPool;
use crate::errors::{render, AppError};
use crate::handlers::role_handlers::helpers::parse_form_body;
use crate::models::{permission, role};
use crate::templates_structs::{
    MatrixCell, MenuBuilderTemplate, PageContext, PageGroup, PermissionRow, RoleColumn,
};

/// GET /menu-builder — render the permission matrix.
pub async fn index(
    session: Session,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/menu-builder")?;

    // Load all roles (columns)
    let all_roles = role::find_all_display(&conn)?;
    let roles: Vec<RoleColumn> = all_roles
        .iter()
        .map(|r| RoleColumn {
            id: r.id,
            name: r.name.clone(),
            label: r.label.clone(),
        })
        .collect();

    // Load all permissions with group_name (rows)
    let all_perms = permission::find_all_with_groups(&conn)?;

    // Load current grants (role_id, permission_id) pairs
    let grants = permission::find_all_role_grants(&conn)?;

    // Group permissions by group_name and build matrix cells
    let mut groups: Vec<PageGroup> = Vec::new();
    let mut current_group: Option<String> = None;
    let mut current_perms: Vec<PermissionRow> = Vec::new();

    for perm in &all_perms {
        if current_group.as_deref() != Some(&perm.group_name) {
            if let Some(gn) = current_group.take() {
                groups.push(PageGroup {
                    group_name: gn,
                    permissions: std::mem::take(&mut current_perms),
                });
            }
            current_group = Some(perm.group_name.clone());
        }

        let cells: Vec<MatrixCell> = roles
            .iter()
            .map(|r| MatrixCell {
                role_id: r.id,
                permission_id: perm.id,
                checked: grants.contains(&(r.id, perm.id)),
            })
            .collect();

        current_perms.push(PermissionRow {
            code: perm.code.clone(),
            label: perm.label.clone(),
            cells,
        });
    }

    // Flush last group
    if let Some(gn) = current_group {
        groups.push(PageGroup {
            group_name: gn,
            permissions: current_perms,
        });
    }

    let col_count = roles.len() + 1;

    render(MenuBuilderTemplate {
        ctx,
        roles,
        page_groups: groups,
        col_count,
    })
}

/// POST /menu-builder — save permission matrix changes.
pub async fn save(
    session: Session,
    pool: web::Data<DbPool>,
    body: String,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    let conn = pool.get()?;

    // Parse form and validate CSRF
    let params = parse_form_body(&body);
    let csrf_token = params
        .iter()
        .find(|(k, _)| k == "csrf_token")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    csrf::validate_csrf(&session, csrf_token)?;

    // Parse submitted checkbox names: perm_{role_id}_{permission_id}
    let submitted: HashSet<(i64, i64)> = params
        .iter()
        .filter_map(|(key, _)| {
            let rest = key.strip_prefix("perm_")?;
            let mut parts = rest.splitn(2, '_');
            let role_id = parts.next()?.parse::<i64>().ok()?;
            let perm_id = parts.next()?.parse::<i64>().ok()?;
            Some((role_id, perm_id))
        })
        .collect();

    // Load current grants
    let current = permission::find_all_role_grants(&conn)?;

    // Compute diff
    let to_grant: Vec<(i64, i64)> = submitted.difference(&current).copied().collect();
    let to_revoke: Vec<(i64, i64)> = current.difference(&submitted).copied().collect();
    let changes = to_grant.len() + to_revoke.len();

    // Apply changes
    for (role_id, perm_id) in &to_grant {
        permission::grant_permission(&conn, *role_id, *perm_id)?;
    }
    for (role_id, perm_id) in &to_revoke {
        permission::revoke_permission(&conn, *role_id, *perm_id)?;
    }

    // Audit log
    if changes > 0 {
        let user_id = get_user_id(&session).unwrap_or(0);
        let summary = format!("{} granted, {} revoked via Menu Builder", to_grant.len(), to_revoke.len());
        if let Err(e) = audit::log(
            &conn,
            user_id,
            "role.permissions_changed",
            "role",
            0,
            json!({ "summary": summary, "granted": to_grant.len(), "revoked": to_revoke.len() }),
        ) {
            eprintln!("Audit log failed: {}", e);
        }
    }

    // Flash message and redirect
    let msg = if changes > 0 {
        format!(
            "Permissions updated ({} granted, {} revoked)",
            to_grant.len(),
            to_revoke.len()
        )
    } else {
        "No changes made".to_string()
    };
    session
        .insert("flash", &msg)
        .map_err(|e| AppError::Session(e.to_string()))?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/menu-builder"))
        .finish())
}
