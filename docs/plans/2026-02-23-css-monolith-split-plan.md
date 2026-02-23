# TD.1 CSS Monolith Split — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the 6,144-line `static/css/style.css` monolith with a PostCSS modular build, making the split files the source of truth.

**Architecture:** A Node extraction script parses the monolith by section-comment markers and writes each section to the correct module file. PostCSS (`postcss-import`) concatenates the modules back into `style.css`. A diff-check verifies zero regression.

**Tech Stack:** PostCSS, postcss-import, postcss-cli (already in package.json), Node.js for extraction script.

**Design doc:** `docs/plans/2026-02-23-css-monolith-split-design.md`

---

### Task 1: Setup — backup monolith and install PostCSS deps

**Files:**
- Existing: `static/css/style.css`
- Existing: `package.json`

**Step 1: Back up the monolith**

```bash
cp static/css/style.css static/css/style.css.bak
```

**Step 2: Install PostCSS dependencies**

```bash
npm install
```

Expected: `node_modules/` created with postcss, postcss-cli, postcss-import.

**Step 3: Verify PostCSS CLI works**

```bash
npx postcss --version
```

Expected: Version number printed (8.x).

---

### Task 2: Write the extraction script

**Files:**
- Create: `scripts/split-css.js`

**Step 1: Create the extraction script**

The script must:
1. Read `static/css/style.css.bak` (the original monolith backup)
2. Split content into sections at lines matching `^/\*\s*={3,}` (the section comment pattern)
3. Map each section to a target file using a lookup table
4. Handle special cases:
   - **Header comment** (L1–5): Discard (PostCSS build doesn't need it)
   - **Tokens + Dark Mode** (L6–96): Both → `base/variables.css`
   - **Cards & Dashboard** (L704–1203): Split by selector prefix — `.dash-*` and `.stat-*` rules → `pages/dashboard.css`, everything else → `components/cards.css`
   - **Data Manager + Meeting Outlook Calendar** (L3951–4513): Both are calendar/data-manager related. The "Data Manager" block (L3951–4132) contains `.dm-*` selectors for the data manager page. The "Meeting Outlook Calendar" block (L4133–4513) contains calendar/meeting styles. Route: Data Manager → `pages/data-manager.css` (append), Meeting Outlook Calendar → `pages/calendar.css` (append)
   - **Users List** has 5 sub-sections: "Users List Redesign" (L4688), "Filter Builder" (L5054), "Table Controls Bar" (L5174), "Column Picker Popover" (L5208), "Users page — editorial directory redesign" (L5686) → all → `pages/users-list.css`
   - **Point Paper** (L5292–5685): Contains embedded ToR Context Bar sub-section (L5577–5685, marked with `/* ── ToR Context Bar ── */`). Extract ToR Context Bar rules → `components/tor-context.css`, remainder → `pages/point-paper.css`
   - **Utility** (L4602–4606): Append to `utilities/utilities.css`
   - **Missing utility classes** (L3284–3300): Append to `utilities/utilities.css`
5. Write each section to its target file (overwriting existing modular files)
6. Print a summary: file count, total lines written, any unmapped sections

```javascript
#!/usr/bin/env node
/**
 * TD.1 CSS Monolith Split — Extraction Script
 *
 * Parses static/css/style.css.bak by section-comment headers
 * and writes each section to its modular target file.
 */
const fs = require('fs');
const path = require('path');

const CSS_DIR = path.join(__dirname, '../static/css');
const SOURCE = path.join(CSS_DIR, 'style.css.bak');

// Section header pattern: /* === Name === */ or /* ======...\n   Name\n   ======... */
const SECTION_RE = /^\/\*\s*={3,}/;

// Mapping: section name (lowercased, trimmed) → target file relative to CSS_DIR
// Sections that map to the same file will be concatenated in source order.
const SECTION_MAP = {
  'tokens':                               'base/variables.css',
  'dark mode':                            'base/variables.css',
  'reset & base':                         'base/reset.css',
  'typography':                           'base/typography.css',

  'app layout':                           'layout/app.css',
  'page header':                          'layout/page-header.css',
  'search & filter bars':                 'layout/search-filter.css',
  'empty states':                         'layout/empty-states.css',
  'pagination':                           'layout/pagination.css',
  'form layout':                          'layout/form-layout.css',

  'sidebar':                              'components/sidebar.css',
  'navbar':                               'components/navbar.css',
  'buttons':                              'components/buttons.css',
  'forms':                                'components/forms.css',
  'tables':                               'components/tables.css',
  'alerts':                               'components/alerts.css',
  'cards & dashboard':                    '__SPLIT_CARDS_DASHBOARD__',
  'badges':                               'components/badges.css',
  'tab bar':                              'components/tab-bar.css',
  'relation cards':                       'components/relation-cards.css',
  'graph panel (shared between governance map and workflow builder)': 'components/graph-panel.css',
  'pipeline & tabs':                      'components/pipeline-tabs.css',
  'detail card':                          'components/detail-card.css',
  'section component':                    'components/section.css',
  'tabs':                                 'components/tabs.css',

  'error pages':                          'pages/error.css',
  'login':                                'pages/login.css',
  'ontology concepts':                    'pages/ontology.css',
  'schema tables':                        'pages/schema-tables.css',
  'graph view':                           'pages/graph.css',
  'data browser':                         'pages/data-browser.css',
  'entity detail':                        'pages/entity-detail.css',
  'role permissions ui':                  'pages/role-permissions.css',
  'inline form (rejection reason rows)':  'pages/inline-form.css',
  'tor card grid (list page)':            'pages/tor-grid.css',
  'tor info grid (detail header)':        'pages/tor-info-grid.css',
  'positions list':                       'pages/positions-list.css',
  'functions list':                       'pages/functions-list.css',
  'protocol steps':                       'pages/protocol-steps.css',
  'dependency flow':                      'pages/dependency-flow.css',
  'governance map cards':                 'pages/governance-cards.css',
  'governance graph (dag)':               'pages/governance-graph.css',
  'data manager':                         'pages/calendar.css',
  'meeting outlook calendar':             'pages/calendar.css',
  'profile section':                      'pages/profile.css',
  'users list redesign':                  'pages/users-list.css',
  'filter builder':                       'pages/users-list.css',
  'table controls bar':                   'pages/users-list.css',
  'column picker popover':                'pages/users-list.css',
  'users page — editorial directory redesign': 'pages/users-list.css',
  'point paper (agenda point detail)':    '__SPLIT_POINT_PAPER__',
  'role builder — compact precision layout': 'pages/role-builder.css',

  'utilities':                            'utilities/utilities.css',
  'scrollbar':                            'utilities/scrollbar.css',
  'animations':                           'utilities/animations.css',
  'focus visible':                        'utilities/focus.css',
  'muted id columns':                     'utilities/muted-id.css',
  'responsive':                           'utilities/responsive.css',
  'status badges':                        'utilities/status-badges.css',
  'missing utility classes':              'utilities/utilities.css',
  'theme selection':                      'utilities/theme-selection.css',
  'utility':                              'utilities/utilities.css',
};

function parseSections(lines) {
  const sections = [];
  let current = null;

  for (let i = 0; i < lines.length; i++) {
    if (SECTION_RE.test(lines[i])) {
      // Extract section name from this line or next lines
      const name = extractSectionName(lines, i);
      if (name && name !== 'ahlt — design system') {
        if (current) {
          current.endLine = i - 1;
          sections.push(current);
        }
        current = { name, startLine: i, endLine: null, lines: [] };
      }
    }
    if (current) {
      current.lines.push(lines[i]);
    }
  }
  if (current) {
    current.endLine = lines.length - 1;
    sections.push(current);
  }
  return sections;
}

function extractSectionName(lines, idx) {
  const line = lines[idx];
  // Pattern 1: /* === Name === */  (single line)
  const singleLine = line.match(/\/\*\s*={3,}\s*(.+?)\s*={3,}\s*\*\//);
  if (singleLine) return singleLine[1].trim().toLowerCase();

  // Pattern 2: Multi-line block comment with name on next line
  //   /* ==================
  //      Name
  //      ================== */
  if (idx + 1 < lines.length) {
    const nextLine = lines[idx + 1].trim();
    // Skip lines that are just === or */
    if (nextLine && !nextLine.match(/^[=*\/\s]+$/) && !nextLine.startsWith('/*')) {
      // Check if it's followed by a closing === */ line
      if (idx + 2 < lines.length && /={3,}\s*\*\//.test(lines[idx + 2])) {
        return nextLine.toLowerCase();
      }
    }
  }
  return null;
}

function splitCardsDashboard(sectionLines) {
  const cardLines = [];
  const dashLines = [];
  let inDash = false;

  for (const line of sectionLines) {
    // .dash-* and .stat-* selectors belong to dashboard
    if (/^\.(dash-|stat-)/.test(line.trim()) || /^\/\*.*dash/i.test(line.trim())) {
      inDash = true;
    }
    // New top-level selector that isn't dash/stat switches back to cards
    if (/^\.[a-z]/.test(line) && !/^\.(dash-|stat-)/.test(line.trim()) && !/^\.card/.test(line.trim()) === false) {
      // Keep tracking - we need a smarter approach
    }

    // Simple heuristic: track open/close braces to detect rule boundaries
    if (inDash) {
      dashLines.push(line);
    } else {
      cardLines.push(line);
    }

    // Reset at empty lines between rules (section boundaries)
    if (line.trim() === '' && !inDash) {
      // stay in cards mode
    }
  }

  // Fallback: if heuristic is too complex, use line-number based split instead.
  // The Cards section (L704-~950) contains .card-* rules.
  // The Dashboard section (~951-1203) contains .dash-* and .stat-* rules.
  // We'll use a simpler approach: find the first .dash- selector line index.
  return { cardLines, dashLines };
}

function splitPointPaper(sectionLines) {
  const pointPaperLines = [];
  const torContextLines = [];
  let inTorContext = false;

  for (const line of sectionLines) {
    if (/\/\*\s*──\s*ToR Context Bar\s*──\s*\*\//.test(line)) {
      inTorContext = true;
    }
    if (inTorContext) {
      torContextLines.push(line);
    } else {
      pointPaperLines.push(line);
    }
  }
  return { pointPaperLines, torContextLines };
}

function main() {
  const content = fs.readFileSync(SOURCE, 'utf8');
  const lines = content.split('\n');
  console.log(`Read ${lines.length} lines from ${SOURCE}`);

  const sections = parseSections(lines);
  console.log(`Found ${sections.length} sections\n`);

  // Collect output per target file (may accumulate multiple sections)
  const fileContents = {};
  const unmapped = [];

  for (const section of sections) {
    const target = SECTION_MAP[section.name];
    if (!target) {
      unmapped.push(`  L${section.startLine + 1}: "${section.name}" (${section.lines.length} lines)`);
      continue;
    }

    if (target === '__SPLIT_CARDS_DASHBOARD__') {
      // Find the line index of first .dash- selector relative to section start
      const sectionText = section.lines.join('\n');
      const dashIdx = section.lines.findIndex(l => /^\.(dash-|stat-grid|stat-card)/.test(l.trim()));
      if (dashIdx > 0) {
        // Look back to find the preceding blank line or comment
        let splitAt = dashIdx;
        while (splitAt > 0 && section.lines[splitAt - 1].trim() !== '') splitAt--;
        const cardContent = section.lines.slice(0, splitAt).join('\n');
        const dashContent = section.lines.slice(splitAt).join('\n');
        fileContents['components/cards.css'] = (fileContents['components/cards.css'] || '') + cardContent + '\n';
        fileContents['pages/dashboard.css'] = (fileContents['pages/dashboard.css'] || '') + dashContent + '\n';
      } else {
        // Fallback: everything to cards
        fileContents['components/cards.css'] = (fileContents['components/cards.css'] || '') + section.lines.join('\n') + '\n';
      }
      continue;
    }

    if (target === '__SPLIT_POINT_PAPER__') {
      const { pointPaperLines, torContextLines } = splitPointPaper(section.lines);
      fileContents['pages/point-paper.css'] = (fileContents['pages/point-paper.css'] || '') + pointPaperLines.join('\n') + '\n';
      if (torContextLines.length > 0) {
        fileContents['components/tor-context.css'] = (fileContents['components/tor-context.css'] || '') + torContextLines.join('\n') + '\n';
      }
      continue;
    }

    fileContents[target] = (fileContents[target] || '') + section.lines.join('\n') + '\n';
  }

  if (unmapped.length > 0) {
    console.log('⚠ UNMAPPED SECTIONS:');
    unmapped.forEach(u => console.log(u));
    console.log('');
  }

  // Write files
  let totalLines = 0;
  const written = [];
  for (const [relPath, content] of Object.entries(fileContents)) {
    const absPath = path.join(CSS_DIR, relPath);
    fs.mkdirSync(path.dirname(absPath), { recursive: true });
    // Trim trailing whitespace but ensure single newline at end
    const cleaned = content.replace(/\n{3,}/g, '\n\n').trimEnd() + '\n';
    fs.writeFileSync(absPath, cleaned);
    const lineCount = cleaned.split('\n').length;
    totalLines += lineCount;
    written.push(`  ${relPath} (${lineCount} lines)`);
  }

  console.log(`Written ${written.length} files (${totalLines} total lines):`);
  written.sort().forEach(w => console.log(w));

  // Check for sections in monolith not yet handled
  // (toast and warning-styles don't exist in monolith — they were added directly to modular files)
  console.log('\nNote: toast.css and warning-styles.css have no monolith source.');
  console.log('They must be preserved from the existing modular files if they have content.');
}

main();
```

**Step 2: Run extraction and review output**

```bash
node scripts/split-css.js
```

Expected: Summary showing ~50 files written, no unmapped sections. If any sections are unmapped, add them to the mapping table and re-run.

**Step 3: Inspect a few files to sanity-check**

```bash
head -20 static/css/base/variables.css   # Should start with :root { --bg:
head -20 static/css/pages/dashboard.css   # Should have .dash-* rules
wc -l static/css/pages/users-list.css     # Should be ~500+ lines (5 sections merged)
```

**Step 4: Commit extraction script**

```bash
git add scripts/split-css.js
git commit -m "build: add CSS monolith extraction script"
```

---

### Task 3: Preserve modular-only files

**Files:**
- Check: `static/css/components/toast.css`
- Check: `static/css/utilities/warning-styles.css`

**Step 1: Check which existing modular files have content not in the monolith**

`toast.css` and `warning-styles.css` were confirmed absent from the monolith. These must be preserved as-is (the extraction script won't overwrite them since no section maps to them). Verify they still exist after extraction.

```bash
ls -la static/css/components/toast.css static/css/utilities/warning-styles.css
```

**Step 2: Check for any other modular files that might have diverged content**

```bash
# List files in modular dirs NOT produced by the extraction
for f in static/css/components/*.css static/css/utilities/*.css; do
  base=$(basename "$f")
  rel="$(echo "$f" | sed 's|static/css/||')"
  if ! grep -q "\"$rel\"" scripts/split-css.js 2>/dev/null; then
    echo "NOT in extraction map: $f ($(wc -l < "$f") lines)"
  fi
done
```

If any files are found that are not in the extraction map but have content, decide whether to keep them (add to index.css) or discard.

---

### Task 4: Regenerate index.css

**Files:**
- Modify: `static/css/index.css`

**Step 1: Generate new index.css from the actual files on disk**

The import order must be: base → layout → components → pages → utilities. Within each category, order alphabetically (except `base/variables.css` first, `base/reset.css` second).

Write the new `index.css` by listing all `.css` files in each directory in the correct order.

**Step 2: Verify all modular files are imported**

```bash
# Count @import lines vs actual CSS files
echo "Imports: $(grep -c '@import' static/css/index.css)"
echo "Files:   $(find static/css/base static/css/components static/css/layout static/css/pages static/css/utilities -name '*.css' | wc -l)"
```

These numbers must match.

---

### Task 5: Build with PostCSS and verify round-trip

**Step 1: Run PostCSS build**

```bash
npm run css:build
```

Expected: No errors. `static/css/style.css` regenerated.

**Step 2: Diff against backup**

```bash
diff static/css/style.css.bak static/css/style.css
```

Expected: Identical (exit code 0) or only cosmetic differences (trailing whitespace, blank line count). If there are content differences:
- Missing rules → a section wasn't mapped or was lost in a split. Fix the extraction script and re-run from Task 2.
- Extra rules → a modular-only file (toast, warning-styles) was included. Verify these are intentional additions.
- Reordered rules → the import order in index.css differs from the monolith order. Adjust index.css to match.

**Step 3: Count lines to verify completeness**

```bash
echo "Original: $(wc -l < static/css/style.css.bak)"
echo "Rebuilt:  $(wc -l < static/css/style.css)"
```

Should be equal (or rebuilt slightly larger if toast/warning-styles add content).

**Step 4: Run the existing verify script**

```bash
npm run css:verify
```

Expected: All patterns found, exit 0.

---

### Task 6: Commit modular files

**Step 1: Stage all modular CSS files and updated index.css**

```bash
git add static/css/base/ static/css/components/ static/css/layout/ static/css/pages/ static/css/utilities/ static/css/index.css
```

**Step 2: Commit**

```bash
git commit -m "refactor: split CSS monolith into PostCSS modular build (TD.1)

Extracted 6,144-line style.css into ~50 modular files across
base/, components/, layout/, pages/, utilities/ directories.
PostCSS build via 'npm run css:build' produces identical output."
```

---

### Task 7: Make style.css a build artifact

**Files:**
- Modify: `.gitignore`
- Modify: `Dockerfile` (if CSS build step needed)

**Step 1: Add style.css to .gitignore**

Append to `.gitignore`:
```
# CSS build artifact — source of truth is static/css/index.css + modular files
static/css/style.css
```

**Step 2: Remove style.css from git tracking**

```bash
git rm --cached static/css/style.css
```

**Step 3: Check if Dockerfile needs css:build step**

Review `Dockerfile` or `docker-compose.*.yml` for the build process. If the app serves `static/css/style.css` at runtime, the Docker build must run `npm run css:build` before the Rust build.

**Step 4: Commit**

```bash
git add .gitignore
git commit -m "build: gitignore style.css, treat as PostCSS build artifact"
```

---

### Task 8: Cleanup

**Step 1: Remove backup file**

```bash
rm static/css/style.css.bak
```

**Step 2: Remove extraction script (one-time use)**

```bash
git rm scripts/split-css.js
git commit -m "chore: remove one-time CSS extraction script"
```

**Step 3: Verify dev workflow**

```bash
# Edit a modular file (e.g. add a comment to variables.css)
echo "/* test */" >> static/css/base/variables.css
npm run css:build
grep "test" static/css/style.css  # Should find it
git checkout static/css/base/variables.css  # Undo test edit
npm run css:build  # Rebuild clean
```

**Step 4: Update backlog**

Mark TD.1 as complete in `docs/BACKLOG.md`.
