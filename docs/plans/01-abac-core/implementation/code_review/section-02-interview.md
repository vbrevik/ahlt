# Code Review Interview: section-02 ABAC implementation

## Triage Decisions

**S1 — require_tor_capability user_id order**: Auto-fix. Restructure to call get_user_id first for accurate error semantics. The plan's two-phase description doesn't specify ordering, but checking user_id first is cleaner.

**S2 — LIKE 'can_%' doc mismatch**: Auto-fix (doc update only). Per implementation plan's integration notes, the broad LIKE filter is intentional for forward-compatibility with future suggestion/proposal ABAC splits. Update module doc to list all 6 capability keys.

**M3 — hardcoded belongs_to_tor comment**: Auto-fix. One-line comment.

## Fixes Applied

1. Restructured `require_tor_capability` to check get_user_id before Phase 1
2. Updated module doc comment to list all 6 capability keys
3. Added comment on hardcoded "belongs_to_tor" in require_tor_capability
