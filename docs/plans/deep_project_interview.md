# Deep Project Interview Transcript
**Date:** 2026-02-20
**Requirements file:** `docs/plans/2026-02-20-abac-design.md`

---

## Q1: What is your primary concern going into implementation?

**A:** Getting the auth module right (`abac.rs`). The core `has_resource_capability` and `require_tor_capability` functions need to be correct before anything else.

---

## Q2: How should deep-project relate to the existing implementation plan?

**A:** Build on it — incorporate its tasks into deep-plan splits. The existing plan has the right tasks; just need TDD framing and section detail from deep-plan.

---

## Q3: Should this be one deep-plan or split into multiple?

**A:** Three splits:
1. `abac.rs` — core auth module + unit tests
2. Handler migration — replacing `require_permission("tor.edit")` in 9 handlers across `meeting_handlers/crud.rs` and `minutes_handlers/crud.rs`
3. Template + UI — `MeetingDetailTemplate` context wiring + `detail.html` button visibility guards

---

## Summary Notes

- The ABAC design doc is complete and approved. Both the design (`abac-design.md`) and a detailed implementation plan (`abac-implementation-plan.md`) exist.
- No schema changes required. The EAV graph already has `fills_position` and `belongs_to_tor` relation types seeded.
- Primary constraint: Askama's `{% if %}` does not support `||` — nested `{% if %}` blocks required for OR conditions.
- The `confirm_calendar` handler is a special case: it returns JSON responses (not `AppError`), so it uses `has_resource_capability` directly rather than `require_tor_capability` with `?`.
- The `save_attendance` and `save_action_items` handlers only receive `minutes_id`, not `tor_id` — they must resolve `tor_id` via `minutes → meeting_id → meeting.tor_id`.
- Existing implementation plan covers: Tasks 1–2 (abac.rs), Tasks 3–6 (handler migration), Tasks 7–9 (template context + UI + verification).
