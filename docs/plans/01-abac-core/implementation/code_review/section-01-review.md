# Code Review: section-01-failing-tests

**Verdict:** Approve with fixes

## Summary

Test file is correct. SQL uses right column names (`source_id`/`target_id`). All 7 tests present. Helpers complete with no stubs. Two fixes needed.

## Issues

### S1 — Planning docs use wrong column names (SIGNIFICANT)
`section-01-failing-tests.md` line 82 and `section-03-bulk-loader-and-helper.md` lines 69, 127-138 use `from_entity_id`/`to_entity_id`. Schema uses `source_id`/`target_id`. The test file itself is correct; the docs need updating before section-03 to prevent the implementer from following broken SQL examples.

**Action: Auto-fix** — update the planning docs.

### M1 — assert_eq! with bool (MINOR)
Tests 1-5 use `assert_eq!(result.unwrap(), true/false)`. Clippy (`bool_assert_comparison`) prefers `assert!(result.unwrap())` and `assert!(!result.unwrap())`.

**Action: Auto-fix** — convert 5 assertions before green phase.

### M2 — single-property create_function (MINOR)
Test 6 uses direct SQL to add properties 2-3. Intentional per TDD plan. No action needed.

## Coverage
All 7 required scenarios covered. Test isolation clean (TempDir per test).
