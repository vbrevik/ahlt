# Interview Transcript — ABAC Core Module

Conducted during deep-plan workflow for `docs/plans/01-abac-core/spec.md`.

---

## Q1: Multi-Position OR Logic

**Question:** If a user fills TWO positions in the same ToR (e.g., Chair AND Secretary), and only one of those functions has `can_call_meetings=true`, should `has_resource_capability` return `true`? The SQL uses `COUNT(*)` which naturally ORs across all positions. Is this the intended behavior?

**Answer:** Yes — OR across all positions.

**Implication:** If ANY of the user's positions in the ToR has the capability, the function returns `true`. This is the natural behavior of the `COUNT(*) > 0` query and no special handling is needed. The 7 test cases can assume single-position membership since multi-position OR is handled implicitly by the query design.

---

## Q2: DB Error Handling in `has_resource_capability`

**Question:** When the rusqlite query itself fails with a DB error (not a capability miss), what should happen?

**Answer:** Propagate as `AppError::Db`.

**Implication:** The function signature `Result<bool, AppError>` is correct. DB errors should bubble up to callers. The `rusqlite::Error` → `AppError::Db` conversion is already implemented in the codebase's `From` impl. Do NOT swallow errors silently.

---

## Q3: Unauthenticated Session in `require_tor_capability`

**Question:** If `get_user_id(&session)` returns `None` (unauthenticated user), what should `require_tor_capability` return?

**Answer:** `AppError::Session`.

**Implication:** Use `get_user_id(&session).ok_or(AppError::Session("Not authenticated".to_string()))` or equivalent. This distinguishes the "no session" case from the "has session but lacks capability" case, which is semantically cleaner and easier to debug.

---

## Q4: Generic `belongs_to_rel` Parameter

**Question:** The spec includes a `belongs_to_rel: &str` parameter for generic reuse. Confirmed design or hard-code?

**Answer:** Keep generic with `belongs_to_rel` param (per spec).

**Implication:** The full signature `has_resource_capability(conn, user_id, resource_id, belongs_to_rel, capability)` is confirmed. For all current callers, `belongs_to_rel = "belongs_to_tor"`. The `load_tor_capabilities` and `require_tor_capability` functions hard-code `"belongs_to_tor"` internally since they are ToR-specific helpers.
