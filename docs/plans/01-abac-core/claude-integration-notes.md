# Integration Notes — Opus Review

## Integrating

### 1. Test 6 Extra INSERT Documentation
**Why:** The plan said `create_function` creates "one entity_property" but test 6 needs three properties (2 true + 1 false). Gap in implementation guidance.
**Action:** Update plan to explicitly note that test 6 requires additional raw `INSERT INTO entity_properties` calls after `create_function`.

### 2. `require_tor_capability` Coverage Gap + Pure Function Extraction
**Why:** Zero direct unit tests for the three-branch function is a real risk. The pure-function extraction idea is low-cost and catches logic errors early.
**Action:** Add a note proposing an optional `check_tor_capability(conn, user_id, has_global_edit: bool, tor_id, capability)` pure function as the testable core, with `require_tor_capability` as a thin session-unwrapping wrapper. Mark as optional — implementer can decide whether to extract it or accept the coverage gap.

### 3. `require_tor_membership` Coexistence Note
**Why:** Existing function with 37+ call sites could cause confusion with the new ABAC capability check. Future maintainers need to know which to use when.
**Action:** Add a brief architectural note: `require_tor_membership` = "is this user a member at all?"; `require_tor_capability` = "does this member have a specific capability?" They coexist and serve different purposes.

### 4. `fills_position` Semi-Generic Documentation
**Why:** The function is described as "generic" but only `belongs_to_rel` is parameterized; `fills_position` is hard-coded.
**Action:** Add a note that genericity is intentionally limited to the `belongs_to_rel` dimension. `fills_position` is assumed for all current use cases. Full genericity (adding `fills_rel` param) is deferred.

### 5. `load_tor_capabilities` Returns All 6 Capabilities
**Why:** The plan only discusses 3 but the `LIKE 'can_%'` filter returns all 6. Good behavior, but undocumented.
**Action:** Note explicitly that all `can_*` properties flow into the `Permissions` struct — future splits for suggestion/proposal ABAC can reuse this at zero cost.

### 6. Fail-Closed Typo Trade-off
**Why:** Silent failure on misspelled relation type names is a debuggability concern.
**Action:** Add a note that this is an intentional fail-closed trade-off. Document that relation type names should be used as constants, not inline string literals.

## Not Integrating

### `last_insert_rowid()` in Test Helpers
**Reason:** The project's existing test helpers (e.g., `meeting_test.rs`) use select-after-insert. Following project convention is correct even if it's not the most efficient pattern. Opus acknowledged "negligible practical impact."

### Performance Note on Subquery Evaluation
**Reason:** Not actionable for this scope. Current scale doesn't warrant optimization.

### Audit Logging for Authorization Decisions
**Reason:** Split 2 concern. Not applicable to the read-only auth module in Split 1.

### Error Message UX for ABAC Denials
**Reason:** Existing limitation of `AppError::PermissionDenied`. Out of scope for auth module implementation.
