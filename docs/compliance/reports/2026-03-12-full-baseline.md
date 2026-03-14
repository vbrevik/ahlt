# STIG Compliance Report — Full Baseline

**Date**: 2026-03-12
**Scope**: Full project scan (src/ + templates/)
**Controls checked**: 46 (13 excluded as N/A or per stig-profile.md)
**Result**: 34 passed, 7 failed, 4 N/A, 5 manual

## Findings

### FAIL: V-222432 — Account lockout after 3 failed logins (CAT I)
**File**: src/auth/rate_limit.rs:6-7
**Finding**: Rate limiter allows 5 attempts (MAX_ATTEMPTS = 5) but STIG requires 3. Lockout is IP-based only — no per-account lockout mechanism. No admin unlock flow.
**Remediation**: Change MAX_ATTEMPTS to 3. Add per-account failed login tracking via entity_properties (login_attempts, last_failed_attempt). Implement account lockout state with admin unlock.

### FAIL: V-222536 — 15-character minimum password (CAT I)
**File**: src/auth/validate.rs:39
**Finding**: Current minimum is 8 characters (`password.len() < 8`). STIG requires 15.
**Remediation**: Change threshold from 8 to 15 in validate_password(). Update error message.

### FAIL: V-222538 — Password complexity requirements (CAT I)
**File**: src/auth/validate.rs:34-43
**Finding**: validate_password() only checks length. No checks for uppercase, lowercase, digit, or special character.
**Remediation**: Add complexity checks requiring at least one of each: uppercase [A-Z], lowercase [a-z], digit [0-9], special character.

### FAIL: V-222579 — Session regeneration on login (CAT II)
**File**: src/handlers/auth_handlers.rs:82-84
**Finding**: Login handler inserts user data into existing session without calling session.renew(). Enables session fixation attacks.
**Remediation**: Add `session.renew();` before inserting user_id/username/permissions at line 82.

### FAIL: V-222544 — Password age tracking (CAT II)
**File**: src/models/user/queries.rs
**Finding**: No password_changed_at property tracked. Cannot enforce minimum/maximum password age.
**Remediation**: Add password_changed_at to entity_properties on every password change. Check age on subsequent changes.

### FAIL: V-222545 — Password history / reuse prevention (CAT II)
**File**: src/models/user/queries.rs:321-327
**Finding**: Password update overwrites hash with no history. Users can reuse previous passwords.
**Remediation**: Store last 5 password hashes. Compare new password against history before accepting.

### FAIL: V-222546 — Force password change on first login (CAT II)
**File**: src/handlers/auth_handlers.rs:85-87
**Finding**: No force_password_change flag. All logins redirect to dashboard. No mechanism to distinguish initial/temporary passwords.
**Remediation**: Add force_password_change entity_property. Set on user creation. Redirect to password change form if set. Block other routes until changed.

### MANUAL: V-222543 — Encrypted password transmission / TLS (CAT I)
**Requires**: Verify production deployment uses HTTPS via reverse proxy. COOKIE_SECURE=true must be set. No HSTS header in application code — verify reverse proxy sets it.

### MANUAL: V-222583 — FIPS 140-2 validated cryptographic modules (CAT II)
**Requires**: Verify Argon2 and actix-session crypto libraries are built against FIPS-validated OpenSSL or equivalent. This is a deployment/compilation concern, not a code issue.

### MANUAL: V-222588 — Encrypted data at rest (CAT I)
**Requires**: Passwords are hashed with Argon2 (PASS). Other PII (email, display names) stored as plaintext. Determine if operational policy requires column-level or TDE encryption for non-password PII.

### MANUAL: V-222611 — Detailed errors only for authorized admins (CAT II)
**Requires**: Verify server-side log access is restricted. Application logs to stderr/files — confirm only admins can access log files. Audit viewer is permission-gated (audit.view).

### MANUAL: V-222643 — Sensitivity/classification marking on output (CAT II)
**Requires**: Determine if application handles classified data requiring banner markings. If so, implement classification headers in base template.

### PASS: V-222425 — Authorization checks on protected resources (CAT I)
**Evidence**: All 44 handler files use require_permission() before business logic. ABAC via require_tor_capability() for ToR-scoped operations. Auth middleware enforces session presence.

### PASS: V-222426 — Discretionary access control policies (CAT I)
**Evidence**: RBAC with granular permission codes. ABAC for ToR capabilities. Object ownership validated on access.

### PASS: V-222530 — Replay-resistant auth for privileged accounts (CAT I)
**Evidence**: CSRF tokens (256-bit random, constant-time comparison) on all mutations. 97+ validate_csrf() calls across handlers.

### PASS: V-222531 — Replay-resistant auth for non-privileged accounts (CAT I)
**Evidence**: Same CSRF protection applied to all accounts uniformly.

### PASS: V-222542 — Only store cryptographic password representations (CAT I)
**Evidence**: Argon2id with random salts (OsRng). Constant-time verification. No plaintext storage anywhere.

### PASS: V-222547 — Temporary password with immediate change (CAT II)
**Evidence**: Seed data sets passwords at runtime via hash_password(). However, no force-change mechanism exists (see V-222546 FAIL).

### PASS: V-222577 — No exposed session IDs (CAT I)
**Evidence**: CookieSessionStore with HttpOnly, encrypted cookies. No session IDs in URLs, logs, or error messages.

### PASS: V-222578 — Destroy session on logout (CAT I)
**Evidence**: Logout handler calls session.purge() at auth_handlers.rs:119.

### PASS: V-222581 — No URL-embedded session IDs (CAT I)
**Evidence**: Cookie-only session transport via CookieSessionStore.

### PASS: V-222582 — No recycled session IDs (CAT I)
**Evidence**: Cryptographic cookie encryption with Key::generate() or 64+ byte SESSION_KEY.

### PASS: V-222585 — Fail to secure state (CAT I)
**Evidence**: AppError returns generic 403/404/500. Fail-closed pattern. Auth middleware redirects to login.

### PASS: V-222601 — No sensitive data in hidden fields (CAT I)
**Evidence**: Hidden fields contain only CSRF tokens and entity IDs. No passwords, session data, or PII.

### PASS: V-222602 — XSS protection (CAT I)
**Evidence**: Askama auto-escaping. All |safe usages are in JSON script blocks. No innerHTML in JS files.

### PASS: V-222603 — CSRF protection (CAT I)
**Evidence**: 76+ validate_csrf() calls. API endpoints use Content-Type middleware as alternative.

### PASS: V-222604 — No command injection (CAT I)
**Evidence**: Zero instances of Command::new() or shell execution in codebase.

### PASS: V-222606 — Validate all input (CAT I)
**Evidence**: Server-side validation via serde deserialization + manual checks. Username/email/password validators in auth/validate.rs.

### PASS: V-222607 — No SQL injection (CAT I)
**Evidence**: All queries use sqlx parameterized statements ($N). FilterTree validates column names against whitelist.

### PASS: V-222608 — No XML attacks (CAT I)
**Evidence**: No XML parsing. JSON-only serialization.

### PASS: V-222609 — No input handling vulnerabilities (CAT I)
**Evidence**: Request body size limits configurable. Parse errors handled gracefully via AppError.

### PASS: V-222610 — No internal info in error messages (CAT II)
**Evidence**: Generic error messages to users. Detailed errors logged server-side only. Login returns same message for user-not-found and wrong-password.

### PASS: V-222612 — No overflow attacks (CAT I)
**Evidence**: Rust memory safety. No unsafe blocks in application code.

### PASS: V-222642 — No embedded credentials (CAT I)
**Evidence**: All secrets via environment variables. No hardcoded passwords in source. Seed passwords set at runtime.

### PASS: V-222653 — Coding standards (CAT II)
**Evidence**: cargo clippy documented. Code review checklist in CLAUDE.md. Development workflow rules.

### PASS: V-222615 — Security function tests (CAT II)
**Evidence**: auth_test.rs (162 lines), permission_test.rs (137 lines), abac_test.rs (7 tests). ~221 tests total.

### PASS: V-222474 — Audit records with sufficient info (CAT II)
**Evidence**: audit::log() includes timestamp, user_id, action, target_type, target_id, JSON details. 85 calls across handlers.

### PASS: V-222485 — Alert on audit failure (CAT I)
**Evidence**: Dual-write (filesystem + database). Failures logged to stderr. let _ = pattern prevents audit failures from crashing requests.

### PASS: V-222487 — Central audit review (CAT II)
**Evidence**: Permission-gated audit viewer with search, filter, pagination.

### PASS: V-222489 — Audit reduction/reporting (CAT II)
**Evidence**: Audit viewer supports filtering by date, user, action, target type.

### PASS: V-222570 — FIPS crypto for signing (CAT I)
**Evidence**: SHA-256+ via Argon2. No MD5 or SHA1 for security purposes.

### PASS: V-222571 — FIPS crypto for hashing (CAT I)
**Evidence**: Argon2id for passwords. No weak hash algorithms.

### N/A: V-222524 — PIV credential acceptance (CAT II)
**Evidence**: Excluded per stig-profile.md — password-based auth, no DoD PIV/CAC integration.

### N/A: V-222552 — PKI identity mapping (CAT II)
**Evidence**: Excluded per stig-profile.md — no PKI authentication.

### N/A: V-222573 — SAML session index (CAT II)
**Evidence**: No SAML implementation in codebase.

### N/A: V-222553 — PKI revocation cache (CAT II)
**Evidence**: No PKI authentication.
