diff --git a/src/auth/abac.rs b/src/auth/abac.rs
new file mode 100644
index 0000000..50a225c
--- /dev/null
+++ b/src/auth/abac.rs
@@ -0,0 +1,142 @@
+//! Attribute-Based Access Control (ABAC) for resource-scoped capabilities.
+//!
+//! Provides capability checks for EAV-graph resources. Currently used for ToR
+//! member roles (Chairperson, Secretary) that need fine-grained operation access
+//! without global `tor.edit` permission.
+//!
+//! ## Graph model
+//!
+//! ```text
+//! user --(fills_position)--> tor_function --(belongs_to_tor)--> tor
+//!                            tor_function has entity_properties:
+//!                              can_call_meetings    = 'true' | 'false'
+//!                              can_manage_agenda    = 'true' | 'false'
+//!                              can_record_decisions = 'true' | 'false'
+//! ```
+
+use crate::auth::session::{get_user_id, require_permission, Permissions};
+use crate::errors::AppError;
+use actix_session::Session;
+use rusqlite::{params, Connection};
+
+/// Check whether a user holds a specific capability in a given resource,
+/// by traversing the EAV graph:
+///   user --(fills_position)--> tor_function --(belongs_to_rel)--> resource
+///
+/// Returns `Ok(true)` if ANY of the user's positions in the resource
+/// has the capability property set to `'true'`.
+/// Returns `Ok(false)` for non-members, wrong-resource, or missing/false flag.
+/// Returns `Err` on database error.
+///
+/// Fail-closed: a misspelled `belongs_to_rel` causes the scalar subquery to
+/// return NULL, so the WHERE clause evaluates to UNKNOWN (false in SQL
+/// three-valued logic), and the function returns `Ok(false)`.
+pub fn has_resource_capability(
+    conn: &Connection,
+    user_id: i64,
+    resource_id: i64,
+    belongs_to_rel: &str,
+    capability: &str,
+) -> Result<bool, AppError> {
+    let count: i64 = conn.query_row(
+        "SELECT COUNT(*)
+         FROM entity_properties ep
+         JOIN entities func
+             ON ep.entity_id = func.id
+             AND func.entity_type = 'tor_function'
+         JOIN relations r_fills
+             ON r_fills.target_id = func.id
+             AND r_fills.source_id = ?1
+             AND r_fills.relation_type_id = (
+                 SELECT id FROM entities
+                 WHERE entity_type = 'relation_type' AND name = 'fills_position'
+             )
+         JOIN relations r_belongs
+             ON r_belongs.source_id = func.id
+             AND r_belongs.target_id = ?2
+             AND r_belongs.relation_type_id = (
+                 SELECT id FROM entities
+                 WHERE entity_type = 'relation_type' AND name = ?3
+             )
+         WHERE ep.key = ?4
+           AND ep.value = 'true'",
+        params![user_id, resource_id, belongs_to_rel, capability],
+        |row| row.get(0),
+    )?;
+    Ok(count > 0)
+}
+
+/// Load all capability keys the user holds in a specific ToR, in a single
+/// database query. Returns only keys whose value is `'true'`.
+///
+/// Used at page-render time to populate template contexts with ABAC capabilities
+/// (e.g., `ctx.tor_capabilities.has("can_call_meetings")`).
+///
+/// The `LIKE 'can_%'` filter captures all six capability types, making this
+/// function forward-compatible when new capabilities are added.
+pub fn load_tor_capabilities(
+    conn: &Connection,
+    user_id: i64,
+    tor_id: i64,
+) -> Result<Permissions, AppError> {
+    let mut stmt = conn.prepare(
+        "SELECT DISTINCT ep.key
+         FROM entity_properties ep
+         JOIN entities func
+             ON ep.entity_id = func.id
+             AND func.entity_type = 'tor_function'
+         JOIN relations r_fills
+             ON r_fills.target_id = func.id
+             AND r_fills.source_id = ?1
+             AND r_fills.relation_type_id = (
+                 SELECT id FROM entities
+                 WHERE entity_type = 'relation_type' AND name = 'fills_position'
+             )
+         JOIN relations r_belongs
+             ON r_belongs.source_id = func.id
+             AND r_belongs.target_id = ?2
+             AND r_belongs.relation_type_id = (
+                 SELECT id FROM entities
+                 WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'
+             )
+         WHERE ep.key LIKE 'can_%'
+           AND ep.value = 'true'",
+    )?;
+    let keys = stmt
+        .query_map(params![user_id, tor_id], |row| row.get::<_, String>(0))?
+        .collect::<Result<Vec<_>, _>>()?;
+    Ok(Permissions(keys))
+}
+
+/// Handler-level guard for ToR resource capabilities.
+///
+/// Two-phase check:
+/// 1. If the session has global `tor.edit`, access is granted immediately
+///    (admin bypass — no DB query needed).
+/// 2. Otherwise, look up the user's ABAC capability via `has_resource_capability`.
+///    Returns `Ok(())` if the user holds the capability, or `Err(PermissionDenied)`
+///    if not.
+///
+/// Error semantics:
+/// - Unauthenticated session → `AppError::Session`
+/// - Capability not held → `AppError::PermissionDenied(capability)`
+/// - Database error → `AppError::Db`
+pub fn require_tor_capability(
+    conn: &Connection,
+    session: &Session,
+    tor_id: i64,
+    capability: &str,
+) -> Result<(), AppError> {
+    // Phase 1: global bypass for users with tor.edit
+    if require_permission(session, "tor.edit").is_ok() {
+        return Ok(());
+    }
+    // Phase 2: resource-level capability check
+    let user_id = get_user_id(session)
+        .ok_or_else(|| AppError::Session("Not authenticated".to_string()))?;
+    if has_resource_capability(conn, user_id, tor_id, "belongs_to_tor", capability)? {
+        Ok(())
+    } else {
+        Err(AppError::PermissionDenied(capability.to_string()))
+    }
+}
diff --git a/src/auth/mod.rs b/src/auth/mod.rs
index 825a80a..6df7938 100644
--- a/src/auth/mod.rs
+++ b/src/auth/mod.rs
@@ -1,3 +1,4 @@
+pub mod abac;
 pub mod csrf;
 pub mod middleware;
 pub mod password;
