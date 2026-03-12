# STIG Compliance Skill — Design Specification

**Date**: 2026-03-12
**Status**: Approved
**Approach**: Hub-and-spoke (Approach C) — single skill with guard + review modes

## Goal

Create a general-purpose Claude Code skill that integrates DISA ASD STIG compliance checks into the development workflow, serving as both development guardrails (preventing violations before code is written) and compliance documentation (producing audit-ready artifacts after implementation).

## Skill Structure

```
~/.claude/skills/stig-compliance/
├── SKILL.md                          # Main skill (~250 lines)
├── references/
│   └── asd-stig-controls.md          # Curated ASD STIG controls with check patterns
└── scripts/
    └── update-tracker.md             # Instructions for tracker maintenance
```

Per-project overlay (optional):
```
<project>/.claude/rules/stig-profile.md
```

## Invocation

- **`/stig-compliance`** — auto-detects mode from context (guard during prompt-contracts, review otherwise)
- **`/stig-compliance guard`** — force guard mode
- **`/stig-compliance review`** — force review mode
- **`/stig-compliance review auth,crypto`** — review with manual category override
- **`/stig-compliance review --full`** — full project baseline scan, processed category-by-category with incremental output. Scopes to files matching the project overlay's pattern mappings (or `src/` + `templates/` by default). Warns user about expected duration before starting.

## Mode 1: Guard (Pre-Implementation)

Runs during prompt-contracts to inject STIG constraints before code is written.

### Process

1. Read the task description from the prompt contract being built
2. Auto-detect applicable categories by reading trigger patterns from `<!-- Trigger patterns: ... -->` comments in each category heading of `references/asd-stig-controls.md`. This keeps detection logic co-located with the controls (single source of truth). Example mappings:
   - `session`, `login`, `password` → `auth`, `session-management`
   - `sqlx`, `query`, `database` → `input-validation`, `injection`
   - `error`, `AppError`, `response` → `error-handling`, `information-disclosure`
   - `upload`, `form`, `input` → `input-validation`
   - `crypto`, `hash`, `tls`, `argon` → `cryptography`
   - `audit`, `log` → `audit-logging`
3. Pull matching V-controls from `references/asd-stig-controls.md`
4. Output STIG constraints as a dedicated section appended below the standard CONSTRAINTS block (not merged into the Always/Ask First/Never tiers):

```
## STIG Constraints (auto-detected: auth, session-management)
- V-222596 (CAT II): Application must not expose session IDs in URLs or error messages
- V-222577 (CAT I): Application must enforce approved authorizations for access at the application level
- V-222609 (CAT II): Application must destroy session IDs upon user logout
```

The developer can accept, modify, or dismiss. Advisory only — no enforcement.

## Mode 2: Review (Post-Implementation)

Runs after code changes to verify compliance and produce documentation.

### Process

1. Identify changed files via git diff (staged + unstaged) or accept a file list
2. Load applicable controls — same auto-detect logic as guard mode, but based on actual code patterns. Manual override narrows scope.
3. Review each applicable control semantically. For each control, assign a status:
   - **PASS** — code satisfies the control, with brief evidence
   - **FAIL** — violation found, with specific line reference and remediation guidance
   - **N/A** — control doesn't apply to the changed code
   - **MANUAL** — requires human judgement (e.g., "is this data classified?")

### Output (three artifacts)

**a) Inline chat summary:**
```
STIG Review: 3 passed, 1 failed, 2 N/A, 1 manual

FAIL   V-222602  Error messages reveal internal paths  src/errors.rs:45
MANUAL V-222541  Verify data classification level      src/handlers/export.rs
```

**b) Per-check report file** — `docs/compliance/reports/YYYY-MM-DD-<topic>.md` (topic = comma-joined detected categories, e.g., `2026-03-12-auth-session.md`; collisions get `-2` suffix):
- Control ID, title, CAT severity (I/II/III)
- Finding status
- Evidence (code references)
- Remediation steps (for FAILs)

**c) Cumulative tracker update** — `docs/compliance/stig-status.md`:
- Living document structured by control category
- New controls added on first encounter; existing controls get status updated
- Unreviewed controls show as `NOT ASSESSED`

Format:
```markdown
## auth
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222577 | Enforce approved authorizations | I | PASS | 2026-03-12 | require_permission() in all handlers |
| V-222596 | No session IDs in URLs | II | NOT ASSESSED | — | — |
```

## Reference Data

### Embedded Controls (`references/asd-stig-controls.md`)

Curated subset of ASD STIG organized by category. Each control entry contains:

```markdown
---
last-updated: 2026-03-12
stig-version: ASD STIG V5R3
update-cadence: DISA releases quarterly — check for updates each quarter
---

## auth
<!-- Trigger patterns: session, login, password, authentication, authorization, permission -->
### V-222577 (CAT I)
**Title**: Application must enforce approved authorizations
**Check**: Verify role/permission checks exist before resource access
**Patterns**: require_permission, session checks, ABAC guards
**Fix**: Add require_permission() or require_tor_capability() before business logic
```

Each category heading includes a `<!-- Trigger patterns: ... -->` comment listing the keywords that auto-detection uses. This keeps detection logic and controls in a single source of truth.

**Initial scope**: ~40-50 highest-impact controls (all CAT I + most relevant CAT II).

**Categories**: `auth`, `session-management`, `input-validation`, `error-handling`, `cryptography`, `audit-logging`, `injection`, `information-disclosure`, `configuration`.

### Web Fallback

When a V-ID is referenced that isn't in the local reference file:

1. The skill searches the web for its description and check procedure
2. If web search is unavailable, the skill reports the V-ID as "UNKNOWN — not in local reference, web lookup unavailable" and moves on
3. If found, the skill asks the user before appending to the reference file
4. Appended entries are tagged with `Source: web-lookup, YYYY-MM-DD` to distinguish them from curated entries

## Project Overlay (`.claude/rules/stig-profile.md`)

Optional per-project file that maps abstract STIG requirements to project-specific code patterns:

```markdown
# STIG Profile: im-ctrl

## Stack
- Language: Rust
- Framework: Actix-web 4
- ORM: sqlx 0.8 (PostgreSQL)
- Templates: Askama 0.14
- Auth: argon2, actix-session

## Pattern Mappings
- Permission check: `require_permission(&session, "code")?`
- CSRF validation: `csrf::validate_csrf(&session, &token)?`
- Audit logging: `audit::log(&pool, user_id, "action", "type", id, details)`
- Input validation: Form structs with serde, manual validation in handlers
- Error handling: `AppError` enum, `render()` helper
- Session management: actix-session with cookie backend

## Excluded Controls
- V-222659 (PKI client cert) — not applicable, using password auth
```

Without this file, the skill works but gives generic guidance. With it, checks are precise and project-aware.

## Workflow Integration

### Integration with prompt-contracts

Add the following instruction to `prompt-contracts/SKILL.md` in the CONSTRAINTS-building section:

> "When building CONSTRAINTS, if the task involves auth, session management, input handling, error handling, cryptography, or logging, suggest the user run `/stig-compliance guard` to inject applicable DISA STIG controls."

This is a text-based suggestion — no runtime skill discovery. The developer decides whether to invoke it. Delivering this modification to prompt-contracts is a deliverable of this project.

### Manual invocation

- `/stig-compliance` — anytime during development
- `/stig-compliance review` — after coding, before `/wrap-up`
- `/stig-compliance review auth,crypto` — targeted check

### Suggested workflow chain

```
prompt-contracts (guard mode auto-injects)
  → implement code
  → /stig-compliance review
  → fix any FAILs
  → /wrap-up (commit)
```

### Advisory, not blocking

The skill does not block commits or fail builds. The developer decides whether to address findings. This matches the existing skill philosophy (prompt-contracts suggests, measure-effectiveness scores, neither blocks).

### Tracker maintenance

- `docs/compliance/stig-status.md` grows organically as reviews happen
- `/stig-compliance review --full` baselines all controls against the entire codebase

## Design Principles

1. **Shift-left** — inject STIG awareness at coding time, not after deployment
2. **Semantic review** — use AI understanding of code intent, not just regex/AST matching
3. **Advisory** — inform and guide, never block
4. **Extensible** — start with ASD STIG, framework supports adding Web Server, PostgreSQL, Container STIGs later
5. **Portable** — general-purpose skill with project-specific overlays
6. **Incremental** — tracker builds over time, no upfront full-scan required

## Prior Art

- **MITRE SAF** — Vulcan + InSpec + Heimdall for policy-as-code STIG automation in CI/CD
- **Parasoft/SonarQube** — SAST tools with STIG rule mappings (post-hoc, not pre-implementation)
- **Anchore/OpenSCAP** — infrastructure policy-as-code (container/OS level, not application code)

This skill fills the gap at the AI-assisted development layer — no existing tool injects STIG constraints before code is written or reviews code with semantic understanding.
