# Dark Mode Header Toggle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a quick-access dark mode toggle to the header that persists theme preference to the database and syncs across the user's devices.

**Architecture:** Add theme preference storage to `entity_properties`, create API endpoint for updates, wire toggle button in header to call endpoint, load theme on page init before CSS loads.

**Tech Stack:** Rust (Actix-web handlers, sqlx queries), Askama templates, JavaScript (localStorage + API calls), CSS (toggle switch styling)

---

## Task 1: Add Theme Model Functions

**Files:**
- Modify: `src/models/user.rs`
- Test: `tests/user_theme_test.rs` (new)

**Step 1: Read current user model to understand structure**

Run: `head -100 src/models/user.rs`
Expected: See existing functions like `find_by_id()`, `update()`, etc.

**Step 2: Write failing test for get_user_theme**

Create `tests/user_theme_test.rs`:
```rust
#[sqlx::test]
async fn test_get_user_theme_returns_saved_preference(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Setup
    let user_id = 1i64;
    let theme = "dark";
    
    // Set theme via entity_properties
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)"
    )
    .bind(user_id)
    .bind("theme_preference")
    .bind(theme)
    .execute(&pool)
    .await?;
    
    // Test
    let result = ahlt::models::user::get_user_theme(&pool, user_id).await?;
    assert_eq!(result, "dark");
    
    Ok(())
}

#[sqlx::test]
async fn test_get_user_theme_returns_default_when_not_set(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let user_id = 999i64;
    let result = ahlt::models::user::get_user_theme(&pool, user_id).await?;
    // Default should be "auto" (system preference)
    assert_eq!(result, "auto");
    Ok(())
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test --test user_theme_test test_get_user_theme_returns_saved_preference -- --nocapture 2>&1 | tail -20`
Expected: `error[E0433]: cannot find function 'get_user_theme' in module 'ahlt::models::user'`

**Step 4: Add theme getter/setter to src/models/user.rs**

At end of `src/models/user.rs`, add:

```rust
/// Get user's theme preference from entity_properties
/// Returns: "light", "dark", or "auto" (default)
pub async fn get_user_theme(pool: &PgPool, user_id: i64) -> Result<String, sqlx::Error> {
    let result = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'theme_preference' LIMIT 1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    
    Ok(result.flatten().unwrap_or_else(|| "auto".to_string()))
}

/// Set user's theme preference in entity_properties
pub async fn set_user_theme(pool: &PgPool, user_id: i64, theme: &str) -> Result<(), sqlx::Error> {
    // Validate theme value
    if !["light", "dark", "auto"].contains(&theme) {
        return Err(sqlx::Error::RowNotFound);
    }
    
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) 
         VALUES ($1, $2, $3)
         ON CONFLICT (entity_id, key) DO UPDATE SET value = $3"
    )
    .bind(user_id)
    .bind("theme_preference")
    .bind(theme)
    .execute(pool)
    .await?;
    
    Ok(())
}
```

**Step 5: Run test to verify it passes**

Run: `cargo test --test user_theme_test -- --nocapture 2>&1 | tail -20`
Expected: `test result: ok. 2 passed`

**Step 6: Commit**

```bash
git add tests/user_theme_test.rs src/models/user.rs
git commit -m "feat(user): add get_user_theme and set_user_theme functions"
```

---

## Task 2: Create Theme API Endpoint

**Files:**
- Modify: `src/handlers/api_v1/users.rs`
- Modify: `src/main.rs` (add route)
- Test: Integration test in `tests/user_theme_test.rs`

**Step 1: Read existing API structure**

Run: `head -50 src/handlers/api_v1/users.rs`
Expected: See existing endpoint handlers and response patterns

**Step 2: Add theme endpoint handler to src/handlers/api_v1/users.rs**

At end of file, add:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
pub struct UpdateThemeRequest {
    pub theme: String,
}

pub async fn update_theme(
    pool: web::Data<PgPool>,
    session: Session,
    body: web::Json<UpdateThemeRequest>,
) -> Result<HttpResponse, AppError> {
    use crate::auth::session::get_user_id;
    
    let user_id = get_user_id(&session)
        .ok_or_else(|| AppError::Session("User not logged in".to_string()))?;
    
    // Validate theme value
    if !["light", "dark", "auto"].contains(&body.theme.as_str()) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid theme value. Must be 'light', 'dark', or 'auto'"
        })));
    }
    
    crate::models::user::set_user_theme(&pool, user_id, &body.theme).await?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "theme": body.theme
    })))
}
```

**Step 3: Add route in src/main.rs**

Find the API v1 scope registration (look for `web::scope("/api/v1")` or similar):

Add this route inside the scope (before `.service(web::resource(...)`):
```rust
.route("/user/theme", web::post().to(api_v1::users::update_theme))
```

**Step 4: Write integration test**

Add to `tests/user_theme_test.rs`:

```rust
#[actix_web::test]
async fn test_update_theme_api_endpoint() -> Result<(), Box<dyn std::error::Error>> {
    use actix_web::{test, web, App};
    use serde_json::json;
    
    // Mock pool setup (use real DB for integration test)
    let pool = setup_test_db().await?;
    let user_id = create_test_user(&pool, "testuser", "test@example.com").await?;
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/api/v1/user/theme", web::post().to(ahlt::handlers::api_v1::users::update_theme))
    ).await;
    
    // Create session with user_id
    let req = test::TestRequest::post()
        .uri("/api/v1/user/theme")
        .set_json(json!({"theme": "dark"}))
        .to_request();
    
    // Mock session (or use real session if available)
    // This is pseudocode - adjust based on project's session handling
    
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    
    Ok(())
}
```

**Step 5: Run tests**

Run: `cargo test --test user_theme_test -- --nocapture 2>&1 | tail -30`
Expected: All tests pass

**Step 6: Commit**

```bash
git add src/handlers/api_v1/users.rs src/main.rs tests/user_theme_test.rs
git commit -m "feat(api): add POST /api/v1/user/theme endpoint"
```

---

## Task 3: Add Toggle Button to Header

**Files:**
- Modify: `templates/partials/nav.html` (or wherever header is rendered)
- Check: `grep -n "navbar\|header" templates/base.html` to find nav partial

**Step 1: Inspect current nav structure**

Run: `head -30 templates/partials/nav.html`
Expected: See HTML structure of navigation

**Step 2: Add toggle button to nav.html**

Find the top-right corner of the header (usually where user profile icon is). Add before closing `</nav>` or in a controls section:

```html
<div class="theme-toggle-container">
    <button id="theme-toggle" class="theme-toggle-btn" title="Toggle dark mode" aria-label="Toggle dark mode">
        <span class="theme-icon">🌙</span>
    </button>
</div>
```

**Step 3: Verify it renders**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` (no template errors)

**Step 4: Commit**

```bash
git add templates/partials/nav.html
git commit -m "ui(header): add theme toggle button"
```

---

## Task 4: Add Toggle Switch Styling

**Files:**
- Modify: `static/css/style.css`

**Step 1: Find existing CSS structure**

Run: `grep -n "navbar\|header" static/css/style.css | head -10`
Expected: See where nav/header styles are

**Step 2: Add CSS for toggle button**

At end of `static/css/style.css`, add:

```css
/* Theme Toggle Switch */
.theme-toggle-container {
    display: flex;
    align-items: center;
    margin-left: 1rem;
}

.theme-toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2.5rem;
    height: 2.5rem;
    border: 1px solid var(--border-color, #e5e7eb);
    background-color: var(--bg-default, #ffffff);
    border-radius: 0.5rem;
    cursor: pointer;
    transition: background-color 0.2s, border-color 0.2s;
    font-size: 1.25rem;
}

.theme-toggle-btn:hover {
    background-color: var(--bg-subtle, #f3f4f6);
    border-color: var(--border-hover, #d1d5db);
}

.theme-toggle-btn:active {
    background-color: var(--bg-subtle);
    transform: scale(0.95);
}

/* Dark mode variant */
:root.dark .theme-toggle-btn {
    background-color: var(--bg-default, #1f2937);
    border-color: var(--border-color, #374151);
}

:root.dark .theme-toggle-btn:hover {
    background-color: var(--bg-subtle, #374151);
    border-color: var(--border-hover, #4b5563);
}

.theme-icon {
    display: inline-block;
    transition: transform 0.3s ease;
}

.theme-toggle-btn.transitioning .theme-icon {
    animation: spin 0.3s ease-in-out;
}

@keyframes spin {
    0% { transform: rotate(0deg); }
    50% { transform: rotate(180deg); }
    100% { transform: rotate(360deg); }
}
```

**Step 3: Verify styles load**

Run: `cargo build 2>&1 | tail -3`
Expected: `Finished` (no CSS errors)

**Step 4: Commit**

```bash
git add static/css/style.css
git commit -m "style(theme-toggle): add button styling with dark mode support"
```

---

## Task 5: Add JavaScript Toggle Functionality

**Files:**
- Modify: `templates/base.html`

**Step 1: Inspect current base.html structure**

Run: `grep -n "toggleTheme\|theme-preference" templates/base.html`
Expected: See existing theme functions

**Step 2: Add toggle click handler**

In `templates/base.html`, find the `<script>` section with `window.toggleTheme` definition. Add before the closing `</script>` tag:

```javascript
    // Header theme toggle button
    const themeToggle = document.getElementById('theme-toggle');
    if (themeToggle) {
        themeToggle.addEventListener('click', async () => {
            const currentTheme = localStorage.getItem('theme-preference') || 'auto';
            let nextTheme = currentTheme === 'light' ? 'dark' : 'light';
            
            // Add animation class
            themeToggle.classList.add('transitioning');
            
            try {
                // Call API to save theme
                const response = await fetch('/api/v1/user/theme', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ theme: nextTheme })
                });
                
                if (response.ok) {
                    // Apply theme instantly
                    window.toggleTheme(nextTheme);
                } else {
                    console.error('Failed to save theme preference');
                    // Revert animation
                    themeToggle.classList.remove('transitioning');
                }
            } catch (err) {
                console.error('Error saving theme:', err);
                themeToggle.classList.remove('transitioning');
            }
        });
        
        // Remove animation class after animation completes
        themeToggle.addEventListener('animationend', () => {
            themeToggle.classList.remove('transitioning');
        });
    }
```

**Step 3: Update theme icon based on current theme**

Add this before the click handler above:

```javascript
    // Update toggle icon to reflect current theme
    function updateThemeIcon() {
        const currentTheme = localStorage.getItem('theme-preference') || 'auto';
        const icon = themeToggle?.querySelector('.theme-icon');
        if (icon) {
            icon.textContent = currentTheme === 'dark' ? '☀️' : '🌙';
        }
    }
    
    // Update icon on load
    updateThemeIcon();
```

**Step 4: Verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` (no template errors)

**Step 5: Commit**

```bash
git add templates/base.html
git commit -m "feat(header-toggle): add click handler and API integration"
```

---

## Task 6: Load Theme from Database on Page Init

**Files:**
- Modify: `templates/base.html` (init script at top of `<head>`)

**Step 1: Update page init to load from DB**

Find the early theme init script in `<head>` (runs before CSS loads). Replace with:

```html
<script>
    (function() {
        // Fetch theme from server-side context if available
        // This will be injected by the template engine
        const serverTheme = document.documentElement.getAttribute('data-theme') || null;
        
        // Fallback order: server → localStorage → system preference
        let theme = serverTheme;
        if (!theme) {
            theme = localStorage.getItem('theme-preference');
        }
        if (!theme) {
            const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            theme = prefersDark ? 'dark' : 'light';
        }
        
        // Apply theme before CSS loads (prevents flash)
        if (theme === 'dark') {
            document.documentElement.classList.add('dark');
        }
        
        // Persist theme
        localStorage.setItem('theme-preference', theme);
    })();
</script>
```

**Step 2: Update PageContext to include theme**

Modify `src/templates_structs.rs` to add theme field to `PageContext`:

Find the `pub struct PageContext` definition and add:
```rust
pub theme: String,  // "light", "dark", "auto"
```

Add initialization in the `build()` method:
```rust
let theme = crate::models::user::get_user_theme(&pool, user_id).await
    .unwrap_or_else(|_| "auto".to_string());

// ... later in the struct construction
theme,
```

**Step 3: Pass theme to template**

In `templates/base.html`, add `data-theme` attribute to `<html>` tag:

```html
<html lang="en" data-theme="{{ ctx.theme }}">
```

This way, the init script can read it server-side.

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: No errors

**Step 5: Commit**

```bash
git add templates/base.html src/templates_structs.rs
git commit -m "feat(theme): load preference from database on page init"
```

---

## Task 7: Test the Complete Feature

**Files:**
- Manual testing
- `tests/user_theme_test.rs` (run existing tests)

**Step 1: Build and start the server**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished`

Run in separate terminal: `DATABASE_URL=postgresql://ahlt@localhost/ahlt_dev cargo run`
Expected: Server starts on `http://localhost:8080`

**Step 2: Manual browser test**

1. Open `http://localhost:8080`
2. Log in as admin / admin123
3. Look for theme toggle button in top-right corner (🌙 icon)
4. Click it → should switch to dark mode instantly
5. Refresh page → should stay in dark mode
6. Log out and log back in → should remember dark mode
7. Click again → should switch back to light mode

**Step 3: Check browser console**

Open DevTools (F12), Console tab. Should see no errors.

**Step 4: Verify database persistence**

Run: 
```bash
psql postgresql://ahlt@localhost/ahlt_dev -c "SELECT entity_id, key, value FROM entity_properties WHERE key = 'theme_preference' LIMIT 5;"
```
Expected: See theme_preference entries for user_id

**Step 5: Run all theme tests**

Run: `cargo test user_theme -- --nocapture 2>&1 | tail -30`
Expected: All tests pass

**Step 6: Run full test suite**

Run: `cargo test 2>&1 | grep -E "^test result|failures:"` 
Expected: `test result: ok` (no new failures)

**Step 7: Commit final state**

```bash
git add .
git commit -m "test(theme): verify header toggle works end-to-end"
```

---

## Task 8: Update Documentation

**Files:**
- Modify: `docs/BACKLOG.md`

**Step 1: Mark feature complete in backlog**

Edit `docs/BACKLOG.md` and find the DONE section. Add:
```markdown
- P8: Dark Mode Header Toggle (persistent DB storage, syncs across devices) ✓ done
```

**Step 2: Commit**

```bash
git add docs/BACKLOG.md
git commit -m "docs: mark P8 dark mode header toggle as complete"
```

---

## Summary

| Task | Files | Time Est. | Type |
|------|-------|-----------|------|
| 1 | `src/models/user.rs`, `tests/user_theme_test.rs` | 15 min | Backend |
| 2 | `src/handlers/api_v1/users.rs`, `src/main.rs` | 15 min | API |
| 3 | `templates/partials/nav.html` | 5 min | UI |
| 4 | `static/css/style.css` | 15 min | Styling |
| 5 | `templates/base.html` | 15 min | JavaScript |
| 6 | `src/templates_structs.rs`, `templates/base.html` | 15 min | Integration |
| 7 | Manual + automated testing | 20 min | QA |
| 8 | `docs/BACKLOG.md` | 5 min | Docs |

**Total: ~2 hours**

---

## Verification Checklist

- [ ] Toggle button visible in header
- [ ] Click toggles between light/dark instantly
- [ ] Theme persists in database
- [ ] Theme loads on page reload
- [ ] Theme icon updates (☀️ ↔ 🌙)
- [ ] All tests pass
- [ ] No console errors
- [ ] No page flash on load
- [ ] Works across multiple devices/logins
- [ ] Fallback to localStorage works
- [ ] Backlog marked complete

---
