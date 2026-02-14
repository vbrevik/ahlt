use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

pub type DbPool = Pool<SqliteConnectionManager>;

const MIGRATIONS: &str = "
CREATE TABLE IF NOT EXISTS entities (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    UNIQUE(entity_type, name)
);

CREATE TABLE IF NOT EXISTS entity_properties (
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    key       TEXT NOT NULL,
    value     TEXT NOT NULL,
    PRIMARY KEY (entity_id, key)
);

CREATE TABLE IF NOT EXISTS relations (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    relation_type_id INTEGER NOT NULL REFERENCES entities(id),
    source_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    UNIQUE(relation_type_id, source_id, target_id)
);

CREATE TABLE IF NOT EXISTS relation_properties (
    relation_id INTEGER NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (relation_id, key),
    FOREIGN KEY (relation_id) REFERENCES relations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_properties_entity ON entity_properties(entity_id);
";

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
    let _member_of_id = insert_entity(&conn, "relation_type", "member_of", "Member Of", 0);
    let _has_tor_role_id = insert_entity(&conn, "relation_type", "has_tor_role", "Has ToR Role", 0);
    let _belongs_to_tor_id = insert_entity(&conn, "relation_type", "belongs_to_tor", "Belongs to ToR", 0);

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

    // Governance: module header
    let _nav_governance_id = insert_entity(&conn, "nav_item", "governance", "Governance", 3);
    insert_prop(&conn, _nav_governance_id, "url", "/tor");

    // Governance -> Terms of Reference: sidebar child
    let nav_gov_tor_id = insert_entity(&conn, "nav_item", "governance.tor", "Terms of Reference", 1);
    insert_prop(&conn, nav_gov_tor_id, "url", "/tor");
    insert_prop(&conn, nav_gov_tor_id, "parent", "governance");

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

    // Create audit directory with secure permissions
    let audit_path = "data/audit";
    if !std::path::Path::new(audit_path).exists() {
        std::fs::create_dir_all(audit_path)
            .expect("Failed to create audit directory");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut file_perms = std::fs::metadata(audit_path)
                .expect("Failed to get audit dir metadata")
                .permissions();
            file_perms.set_mode(0o700); // Owner read/write/execute only
            std::fs::set_permissions(audit_path, file_perms)
                .expect("Failed to set audit dir permissions");
        }
    }

    log::info!("Seeded ontology: 21 relation types, 2 roles, {} permissions (21 base + 9 Phase 2b), 11 nav items, 5 settings, 1 admin user, workflow entities (3 suggestion statuses + 3 transitions, 5 proposal statuses + 5 transitions)", perms.len());
    log::info!("Default admin created — username: admin, password: admin123");
}
