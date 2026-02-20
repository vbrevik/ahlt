# Opus Review

**Model:** claude-opus-4-6
**Generated:** 2026-02-20T19:45:00Z

---

# Review: Implementation Plan -- ABAC Core Module

**File reviewed:** `/Users/vidarbrevik/projects/im-ctrl/docs/plans/01-abac-core/claude-plan.md`

## Overall Assessment

This is a well-structured, carefully thought-out plan. The three-function architecture is clean, the SQL queries are correct, the test strategy is sensible, and the two-phase authorization model is a sound design that preserves backward compatibility. The plan correctly identifies and follows existing codebase patterns. That said, I have identified several issues ranging from minor to significant that should be addressed before implementation.

---

## 1. Critical: Test Helper `create_function` Will Break Under Parallel Execution With Name Collisions

**Section:** Test Strategy, test helpers (lines 96-106) and supporting doc `/Users/vidarbrevik/projects/im-ctrl/docs/plans/01-abac-core/claude-research.md` lines 242-256.

The `create_function` helper in the research doc uses a pattern of:
```rust
conn.execute("INSERT INTO entities (entity_type, name, label) VALUES ('tor_function', ?1, ?1)", [name]).unwrap();
```

The `entities` table has a `UNIQUE(entity_type, name)` constraint (line 11 of `/Users/vidarbrevik/projects/im-ctrl/src/schema.sql`). While TempDir isolation prevents cross-test collisions, if any single test were to create two `tor_function` entities with the same name, the insert would fail. The test case names in the plan (e.g., "chair_alpha", "member_beta") are unique across tests, so this is not an immediate problem. But it is a latent footgun: the `create_function` helper encourages a pattern where the name is meaningful but there is no protection against accidental reuse within a test.

Additionally, the `create_function` helper only inserts a single capability property. Test case 6 (`test_load_tor_capabilities_returns_all_true_flags`) needs a function with three properties. The plan describes adding them via raw SQL after calling `create_function`, which is fine, but the plan's test strategy section (lines 96-106) does not mention this detail -- it only says the helper creates "one entity_property." The claude-spec.md (lines 162) references "2 true + 1 false" but the plan itself does not spell out that test 6 requires additional raw INSERT statements beyond the helper. This is a gap in the plan's description.

**Recommendation:** Note explicitly in the plan that test 6 requires manual `INSERT INTO entity_properties` calls beyond what `create_function` provides, or extend the helper to accept multiple capability pairs.

---

## 2. Significant: `require_tor_capability` Cannot Be Unit Tested Without a Session Mock

**Section:** Three Functions to Implement -- `require_tor_capability` (lines 69-77) and Test Strategy (lines 107-123).

The plan correctly notes on line 123: "Note: `require_tor_capability` is tested indirectly through integration tests in later splits." However, this means the core handler-level gating function has zero direct test coverage in this split. The function contains three branches (unauthenticated, global bypass, ABAC check) and two distinct error types (`AppError::Session` vs `AppError::PermissionDenied`).

The reason it is hard to test is that `actix_session::Session` requires an Actix runtime and HTTP request context. This is a real constraint, but the plan should acknowledge it more explicitly as a risk. If the implementer makes a logic error in the Phase 1/Phase 2 flow (e.g., inverting the `is_ok()` check on `require_permission`), there is no test to catch it until Split 2.

**Recommendation:** Consider adding a comment in the plan about this coverage gap and whether a simple `check_tor_capability(conn, user_id, has_global_edit, tor_id, capability) -> Result<(), AppError>` pure function could be extracted and tested directly, with `require_tor_capability` becoming a thin wrapper that extracts session data and delegates. This would allow unit testing of the branching logic without mocking Actix sessions.

---

## 3. Significant: Relationship Between `require_tor_membership` and ABAC Not Addressed

**Section:** Authorization Logic (lines 36-43).

The codebase already has `tor::require_tor_membership()` in `/Users/vidarbrevik/projects/im-ctrl/src/models/tor/queries.rs` (line 398), which checks whether a user fills any position in a ToR. This function is called in 37+ handler call sites (suggestions, proposals, opinions, agenda, queues, workflows). The new ABAC system adds a parallel but different check: membership WITH a specific capability.

The plan does not discuss:
- Whether `require_tor_membership` will eventually be superseded or coexist with `require_tor_capability`
- Whether handlers that currently use `require_tor_membership` (like suggestion_handlers, proposal_handlers) should eventually migrate to ABAC
- Whether there is conceptual overlap that could lead to confusion about which check to use when

This is not a blocker for Split 1, but it is a missing architectural consideration that should be noted to prevent future confusion.

**Recommendation:** Add a brief note clarifying the relationship: `require_tor_membership` checks "is this user a member at all?" while `require_tor_capability` checks "does this user have a specific capability?" They serve different purposes and will coexist.

---

## 4. Minor: `create_function` Uses Select-After-Insert Instead of `last_insert_rowid()`

**Section:** Test helpers in `/Users/vidarbrevik/projects/im-ctrl/docs/plans/01-abac-core/claude-research.md` lines 242-256.

The `create_function`, `create_user`, and `create_tor` helpers all do a SELECT after INSERT to get the entity ID:
```rust
conn.execute("INSERT INTO entities ...", [name]).unwrap();
let func_id: i64 = conn.query_row("SELECT id FROM entities WHERE name = ?1 AND entity_type = 'tor_function'", [name], |r| r.get(0)).unwrap();
```

The `tor::create()` function in `/Users/vidarbrevik/projects/im-ctrl/src/models/tor/queries.rs` line 172 uses `conn.last_insert_rowid()`, which is the canonical SQLite pattern and avoids a second query. The test helpers should follow this pattern for consistency and to avoid the (admittedly theoretical) risk that the SELECT returns the wrong row if the UNIQUE constraint has been worked around.

This is test-only code, so the practical impact is negligible, but it is worth noting for code quality.

---

## 5. Minor: `has_resource_capability` Hard-codes `fills_position` Despite Being "Generic"

**Section:** Three Functions to Implement -- `has_resource_capability` (lines 49-57) and SQL in `/Users/vidarbrevik/projects/im-ctrl/docs/plans/01-abac-core/claude-spec.md` lines 116-131.

The function signature accepts `belongs_to_rel: &str` as a parameter (line 54: "This makes the function reusable for other resource types in the future"). However, the SQL query hard-codes `'fills_position'` as the relation type name. If the function is meant to be generic for future use with different resource types, the `fills_position` relation might not be the correct edge type.

**Recommendation:** Either (a) make the observation explicit in the plan that `fills_position` is intentionally hard-coded and genericity is limited to the `belongs_to_rel` parameter, or (b) add a second parameter `fills_rel: &str` for full genericity. Option (a) is the pragmatic choice for now.

---

## 6. Minor: `load_tor_capabilities` Returns All 6 Capability Types, Not Just 3

**Section:** Three Functions to Implement -- `load_tor_capabilities` (lines 59-66).

The plan says the query filters for `ep.key LIKE 'can_%'`. This returns all six capabilities (`can_review_suggestions`, `can_create_proposals`, `can_approve_proposals`, plus the three meeting-related ones), not just the three currently used. The plan only discusses three but the full set will flow into `Permissions`. This is fine behavior -- future splits can leverage it -- but the plan should acknowledge it.

---

## 7. Edge Case: Non-Existent Relation Type Name Silently Returns False

If someone calls `has_resource_capability` with a typo in `belongs_to_rel`, the subquery returns NULL, the WHERE clause evaluates to false via SQL's three-valued logic, and the function returns `Ok(false)`. This is fail-closed (correct security posture) but hard to debug. Worth noting as a known trade-off.

---

## Summary of Recommendations

| Priority | Issue | Action |
|----------|-------|--------|
| Significant | `require_tor_capability` has no unit test coverage | Consider extracting a pure function for the branching logic |
| Significant | Relationship to existing `require_tor_membership` not discussed | Add a note clarifying the distinction |
| Minor | Test 6 needs extra INSERT calls beyond `create_function` | Document explicitly or extend the helper |
| Minor | `has_resource_capability` is only semi-generic (fills_position hard-coded) | Document the intentional limitation |
| Minor | `load_tor_capabilities` returns 6 capability types, not just 3 | Acknowledge in the plan |
| Low | Typo in relation type name silently denies access | Note as fail-closed trade-off |
| Low | Test helpers use SELECT-after-INSERT instead of `last_insert_rowid` | Minor consistency nit |

The plan is solid and ready for implementation with these observations incorporated. The SQL queries are correct, the test strategy covers the important code paths for the two query functions, and the module integration is clean.
