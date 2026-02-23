# E2E Test Suite

End-to-end tests for the Ahlt platform. These tests require a running server with staging seed data.

## Prerequisites

- **Node.js** >= 18 (tested with v22)
- **Playwright** (Chromium browser)
- **Rust toolchain** (for building and running the server)
- **PostgreSQL** running with `ahlt_staging` database

## One-Time Setup

### 1. Install Playwright

Create a temporary working directory for Playwright (the test scripts import from `@playwright/test`):

```bash
mkdir -p /tmp/pw-test
cd /tmp/pw-test
npm init -y
npm install @playwright/test
npx playwright install chromium
```

### 2. Start Infrastructure

```bash
# From project root
make infra   # or: docker compose up -d postgres neo4j
```

### 3. Start the Server with Staging Data

The E2E tests expect the staging seed data (demo users, ToRs, meetings, etc.):

```bash
APP_ENV=staging cargo run
```

Wait until you see `starting service` in the terminal output. The server listens on **http://localhost:8080**.

**Credentials:** `admin` / `admin123`

## Test Suites

### Playwright (JavaScript)

| File | Tests | Coverage |
|------|-------|----------|
| `users-table.test.mjs` | 46 | Users table: filter builder, sorting, column picker, per-page, CSV export, URL state |
| `test_admin_screens.py` | — | Admin screen smoke test with screenshots (Python/Playwright) |

### Rust curl-based E2E

| File | Tests | Coverage |
|------|-------|----------|
| `tests/calendar_confirmation_e2e.rs` | 4 | Calendar outlook: page load, meeting creation, projected/confirmed styling, event data |

## Running Tests

### Playwright JS Tests

With the server running (`APP_ENV=staging cargo run`):

```bash
cd /tmp/pw-test
node /path/to/im-ctrl/scripts/users-table.test.mjs
```

Output shows pass/fail per test with a summary at the end.

### Python Admin Screen Tests

```bash
pip install playwright   # if not already installed
python scripts/test_admin_screens.py
```

Screenshots are saved to `/tmp/admin_*.png`.

### Rust E2E Tests

These tests are `#[ignore]` by default because they start their own server process. Run them explicitly:

```bash
# Server must NOT already be running on port 8080
# Tests start/stop their own server via `cargo run`
cargo test --test calendar_confirmation_e2e -- --ignored --test-threads=1 --nocapture
```

**Important:** Use `--test-threads=1` — each test starts and stops a server on port 8080, so they must run sequentially.

## Cookie Isolation

Each Rust E2E test uses a unique cookie file to prevent session bleed between tests:

```
/tmp/cookies-test_can_view_outlook_calendar.txt
/tmp/cookies-test_can_create_and_confirm_projected_meeting.txt
/tmp/cookies-test_projected_vs_confirmed_styling.txt
/tmp/cookies-test_calendar_event_data_structure.txt
```

This prevents CI flakes where one test's login session interferes with another. The Playwright JS suite handles this differently — it creates one shared browser context with a single login, then runs all tests sequentially in that context.

## Troubleshooting

- **Port 8080 in use:** Kill any existing server before running Rust E2E tests (`lsof -ti:8080 | xargs kill`)
- **Playwright not found:** Ensure you ran `npm install @playwright/test` in `/tmp/pw-test` and are running from that directory
- **Login fails:** Verify the server is running with `APP_ENV=staging` (staging fixtures set the admin password to `admin123`)
- **Empty calendar:** Staging seed data includes pre-configured ToRs and meetings. If the database was recreated without `APP_ENV=staging`, the seed data won't include demo content
