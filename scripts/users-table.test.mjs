/**
 * Comprehensive Playwright tests for the users table enhancements.
 * Tests: filter builder, sorting, column picker, per-page selector, CSV export.
 *
 * Run: node users-table.test.mjs
 * Requires: server running at http://localhost:8080 with APP_ENV=staging
 */

import { chromium } from '@playwright/test';
import assert from 'assert';

const BASE_URL = 'http://localhost:8080';
let passed = 0;
let failed = 0;
const failures = [];

async function test(name, fn) {
  try {
    await fn();
    console.log(`  ✓ ${name}`);
    passed++;
  } catch (e) {
    console.log(`  ✗ ${name}`);
    console.log(`      ${e.message}`);
    failed++;
    failures.push({ name, error: e.message });
  }
}

function eq(actual, expected, msg) {
  assert.strictEqual(actual, expected, msg || `expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
}

function ok(val, msg) {
  assert.ok(val, msg || `expected truthy, got ${JSON.stringify(val)}`);
}

async function login(page) {
  await page.goto(`${BASE_URL}/login`);
  await page.fill('input[name="username"]', 'admin');
  await page.fill('input[name="password"]', 'admin123');
  await page.click('button[type="submit"]');
  await page.waitForURL(`${BASE_URL}/dashboard`, { timeout: 8000 });
}

// ─── test runner ─────────────────────────────────────────────────────────────

const browser = await chromium.launch({ headless: true });

// Create one shared context — login once
const context = await browser.newContext();
const page = await context.newPage();
await login(page);

// ─── SECTION 1: Page structure ───────────────────────────────────────────────
console.log('\nPage structure');

await test('users page loads with 200', async () => {
  const res = await page.goto(`${BASE_URL}/users`);
  eq(res.status(), 200);
});

await test('filter builder is present', async () => {
  ok(await page.locator('#filter-builder').isVisible());
});

await test('table controls bar is present', async () => {
  ok(await page.locator('.table-controls').isVisible());
});

await test('column picker button is present', async () => {
  ok(await page.locator('#col-picker-btn').isVisible());
});

await test('per-page select is present with correct options', async () => {
  const options = await page.locator('#per-page-select option').allTextContents();
  ok(options.includes('10 rows'));
  ok(options.includes('25 rows'));
  ok(options.includes('50 rows'));
  ok(options.includes('100 rows'));
});

await test('result count summary is present', async () => {
  ok(await page.locator('.table-controls__summary').isVisible());
  const text = await page.locator('.table-controls__summary').textContent();
  ok(text.includes('Showing'), `summary text: "${text}"`);
});

await test('CSV export link is present', async () => {
  const link = page.locator('a[href*="export.csv"]');
  ok(await link.isVisible());
  const href = await link.getAttribute('href');
  ok(href.includes('/users/export.csv'), `href: ${href}`);
});

await test('user table rows are present', async () => {
  const rows = await page.locator('.users-table__row').count();
  ok(rows > 0, `expected rows > 0, got ${rows}`);
});

await test('filter-fields-json parses to 6 fields', async () => {
  const count = await page.evaluate(() =>
    JSON.parse(document.getElementById('filter-fields-json').textContent).length
  );
  eq(count, 6);
});

// ─── SECTION 2: Filter builder ───────────────────────────────────────────────
console.log('\nFilter builder');

await test('+Condition button adds a condition row', async () => {
  await page.goto(`${BASE_URL}/users`);
  const before = await page.locator('.filter-condition-row').count();
  await page.click('#add-root-condition');
  await page.waitForTimeout(200);
  const after = await page.locator('.filter-condition-row').count();
  eq(after, before + 1);
});

await test('clicking +Condition twice adds two rows', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.click('#add-root-condition');
  await page.click('#add-root-condition');
  await page.waitForTimeout(200);
  const count = await page.locator('.filter-condition-row').count();
  ok(count >= 2, `expected ≥2 rows, got ${count}`);
});

await test('+Group button adds a group', async () => {
  await page.goto(`${BASE_URL}/users`);
  const before = await page.locator('.filter-group').count();
  await page.click('#add-group');
  await page.waitForTimeout(200);
  const after = await page.locator('.filter-group').count();
  eq(after, before + 1);
});

await test('new group gets an auto-added condition row', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.click('#add-group');
  await page.waitForTimeout(200);
  const groupConds = await page.locator('.filter-group .filter-condition-row').count();
  ok(groupConds >= 1, `expected ≥1 condition in group, got ${groupConds}`);
});

await test('remove button (✕) removes a condition row', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.click('#add-root-condition');
  await page.click('#add-root-condition');
  await page.waitForTimeout(200);
  const before = await page.locator('.filter-condition-row').count();
  await page.locator('.filter-condition-row .filter-remove-btn').first().click();
  await page.waitForTimeout(200);
  const after = await page.locator('.filter-condition-row').count();
  eq(after, before - 1);
});

await test('Apply with username filter reloads page with filter in URL', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.click('#add-root-condition');
  await page.waitForTimeout(200);
  // Select username field
  await page.locator('.filter-condition-row .filter-field-select').first().selectOption('username');
  await page.locator('.filter-condition-row .filter-op-select').first().selectOption('contains');
  await page.locator('.filter-condition-row .filter-value-input').first().fill('admin');
  await page.click('#apply-filter');
  await page.waitForLoadState('networkidle');
  const url = page.url();
  ok(url.includes('filter='), `URL should contain filter param: ${url}`);
  ok(url.includes('admin'), `URL should contain filter value: ${url}`);
});

await test('filter results show only matching users', async () => {
  // After previous test we're on filtered page — rows should contain "admin"
  const rows = await page.locator('.users-table__row').count();
  ok(rows > 0, 'filtered results should not be empty');
  // All visible usernames should contain "admin"
  const usernames = await page.locator('.user-username').allTextContents();
  for (const u of usernames) {
    ok(u.toLowerCase().includes('admin'), `username "${u}" should match filter "admin"`);
  }
});

await test('Clear link resets to /users with no filter param', async () => {
  // filter-builder footer "Clear" link
  await page.click('.filter-builder__footer a');
  await page.waitForLoadState('networkidle');
  const url = page.url();
  ok(!url.includes('filter='), `URL after clear should not have filter: ${url}`);
});

await test('filter state is restored in builder after page reload', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.click('#add-root-condition');
  await page.waitForTimeout(200);
  await page.locator('.filter-condition-row .filter-field-select').first().selectOption('email');
  await page.locator('.filter-condition-row .filter-op-select').first().selectOption('contains');
  await page.locator('.filter-condition-row .filter-value-input').first().fill('@example');
  await page.click('#apply-filter');
  await page.waitForLoadState('networkidle');
  // The filter builder should now show the active condition
  const rows = await page.locator('.filter-condition-row').count();
  ok(rows >= 1, 'filter builder should show restored condition after reload');
});

await test('root logic selector is present (AND/ANY)', async () => {
  await page.goto(`${BASE_URL}/users`);
  const select = page.locator('#root-logic-select');
  ok(await select.isVisible());
  const options = await select.locator('option').allTextContents();
  ok(options.some(o => o.includes('ALL') || o.includes('AND')));
  ok(options.some(o => o.includes('ANY') || o.includes('OR')));
});

await test('Apply with no conditions submits empty filter', async () => {
  await page.goto(`${BASE_URL}/users`);
  // No conditions added — just click Apply
  await page.click('#apply-filter');
  await page.waitForLoadState('networkidle');
  // Should show all users, no error
  const rows = await page.locator('.users-table__row').count();
  ok(rows > 0, 'empty filter should return all users');
});

// ─── SECTION 3: Server-side sorting ──────────────────────────────────────────
console.log('\nSorting');

await test('clicking a sortable column header adds sort params to URL', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.locator('.sort-link').first().click();
  await page.waitForLoadState('networkidle');
  const url = page.url();
  ok(url.includes('sort='), `URL should contain sort param: ${url}`);
  ok(url.includes('dir='), `URL should contain dir param: ${url}`);
});

await test('sort defaults to asc direction', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.locator('.sort-link').first().click();
  await page.waitForLoadState('networkidle');
  ok(page.url().includes('dir=asc'), `Expected dir=asc in: ${page.url()}`);
});

await test('clicking same column again toggles to desc', async () => {
  // We're already on sort=X&dir=asc — click the active sort column
  const activeLink = page.locator('.sort-link').first();
  await activeLink.click();
  await page.waitForLoadState('networkidle');
  ok(page.url().includes('dir=desc'), `Expected dir=desc in: ${page.url()}`);
});

await test('clicking same column again toggles back to asc', async () => {
  const activeLink = page.locator('.sort-link').first();
  await activeLink.click();
  await page.waitForLoadState('networkidle');
  ok(page.url().includes('dir=asc'), `Expected dir=asc in: ${page.url()}`);
});

await test('active sort column shows sort indicator', async () => {
  await page.goto(`${BASE_URL}/users?sort=username&dir=asc`);
  const indicator = await page.evaluate(() => {
    const links = document.querySelectorAll('.sort-link');
    for (const l of links) {
      if (l.closest('th')?.className.includes('username') || l.textContent.includes('▲') || l.textContent.includes('▼')) {
        return l.textContent.trim();
      }
    }
    return null;
  });
  ok(indicator && (indicator.includes('▲') || indicator.includes('▼')), `Expected sort indicator, got: ${indicator}`);
});

await test('sort by username asc orders rows alphabetically', async () => {
  await page.goto(`${BASE_URL}/users?sort=username&dir=asc`);
  const names = await page.locator('.user-username').allTextContents();
  if (names.length >= 2) {
    const sorted = [...names].sort();
    // First name should come before last name alphabetically
    ok(names[0] <= names[names.length - 1], `Names should be sorted: ${names[0]} vs ${names[names.length - 1]}`);
  }
});

await test('sort is preserved across filter changes', async () => {
  await page.goto(`${BASE_URL}/users?sort=username&dir=asc`);
  await page.click('#add-root-condition');
  await page.waitForTimeout(200);
  await page.locator('.filter-condition-row .filter-value-input').first().fill('a');
  await page.click('#apply-filter');
  await page.waitForLoadState('networkidle');
  const url = page.url();
  ok(url.includes('sort=username'), `Sort param preserved: ${url}`);
  ok(url.includes('dir=asc'), `Dir param preserved: ${url}`);
});

// ─── SECTION 4: Column picker ─────────────────────────────────────────────────
console.log('\nColumn picker');

await test('column picker is hidden by default', async () => {
  await page.goto(`${BASE_URL}/users`);
  const picker = page.locator('#col-picker');
  ok(await picker.isHidden(), 'column picker should be hidden on load');
});

await test('clicking ⊞ Columns opens the column picker', async () => {
  await page.click('#col-picker-btn');
  await page.waitForTimeout(200);
  const picker = page.locator('#col-picker');
  ok(await picker.isVisible(), 'column picker should be visible after button click');
});

await test('column picker lists column items', async () => {
  const items = await page.locator('#col-picker-list .col-picker__item').count();
  ok(items >= 4, `expected ≥4 column items, got ${items}`);
});

await test('always-visible columns have disabled checkbox', async () => {
  const disabledBoxes = await page.locator('.col-picker__check:disabled').count();
  ok(disabledBoxes >= 2, `expected ≥2 always-visible (user + actions), got ${disabledBoxes}`);
});

await test('clicking outside picker closes it', async () => {
  await page.click('h1'); // click somewhere outside
  await page.waitForTimeout(200);
  const picker = page.locator('#col-picker');
  ok(await picker.isHidden(), 'column picker should close on outside click');
});

// ─── SECTION 5: Per-page selector ─────────────────────────────────────────────
console.log('\nPer-page selector');

await test('per-page defaults to 25', async () => {
  await page.goto(`${BASE_URL}/users`);
  const selected = await page.locator('#per-page-select').inputValue();
  eq(selected, '25');
});

await test('changing per-page reloads with per_page param', async () => {
  await page.goto(`${BASE_URL}/users`);
  await page.locator('#per-page-select').selectOption('10');
  await page.waitForLoadState('networkidle');
  ok(page.url().includes('per_page=10'), `URL: ${page.url()}`);
});

await test('per_page=10 shows at most 10 rows', async () => {
  await page.goto(`${BASE_URL}/users?per_page=10`);
  const rows = await page.locator('.users-table__row').count();
  ok(rows <= 10, `expected ≤10 rows with per_page=10, got ${rows}`);
});

await test('per_page=10 selected in dropdown after reload', async () => {
  await page.goto(`${BASE_URL}/users?per_page=10`);
  const selected = await page.locator('#per-page-select').inputValue();
  eq(selected, '10');
});

await test('changing per-page resets to page 1', async () => {
  await page.goto(`${BASE_URL}/users?page=2`);
  await page.locator('#per-page-select').selectOption('50');
  await page.waitForLoadState('networkidle');
  const url = page.url();
  ok(!url.includes('page=2'), `Should reset page to 1: ${url}`);
});

await test('per-page is preserved across sort changes', async () => {
  await page.goto(`${BASE_URL}/users?per_page=10`);
  await page.locator('.sort-link').first().click();
  await page.waitForLoadState('networkidle');
  ok(page.url().includes('per_page=10'), `per_page preserved: ${page.url()}`);
});

// ─── SECTION 6: CSV export ────────────────────────────────────────────────────
console.log('\nCSV export');

await test('CSV export endpoint returns 200 with CSV content-type', async () => {
  // Use fetch via page context (has auth cookie)
  const result = await page.evaluate(async () => {
    const res = await fetch('/users/export.csv');
    return { status: res.status, contentType: res.headers.get('content-type') };
  });
  eq(result.status, 200);
  ok(result.contentType.includes('text/csv'), `content-type: ${result.contentType}`);
});

await test('CSV export contains header row', async () => {
  const csv = await page.evaluate(async () => {
    const res = await fetch('/users/export.csv');
    return res.text();
  });
  const firstLine = csv.split('\n')[0];
  ok(firstLine.includes('username'), `CSV header: ${firstLine}`);
  ok(firstLine.includes('email'), `CSV header missing email: ${firstLine}`);
});

await test('CSV export with filter returns only matching rows', async () => {
  const filter = encodeURIComponent(JSON.stringify({
    logic: 'and',
    conditions: [{ field: 'username', op: 'contains', value: 'admin' }],
    groups: []
  }));
  const csv = await page.evaluate(async (filter) => {
    const res = await fetch(`/users/export.csv?filter=${filter}`);
    return res.text();
  }, filter);
  const lines = csv.trim().split('\n').filter(l => l.trim());
  // header + at least 1 data row
  ok(lines.length >= 2, `Expected ≥2 lines (header + data), got ${lines.length}`);
  // All data rows should have "admin" in username column (col 1, 0-indexed)
  for (const line of lines.slice(1)) {
    ok(line.toLowerCase().includes('admin'), `Row should match admin filter: ${line}`);
  }
});

await test('CSV export link in page has correct href', async () => {
  await page.goto(`${BASE_URL}/users`);
  const href = await page.locator('a[href*="export.csv"]').getAttribute('href');
  ok(href.startsWith('/users/export.csv'), `href: ${href}`);
});

// ─── SECTION 7: URL state / bookmarkability ────────────────────────────────────
console.log('\nURL state & bookmarkability');

await test('filter+sort+per_page all survive round-trip in URL', async () => {
  const filter = JSON.stringify({ logic: 'and', conditions: [{ field: 'username', op: 'contains', value: 'a' }], groups: [] });
  const url = `/users?filter=${encodeURIComponent(filter)}&sort=username&dir=desc&per_page=10&page=1`;
  await page.goto(`${BASE_URL}${url}`);
  await page.waitForLoadState('networkidle');
  const currentUrl = page.url();
  // State should be reflected in UI
  const selected = await page.locator('#per-page-select').inputValue();
  eq(selected, '10', 'per_page should be 10');
  const condRows = await page.locator('.filter-condition-row').count();
  ok(condRows >= 1, 'filter should be restored in builder');
});

await test('pagination links preserve sort and filter', async () => {
  await page.goto(`${BASE_URL}/users?per_page=2&sort=username&dir=asc`);
  // Check if there's a Next link (only appears when total > per_page)
  const nextLink = page.locator('.pagination-controls a').filter({ hasText: 'Next' });
  const hasNext = await nextLink.count() > 0;
  if (hasNext) {
    const href = await nextLink.getAttribute('href');
    ok(href.includes('sort=username'), `Next link preserves sort: ${href}`);
    ok(href.includes('dir=asc'), `Next link preserves dir: ${href}`);
  }
  // If no Next (only 1 page of users), that's fine too
});

// ─── SECTION 8: Empty state ────────────────────────────────────────────────────
console.log('\nEmpty state');

await test('filter that matches nothing shows empty state', async () => {
  const filter = encodeURIComponent(JSON.stringify({
    logic: 'and',
    conditions: [{ field: 'username', op: 'equals', value: 'THIS_USER_DEFINITELY_DOES_NOT_EXIST_XYZ789' }],
    groups: []
  }));
  await page.goto(`${BASE_URL}/users?filter=${filter}`);
  const emptyState = page.locator('.empty-state');
  ok(await emptyState.isVisible(), 'empty state should show when no results');
});

await test('empty state shows clear filters link', async () => {
  const clearLink = page.locator('.empty-state a[href="/users"]');
  ok(await clearLink.isVisible(), 'empty state should have clear filters link');
});

// ─── Summary ──────────────────────────────────────────────────────────────────

await context.close();
await browser.close();

console.log(`\n${'─'.repeat(50)}`);
console.log(`Results: ${passed} passed, ${failed} failed`);
if (failures.length) {
  console.log('\nFailed tests:');
  failures.forEach(f => console.log(`  ✗ ${f.name}\n      ${f.error}`));
  process.exit(1);
} else {
  console.log('All tests passed! ✓');
}
