use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize)]
pub struct NavItemPreview {
    pub id: i64,
    pub label: String,
    pub path: String,
    pub module_name: String,
}

pub async fn find_accessible_nav_items(
    pool: &PgPool,
    permission_ids: &[i64],
) -> Result<Vec<NavItemPreview>, sqlx::Error> {
    if permission_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Convert permission IDs to permission codes (entity names)
    let permission_codes: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT name FROM entities WHERE id = ANY($1)"
    )
    .bind(permission_ids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(name,)| name)
    .collect();

    if permission_codes.is_empty() {
        return Ok(Vec::new());
    }

    // Query all active nav items with permission and parent info
    // Same join pattern as nav_item.rs find_navigation
    #[derive(sqlx::FromRow)]
    struct RawItem {
        id: i64,
        name: String,
        label: String,
        url: String,
        permission_code: String,
        parent: String,
    }

    let all_items: Vec<RawItem> = sqlx::query_as::<_, RawItem>(
        "SELECT e.id, e.name, e.label,
                COALESCE(p_url.value, '') AS url,
                COALESCE(perm.name, '') AS permission_code,
                COALESCE(p_parent.value, '') AS parent
         FROM entities e
         LEFT JOIN entity_properties p_url
             ON e.id = p_url.entity_id AND p_url.key = 'url'
         LEFT JOIN entity_properties p_parent
             ON e.id = p_parent.entity_id AND p_parent.key = 'parent'
         LEFT JOIN relations r_perm
             ON e.id = r_perm.source_id
             AND r_perm.relation_type_id = (
                 SELECT id FROM entities
                 WHERE entity_type = 'relation_type' AND name = 'requires_permission'
             )
         LEFT JOIN entities perm
             ON r_perm.target_id = perm.id AND perm.entity_type = 'permission'
         WHERE e.entity_type = 'nav_item' AND e.is_active = true
         ORDER BY e.sort_order, e.id"
    )
    .fetch_all(pool)
    .await?;

    let top_level: Vec<&RawItem> = all_items.iter().filter(|i| i.parent.is_empty()).collect();
    let children: Vec<&RawItem> = all_items.iter().filter(|i| !i.parent.is_empty()).collect();

    let has_permission = |code: &str| -> bool {
        code.is_empty() || permission_codes.iter().any(|c| c == code)
    };

    let mut results: Vec<NavItemPreview> = Vec::new();

    for module in &top_level {
        let module_children: Vec<&&RawItem> = children.iter()
            .filter(|c| c.parent == module.name)
            .collect();

        if module_children.is_empty() {
            // Standalone module (e.g. Dashboard) — show as own item if permitted
            if has_permission(&module.permission_code) {
                results.push(NavItemPreview {
                    id: module.id,
                    label: module.label.clone(),
                    path: module.url.clone(),
                    module_name: module.label.clone(),
                });
            }
        } else {
            // Module with children — include accessible children
            for child in &module_children {
                if has_permission(&child.permission_code) {
                    results.push(NavItemPreview {
                        id: child.id,
                        label: child.label.clone(),
                        path: child.url.clone(),
                        module_name: module.label.clone(),
                    });
                }
            }
        }
    }

    Ok(results)
}
