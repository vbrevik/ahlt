<!-- PROJECT_CONFIG
runtime: rust-cargo
test_command: cargo test --test abac_test
END_PROJECT_CONFIG -->

<!-- SECTION_MANIFEST
section-01-failing-tests
section-02-has-resource-capability
section-03-bulk-loader-and-helper
END_MANIFEST -->

# Implementation Sections Index

Three sequential sections following TDD discipline: write red tests first, then implement function-by-function until green.

## Dependency Graph

| Section | Depends On | Blocks | Parallelizable |
|---------|------------|--------|----------------|
| section-01-failing-tests | - | section-02 | No |
| section-02-has-resource-capability | section-01 | section-03 | No |
| section-03-bulk-loader-and-helper | section-02 | - | No |

## Execution Order

1. `section-01-failing-tests` (write all 7 tests + helpers — confirm compile failure)
2. `section-02-has-resource-capability` (implement this function — tests 1-5 green)
3. `section-03-bulk-loader-and-helper` (implement remaining 2 functions — tests 6-7 green)

## Section Summaries

### section-01-failing-tests

Create `tests/abac_test.rs` with:
- Module import: `mod common;` + `use ahlt::auth::abac;` (will fail to compile — that's the TDD red state)
- Six helper functions (`create_function`, `create_user`, `create_tor`, `rel_type`, `fills_position`, `belongs_to_tor`)
- Seven `#[test]` functions covering all specified scenarios
- Run `cargo test --test abac_test` — expect compile error `unresolved import ahlt::auth::abac`

Does NOT modify any `src/` files.

### section-02-has-resource-capability

Create `src/auth/abac.rs` with the `has_resource_capability` function only. Add `pub mod abac;` to `src/auth/mod.rs`.

After this section: `cargo test --test abac_test` — tests 1-5 pass, tests 6-7 fail (functions not yet implemented).

### section-03-bulk-loader-and-helper

Add `load_tor_capabilities` and `require_tor_capability` to `src/auth/abac.rs`.

After this section: all 7 tests pass. Run `cargo test` (full suite) to confirm no regressions. Run `cargo clippy` to confirm no new warnings.
