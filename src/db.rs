use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

pub type DbPool = Pool<SqliteConnectionManager>;

pub const MIGRATIONS: &str = include_str!("schema.sql");

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = SqliteConnectionManager::file(database_url).with_init(|conn| {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(())
    });
    Pool::builder()
        .max_size(8)
        .build(manager)
        .expect("Failed to create DB pool")
}

pub fn run_migrations(pool: &DbPool) {
    let conn = pool.get().expect("Failed to get DB connection for migrations");
    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");
    log::info!("Database migrations complete");
}

/// Helper: insert entity and return its id.
fn insert_entity(conn: &rusqlite::Connection, entity_type: &str, name: &str, label: &str, sort_order: i64) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![entity_type, name, label, sort_order],
    ).unwrap();
    conn.last_insert_rowid()
}

/// Helper: insert property.
fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![entity_id, key, value],
    ).unwrap();
}

/// Helper: insert relation.
fn insert_relation(conn: &rusqlite::Connection, rel_type_id: i64, source_id: i64, target_id: i64) {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![rel_type_id, source_id, target_id],
    ).unwrap();
}

/// Seed the full ontology: relation types, roles, permissions, role-permission relations,
/// and default admin user. Only runs if no entities exist yet.
pub fn seed_ontology(pool: &DbPool, admin_password_hash: &str) {
    let conn = pool.get().expect("Failed to get DB connection for seeding");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
        .unwrap_or(0);

    if count > 0 {
        return;
    }

    log::info!("Seeding ontology...");

    // --- Relation types ---
    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role", 0);
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission", 0);
    let _requires_perm_id = insert_entity(&conn, "relation_type", "requires_permission", "Requires Permission", 0);

    // --- ToR relation types ---
    // DEPRECATED: replaced by fills_position
    let _member_of_id = insert_entity(&conn, "relation_type", "member_of", "Member Of", 0);
    // DEPRECATED: replaced by fills_position
    let _has_tor_role_id = insert_entity(&conn, "relation_type", "has_tor_role", "Has ToR Role", 0);
    let _belongs_to_tor_id = insert_entity(&conn, "relation_type", "belongs_to_tor", "Belongs to ToR", 0);
    let _fills_position_id = insert_entity(&conn, "relation_type", "fills_position", "Fills Position", 0);

    // --- Protocol relation types ---
    let _protocol_of_id = insert_entity(&conn, "relation_type", "protocol_of", "Protocol Of", 0);

    // --- Dependency relation types ---
    let _feeds_into_id = insert_entity(&conn, "relation_type", "feeds_into", "Feeds Into", 0);
    let _escalates_to_id = insert_entity(&conn, "relation_type", "escalates_to", "Escalates To", 0);

    // --- Minutes relation types ---
    let _minutes_of_id = insert_entity(&conn, "relation_type", "minutes_of", "Minutes Of", 0);
    let _section_of_id = insert_entity(&conn, "relation_type", "section_of", "Section Of", 0);

    // --- Presentation template relation types ---
    let _template_of_id = insert_entity(&conn, "relation_type", "template_of", "Template Of", 0);
    let _slide_of_id = insert_entity(&conn, "relation_type", "slide_of", "Slide Of", 0);
    let _requires_template_id = insert_entity(&conn, "relation_type", "requires_template", "Requires Template", 0);

    // --- Item workflow relation types ---
    let _suggested_to_id = insert_entity(&conn, "relation_type", "suggested_to", "Suggested To", 0);
    let _spawns_proposal_id = insert_entity(&conn, "relation_type", "spawns_proposal", "Spawns Proposal", 0);
    let _submitted_to_id = insert_entity(&conn, "relation_type", "submitted_to", "Submitted To", 0);

    // --- Phase 2b: Workflow, agenda, COA, and opinion relation types ---
    let _transition_from_id = insert_entity(&conn, "relation_type", "transition_from", "Transition From", 0);
    let _transition_to_id = insert_entity(&conn, "relation_type", "transition_to", "Transition To", 0);
    let _considers_coa_id = insert_entity(&conn, "relation_type", "considers_coa", "Considers COA", 0);
    let _originates_from_id = insert_entity(&conn, "relation_type", "originates_from", "Originates From", 0);
    let _has_section_id = insert_entity(&conn, "relation_type", "has_section", "Has Section", 0);
    let _has_subsection_id = insert_entity(&conn, "relation_type", "has_subsection", "Has Subsection", 0);
    let _agenda_submitted_to_id = insert_entity(&conn, "relation_type", "agenda_submitted_to", "Agenda Submitted To", 0);
    let _spawns_agenda_point_id = insert_entity(&conn, "relation_type", "spawns_agenda_point", "Spawns Agenda Point", 0);
    let _opinion_by_id = insert_entity(&conn, "relation_type", "opinion_by", "Opinion By", 0);
    let _opinion_on_id = insert_entity(&conn, "relation_type", "opinion_on", "Opinion On", 0);
    let _prefers_coa_id = insert_entity(&conn, "relation_type", "prefers_coa", "Prefers COA", 0);
    let _presents_id = insert_entity(&conn, "relation_type", "presents", "Presents", 0);

    // --- Warning system relation types ---
    let _targets_user_id = insert_entity(&conn, "relation_type", "targets_user", "Targets User", 0);
    let _for_warning_id = insert_entity(&conn, "relation_type", "for_warning", "For Warning", 0);
    let _for_user_id = insert_entity(&conn, "relation_type", "for_user", "For User", 0);
    let _on_receipt_id = insert_entity(&conn, "relation_type", "on_receipt", "On Receipt", 0);
    let _forwarded_to_user_id = insert_entity(&conn, "relation_type", "forwarded_to_user", "Forwarded To User", 0);

    // --- Roles ---
    let admin_role_id = insert_entity(&conn, "role", "admin", "Administrator", 1);
    insert_prop(&conn, admin_role_id, "description", "Full system access");

    let user_role_id = insert_entity(&conn, "role", "user", "User", 2);
    insert_prop(&conn, user_role_id, "description", "Standard user access");
    insert_prop(&conn, user_role_id, "is_default", "1");

    // --- Permissions ---
    let perms = [
        ("dashboard.view", "View Dashboard", "Dashboard"),
        ("users.list", "List Users", "Users"),
        ("users.create", "Create Users", "Users"),
        ("users.edit", "Edit Users", "Users"),
        ("users.delete", "Delete Users", "Users"),
        ("roles.manage", "Manage Roles", "Roles"),
        ("settings.manage", "Manage Settings", "Settings"),
        ("audit.view", "View Audit Log", "Admin"),
        ("tor.list", "List Terms of Reference", "Governance"),
        ("tor.create", "Create Terms of Reference", "Governance"),
        ("tor.edit", "Edit Terms of Reference", "Governance"),
        ("tor.manage_members", "Manage ToR Members", "Governance"),
        ("suggestion.view", "View suggestions in member ToRs", "Workflow"),
        ("suggestion.create", "Submit new suggestions", "Workflow"),
        ("suggestion.review", "Accept or reject suggestions", "Workflow"),
        ("proposal.view", "View proposals in member ToRs", "Workflow"),
        ("proposal.create", "Create new proposals", "Workflow"),
        ("proposal.submit", "Submit draft proposals for review", "Workflow"),
        ("proposal.edit", "Edit draft proposals", "Workflow"),
        ("proposal.review", "Move proposals to under_review status", "Workflow"),
        ("proposal.approve", "Approve or reject proposals under review", "Workflow"),
        // --- Phase 2b: Agenda, workflow, and COA permissions ---
        ("agenda.view", "View Agenda", "Governance"),
        ("agenda.create", "Create Agenda Points", "Governance"),
        ("agenda.queue", "Queue Proposals for Agenda", "Governance"),
        ("agenda.manage", "Manage Agenda Status", "Governance"),
        ("agenda.participate", "Participate in Meeting", "Governance"),
        ("agenda.decide", "Make Final Decisions", "Governance"),
        ("coa.create", "Create Courses of Action", "Workflow"),
        ("coa.edit", "Edit Courses of Action", "Workflow"),
        ("workflow.manage", "Manage Workflow System", "Governance"),
        ("warnings.view", "View Warnings", "Admin"),
        ("minutes.generate", "Generate Meeting Minutes", "Governance"),
        ("minutes.edit", "Edit Meeting Minutes", "Governance"),
        ("minutes.approve", "Approve Meeting Minutes", "Governance"),
    ];

    let mut perm_ids: Vec<(i64, &str)> = Vec::new();
    for (code, label, group) in &perms {
        let id = insert_entity(&conn, "permission", code, label, 0);
        insert_prop(&conn, id, "group_name", group);
        perm_ids.push((id, code));
    }

    // Query permission IDs for later nav→permission relations
    let dashboard_view_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='dashboard.view'",
        [],
        |row| row.get(0),
    ).unwrap();

    let users_list_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='users.list'",
        [],
        |row| row.get(0),
    ).unwrap();

    let roles_manage_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='roles.manage'",
        [],
        |row| row.get(0),
    ).unwrap();

    let settings_manage_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='settings.manage'",
        [],
        |row| row.get(0),
    ).unwrap();

    let audit_view_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='audit.view'",
        [],
        |row| row.get(0),
    ).unwrap();

    // Get requires_permission relation type ID
    let requires_permission_rel_type_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='requires_permission'",
        [],
        |row| row.get(0),
    ).unwrap();

    // --- Role-permission relations ---
    // Admin gets all permissions
    for (perm_id, _) in &perm_ids {
        insert_relation(&conn, has_perm_id, admin_role_id, *perm_id);
    }

    // User gets dashboard.view + users.list
    let basic_perms = ["dashboard.view", "users.list"];
    for (perm_id, code) in &perm_ids {
        if basic_perms.contains(code) {
            insert_relation(&conn, has_perm_id, user_role_id, *perm_id);
        }
    }

    // --- Default admin user ---
    let admin_user_id = insert_entity(&conn, "user", "admin", "Administrator", 0);
    insert_prop(&conn, admin_user_id, "password", admin_password_hash);
    insert_prop(&conn, admin_user_id, "email", "admin@example.com");
    insert_relation(&conn, has_role_id, admin_user_id, admin_role_id);

    // --- Nav items (two-tier: modules in header, pages in sidebar) ---
    // Dashboard: standalone top-level item (no children → no sidebar)
    let nav_dashboard_id = insert_entity(&conn, "nav_item", "dashboard", "Dashboard", 1);
    insert_prop(&conn, nav_dashboard_id, "url", "/dashboard");

    // Admin: module header item (children appear in sidebar)
    let _nav_admin_id = insert_entity(&conn, "nav_item", "admin", "Admin", 2);
    insert_prop(&conn, _nav_admin_id, "url", "/users");

    // Admin → Users: sidebar child
    let nav_admin_users_id = insert_entity(&conn, "nav_item", "admin.users", "Users", 1);
    insert_prop(&conn, nav_admin_users_id, "url", "/users");
    insert_prop(&conn, nav_admin_users_id, "parent", "admin");

    // Admin → Roles: sidebar child
    let nav_admin_roles_id = insert_entity(&conn, "nav_item", "admin.roles", "Roles", 2);
    insert_prop(&conn, nav_admin_roles_id, "url", "/roles");
    insert_prop(&conn, nav_admin_roles_id, "parent", "admin");

    // Admin → Ontology: sidebar child
    let nav_admin_ontology_id = insert_entity(&conn, "nav_item", "admin.ontology", "Ontology", 3);
    insert_prop(&conn, nav_admin_ontology_id, "url", "/ontology");
    insert_prop(&conn, nav_admin_ontology_id, "parent", "admin");

    // --- Settings ---
    let setting_name_id = insert_entity(&conn, "setting", "app.name", "Application Name", 1);
    insert_prop(&conn, setting_name_id, "value", "Ahlt");
    insert_prop(&conn, setting_name_id, "description", "The name displayed in the navbar and page titles");
    insert_prop(&conn, setting_name_id, "setting_type", "text");

    let setting_desc_id = insert_entity(&conn, "setting", "app.description", "Application Description", 2);
    insert_prop(&conn, setting_desc_id, "value", "Administration Platform");
    insert_prop(&conn, setting_desc_id, "description", "A short description of this application");
    insert_prop(&conn, setting_desc_id, "setting_type", "text");

    // Admin → Settings: sidebar child
    let nav_admin_settings_id = insert_entity(&conn, "nav_item", "admin.settings", "Settings", 4);
    insert_prop(&conn, nav_admin_settings_id, "url", "/settings");
    insert_prop(&conn, nav_admin_settings_id, "parent", "admin");

    // Admin → Audit Log: sidebar child
    let nav_admin_audit_id = insert_entity(&conn, "nav_item", "admin.audit", "Audit Log", 5);
    insert_prop(&conn, nav_admin_audit_id, "url", "/audit");
    insert_prop(&conn, nav_admin_audit_id, "parent", "admin");

    // Admin → Menu Builder: sidebar child
    let nav_admin_menu_builder_id = insert_entity(&conn, "nav_item", "admin.menu_builder", "Menu Builder", 6);
    insert_prop(&conn, nav_admin_menu_builder_id, "url", "/menu-builder");
    insert_prop(&conn, nav_admin_menu_builder_id, "parent", "admin");

    // Admin → Warnings: sidebar child
    let nav_admin_warnings_id = insert_entity(&conn, "nav_item", "admin.warnings", "Warnings", 7);
    insert_prop(&conn, nav_admin_warnings_id, "url", "/warnings");
    insert_prop(&conn, nav_admin_warnings_id, "parent", "admin");

    // Admin → Role Builder: sidebar child
    let nav_admin_role_builder_id = insert_entity(&conn, "nav_item", "admin.role_builder", "Role Builder", 8);
    insert_prop(&conn, nav_admin_role_builder_id, "url", "/roles/builder");
    insert_prop(&conn, nav_admin_role_builder_id, "parent", "admin");

    // Governance: module header
    let _nav_governance_id = insert_entity(&conn, "nav_item", "governance", "Governance", 3);
    insert_prop(&conn, _nav_governance_id, "url", "/tor");

    // Governance -> Terms of Reference: sidebar child
    let nav_gov_tor_id = insert_entity(&conn, "nav_item", "governance.tor", "Terms of Reference", 1);
    insert_prop(&conn, nav_gov_tor_id, "url", "/tor");
    insert_prop(&conn, nav_gov_tor_id, "parent", "governance");

    // Governance -> Map: sidebar child
    let nav_gov_map_id = insert_entity(&conn, "nav_item", "governance.map", "Governance Map", 2);
    insert_prop(&conn, nav_gov_map_id, "url", "/governance/map");
    insert_prop(&conn, nav_gov_map_id, "parent", "governance");

    // --- Audit settings ---
    let audit_enabled_id = insert_entity(&conn, "setting", "audit.enabled", "Enable Audit Logging", 3);
    insert_prop(&conn, audit_enabled_id, "value", "true");
    insert_prop(&conn, audit_enabled_id, "setting_type", "boolean");
    insert_prop(&conn, audit_enabled_id, "description", "Master toggle for audit logging (database and filesystem)");

    let audit_log_path_id = insert_entity(&conn, "setting", "audit.log_path", "Audit Log Directory", 4);
    insert_prop(&conn, audit_log_path_id, "value", "data/audit/");
    insert_prop(&conn, audit_log_path_id, "setting_type", "text");
    insert_prop(&conn, audit_log_path_id, "description", "Directory path for audit log files (absolute or relative)");

    let audit_retention_id = insert_entity(&conn, "setting", "audit.retention_days", "Audit Retention (Days)", 5);
    insert_prop(&conn, audit_retention_id, "value", "90");
    insert_prop(&conn, audit_retention_id, "setting_type", "number");
    insert_prop(&conn, audit_retention_id, "description", "Days to keep audit entries in database (0 = forever)");

    // --- Warning retention settings ---
    let warn_ret_resolved = insert_entity(&conn, "setting", "warnings.retention_resolved_days", "Warning Retention (Resolved)", 6);
    insert_prop(&conn, warn_ret_resolved, "value", "30");
    insert_prop(&conn, warn_ret_resolved, "description", "Days to keep resolved warnings before deletion");
    insert_prop(&conn, warn_ret_resolved, "setting_type", "number");

    let warn_ret_info = insert_entity(&conn, "setting", "warnings.retention_info_days", "Warning Auto-Resolve (Info)", 7);
    insert_prop(&conn, warn_ret_info, "value", "90");
    insert_prop(&conn, warn_ret_info, "description", "Days before info-severity warnings are auto-resolved");
    insert_prop(&conn, warn_ret_info, "setting_type", "number");

    let warn_ret_deleted = insert_entity(&conn, "setting", "warnings.retention_deleted_days", "Warning Retention (Deleted)", 8);
    insert_prop(&conn, warn_ret_deleted, "value", "7");
    insert_prop(&conn, warn_ret_deleted, "description", "Days to keep fully-dismissed warnings before deletion");
    insert_prop(&conn, warn_ret_deleted, "setting_type", "number");

    // --- Nav→permission relations ---
    // Dashboard requires dashboard.view
    insert_relation(&conn, requires_permission_rel_type_id, nav_dashboard_id, dashboard_view_perm_id);

    // Admin > Users requires users.list
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_users_id, users_list_perm_id);

    // Admin > Roles requires roles.manage
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_roles_id, roles_manage_perm_id);

    // Admin > Ontology requires settings.manage
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_ontology_id, settings_manage_perm_id);

    // Admin > Settings requires settings.manage
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_settings_id, settings_manage_perm_id);

    // Admin > Audit Log requires audit.view
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_audit_id, audit_view_perm_id);

    // Admin > Menu Builder requires roles.manage
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_menu_builder_id, roles_manage_perm_id);

    // Admin > Role Builder requires roles.manage
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_role_builder_id, roles_manage_perm_id);

    // Admin > Warnings requires warnings.view
    let warnings_view_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='warnings.view'",
        [], |row| row.get(0),
    ).unwrap();
    insert_relation(&conn, requires_permission_rel_type_id, nav_admin_warnings_id, warnings_view_perm_id);

    // Governance > Terms of Reference requires tor.list
    let tor_list_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='tor.list'",
        [], |row| row.get(0),
    ).unwrap();

    insert_relation(&conn, requires_permission_rel_type_id, nav_gov_tor_id, tor_list_perm_id);

    // Governance -> Item Workflow: sidebar child
    let nav_gov_workflow_id = insert_entity(&conn, "nav_item", "governance.workflow", "Item Workflow", 2);
    insert_prop(&conn, nav_gov_workflow_id, "url", "/workflow");
    insert_prop(&conn, nav_gov_workflow_id, "parent", "governance");

    // Workflow requires suggestion.view permission
    let suggestion_view_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='permission' AND name='suggestion.view'",
        [], |row| row.get(0),
    ).unwrap();
    insert_relation(&conn, requires_permission_rel_type_id, nav_gov_workflow_id, suggestion_view_perm_id);

    // --- Suggestion Workflow Definitions ---
    let transition_from_rel_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='transition_from'",
        [], |row| row.get(0),
    ).unwrap();
    let transition_to_rel_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='transition_to'",
        [], |row| row.get(0),
    ).unwrap();

    // Suggestion workflow statuses
    let s_open = insert_entity(&conn, "workflow_status", "suggestion.open", "Open", 1);
    insert_prop(&conn, s_open, "entity_type_scope", "suggestion");
    insert_prop(&conn, s_open, "status_code", "open");
    insert_prop(&conn, s_open, "label", "Open");
    insert_prop(&conn, s_open, "is_initial", "true");
    insert_prop(&conn, s_open, "order", "1");

    let s_accepted = insert_entity(&conn, "workflow_status", "suggestion.accepted", "Accepted", 2);
    insert_prop(&conn, s_accepted, "entity_type_scope", "suggestion");
    insert_prop(&conn, s_accepted, "status_code", "accepted");
    insert_prop(&conn, s_accepted, "label", "Accepted");
    insert_prop(&conn, s_accepted, "is_terminal", "true");
    insert_prop(&conn, s_accepted, "order", "2");

    let s_rejected = insert_entity(&conn, "workflow_status", "suggestion.rejected", "Rejected", 3);
    insert_prop(&conn, s_rejected, "entity_type_scope", "suggestion");
    insert_prop(&conn, s_rejected, "status_code", "rejected");
    insert_prop(&conn, s_rejected, "label", "Rejected");
    insert_prop(&conn, s_rejected, "is_terminal", "true");
    insert_prop(&conn, s_rejected, "order", "3");

    // Suggestion workflow transitions
    let st_accept = insert_entity(&conn, "workflow_transition", "suggestion.open_to_accepted", "Accept", 0);
    insert_prop(&conn, st_accept, "entity_type_scope", "suggestion");
    insert_prop(&conn, st_accept, "from_status_code", "open");
    insert_prop(&conn, st_accept, "to_status_code", "accepted");
    insert_prop(&conn, st_accept, "transition_label", "Accept");
    insert_prop(&conn, st_accept, "required_permission", "suggestion.review");
    insert_prop(&conn, st_accept, "requires_outcome", "false");
    insert_relation(&conn, transition_from_rel_id, st_accept, s_open);
    insert_relation(&conn, transition_to_rel_id, st_accept, s_accepted);

    let st_reject = insert_entity(&conn, "workflow_transition", "suggestion.open_to_rejected", "Reject", 0);
    insert_prop(&conn, st_reject, "entity_type_scope", "suggestion");
    insert_prop(&conn, st_reject, "from_status_code", "open");
    insert_prop(&conn, st_reject, "to_status_code", "rejected");
    insert_prop(&conn, st_reject, "transition_label", "Reject");
    insert_prop(&conn, st_reject, "required_permission", "suggestion.review");
    insert_prop(&conn, st_reject, "requires_outcome", "true");
    insert_relation(&conn, transition_from_rel_id, st_reject, s_open);
    insert_relation(&conn, transition_to_rel_id, st_reject, s_rejected);

    let st_reverse = insert_entity(&conn, "workflow_transition", "suggestion.accepted_to_rejected", "Reverse", 0);
    insert_prop(&conn, st_reverse, "entity_type_scope", "suggestion");
    insert_prop(&conn, st_reverse, "from_status_code", "accepted");
    insert_prop(&conn, st_reverse, "to_status_code", "rejected");
    insert_prop(&conn, st_reverse, "transition_label", "Reverse");
    insert_prop(&conn, st_reverse, "required_permission", "suggestion.review");
    insert_prop(&conn, st_reverse, "requires_outcome", "false");
    insert_relation(&conn, transition_from_rel_id, st_reverse, s_accepted);
    insert_relation(&conn, transition_to_rel_id, st_reverse, s_rejected);

    // Proposal workflow statuses
    let p_draft = insert_entity(&conn, "workflow_status", "proposal.draft", "Draft", 1);
    insert_prop(&conn, p_draft, "entity_type_scope", "proposal");
    insert_prop(&conn, p_draft, "status_code", "draft");
    insert_prop(&conn, p_draft, "label", "Draft");
    insert_prop(&conn, p_draft, "is_initial", "true");
    insert_prop(&conn, p_draft, "order", "1");

    let p_submitted = insert_entity(&conn, "workflow_status", "proposal.submitted", "Submitted", 2);
    insert_prop(&conn, p_submitted, "entity_type_scope", "proposal");
    insert_prop(&conn, p_submitted, "status_code", "submitted");
    insert_prop(&conn, p_submitted, "label", "Submitted");
    insert_prop(&conn, p_submitted, "order", "2");

    let p_under_review = insert_entity(&conn, "workflow_status", "proposal.under_review", "Under Review", 3);
    insert_prop(&conn, p_under_review, "entity_type_scope", "proposal");
    insert_prop(&conn, p_under_review, "status_code", "under_review");
    insert_prop(&conn, p_under_review, "label", "Under Review");
    insert_prop(&conn, p_under_review, "order", "3");

    let p_approved = insert_entity(&conn, "workflow_status", "proposal.approved", "Approved", 4);
    insert_prop(&conn, p_approved, "entity_type_scope", "proposal");
    insert_prop(&conn, p_approved, "status_code", "approved");
    insert_prop(&conn, p_approved, "label", "Approved");
    insert_prop(&conn, p_approved, "is_terminal", "true");
    insert_prop(&conn, p_approved, "order", "4");

    let p_rejected = insert_entity(&conn, "workflow_status", "proposal.rejected", "Rejected", 5);
    insert_prop(&conn, p_rejected, "entity_type_scope", "proposal");
    insert_prop(&conn, p_rejected, "status_code", "rejected");
    insert_prop(&conn, p_rejected, "label", "Rejected");
    insert_prop(&conn, p_rejected, "is_terminal", "true");
    insert_prop(&conn, p_rejected, "order", "5");

    // Proposal workflow transitions
    let pt_submit = insert_entity(&conn, "workflow_transition", "proposal.draft_to_submitted", "Submit", 0);
    insert_prop(&conn, pt_submit, "entity_type_scope", "proposal");
    insert_prop(&conn, pt_submit, "from_status_code", "draft");
    insert_prop(&conn, pt_submit, "to_status_code", "submitted");
    insert_prop(&conn, pt_submit, "transition_label", "Submit");
    insert_prop(&conn, pt_submit, "required_permission", "proposal.submit");
    insert_prop(&conn, pt_submit, "requires_outcome", "false");
    insert_relation(&conn, transition_from_rel_id, pt_submit, p_draft);
    insert_relation(&conn, transition_to_rel_id, pt_submit, p_submitted);

    let pt_review = insert_entity(&conn, "workflow_transition", "proposal.submitted_to_review", "Start Review", 0);
    insert_prop(&conn, pt_review, "entity_type_scope", "proposal");
    insert_prop(&conn, pt_review, "from_status_code", "submitted");
    insert_prop(&conn, pt_review, "to_status_code", "under_review");
    insert_prop(&conn, pt_review, "transition_label", "Start Review");
    insert_prop(&conn, pt_review, "required_permission", "proposal.review");
    insert_prop(&conn, pt_review, "requires_outcome", "false");
    insert_relation(&conn, transition_from_rel_id, pt_review, p_submitted);
    insert_relation(&conn, transition_to_rel_id, pt_review, p_under_review);

    let pt_approve = insert_entity(&conn, "workflow_transition", "proposal.review_to_approved", "Approve", 0);
    insert_prop(&conn, pt_approve, "entity_type_scope", "proposal");
    insert_prop(&conn, pt_approve, "from_status_code", "under_review");
    insert_prop(&conn, pt_approve, "to_status_code", "approved");
    insert_prop(&conn, pt_approve, "transition_label", "Approve");
    insert_prop(&conn, pt_approve, "required_permission", "proposal.approve");
    insert_prop(&conn, pt_approve, "requires_outcome", "false");
    insert_relation(&conn, transition_from_rel_id, pt_approve, p_under_review);
    insert_relation(&conn, transition_to_rel_id, pt_approve, p_approved);

    let pt_reject_draft = insert_entity(&conn, "workflow_transition", "proposal.draft_to_rejected", "Reject", 0);
    insert_prop(&conn, pt_reject_draft, "entity_type_scope", "proposal");
    insert_prop(&conn, pt_reject_draft, "from_status_code", "draft");
    insert_prop(&conn, pt_reject_draft, "to_status_code", "rejected");
    insert_prop(&conn, pt_reject_draft, "transition_label", "Reject");
    insert_prop(&conn, pt_reject_draft, "required_permission", "proposal.approve");
    insert_prop(&conn, pt_reject_draft, "requires_outcome", "true");
    insert_relation(&conn, transition_from_rel_id, pt_reject_draft, p_draft);
    insert_relation(&conn, transition_to_rel_id, pt_reject_draft, p_rejected);

    let pt_reject_review = insert_entity(&conn, "workflow_transition", "proposal.review_to_rejected", "Reject", 0);
    insert_prop(&conn, pt_reject_review, "entity_type_scope", "proposal");
    insert_prop(&conn, pt_reject_review, "from_status_code", "under_review");
    insert_prop(&conn, pt_reject_review, "to_status_code", "rejected");
    insert_prop(&conn, pt_reject_review, "transition_label", "Reject");
    insert_prop(&conn, pt_reject_review, "required_permission", "proposal.approve");
    insert_prop(&conn, pt_reject_review, "requires_outcome", "true");
    insert_relation(&conn, transition_from_rel_id, pt_reject_review, p_under_review);
    insert_relation(&conn, transition_to_rel_id, pt_reject_review, p_rejected);

    log::info!("Seeded ontology: 35 relation types, 2 roles, {} permissions, 12 nav items, 8 settings, 1 admin user, workflow + warning entities", perms.len());
    log::info!("Default admin created — username: admin, password: admin123");
}

/// Extended seed for staging environment with realistic test data.
/// Calls seed_ontology() first, then adds additional users, roles, and sample data.
/// Safe to call repeatedly — checks for existing staging data before inserting.
pub fn seed_staging(pool: &DbPool, admin_password_hash: &str) {
    seed_ontology(pool, admin_password_hash);

    let conn = pool.get().expect("Failed to get DB connection for staging seed");

    // --- Block 1: users and roles ---
    let staging_marker: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE entity_type='user' AND name='alice'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if staging_marker == 0 {
        log::info!("Seeding staging data...");

        let has_role_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_role'",
            [], |row| row.get(0),
        ).unwrap();
        let has_perm_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'",
            [], |row| row.get(0),
        ).unwrap();

        // Additional roles
        let editor_role_id = insert_entity(&conn, "role", "editor", "Editor", 3);
        insert_prop(&conn, editor_role_id, "description", "Can create and edit content");

        let viewer_role_id = insert_entity(&conn, "role", "viewer", "Viewer", 4);
        insert_prop(&conn, viewer_role_id, "description", "Read-only access");

        let manager_role_id = insert_entity(&conn, "role", "manager", "Manager", 5);
        insert_prop(&conn, manager_role_id, "description", "Can manage governance workflows");

        // Editor permissions
        let editor_perms = [
            "dashboard.view", "users.list", "users.create", "users.edit",
            "suggestion.view", "suggestion.create", "proposal.view", "proposal.create",
            "proposal.edit", "proposal.submit", "settings.manage",
        ];
        for perm_code in &editor_perms {
            let perm_id: i64 = conn.query_row(
                "SELECT id FROM entities WHERE entity_type='permission' AND name=?1",
                [perm_code], |row| row.get(0),
            ).unwrap();
            insert_relation(&conn, has_perm_id, editor_role_id, perm_id);
        }

        // Viewer permissions
        let viewer_perms = ["dashboard.view", "users.list"];
        for perm_code in &viewer_perms {
            let perm_id: i64 = conn.query_row(
                "SELECT id FROM entities WHERE entity_type='permission' AND name=?1",
                [perm_code], |row| row.get(0),
            ).unwrap();
            insert_relation(&conn, has_perm_id, viewer_role_id, perm_id);
        }

        // Manager permissions
        let manager_perms = [
            "dashboard.view", "users.list", "users.create", "users.edit",
            "tor.list", "tor.create", "tor.edit", "tor.manage_members",
            "suggestion.view", "suggestion.create", "suggestion.review",
            "proposal.view", "proposal.create", "proposal.edit", "proposal.submit",
            "proposal.review", "proposal.approve",
            "agenda.view", "agenda.create", "agenda.queue", "agenda.manage",
            "agenda.participate", "agenda.decide",
            "coa.create", "coa.edit",
        ];
        for perm_code in &manager_perms {
            let perm_id: i64 = conn.query_row(
                "SELECT id FROM entities WHERE entity_type='permission' AND name=?1",
                [perm_code], |row| row.get(0),
            ).unwrap();
            insert_relation(&conn, has_perm_id, manager_role_id, perm_id);
        }

        // Additional users
        let alice_id = insert_entity(&conn, "user", "alice", "Alice Editor", 0);
        insert_prop(&conn, alice_id, "password", admin_password_hash);
        insert_prop(&conn, alice_id, "email", "alice@example.com");
        insert_relation(&conn, has_role_id, alice_id, editor_role_id);

        let bob_id = insert_entity(&conn, "user", "bob", "Bob Viewer", 0);
        insert_prop(&conn, bob_id, "password", admin_password_hash);
        insert_prop(&conn, bob_id, "email", "bob@example.com");
        insert_relation(&conn, has_role_id, bob_id, viewer_role_id);

        let charlie_id = insert_entity(&conn, "user", "charlie", "Charlie Manager", 0);
        insert_prop(&conn, charlie_id, "password", admin_password_hash);
        insert_prop(&conn, charlie_id, "email", "charlie@example.com");
        insert_relation(&conn, has_role_id, charlie_id, manager_role_id);

        let diana_id = insert_entity(&conn, "user", "diana", "Diana Admin", 0);
        insert_prop(&conn, diana_id, "password", admin_password_hash);
        insert_prop(&conn, diana_id, "email", "diana@example.com");
        let admin_role_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type='role' AND name='admin'",
            [], |row| row.get(0),
        ).unwrap();
        insert_relation(&conn, has_role_id, diana_id, admin_role_id);

        log::info!("Staging data seeded: 3 additional roles (editor, viewer, manager), 4 additional users (alice, bob, charlie, diana)");
    } else {
        log::info!("Staging user data already seeded, skipping");
    }

    // --- Block 2: ToR sample data ---
    let tor_marker: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE entity_type='tor' AND name='budget_committee'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if tor_marker > 0 {
        log::info!("ToR staging data already seeded, skipping");
        return;
    }

    log::info!("Seeding ToR staging data...");

    let belongs_to_tor_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='belongs_to_tor'",
        [], |row| row.get(0),
    ).unwrap();
    let fills_position_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='fills_position'",
        [], |row| row.get(0),
    ).unwrap();
    let suggested_to_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='suggested_to'",
        [], |row| row.get(0),
    ).unwrap();
    let submitted_to_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='submitted_to'",
        [], |row| row.get(0),
    ).unwrap();

    let alice_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='user' AND name='alice'",
        [], |row| row.get(0),
    ).unwrap();
    let charlie_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='user' AND name='charlie'",
        [], |row| row.get(0),
    ).unwrap();

    // ToR 1: Budget Committee (monthly, wednesday)
    let bc_id = insert_entity(&conn, "tor", "budget_committee", "Budget Committee", 1);
    insert_prop(&conn, bc_id, "description", "Oversees budget planning and expenditure approvals");
    insert_prop(&conn, bc_id, "status", "active");
    insert_prop(&conn, bc_id, "meeting_cadence", "monthly");
    insert_prop(&conn, bc_id, "cadence_day", "wednesday");
    insert_prop(&conn, bc_id, "cadence_time", "10:00");
    insert_prop(&conn, bc_id, "cadence_duration_minutes", "90");
    insert_prop(&conn, bc_id, "default_location", "Conference Room A");

    let bc_chair = insert_entity(&conn, "tor_function", "bc_chair", "Chair", 0);
    insert_prop(&conn, bc_chair, "membership_type", "mandatory");
    insert_relation(&conn, belongs_to_tor_rt, bc_chair, bc_id);
    insert_relation(&conn, fills_position_rt, charlie_id, bc_chair);

    let bc_member = insert_entity(&conn, "tor_function", "bc_member", "Member", 0);
    insert_prop(&conn, bc_member, "membership_type", "optional");
    insert_relation(&conn, belongs_to_tor_rt, bc_member, bc_id);
    insert_relation(&conn, fills_position_rt, alice_id, bc_member);

    // Suggestion for Budget Committee
    let bc_sug = insert_entity(&conn, "suggestion", "suggestion_2026_01_15_bc", "Need new procurement process for capital expenditures", 0);
    insert_prop(&conn, bc_sug, "description", "Need new procurement process for capital expenditures over 50k. Current manual approval chain takes too long.");
    insert_prop(&conn, bc_sug, "submitted_date", "2026-01-15");
    insert_prop(&conn, bc_sug, "status", "open");
    insert_prop(&conn, bc_sug, "submitted_by_id", &alice_id.to_string());
    insert_relation(&conn, suggested_to_rt, bc_sug, bc_id);

    // Proposal for Budget Committee
    let bc_prop = insert_entity(&conn, "proposal", "revised_procurement_policy_2026", "Revised Procurement Policy 2026", 0);
    insert_prop(&conn, bc_prop, "title", "Revised Procurement Policy 2026");
    insert_prop(&conn, bc_prop, "description", "Introduce a two-stage digital approval workflow for all capital expenditure requests above 50k, with a 5-day SLA for first-stage review.");
    insert_prop(&conn, bc_prop, "rationale", "Current process averages 18 days for approval. New workflow reduces this to 5 days while maintaining oversight.");
    insert_prop(&conn, bc_prop, "submitted_date", "2026-01-28");
    insert_prop(&conn, bc_prop, "status", "draft");
    insert_prop(&conn, bc_prop, "submitted_by_id", &charlie_id.to_string());
    insert_relation(&conn, submitted_to_rt, bc_prop, bc_id);

    // Agenda point for Budget Committee
    let bc_agenda = insert_entity(&conn, "agenda_point", "agenda_2026_03_05_bc", "Q1 2026 Budget Review", 0);
    insert_prop(&conn, bc_agenda, "title", "Q1 2026 Budget Review");
    insert_prop(&conn, bc_agenda, "description", "Review Q1 expenditure against plan and approve Q2 budget allocations.");
    insert_prop(&conn, bc_agenda, "item_type", "decision");
    insert_prop(&conn, bc_agenda, "scheduled_date", "2026-03-05");
    insert_prop(&conn, bc_agenda, "time_allocation_minutes", "45");
    insert_prop(&conn, bc_agenda, "status", "scheduled");
    insert_prop(&conn, bc_agenda, "created_by", &charlie_id.to_string());
    insert_prop(&conn, bc_agenda, "created_date", "2026-02-10T09:00:00");
    insert_prop(&conn, bc_agenda, "tor_id", &bc_id.to_string());
    insert_relation(&conn, belongs_to_tor_rt, bc_agenda, bc_id);

    // ToR 2: Safety Review Board (working days / daily standup style, monday)
    let srb_id = insert_entity(&conn, "tor", "safety_review_board", "Safety Review Board", 2);
    insert_prop(&conn, srb_id, "description", "Reviews safety incidents, risk assessments, and compliance requirements");
    insert_prop(&conn, srb_id, "status", "active");
    insert_prop(&conn, srb_id, "meeting_cadence", "working_days");
    insert_prop(&conn, srb_id, "cadence_time", "08:30");
    insert_prop(&conn, srb_id, "cadence_duration_minutes", "30");
    insert_prop(&conn, srb_id, "default_location", "Safety Operations Room");

    let srb_chair = insert_entity(&conn, "tor_function", "srb_chair", "Safety Officer", 0);
    insert_prop(&conn, srb_chair, "membership_type", "mandatory");
    insert_relation(&conn, belongs_to_tor_rt, srb_chair, srb_id);
    insert_relation(&conn, fills_position_rt, charlie_id, srb_chair);

    // Suggestion for Safety Review Board
    let srb_sug = insert_entity(&conn, "suggestion", "suggestion_2026_02_03_srb", "Update emergency evacuation procedures", 0);
    insert_prop(&conn, srb_sug, "description", "Update emergency evacuation procedures to include remote workers and new building annex. Current procedures are 3 years old.");
    insert_prop(&conn, srb_sug, "submitted_date", "2026-02-03");
    insert_prop(&conn, srb_sug, "status", "open");
    insert_prop(&conn, srb_sug, "submitted_by_id", &charlie_id.to_string());
    insert_relation(&conn, suggested_to_rt, srb_sug, srb_id);

    // Agenda point for Safety Review Board
    let srb_agenda = insert_entity(&conn, "agenda_point", "agenda_2026_03_10_srb", "Quarterly Safety Assessment", 0);
    insert_prop(&conn, srb_agenda, "title", "Quarterly Safety Assessment");
    insert_prop(&conn, srb_agenda, "description", "Review Q1 safety incidents, near-misses, and corrective actions status.");
    insert_prop(&conn, srb_agenda, "item_type", "informative");
    insert_prop(&conn, srb_agenda, "scheduled_date", "2026-03-10");
    insert_prop(&conn, srb_agenda, "time_allocation_minutes", "20");
    insert_prop(&conn, srb_agenda, "status", "scheduled");
    insert_prop(&conn, srb_agenda, "created_by", &charlie_id.to_string());
    insert_prop(&conn, srb_agenda, "created_date", "2026-02-12T08:30:00");
    insert_prop(&conn, srb_agenda, "tor_id", &srb_id.to_string());
    insert_relation(&conn, belongs_to_tor_rt, srb_agenda, srb_id);

    log::info!("ToR staging data seeded: 2 ToRs (Budget Committee, Safety Review Board) with positions, members, suggestions, proposals, and agenda points");
}
