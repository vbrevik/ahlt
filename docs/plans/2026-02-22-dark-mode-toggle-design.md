# Dark Mode Header Toggle — Design Document

**Date**: 2026-02-22  
**Status**: Approved  
**Feature**: P8 - Header Dark Mode Toggle

## Overview

Add a light/dark toggle switch to the top-right corner of the header for quick access to theme switching. Currently, dark mode is only accessible through the account Preferences tab. This feature makes theme switching more discoverable and accessible.

## Goals

1. **Improve accessibility**: Theme switch visible and clickable from any page
2. **Reduce friction**: No need to navigate to Settings → Account → Preferences
3. **Device sync**: Theme preference persists to database, syncs across user's devices
4. **Non-breaking**: Existing Preferences tab continues to work unchanged

## User Experience

### Visual & Interaction
- **Location**: Top-right corner of header, positioned alongside user profile controls
- **UI Style**: Toggle switch (not button group)
- **Icon/Label**: 
  - Light mode: ☀️ or "Light"
  - Dark mode: 🌙 or "Dark"
- **Behavior**: Click to toggle between light ↔ dark modes
- **Feedback**: Visual state change on click, optional toast confirmation
- **Auto mode**: Remains accessible in account Preferences tab for system-preference-following users

### User Workflows
1. **New user visits**: Theme loads from system preference or defaults to light
2. **User clicks toggle**: Theme switches instantly, preference saves to DB
3. **User returns later**: Theme loads from DB automatically
4. **User on different device**: Loads same theme preference from DB
5. **Power user wants Auto**: Can still set it in Preferences tab

## Architecture

### Data Model
Store theme preference in `entity_properties` table:
```
entity_id: user_id
key: 'theme_preference'
value: 'light' | 'dark' | 'auto'
```

Existing entity_properties table can store this without schema changes.

### Backend Components

#### Handler Endpoint
- **Route**: `POST /api/user/theme`
- **Input**: `{ "theme": "light" | "dark" }`
- **Output**: `{ "success": true, "theme": "..." }`
- **Auth**: Require logged-in user session
- **Action**: Update `entity_properties` for current user

#### Models
- `set_user_theme(&pool, user_id: i64, theme: &str)` — Update DB
- `get_user_theme(&pool, user_id: i64)` — Fetch from DB
- Both handle fallback to system preference if not set

#### Account Handler
- Load current theme from DB when rendering account page
- Display current preference in Preferences tab (informational)
- Existing buttons remain functional

### Frontend Components

#### Header Toggle
- Add toggle switch button in top-right corner
- Positioned in header nav/controls area
- Displays current theme state via icon or text

#### Page Initialization
- Early script runs before CSS loads to prevent theme flash
- Reads theme from DB (if available) or localStorage (fallback) or system preference
- Sets `document.documentElement.classList` to 'dark' or 'light'
- Reuses existing `toggleTheme()` function in base.html

#### Toggle Behavior
```
User clicks toggle
  ↓
Fetch current theme from page state
  ↓
Determine next state (light → dark → light)
  ↓
POST to /api/user/theme with new theme
  ↓
On success: Apply theme instantly via toggleTheme()
On error: Revert toggle to previous state
```

#### Storage Hierarchy
1. Database (if logged in) — primary source, syncs across devices
2. localStorage — fallback for unsigned-in users or DB errors
3. System preference — fallback if no explicit preference set

## Data Flow

```
Page Load:
  1. Early init script runs (before CSS)
  2. Try load from DB (if user_id in session)
  3. Fallback to localStorage
  4. Fallback to system preference
  5. Apply theme class to <html>
  6. Render page without flash

User Clicks Toggle:
  1. Read current theme from DOM state
  2. Calc next theme (light ↔ dark)
  3. POST /api/user/theme
  4. On success: toggleTheme() + update DOM
  5. On error: show toast, revert toggle
  6. Update localStorage as backup

Theme Syncs Across Devices:
  1. User logs in on Device A → theme from DB
  2. User logs in on Device B → same theme from DB
  3. Toggle on Device A → DB updated → Device B eventually loads it
```

## Implementation Scope

### Files to Create/Modify
1. **src/handlers/api_v1/users.rs** — Add `set_theme()` endpoint
2. **src/models/user.rs** — Add theme getter/setter functions
3. **templates/partials/header.html** (or nav partial) — Add toggle button
4. **static/css/style.css** — Add toggle switch styling
5. **templates/base.html** — Update init script to load from DB
6. **docs/BACKLOG.md** — Mark as complete when done

### No Changes Needed
- Account handler (existing Preferences tab stays as-is)
- entity_properties schema (already supports key-value storage)
- Permission checks (theme is user-owned data)

## Error Handling

### API Failures
- Toggle reverts to previous state
- Silent fail or toast notification: "Theme preference could not be saved"
- localStorage still works as fallback

### Initialization
- If DB query fails on page load, falls back to localStorage
- If both fail, uses system preference
- Never blocks page render

## Testing Strategy

### Unit Tests
- `test_set_user_theme()` — Save theme to DB
- `test_get_user_theme()` — Retrieve saved theme
- `test_theme_fallback_order()` — DB → localStorage → system pref

### Integration Tests
- E2E: Toggle theme in header, reload page, verify persists
- Cross-device: Log in as same user on two browsers, toggle on one, verify on other

### Manual Testing
- Theme loads correctly on first visit
- Toggle works instantly without page reload
- Preference persists after logout/login
- Existing Preferences tab still works
- No theme flash on page load

## Success Criteria

1. ✅ Toggle switch visible in header top-right
2. ✅ Clicking toggle switches theme instantly
3. ✅ Theme preference saves to database
4. ✅ Theme preference loads on page init (no flash)
5. ✅ Theme syncs across user's devices
6. ✅ Fallback to localStorage for unsigned-in users
7. ✅ Existing Preferences tab continues to work
8. ✅ No console errors, all tests pass

## Out of Scope (Future)

- Theme options beyond light/dark (e.g., high contrast, custom themes)
- Per-page theme overrides
- Admin-enforced theme for all users
- Browser extension for theme sync
- Scheduling (e.g., dark mode after sunset)

## Deployment Notes

- No database migrations required (uses existing entity_properties table)
- Feature is backward compatible (existing light/dark functionality unchanged)
- localStorage fallback ensures graceful degradation if DB unavailable
- Can be deployed and tested independently
