# Enterprise User Management Gap Analysis

_Date: 2026-02-17 | Scope: User handling only (not roles, governance, or other features)_
_Context: Security-sensitive deployment targeting SOC 2, GDPR, and internal security policy_

---

## Current System Summary

### What Works Well

The system has solid cryptographic foundations and correct security primitives:

- **Argon2id password hashing** — correct algorithm, random salt, per-connection verification
- **CSRF protection** — 32-byte hex tokens with constant-time comparison on all mutations
- **Permission-based RBAC** — role-to-permission relations, session-cached permission CSV
- **IP-based login rate limiting** — 5 failures per 15-minute window, blocks before DB access
- **Last-admin protection** — prevents orphaning the system by removing the last admin
- **Self-deletion protection** — users cannot delete their own account
- **User audit logging** — `user.created`, `user.updated`, `user.deleted` logged with details
- **Secure session cookies** — HttpOnly, `COOKIE_SECURE` env flag for production

### Root Weakness

The system is **strong against outside attacks but has almost no inside observability.**
You can't answer: *Who logged in today? From where? Are any accounts compromised? Who was
deprioritized last month?* This is the defining gap between a basic CRUD system and an
enterprise-grade one.

### Honest Assessment

**Current coverage: ~10–15% of expected enterprise user management.**

The system handles user creation, editing, deletion, and password changes. Everything else —
account lifecycle, session management, compliance controls, audit observability — is absent.

---

## Critical Gaps (compliance-blocking)

These gaps would **block SOC 2 / ISO 27001 certification** or constitute a GDPR violation.
They must be addressed before any compliance audit or regulated deployment.

### C1 — Login Audit Trail

**Standard:** SOC 2 CC6.2 (Logical Access), GDPR Art. 5(2) (Accountability)
**Current:** Logins are not logged. No record of who logged in, when, from where, or whether it failed.
**Required:** Every login attempt (success and failure) must be logged with:
- `user_id` / `username_attempted`
- `ip_address`
- `user_agent`
- `timestamp`
- `outcome` (success / invalid_password / account_locked / rate_limited)

**Implementation path:** Add `audit::log` calls in `src/handlers/auth_handlers.rs` inside the
login POST handler. The audit subsystem already exists — this is additive only.

---

### C2 — Account Lockout (per-user)

**Standard:** SOC 2 CC6.6, ISO 27001 A.9.4.3
**Current:** Only IP-based rate limiting (5 failures / 15-min). No per-user lockout.
**Gap:** VPN users share IPs. An attacker can brute-force a specific user from different IPs,
bypassing the IP rate limit entirely.
**Required:** After N failed login attempts for a given username (recommended: 10 within 30 minutes),
lock the account. Lockout must persist across server restarts (in-memory won't do).
Admin can unlock via user management UI. Optionally: auto-unlock after a time window.

**Implementation path:**
- Add `failed_attempts` and `locked_until` to user entity properties (EAV)
- Check lockout in `find_by_username()` before password verification
- Reset on successful login
- Admin unlock toggle in `src/handlers/user_handlers/crud.rs`

---

### C3 — Session Timeout

**Standard:** SOC 2 CC6.6, ISO 27001 A.9.4.2
**Current:** Sessions never expire. A user who logs in from a shared computer stays logged in indefinitely.
**Required:**
- **Absolute max age:** Session expires N hours after login regardless of activity (recommend: 8–12h for internal tools)
- **Idle timeout:** Session expires after N minutes of inactivity (recommend: 30–60min)

**Implementation path:**
- Set `actix_session` cookie `max_age` in `main.rs`
- Store `last_active_at` in session, check on each authenticated request via middleware
- Redirect to login with "session expired" flash message

---

### C4 — User Disable State (deprovisioning without deletion)

**Standard:** SOC 2 CC6.2, ISO 27001 A.9.2.6 (User deprovisioning)
**Current:** Only hard delete. Deleting a user removes all their audit history associations.
When an employee leaves, you lose traceability of what they did.
**Required:** Ability to **disable** a user account (block login, preserve data/history).
Deletion should be a separate, deliberate admin action after a retention period.

**Key finding:** `is_active` already exists in the `entities` table schema (default 1).
This is the lowest-effort Critical gap — the database field is already there.

**Required UI changes:**
- "Disable account" button on user edit page (sets `is_active = 0`)
- "Enable account" button to re-enable
- Disabled users shown with a badge in user list, cannot log in
- `find_by_username()` must filter `WHERE is_active = 1`

---

### C5 — GDPR Right to Erasure

**Standard:** GDPR Art. 17
**Current:** Only hard delete via `DELETE FROM entities` with CASCADE. Removes all user data
including audit trail references (which may cascade or leave orphaned entries).
**Required:** Structured erasure process:
1. Anonymize PII: replace `name` (username) with `deleted_user_[id]`, clear `email` and `display_name` properties
2. Remove password hash property
3. Retain the entity stub and audit trail entries (for accountability, which GDPR allows)
4. Log the erasure itself as a compliance event
5. **Do not use the same DELETE path as normal admin deletion**

**Implementation path:** New handler `POST /users/{id}/erase` with a dedicated confirmation dialog.
Separate from the normal delete flow, with higher permission requirement.

---

### C6 — GDPR Data Export

**Standard:** GDPR Art. 20 (Right to data portability)
**Current:** None.
**Required:** Admin can export all personal data held about a user in machine-readable format (JSON or CSV):
- Username, email, display name
- Account creation and update timestamps
- Role assignment history (from audit log)
- Login history (once C1 is implemented)
- Any other EAV properties

**Implementation path:** `GET /users/{id}/export` handler returning a JSON file download.
Reads from entities, entity_properties, and audit log entries for the user.

---

### C7 — Password Change Audit Logging

**Standard:** SOC 2 CC6.8 (Credential management), ISO 27001 A.9.4.3
**Current:** Password changes via `/account` are NOT logged. The `user.updated` event from admin
edits has a `password_changed` flag but it's not audited for self-service changes.
**Required:** Log every password change (both self-service and admin-forced) with:
- Who changed it (self or admin user ID)
- Whether it was a forced/reset change
- Timestamp

**Implementation path:** Add one `audit::log` call in `src/handlers/account_handlers.rs`
after successful password update. Same pattern as existing audit calls.

---

### C8 — Email Uniqueness Constraint

**Standard:** Data integrity (prerequisite for C5, C6, I6)
**Current:** No UNIQUE constraint on the email property. Two users can have the same email.
This breaks GDPR data subject identification and any future email-based flows (password reset, notifications).
**Required:** Enforce unique emails at both the database query level and form validation.

**Implementation path:**
- Add a uniqueness check in `src/models/user/queries.rs` before insert/update
- Improve email validation in `src/auth/validate.rs` (currently only checks for `@` and `.`)

---

## Important Gaps (security hygiene)

These aren't formally required by a specific control but would be flagged in a security review
or pen test. Expected by any security-conscious organization.

### I1 — Last Login Tracking

**Why:** Enables detection of dormant accounts (compliance: deactivate accounts unused for >90 days),
anomalous login times, and geographic anomalies.
**Required:** Store `last_login_at` (timestamp) and `last_login_ip` on each successful login.
Display in admin user list/edit view.
**Path:** Store as EAV properties; update in login success handler.

---

### I2 — Password Complexity Requirements

**Why:** 8-character minimum is the floor, not the standard. ISO 27001 and most internal policies
require complexity (uppercase, number, symbol).
**Required:** Minimum 1 uppercase, 1 digit, 1 special character in addition to 8-char minimum.
Password strength indicator in UI.
**Path:** 3 additional checks in `src/auth/validate.rs` — already handles minimum length.

---

### I3 — Password History / Reuse Prevention

**Why:** Without this, "forced rotation" (I7) is meaningless — users rotate back to the same password.
**Required:** Store hashes of last 5 passwords. Reject new password if it matches any recent hash.
**Path:** New EAV property `password_history` storing JSON array of past hashes.

---

### I4 — Concurrent Session Limit

**Why:** Unlimited simultaneous sessions make it hard to detect shared credentials.
One login from Nairobi while an employee is in Oslo should trigger a warning.
**Required:** Maximum N active sessions per user (recommend: 3–5 for internal tools).
New session creation revokes oldest if limit exceeded.
**Path:** Store session IDs as a user property; prune on new login.

---

### I5 — Failed Login Count Per User (not just per IP)

**Why:** Current rate limiting is IP-based. A distributed attack from multiple IPs against one
user account will bypass it completely.
**Required:** Track failed attempts per username in addition to per-IP.
Feed into C2 (account lockout) threshold.
**Path:** Can reuse the `RateLimiter` pattern from `src/auth/rate_limit.rs`, keyed by username.

---

### I6 — Email Format Validation

**Why:** Current validation only checks for `@` and `.`. Accepts `foo@.` as valid.
Broken email means GDPR data subject identification fails, and any future email flows break.
**Required:** RFC 5321 compatible validation. At minimum: proper regex or the `email_address` crate.
**Path:** Update `validate_email()` in `src/auth/validate.rs`.

---

### I7 — Password Expiry / Forced Rotation

**Why:** ISO 27001 A.9.3.1 recommends periodic password rotation for privileged accounts.
**Required:** Admin can set a password expiry policy (e.g., 90 days for admin accounts).
Users prompted to change password on login when expired.
**Path:** Store `password_changed_at` as EAV property; check on login, redirect to forced-change page.

---

### I8 — Audit Log UI Enhancements

**Why:** The audit log exists and is queryable, but filtering by specific user or date range
is limited. A compliance reviewer should be able to pull "all actions by user X last month" in 30 seconds.
**Required:** Add `user_id` and `date range` filters to `/audit` UI.
**Path:** Extend query in `src/models/audit.rs` and form in audit template.

---

## Optional Gaps (advanced / future)

Low compliance pressure but high value as the platform matures.

| # | Feature | Notes | Effort |
|---|---------|-------|--------|
| O1 | **MFA / TOTP** | TOTP (Google Authenticator compatible) — most impactful optional control for security-sensitive systems | Large |
| O2 | **SSO / SAML / OAuth2** | Required if org uses Google Workspace, Azure AD, Okta, etc. Significant architecture change. | Very Large |
| O3 | **Self-service password reset** | Requires email infrastructure (SMTP). Reduces admin burden. | Medium |
| O4 | **Bulk user import / export** | CSV import for initial population; export for migration. Useful for larger organizations. | Medium |
| O5 | **User groups / org hierarchy** | Delegated administration, department-level role assignment. Overkill for <100 users. | Large |
| O6 | **API tokens for programmatic access** | Machine-to-machine access without user sessions. Needed for REST API (F.2 in backlog). | Medium |
| O7 | **Profile extensions** | Phone number, department, manager field, location. Useful if system becomes an org directory. | Small |

---

## Quick Wins (implement first)

These deliver the highest compliance value for the least engineering effort.
Do these first — they're all additive changes to existing code.

| Priority | Gap | Effort | Where |
|----------|-----|--------|-------|
| 1 | **Enable/disable users (C4)** | ~2h | `crud.rs` + `queries.rs` + `find_by_username()` — `is_active` already in schema |
| 2 | **Login audit trail (C1)** | ~1h | Add 2 `audit::log` calls in `auth_handlers.rs` |
| 3 | **Password change audit (C7)** | ~30min | Add 1 `audit::log` call in `account_handlers.rs` |
| 4 | **Password complexity (I2)** | ~30min | 3 checks in `validate.rs` |
| 5 | **Session timeout (C3)** | ~2h | `actix_session` config + idle middleware |
| 6 | **Email uniqueness + validation (C8 + I6)** | ~2h | Schema check + `validate.rs` |

Total estimated effort for all 6 quick wins: ~8–10 hours.

---

## Recommended Implementation Sequence

When ready to implement, tackle in this order:

```
Phase 1 (Quick Wins) — 1-2 sessions
═══════════════════════════════════
C4  User disable state
C1  Login audit trail
C7  Password change audit
I2  Password complexity
C3  Session timeout
C8  Email uniqueness + I6 Email validation

Phase 2 (Account Security) — 2-3 sessions
══════════════════════════════════════════
C2  Account lockout per-user
I5  Failed login count per-user
I1  Last login tracking
I4  Concurrent session limit

Phase 3 (GDPR Compliance) — 2-3 sessions
══════════════════════════════════════════
C5  Right to erasure (structured anonymization)
C6  Data export
I8  Audit log UI (user + date filters)

Phase 4 (Policies) — 1-2 sessions
══════════════════════════════════
I3  Password history / reuse prevention
I7  Password expiry / forced rotation

Phase 5 (Advanced — when justified by scale)
════════════════════════════════════════════
O1  MFA / TOTP
O3  Self-service password reset
O2  SSO / SAML (if org requires)
```

---

## Compliance Coverage After Each Phase

| After Phase | SOC 2 CC6 Coverage | GDPR Coverage | Internal Policy |
|-------------|-------------------|---------------|-----------------|
| Phase 1 | ~30% | ~20% | ~40% |
| Phase 2 | ~60% | ~25% | ~70% |
| Phase 3 | ~65% | ~80% | ~75% |
| Phase 4 | ~75% | ~80% | ~90% |
| Phase 5 (MFA) | ~90% | ~80% | ~95% |

_Note: Full SOC 2 certification requires controls beyond user management (network, change management, incident response, etc.)_
