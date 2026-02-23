# TD.1 CSS Monolith Split — Design

**Date**: 2026-02-23
**Status**: Approved
**Approach**: Automated section extraction (Approach A)

## Problem

`static/css/style.css` is 6,144 lines. A modular PostCSS structure (`index.css` + `base/`, `components/`, `layout/`, `pages/`, `utilities/`) already exists but is out of sync with the monolith. This design treats the monolith as the source of truth and re-extracts it fresh.

## Architecture

### Directory structure (post-split)

```
static/css/
├── index.css              ← PostCSS entry point (@import declarations)
├── style.css              ← Build artifact (generated, gitignored after cutover)
├── base/
│   ├── variables.css      ← Tokens + Dark Mode
│   ├── reset.css          ← Reset & Base
│   └── typography.css     ← Typography
├── components/
│   ├── alerts.css
│   ├── badges.css
│   ├── buttons.css
│   ├── cards.css          ← Generic .card rules only
│   ├── detail-card.css
│   ├── forms.css
│   ├── graph-panel.css
│   ├── navbar.css
│   ├── pipeline-tabs.css
│   ├── relation-cards.css
│   ├── section.css
│   ├── sidebar.css
│   ├── tab-bar.css
│   ├── tables.css
│   ├── tabs.css
│   ├── toast.css
│   └── tor-context.css
├── layout/
│   ├── app.css
│   ├── empty-states.css
│   ├── form-layout.css
│   ├── page-header.css
│   ├── pagination.css
│   └── search-filter.css
├── pages/
│   ├── calendar.css
│   ├── dashboard.css      ← .dash-* rules extracted from Cards & Dashboard
│   ├── data-browser.css
│   ├── dependency-flow.css
│   ├── entity-detail.css
│   ├── error.css
│   ├── functions-list.css
│   ├── governance-cards.css
│   ├── governance-graph.css
│   ├── graph.css
│   ├── inline-form.css
│   ├── login.css
│   ├── menu-builder.css
│   ├── ontology.css
│   ├── point-paper.css
│   ├── positions-list.css
│   ├── profile.css
│   ├── protocol-steps.css
│   ├── role-builder.css
│   ├── role-list.css
│   ├── role-permissions.css
│   ├── schema-tables.css
│   ├── tor-grid.css
│   ├── tor-info-grid.css
│   └── users-list.css
└── utilities/
    ├── animations.css
    ├── focus.css
    ├── muted-id.css
    ├── responsive.css
    ├── scrollbar.css
    ├── status-badges.css
    ├── theme-selection.css
    ├── utilities.css
    └── warning-styles.css
```

### Section-to-file mapping

The monolith has 63 section headers (`/* === Section Name === */`). Each maps to exactly one target file. Key special cases:

- **Cards & Dashboard** (L704–1203, ~500 lines): Split by selector — `.dash-*` rules → `pages/dashboard.css`, generic `.card` rules → `components/cards.css`
- **Users List**: 4 non-contiguous blocks (L4688–5053, L5054–5206, L5208–5291, L5686–5874) → single `pages/users-list.css`
- **Calendar**: 2 contiguous blocks (L3951–4132 overview, L4133–4513 meeting outlook) → single `pages/calendar.css`
- **Theme Selection** (L4552–4601): Goes to `utilities/theme-selection.css` not pages
- **Missing utility classes** (L3284–3300): Appended to `utilities/utilities.css`
- **Dark Mode** (L59–96): Appended to `base/variables.css` after tokens

## Extraction Process

### Step 1: Build extraction script

`scripts/split-css.js` — reads `style.css`, splits on section comment markers, writes each section to its target file per the mapping table.

### Step 2: Run extraction

```bash
cp static/css/style.css static/css/style.css.bak
node scripts/split-css.js
```

### Step 3: Update index.css

Regenerate `index.css` with `@import` statements matching the new file set, ordered: base → layout → components → pages → utilities.

### Step 4: Install and build

```bash
npm install
npm run css:build
```

### Step 5: Verify round-trip

```bash
diff static/css/style.css.bak static/css/style.css
```

Must be identical (modulo the header comment). Any diff indicates a missed section — fix the mapping and re-run.

### Step 6: Commit and gitignore

1. Commit all modular files + updated `index.css`
2. Add `static/css/style.css` to `.gitignore` in a follow-up commit (after CI confirms the build step works)

## Build pipeline

- **Dev**: `npm run css:watch` auto-rebuilds on file changes
- **CI/Docker**: `npm run css:build` as pre-build step
- **Verification**: `npm run css:verify` (existing `scripts/verify-css.js`) as CI gate

## Risk mitigation

- **Zero-regression**: Diff-check after PostCSS build guarantees byte-for-byte match
- **Rollback**: `style.css` stays committed until first successful CI run with build step
- **Template changes**: None — `base.html` links `style.css` which remains the compiled output path
- **No new dependencies**: PostCSS stack already declared in `package.json`
