# Workflow Index Page — Design

**Date:** 2026-02-18
**Status:** Approved

## Problem

`GET /workflow` returns 404. The nav item `governance.workflow` points to `/workflow` but no route exists — all workflow routes are scoped to `/tor/{id}/workflow`. A standalone cross-ToR workflow landing page is needed.

## Goal

A `/workflow` page that aggregates suggestions, proposals, and agenda points across all ToRs, using the same 3-tab layout as the per-ToR workflow view.

## Approach

New cross-ToR queries + new handler + new template. Existing per-ToR types and queries are unchanged.

---

## Data Layer

### New types (one per model module's `types.rs`)

```rust
pub struct CrossTorSuggestionItem {
    pub tor_id: i64,
    pub tor_name: String,
    // + all fields from SuggestionListItem
}

pub struct CrossTorProposalItem {
    pub tor_id: i64,
    pub tor_name: String,
    // + all fields from ProposalListItem
}

pub struct CrossTorAgendaItem {
    pub tor_id: i64,
    pub tor_name: String,
    // + all fields from AgendaPointListItem
}
```

### New query functions (one per model module's `queries.rs`)

```rust
pub fn find_all_cross_tor(conn: &Connection, user_id: Option<i64>) -> Result<Vec<CrossTor*Item>, AppError>
```

- `user_id = None` → global query, no membership filter (admin/reviewer)
- `user_id = Some(id)` → filtered to ToRs the user fills a position in (via `fills_position` relation chain: user → tor_function → tor)

SQL pattern (suggestion example):
```sql
SELECT tor.id, tor.name as tor_name, s.id, ...
FROM entities s
JOIN relations r_st ON r_st.source_id = s.id
    AND r_st.relation_type_id = (SELECT id FROM entities WHERE name = 'suggested_to' AND entity_type = 'relation_type')
JOIN entities tor ON tor.id = r_st.target_id
-- When user_id = Some(id):
JOIN relations r_fp ON r_fp.target_id = tor.id   -- via tor_function linkage
    AND r_fp.relation_type_id = (SELECT id FROM entities WHERE name = 'fills_position' ...)
    AND r_fp.source_id = ?
WHERE s.entity_type = 'suggestion'
ORDER BY tor.name, s.created_at DESC
```

*(Exact join chain for tor_function→tor to be determined during implementation by reading existing `require_tor_membership` query.)*

---

## Handler

New `index` function in `src/handlers/workflow_handlers.rs`:

```
GET /workflow
  1. require_permission("suggestion.view")
  2. get user_id from session
  3. get permissions from session
  4. if permissions.has("workflow.manage"):
       call find_all_cross_tor(conn, None)     [global]
     else:
       call find_all_cross_tor(conn, Some(user_id))  [member ToRs only]
  5. active_tab = query param ?tab= (default "suggestions")
  6. render WorkflowIndexTemplate
```

### Template struct

```rust
pub struct WorkflowIndexTemplate {
    pub ctx: PageContext,
    pub active_tab: String,
    pub suggestions: Vec<CrossTorSuggestionItem>,
    pub proposals: Vec<CrossTorProposalItem>,
    pub agenda_points: Vec<CrossTorAgendaItem>,
}
```

Added to `src/templates_structs.rs`.

### Route registration

```rust
.route("/workflow", web::get().to(handlers::workflow_handlers::index))
```

Registered in `main.rs` **before** `/tor/{id}/workflow` routes.

---

## Template

`templates/workflow/index.html`:

- Inherits from base layout (same as other pages)
- 3-tab navigation: Suggestions / Proposals / Agenda Points (tab state from `?tab=`)
- Each tab's table has a **ToR** column (first column) as a link to `/tor/{tor_id}/workflow?tab=<tab>`
- Clicking a row navigates to the per-ToR workflow view with the correct tab
- Empty state per tab: "No [items] across your Terms of Reference"
- No page-level ToR heading — this is the cross-ToR view

---

## Visibility Rules

| User type | Sees |
|-----------|------|
| Has `workflow.manage` | All ToRs |
| Everyone else | Only ToRs they fill a position in |
| Future (post-ABAC) | Filtered by attributes |

---

## Files Changed

| File | Change |
|------|--------|
| `src/models/suggestion/types.rs` | Add `CrossTorSuggestionItem` |
| `src/models/suggestion/queries.rs` | Add `find_all_cross_tor()` |
| `src/models/proposal/types.rs` | Add `CrossTorProposalItem` |
| `src/models/proposal/queries.rs` | Add `find_all_cross_tor()` |
| `src/models/agenda_point/types.rs` | Add `CrossTorAgendaItem` |
| `src/models/agenda_point/queries.rs` | Add `find_all_cross_tor()` |
| `src/templates_structs.rs` | Add `WorkflowIndexTemplate` |
| `src/handlers/workflow_handlers.rs` | Add `index` handler |
| `templates/workflow/index.html` | New template |
| `src/main.rs` | Register `GET /workflow` route |
