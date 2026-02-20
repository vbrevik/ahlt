# Code Review: section-02 ABAC implementation

**Verdict:** Approve with fixes

## Summary

SQL traversal correct. Fail-closed verified. No injection risk. Two significant issues, one minor.

## Issues

### S1 — require_tor_capability: get_user_id should run before Phase 1 (SIGNIFICANT)
Current code checks Phase 1 (tor.edit) before extracting user_id, meaning an unauthenticated session falls through Phase 1 (correctly) but the error semantics in doc don't match: unauthenticated sessions aren't guaranteed to get AppError::Session if the user_id is somehow present. Restructure to call get_user_id first.

**Action: Auto-fix** — reorder to extract user_id at the top.

### S2 — Module doc only lists 3 of 6 capability keys (SIGNIFICANT)
LIKE 'can_%' returns all 6 capability keys but doc comments list only 3. Per implementation plan, the broad filter is intentional for forward-compatibility (future splits for suggestion/proposal ABAC). Resolution: update doc comment to list all 6.

**Action: Auto-fix** — update module doc comment.

### M3 — Missing comment explaining hardcoded "belongs_to_tor" (MINOR)
**Action: Auto-fix** — add one-line comment.
