# STIG Compliance Skill Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Claude Code skill that integrates DISA ASD STIG compliance into the development workflow with guard mode (pre-implementation constraint injection) and review mode (post-implementation verification with reporting).

**Architecture:** Single skill (`stig-compliance`) with two modes orchestrated from one SKILL.md. Reference controls stored in a co-located markdown file with trigger patterns for auto-detection. Project-specific overlays via `.claude/rules/stig-profile.md`. Three output layers: inline chat, per-check reports, cumulative tracker.

**Tech Stack:** Claude Code skill system (SKILL.md with YAML frontmatter), markdown reference data, git diff for change detection.

**Spec:** `docs/superpowers/specs/2026-03-12-stig-compliance-skill-design.md`

---

## Note on skill files

Files under `~/.claude/skills/stig-compliance/` are personal skill files, **not tracked in the im-ctrl git repo**. Tasks that create these files have no git commit step. Only project files under `/Users/vidarbrevik/projects/im-ctrl/` are committed.

Tasks 1, 2, 3 (skill files) and Tasks 4, 6 (project files) are independent and can run in parallel. Only Task 5 (prompt-contracts modification) and Task 7 (validation) depend on earlier tasks being complete.

---

## Chunk 1: Core Skill and Reference Data

### Task 1: Create the ASD STIG controls reference file

**Files:**
- Create: `~/.claude/skills/stig-compliance/references/asd-stig-controls.md`

This is the foundation — all other tasks depend on it.

- [ ] **Step 1: Create directory structure**

```bash
mkdir -p ~/.claude/skills/stig-compliance/references
mkdir -p ~/.claude/skills/stig-compliance/scripts
```

- [ ] **Step 2: Research all ASD STIG controls**

Use WebSearch with these queries to gather control data:
- `"DISA ASD STIG" "CAT I" findings list application security V-2225 OR V-2226`
- `"Application Security and Development STIG" controls checklist site:public.cyber.mil`
- `"ASD STIG V5" CAT I vulnerabilities complete list`
- `stigviewer.com application security development STIG`

For each control found, capture: V-ID, CAT level, title, check procedure, code patterns, fix guidance. Target all ~32 CAT I + ~20 most relevant CAT II controls.

- [ ] **Step 3: Write the auth and session-management categories**

Create the file with frontmatter and the first two categories. Write complete control entries (not stubs).

- [ ] **Step 4: Write input-validation and injection categories**

Append controls for input validation and injection prevention.

- [ ] **Step 5: Write error-handling and information-disclosure categories**

Append controls for error handling and information disclosure.

- [ ] **Step 6: Write cryptography, audit-logging, and configuration categories**

Append remaining categories to complete the reference file.

- [ ] **Step 7: Verify completeness**

Confirm the file contains:
- All ~32 CAT I controls
- ~20 most relevant CAT II controls
- 9 categories, each with `<!-- Trigger patterns: ... -->` HTML comments
- YAML frontmatter with `last-updated`, `stig-version`, `update-cadence`

The file structure must follow this format:

```markdown
---
last-updated: 2026-03-12
stig-version: ASD STIG V5R3
update-cadence: DISA releases quarterly — check for updates each quarter
---

# ASD STIG Controls Reference

## auth
<!-- Trigger patterns: session, login, password, authentication, authorization, permission, credential -->

### V-222577 (CAT I)
**Title**: Application must enforce approved authorizations for logical access
**Check**: Verify role/permission checks exist before every resource access point
**Patterns**: permission guards, role checks, session validation, ABAC
**Fix**: Add authorization check before business logic in every handler

### V-222578 (CAT I)
...

## session-management
<!-- Trigger patterns: session, cookie, token, logout, timeout, idle -->
...

## input-validation
<!-- Trigger patterns: form, input, upload, query, parameter, request body, user data -->
...

## error-handling
<!-- Trigger patterns: error, exception, panic, unwrap, response, status code, message -->
...

## cryptography
<!-- Trigger patterns: crypto, hash, encrypt, tls, ssl, certificate, argon, bcrypt, key -->
...

## audit-logging
<!-- Trigger patterns: audit, log, event, trail, record, track -->
...

## injection
<!-- Trigger patterns: sql, query, raw, interpolat, concatenat, command, exec, shell -->
...

## information-disclosure
<!-- Trigger patterns: error message, stack trace, debug, version, header, path, internal -->
...

## configuration
<!-- Trigger patterns: config, environment, secret, credential, default, hardcod -->
...
```

Each control entry follows the format shown above: V-ID with CAT level, Title, Check, Patterns, Fix.

No git commit — this is a personal skill file outside the project repo.

---

### Task 2: Create the main SKILL.md

**Files:**
- Create: `~/.claude/skills/stig-compliance/SKILL.md`

- [ ] **Step 1: Write the SKILL.md**

Create `~/.claude/skills/stig-compliance/SKILL.md`:

```markdown
---
name: stig-compliance
description: "DISA ASD STIG compliance checks for development. Guard mode injects STIG constraints into prompt contracts before implementation. Review mode verifies compliance post-implementation with inline findings, per-check reports, and cumulative tracker updates. Invoke with /stig-compliance, /stig-compliance guard, or /stig-compliance review [categories]."
---

# STIG Compliance

Integrate DISA ASD STIG compliance into the development workflow. Two modes: guard (pre-implementation) and review (post-implementation).

## Mode Detection

- `/stig-compliance` — if currently building a prompt contract, run guard mode; otherwise run review mode
- `/stig-compliance guard` — force guard mode
- `/stig-compliance review` — force review mode on git diff
- `/stig-compliance review auth,crypto` — review with manual category override
- `/stig-compliance review --full` — full project baseline (category-by-category, warns about duration first)

## Mode 1: Guard (Pre-Implementation)

Inject relevant STIG constraints before code is written.

### Process

1. Read the current task description or prompt contract being built
2. Auto-detect applicable categories:
   - Read `references/asd-stig-controls.md`
   - Match task description keywords against `<!-- Trigger patterns: ... -->` in each category heading
   - If manual categories provided (e.g., `guard auth,crypto`), use those instead
3. For each matched category, pull all V-controls from the reference file
4. Output a STIG Constraints section for the developer to review:

```
## STIG Constraints (auto-detected: auth, session-management)
- V-222577 (CAT I): Application must enforce approved authorizations for logical access
- V-222596 (CAT II): Application must not expose session IDs in URLs or error messages
- V-222609 (CAT II): Application must destroy session IDs upon user logout
```

5. Present to developer — they can accept, modify, or dismiss
6. If accepted, append below the CONSTRAINTS section of the prompt contract (as a separate section, not merged into Always/Ask First/Never tiers)

**This mode is advisory.** No enforcement.

## Mode 2: Review (Post-Implementation)

Verify compliance and produce documentation after code changes.

### Process

1. **Identify scope:**
   - Default: `git diff` (staged + unstaged changes)
   - `--full`: all files matching project overlay patterns (or `src/` + `templates/` by default), processed category-by-category with incremental output. Warn user about expected duration before starting.
   - Manual file list if provided

2. **Load applicable controls:**
   - Same auto-detect logic as guard mode, but scan actual code patterns in changed files
   - Manual override (`review auth,crypto`) narrows scope
   - Read project overlay (`.claude/rules/stig-profile.md`) if it exists for framework-specific pattern mappings

3. **Review each applicable control** against the code semantically. Assign status:
   - **PASS** — code satisfies the control, with brief evidence
   - **FAIL** — violation found, with specific file:line reference and remediation
   - **N/A** — control doesn't apply to the changed code
   - **MANUAL** — requires human judgement (state what needs verification)

4. **Output three artifacts:**

### Output A: Inline Chat Summary

```
STIG Review: 3 passed, 1 failed, 2 N/A, 1 manual

FAIL   V-222602  Error messages reveal internal paths  src/errors.rs:45
MANUAL V-222541  Verify data classification level      src/handlers/export.rs
```

Show only FAIL and MANUAL in the table. Summarize PASS and N/A as counts.

### Output B: Per-Check Report

Write to `docs/compliance/reports/YYYY-MM-DD-<topic>.md` where topic = comma-joined detected categories (e.g., `2026-03-12-auth-session.md`). If file exists, append `-2` suffix.

Format:

```markdown
# STIG Compliance Report — [categories]

**Date**: YYYY-MM-DD
**Scope**: [git diff | full scan | manual file list]
**Controls checked**: N
**Result**: X passed, Y failed, Z N/A, W manual

## Findings

### FAIL: V-XXXXXX — [Title] (CAT [I/II/III])
**File**: path/to/file.rs:line
**Finding**: [Description of the violation]
**Remediation**: [How to fix it]

### PASS: V-XXXXXX — [Title] (CAT [I/II/III])
**Evidence**: [Brief description of how the code satisfies this control]

### MANUAL: V-XXXXXX — [Title] (CAT [I/II/III])
**Requires**: [What human judgement is needed]
```

### Output C: Cumulative Tracker Update

Update `docs/compliance/stig-status.md`. Create the file if it doesn't exist. Structure by category with markdown tables:

```markdown
# STIG Compliance Status

**Last updated**: YYYY-MM-DD
**Controls tracked**: N (X passed, Y failed, Z not assessed)

## auth
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222577 | Enforce approved authorizations | I | PASS | 2026-03-12 | require_permission() in all handlers |
```

Rules:
- New controls: add row with current status
- Existing controls: update Status, Last Checked, and Evidence/Notes
- Controls never reviewed: show as `NOT ASSESSED` with `—` for date and notes

## Project Overlay

If `.claude/rules/stig-profile.md` exists, read it for:
- **Stack info**: language, framework, ORM — informs which patterns to look for
- **Pattern mappings**: maps abstract STIG concepts to project-specific code (e.g., "permission check" → `require_permission(&session, "code")?`)
- **Excluded controls**: V-IDs marked as not applicable with reason — skip these during review

Without an overlay, the skill works with generic guidance.

## Web Fallback

When a V-ID is referenced that isn't in `references/asd-stig-controls.md`:

1. Search the web for its description and check procedure
2. If web search is unavailable, report as "UNKNOWN — not in local reference, web lookup unavailable" and move on
3. If found, ask the user before appending to the reference file
4. Appended entries are tagged with `Source: web-lookup, YYYY-MM-DD`

## Key Principles

- **Advisory, not blocking** — never prevent commits or fail builds
- **Semantic review** — understand code intent, don't just regex match
- **Incremental** — tracker builds over time, no upfront full-scan required
- **Extensible** — designed to add Web Server, PostgreSQL, Container STIGs later
```

- [ ] **Step 2: Verify file exists and is well-formed**

Confirm the file has valid YAML frontmatter (name + description fields) and all sections from the spec: Mode Detection, Guard mode, Review mode, Project Overlay, Web Fallback, Key Principles.

No git commit — this is a personal skill file outside the project repo.

---

### Task 3: Create the tracker maintenance guide

**Files:**
- Create: `~/.claude/skills/stig-compliance/scripts/update-tracker.md`

- [ ] **Step 1: Write the guide**

```markdown
# Tracker Maintenance Guide

## Quarterly Reference Update

DISA releases STIG updates quarterly. To update:

1. Check https://public.cyber.mil/stigs/ for new ASD STIG version
2. Compare new controls against `references/asd-stig-controls.md`
3. Add new controls to appropriate categories
4. Update removed/modified controls
5. Update the `last-updated` and `stig-version` in the frontmatter
6. Run `/stig-compliance review --full` to re-baseline

## Tracker File Management

- `docs/compliance/stig-status.md` is the cumulative tracker
- Each review updates it automatically
- To reset: delete the file and run `/stig-compliance review --full`
- Per-check reports in `docs/compliance/reports/` are append-only history
```

No git commit — this is a personal skill file outside the project repo.

---

## Chunk 2: Project Integration

### Task 4: Create the im-ctrl project overlay

**Files:**
- Create: `/Users/vidarbrevik/projects/im-ctrl/.claude/rules/stig-profile.md`

- [ ] **Step 1: Write the overlay**

Read the project's CLAUDE.md and existing code patterns to create an accurate mapping:

```markdown
# STIG Profile: im-ctrl

## Stack
- Language: Rust
- Framework: Actix-web 4
- ORM: sqlx 0.8 (async, PostgreSQL)
- Templates: Askama 0.14
- Auth: argon2 0.5, actix-session 0.10
- Serialization: serde + serde_json

## Pattern Mappings
- Permission check: `require_permission(&session, "permission.code")?`
- ABAC capability check: `require_tor_capability(&session, &pool, tor_id, "can_*").await?`
- CSRF validation: `csrf::validate_csrf(&session, &token)?`
- Audit logging: `audit::log(&pool, user_id, "action.name", "target_type", target_id, details).await`
- Input validation: `web::Form<T>` / `web::Json<T>` with serde deserialization, manual validation in handlers
- Error handling: `AppError` enum (`Db`, `Template`, `NotFound`, `PermissionDenied`, `Session`, `Csrf`), `render()` helper
- Session management: actix-session with cookie backend, `Key::generate()` for signing
- Password hashing: argon2 via `hash_password()` / `verify_password()`
- Database queries: `sqlx::query` / `sqlx::query_as` with `$N` parameters (never string interpolation)
- Handler return type: `Result<HttpResponse, AppError>`

## Excluded Controls
- V-222659 (PKI client cert auth) — not applicable, using password-based auth
- V-222656 (CAC authentication) — not applicable, no DoD CAC integration
```

- [ ] **Step 2: Commit**

```bash
git add /Users/vidarbrevik/projects/im-ctrl/.claude/rules/stig-profile.md
git commit -m "feat: add STIG profile overlay for im-ctrl project"
```

---

### Task 5: Add STIG suggestion to prompt-contracts skill

**Files:**
- Modify: `~/.claude/skills/prompt-contracts/SKILL.md`

- [ ] **Step 1: Read current prompt-contracts SKILL.md and locate anchor**

Read `~/.claude/skills/prompt-contracts/SKILL.md`. Grep for the anchor text: `Permanent constraints`. Confirm it exists before proceeding. If not found, search for the CONSTRAINTS section heading and insert after its last paragraph instead.

- [ ] **Step 2: Add STIG integration note**

After the line containing `Permanent constraints`, add:

```markdown

**STIG compliance:** If the task involves auth, session management, input handling, error handling, cryptography, or logging, consider running `/stig-compliance guard` to inject applicable DISA STIG controls as an additional constraints section.
```

No git commit — this is a personal skill file outside the project repo.

---

### Task 6: Create initial compliance directory and tracker

**Files:**
- Create: `/Users/vidarbrevik/projects/im-ctrl/docs/compliance/stig-status.md`
- Create: `/Users/vidarbrevik/projects/im-ctrl/docs/compliance/reports/.gitkeep`

- [ ] **Step 1: Create directories and initial tracker**

```bash
mkdir -p /Users/vidarbrevik/projects/im-ctrl/docs/compliance/reports
```

Write the initial tracker file:

```markdown
# STIG Compliance Status

**Last updated**: 2026-03-12
**Controls tracked**: 0 (0 passed, 0 failed, 0 not assessed)

> Run `/stig-compliance review` after code changes to populate this tracker.
> Run `/stig-compliance review --full` for a full project baseline.
```

- [ ] **Step 2: Create reports directory with .gitkeep**

Create empty `docs/compliance/reports/.gitkeep` to ensure the directory is tracked.

- [ ] **Step 3: Commit**

```bash
git add docs/compliance/stig-status.md docs/compliance/reports/.gitkeep
git commit -m "feat: initialize STIG compliance tracking directory"
```

---

## Chunk 3: Validation

### Task 7: End-to-end validation

- [ ] **Step 1: Test guard mode**

Invoke `/stig-compliance guard` with a task description like "Add a login handler with password validation." Verify:
- Categories `auth`, `session-management`, `input-validation` are auto-detected
- Relevant V-controls are listed with CAT levels
- Output is formatted as a STIG Constraints section

- [ ] **Step 2: Test review mode**

Make a small code change (or use existing unstaged changes) and invoke `/stig-compliance review`. Verify:
- Changed files are identified from git diff
- Applicable controls are matched
- Inline chat summary shows PASS/FAIL/N/A/MANUAL counts
- Per-check report is written to `docs/compliance/reports/`
- Cumulative tracker `docs/compliance/stig-status.md` is updated

- [ ] **Step 3: Test manual category override**

Invoke `/stig-compliance review auth` and verify only auth-category controls are checked.

- [ ] **Step 4: Test web fallback**

Reference an obscure V-ID not in the reference file and verify the skill attempts web lookup, then asks before appending.

- [ ] **Step 5: Fix any issues found during validation**

Address any problems discovered in steps 1-4.

- [ ] **Step 6: Final commit (project files only)**

```bash
git add docs/compliance/stig-status.md docs/compliance/reports/
git commit -m "fix: address issues found during STIG skill validation"
```

Only commit project files that were modified during validation. Skill files under `~/.claude/skills/` are not tracked in git.
