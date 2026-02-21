use sqlx::PgPool;

use crate::auth::session::Permissions;

pub struct NavModule {
    pub label: String,
    pub url: String,
    pub is_active: bool,
}

pub struct NavSidebarItem {
    pub label: String,
    pub url: String,
    pub is_active: bool,
}

#[derive(sqlx::FromRow)]
struct RawNavRow {
    nav_name: String,
    label: String,
    url: String,
    permission_code: String,
    parent: String,
}

/// Returns (header_modules, sidebar_items) for the current user and path.
pub async fn find_navigation(
    pool: &PgPool,
    permissions: &Permissions,
    current_path: &str,
) -> (Vec<NavModule>, Vec<NavSidebarItem>) {
    let raw_rows = sqlx::query_as::<_, RawNavRow>(
        "SELECT e.name AS nav_name, e.label, \
                COALESCE(p_url.value, '') AS url, \
                COALESCE(perm.name, '') AS permission_code, \
                COALESCE(p_parent.value, '') AS parent \
         FROM entities e \
         LEFT JOIN entity_properties p_url \
             ON e.id = p_url.entity_id AND p_url.key = 'url' \
         LEFT JOIN entity_properties p_parent \
             ON e.id = p_parent.entity_id AND p_parent.key = 'parent' \
         LEFT JOIN relations r_perm \
             ON e.id = r_perm.source_id \
             AND r_perm.relation_type_id = ( \
                 SELECT id FROM entities \
                 WHERE entity_type = 'relation_type' AND name = 'requires_permission' \
             ) \
         LEFT JOIN entities perm \
             ON r_perm.target_id = perm.id AND perm.entity_type = 'permission' \
         WHERE e.entity_type = 'nav_item' AND e.is_active = true \
         ORDER BY e.sort_order, e.id"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let rows: Vec<(String, RawNavItem)> = raw_rows
        .into_iter()
        .map(|r| (r.nav_name, RawNavItem {
            label: r.label,
            url: r.url,
            permission_code: r.permission_code,
            parent: r.parent,
        }))
        .collect();

    // Partition: top-level (no parent) vs children
    let top_level: Vec<&(String, RawNavItem)> = rows.iter()
        .filter(|(_, item)| item.parent.is_empty())
        .collect();

    let children: Vec<&(String, RawNavItem)> = rows.iter()
        .filter(|(_, item)| !item.parent.is_empty())
        .collect();

    // Determine active module from current_path
    let active_module_name = find_active_module(current_path, &top_level, &children);

    // Build header modules
    let modules: Vec<NavModule> = top_level.iter()
        .filter(|(name, item)| {
            let module_children: Vec<_> = children.iter()
                .filter(|(_, c)| c.parent == **name)
                .collect();
            if module_children.is_empty() {
                // Standalone: visible by own permission
                item.permission_code.is_empty() || permissions.has(&item.permission_code)
            } else {
                // Module: visible if at least one child is permitted
                module_children.iter()
                    .any(|(_, c)| c.permission_code.is_empty() || permissions.has(&c.permission_code))
            }
        })
        .map(|(name, item)| {
            NavModule {
                label: item.label.clone(),
                url: item.url.clone(),
                is_active: active_module_name.as_deref() == Some(name.as_str()),
            }
        })
        .collect();

    // Build sidebar: children of active module, filtered by permissions
    let sidebar: Vec<NavSidebarItem> = match &active_module_name {
        Some(module_name) => {
            let filtered: Vec<_> = children.iter()
                .filter(|(_, c)| c.parent == *module_name)
                .filter(|(_, c)| c.permission_code.is_empty() || permissions.has(&c.permission_code))
                .collect();

            // Longest-prefix match: only the most specific matching URL is active
            let best_match_len = filtered.iter()
                .filter(|(_, c)| current_path.starts_with(&c.url))
                .map(|(_, c)| c.url.len())
                .max()
                .unwrap_or(0);

            filtered.into_iter()
                .map(|(_, c)| {
                    let is_active = c.url.len() == best_match_len
                        && current_path.starts_with(&c.url);
                    NavSidebarItem {
                        label: c.label.clone(),
                        url: c.url.clone(),
                        is_active,
                    }
                })
                .collect()
        }
        None => vec![],
    };

    (modules, sidebar)
}

struct RawNavItem {
    label: String,
    url: String,
    permission_code: String,
    parent: String,
}

fn find_active_module(
    current_path: &str,
    top_level: &[&(String, RawNavItem)],
    children: &[&(String, RawNavItem)],
) -> Option<String> {
    // Check children first: if path matches a child, return its parent module
    for (_, child) in children {
        if current_path.starts_with(&child.url) && !child.parent.is_empty() {
            return Some(child.parent.clone());
        }
    }
    // Fallback: check top-level items directly
    for (name, item) in top_level {
        if current_path.starts_with(&item.url) {
            return Some(name.clone());
        }
    }
    None
}
