# Code Review Interview: section-01-failing-tests

## Triage Decisions

**S1 — Wrong column names in planning docs**: Auto-fix. Updating section-01 and section-03 spec docs to use `source_id`/`target_id` instead of `from_entity_id`/`to_entity_id`. No user input required.

**M1 — assert_eq! with bool**: Auto-fix. Converting 5 `assert_eq!(result.unwrap(), true/false)` to `assert!(result.unwrap())` / `assert!(!result.unwrap())` to satisfy Clippy.

**M2 — single-property create_function**: Let go. Intentional per TDD plan.

## Fixes Applied

1. Updated `section-01-failing-tests.md` — removed wrong column names from helper SQL description
2. Updated `section-03-bulk-loader-and-helper.md` — fixed SQL examples to use `source_id`/`target_id`
3. Updated `tests/abac_test.rs` — converted 5 boolean assert_eq! to assert!/assert!(!)
