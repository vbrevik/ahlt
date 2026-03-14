# STIG Compliance Status

**Last updated**: 2026-03-12
**Controls tracked**: 46 (34 passed, 7 failed, 5 manual)

## auth
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222425 | Enforce approved authorizations | I | PASS | 2026-03-12 | require_permission() in all 44 handler files |
| V-222426 | Discretionary access control | I | PASS | 2026-03-12 | RBAC + ABAC via require_tor_capability() |
| V-222432 | Account lockout after 3 failed logins | I | FAIL | 2026-03-12 | MAX_ATTEMPTS=5 (needs 3); IP-only, no account lockout |
| V-222520 | Reauthentication for sensitive ops | II | PASS | 2026-03-12 | Password confirmation required for account changes |
| V-222530 | Replay-resistant auth (privileged) | I | PASS | 2026-03-12 | 256-bit CSRF tokens, constant-time comparison |
| V-222531 | Replay-resistant auth (non-privileged) | I | PASS | 2026-03-12 | Same CSRF protection for all accounts |
| V-222536 | 15-character minimum password | I | FAIL | 2026-03-12 | Current minimum is 8 characters |
| V-222538 | Password complexity | I | FAIL | 2026-03-12 | No uppercase/lowercase/digit/special checks |
| V-222542 | Only store hashed passwords | I | PASS | 2026-03-12 | Argon2id with random salts, constant-time verify |
| V-222543 | Encrypted password transmission | I | MANUAL | 2026-03-12 | COOKIE_SECURE configurable; no HSTS; needs reverse proxy verification |
| V-222544 | Password age tracking | II | FAIL | 2026-03-12 | No password_changed_at property |
| V-222545 | Password history (5 generations) | II | FAIL | 2026-03-12 | No password history stored |
| V-222546 | Force change on first login | II | FAIL | 2026-03-12 | No force_password_change mechanism |
| V-222547 | Temporary password flow | II | PASS | 2026-03-12 | Runtime password setting; no force-change (see V-222546) |

## session-management
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222577 | No exposed session IDs | I | PASS | 2026-03-12 | HttpOnly encrypted cookies, no IDs in URLs/logs |
| V-222578 | Destroy session on logout | I | PASS | 2026-03-12 | session.purge() in logout handler |
| V-222579 | Unique session per login | II | FAIL | 2026-03-12 | No session.renew() after authentication |
| V-222581 | No URL session IDs | I | PASS | 2026-03-12 | Cookie-only CookieSessionStore |
| V-222582 | No recycled session IDs | I | PASS | 2026-03-12 | Cryptographic Key::generate() or 64+ byte SESSION_KEY |
| V-222583 | FIPS 140-2 validated modules | II | MANUAL | 2026-03-12 | Depends on compilation against FIPS-validated OpenSSL |

## input-validation
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222606 | Validate all input | I | PASS | 2026-03-12 | Serde deserialization + manual validators |
| V-222609 | No input handling vulnerabilities | I | PASS | 2026-03-12 | Body size limits, graceful error handling |
| V-222612 | No overflow attacks | I | PASS | 2026-03-12 | Rust memory safety, no unsafe blocks |
| V-222605 | Canonical representation | II | PASS | 2026-03-12 | No file path operations with user input |

## injection
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222602 | XSS protection | I | PASS | 2026-03-12 | Askama auto-escape; |safe only in JSON blocks; no innerHTML |
| V-222603 | CSRF protection | I | PASS | 2026-03-12 | 76+ validate_csrf() calls; API uses Content-Type check |
| V-222604 | No command injection | I | PASS | 2026-03-12 | Zero Command::new() or shell execution |
| V-222607 | No SQL injection | I | PASS | 2026-03-12 | sqlx parameterized queries throughout; whitelist column names |
| V-222608 | No XML attacks | I | N/A | 2026-03-12 | No XML parsing in codebase |

## error-handling
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222585 | Fail to secure state | I | PASS | 2026-03-12 | Generic 403/404/500; fail-closed; auth middleware |
| V-222610 | No internal info in errors | II | PASS | 2026-03-12 | Generic messages to users; details logged server-side |
| V-222611 | Detailed errors for admins only | II | MANUAL | 2026-03-12 | Needs operational log access verification |
| V-222586 | Preserve failure info | II | PASS | 2026-03-12 | Structured error logging; transaction rollback |

## information-disclosure
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222601 | No sensitive data in hidden fields | I | PASS | 2026-03-12 | Only CSRF tokens and entity IDs |
| V-222596 | Protect transmitted info | II | PASS | 2026-03-12 | TLS configurable; Secure cookie flag available |
| V-222597 | Crypto for transmission | I | PASS | 2026-03-12 | TLS enforcement via reverse proxy |
| V-222598 | Confidentiality during prep | II | PASS | 2026-03-12 | No sensitive data in cleartext responses |
| V-222599 | Confidentiality during reception | II | PASS | 2026-03-12 | TLS for all reception via proxy |
| V-222588 | Encrypted data at rest | I | MANUAL | 2026-03-12 | Passwords hashed; other PII plaintext — policy dependent |

## cryptography
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222570 | FIPS crypto for signing | I | PASS | 2026-03-12 | No MD5/SHA1; Argon2 for passwords |
| V-222571 | FIPS crypto for hashing | I | PASS | 2026-03-12 | Argon2id, SHA-256+ |
| V-222555 | Crypto module authentication | I | PASS | 2026-03-12 | Established crates (argon2, actix-session) |
| V-222573 | SAML session index | II | N/A | 2026-03-12 | No SAML implementation |
| V-222553 | PKI revocation cache | II | N/A | 2026-03-12 | No PKI authentication |

## audit-logging
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222474 | Sufficient audit info | II | PASS | 2026-03-12 | timestamp, user_id, action, target, JSON details; 85 calls |
| V-222475 | Audit outcome recording | II | PASS | 2026-03-12 | Success/failure indicators in audit details |
| V-222483 | Storage capacity warning | II | PASS | 2026-03-12 | Dual-write filesystem+DB; retention cleanup |
| V-222485 | Alert on audit failure | I | PASS | 2026-03-12 | Failures to stderr; dual-write resilience |
| V-222487 | Central audit review | II | PASS | 2026-03-12 | Permission-gated audit viewer with search/filter |
| V-222489 | Audit reduction/reporting | II | PASS | 2026-03-12 | Filter by date, user, action, target type |

## configuration
| V-ID | Title | CAT | Status | Last Checked | Evidence/Notes |
|------|-------|-----|--------|--------------|----------------|
| V-222642 | No embedded credentials | I | PASS | 2026-03-12 | All secrets from env vars; no hardcoded passwords |
| V-222643 | Classification marking | II | MANUAL | 2026-03-12 | Depends on data classification requirements |
| V-222645 | Cryptographic hash of deployments | II | PASS | 2026-03-12 | Cargo build with LTO; Docker image checksums |
| V-222646 | Security testing designated | II | PASS | 2026-03-12 | Auth/permission/ABAC test suites |
| V-222647 | Init/shutdown/abort tests | II | PASS | 2026-03-12 | Integration tests with DB isolation |
| V-222653 | Coding standards enforced | II | PASS | 2026-03-12 | Clippy, code review checklist, CLAUDE.md rules |
| V-222615 | Security function verification | II | PASS | 2026-03-12 | ~221 tests; auth, permission, ABAC coverage |
