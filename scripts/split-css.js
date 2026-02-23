#!/usr/bin/env node
// split-css.js -- Extract sections from the CSS monolith into modular files.
//
// Reads static/css/style.css.bak (the backed-up monolith) and splits it into
// individual files under static/css/{base,components,layout,pages,utilities}/.
//
// Section boundaries are detected by comment headers matching patterns like:
//   /* === Name === */          (single-line)
//   /* ========...              (multi-line: opening, name line, closing)
//
// Two special cases require intra-section splitting:
//   1. "Cards & Dashboard" -- split at the "Dashboard specifics" sub-comment
//   2. "Point Paper" -- split at the "ToR Context Bar" sub-comment
//
// Usage: node scripts/split-css.js

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SOURCE = path.join(ROOT, 'static', 'css', 'style.css.bak');
const CSS_DIR = path.join(ROOT, 'static', 'css');

// ---------------------------------------------------------------------------
// Section-to-file mapping
// Keys are lowercase section names (trimmed of surrounding whitespace/equals).
// Values are relative paths under static/css/.
// ---------------------------------------------------------------------------
const SECTION_MAP = {
  'tokens':                                     'base/variables.css',
  'dark mode':                                  'base/variables.css',
  'reset & base':                               'base/reset.css',
  'typography':                                 'base/typography.css',

  'app layout':                                 'layout/app.css',
  'page header':                                'layout/page-header.css',
  'search & filter bars':                       'layout/search-filter.css',
  'empty states':                               'layout/empty-states.css',
  'pagination':                                 'layout/pagination.css',
  'form layout':                                'layout/form-layout.css',

  'sidebar':                                    'components/sidebar.css',
  'navbar':                                     'components/navbar.css',
  'buttons':                                    'components/buttons.css',
  'forms':                                      'components/forms.css',
  'tables':                                     'components/tables.css',
  'alerts':                                     'components/alerts.css',
  'cards & dashboard':                          'SPECIAL:cards-dashboard',
  'badges':                                     'components/badges.css',
  'tab bar':                                    'components/tab-bar.css',
  'relation cards':                             'components/relation-cards.css',
  'graph panel':                                'components/graph-panel.css',
  'graph view':                                 'pages/graph.css',
  'pipeline & tabs':                            'components/pipeline-tabs.css',
  'detail card':                                'components/detail-card.css',
  'section component':                          'components/section.css',
  'tabs':                                       'components/tabs.css',

  'error pages':                                'pages/error.css',
  'login':                                      'pages/login.css',
  'ontology concepts':                          'pages/ontology.css',
  'schema tables':                              'pages/schema-tables.css',
  'data browser':                               'pages/data-browser.css',
  'entity detail':                              'pages/entity-detail.css',
  'responsive':                                 'utilities/responsive.css',
  'role permissions ui':                        'pages/role-permissions.css',
  'inline form':                                'pages/inline-form.css',
  'tor card grid':                              'pages/tor-grid.css',
  'tor info grid':                              'pages/tor-info-grid.css',
  'positions list':                             'pages/positions-list.css',
  'functions list':                             'pages/functions-list.css',
  'protocol steps':                             'pages/protocol-steps.css',
  'dependency flow':                            'pages/dependency-flow.css',
  'governance map cards':                       'pages/governance-cards.css',
  'governance graph':                           'pages/governance-graph.css',
  'data manager':                               'pages/calendar.css',
  'meeting outlook calendar':                   'pages/calendar.css',
  'profile section':                            'pages/profile.css',
  'users list redesign':                        'pages/users-list.css',
  'filter builder':                             'pages/users-list.css',
  'table controls bar':                         'pages/users-list.css',
  'column picker popover':                      'pages/users-list.css',
  'users page':                                 'pages/users-list.css',
  'point paper':                                'SPECIAL:point-paper',
  'role builder':                               'pages/role-builder.css',

  'utilities':                                  'utilities/utilities.css',
  'scrollbar':                                  'utilities/scrollbar.css',
  'animations':                                 'utilities/animations.css',
  'focus visible':                              'utilities/focus.css',
  'muted id columns':                           'utilities/muted-id.css',
  'status badges':                              'utilities/status-badges.css',
  'missing utility classes':                    'utilities/utilities.css',
  'theme selection':                            'utilities/theme-selection.css',
  'utility':                                    'utilities/utilities.css',
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Detect whether a line is a section header.
// Returns the extracted section name (lowercase) or null.
//
// Matches patterns like:
//   /* === Name === */                 (single-line)
//   /* === Name (parenthetical) === */ (single-line with parens)
//   /* ========...                     (multi-line opening, name on next line)
function parseSingleLineHeader(line) {
  // Single-line: /* === Some Name === */  or  /* === Some Name (stuff) === */
  const single = line.match(/^\/\*\s*={2,}\s+(.+?)\s+={2,}\s*\*\/$/);
  if (single) {
    return single[1].trim().toLowerCase();
  }
  return null;
}

// Check if a line is a multi-line header opening: /* ====...  (no closing */)
function isMultiLineOpen(line) {
  return /^\/\*\s*={4,}\s*$/.test(line);
}

// Check if a line is a multi-line header closing:    ====... */
function isMultiLineClose(line) {
  return /^\s+={4,}\s*\*\/\s*$/.test(line);
}

// Normalize a section name for lookup in SECTION_MAP.
// Strips parenthetical suffixes and common decorators so e.g.
//   "Graph Panel (shared between governance map and workflow builder)"
// matches the key "graph panel".
function normalizeForLookup(name) {
  // Remove parenthetical like "(stuff)" or trailing " — description"
  let cleaned = name
    .replace(/\s*\(.*?\)\s*/g, '')   // (parenthetical)
    .replace(/\s*—\s*.*/g, '')       // em-dash trailer
    .trim();
  return cleaned;
}

// Look up a section name in SECTION_MAP. Tries exact match first, then
// normalized (without parenthetical/trailer), then prefix match.
function lookupTarget(sectionName) {
  const lower = sectionName.toLowerCase();
  // Exact match
  if (SECTION_MAP[lower]) return SECTION_MAP[lower];
  // Normalized match (strip parenthetical, em-dash)
  const norm = normalizeForLookup(lower);
  if (SECTION_MAP[norm]) return SECTION_MAP[norm];
  // Prefix match — find longest key that the section name starts with
  let best = null;
  let bestLen = 0;
  for (const key of Object.keys(SECTION_MAP)) {
    if (lower.startsWith(key) && key.length > bestLen) {
      best = key;
      bestLen = key.length;
    }
  }
  if (best) return SECTION_MAP[best];
  return null;
}

// Clean up content: collapse 3+ consecutive blank lines into 2,
// ensure single trailing newline.
function cleanContent(text) {
  // Collapse 3+ blank lines to 2
  let result = text.replace(/(\n\s*){3,}\n/g, '\n\n\n');
  // Trim trailing whitespace and ensure single trailing newline
  result = result.trimEnd() + '\n';
  return result;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  if (!fs.existsSync(SOURCE)) {
    console.error(`ERROR: Source file not found: ${SOURCE}`);
    console.error('Run: cp static/css/style.css static/css/style.css.bak');
    process.exit(1);
  }

  const raw = fs.readFileSync(SOURCE, 'utf-8');
  const lines = raw.split('\n');
  console.log(`Read ${lines.length} lines from ${path.relative(ROOT, SOURCE)}`);

  // --- Phase 1: Parse into sections ---
  // Each section: { name, startLine, lines[] }
  const sections = [];
  let currentSection = null;
  let inMultiLineHeader = false;
  let multiLineName = null;

  // The very first section header is the design system header — discard it.
  let discardFirst = true;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    // Handle multi-line header state machine
    if (inMultiLineHeader) {
      if (isMultiLineClose(line)) {
        // End of multi-line header
        inMultiLineHeader = false;
        if (multiLineName) {
          if (discardFirst) {
            discardFirst = false;
            multiLineName = null;
            continue;
          }
          // Start new section
          if (currentSection) {
            sections.push(currentSection);
          }
          currentSection = {
            name: multiLineName.trim(),
            startLine: i + 1,
            lines: [],
          };
          multiLineName = null;
        }
        continue;
      }
      // This line is the name inside the multi-line header
      if (trimmed && !trimmed.match(/^={2,}/)) {
        multiLineName = trimmed;
      }
      continue;
    }

    // Check for multi-line header opening
    if (isMultiLineOpen(line)) {
      inMultiLineHeader = true;
      multiLineName = null;
      continue;
    }

    // Check for single-line header
    const sectionName = parseSingleLineHeader(trimmed);
    if (sectionName) {
      if (discardFirst) {
        discardFirst = false;
        continue;
      }
      // Start new section
      if (currentSection) {
        sections.push(currentSection);
      }
      currentSection = {
        name: sectionName,
        startLine: i + 1,
        lines: [],
      };
      continue;
    }

    // Regular content line — add to current section
    if (currentSection) {
      currentSection.lines.push(line);
    }
    // Lines before any section header are silently discarded
  }

  // Push final section
  if (currentSection) {
    sections.push(currentSection);
  }

  console.log(`Found ${sections.length} sections`);

  // --- Phase 2: Map sections to target files ---
  // fileContents: { relativePath: contentString[] }
  const fileContents = {};
  const unmapped = [];

  function appendToFile(relPath, content) {
    if (!fileContents[relPath]) {
      fileContents[relPath] = [];
    }
    fileContents[relPath].push(content);
  }

  for (const section of sections) {
    const target = lookupTarget(section.name);
    const content = section.lines.join('\n');

    if (!target) {
      unmapped.push(section.name);
      continue;
    }

    // --- Special case 1: Cards & Dashboard ---
    if (target === 'SPECIAL:cards-dashboard') {
      // Split at the "Dashboard specifics" sub-comment.
      // Rules with .dash- or .stat-grid or .stat-card go to pages/dashboard.css.
      // Everything before goes to components/cards.css.
      const sectionText = content;
      const dashMarkerIdx = sectionText.indexOf('/* --- Dashboard specifics --- */');

      if (dashMarkerIdx !== -1) {
        // Find the preceding blank line for a clean split
        let splitIdx = dashMarkerIdx;
        // Walk back to find preceding blank line
        while (splitIdx > 0 && sectionText[splitIdx - 1] !== '\n') {
          splitIdx--;
        }
        // splitIdx now points to start of the marker line. Go back one more
        // to include the blank line in the cards portion.
        const cardsPart = sectionText.substring(0, splitIdx);
        const dashPart = sectionText.substring(splitIdx);
        appendToFile('components/cards.css', cardsPart);
        appendToFile('pages/dashboard.css', dashPart);
      } else {
        // Fallback: split by scanning for first .dash- selector
        const sectionLines = section.lines;
        let splitLine = -1;
        for (let j = 0; j < sectionLines.length; j++) {
          if (/\.(dash-|stat-grid|stat-card)/.test(sectionLines[j])) {
            // Walk back to find the preceding blank line
            splitLine = j;
            while (splitLine > 0 && sectionLines[splitLine - 1].trim() !== '') {
              splitLine--;
            }
            break;
          }
        }
        if (splitLine > 0) {
          appendToFile('components/cards.css', sectionLines.slice(0, splitLine).join('\n'));
          appendToFile('pages/dashboard.css', sectionLines.slice(splitLine).join('\n'));
        } else {
          // Can't split — put everything in cards
          appendToFile('components/cards.css', content);
          console.warn('WARNING: Could not find dashboard split point in Cards & Dashboard section');
        }
      }
      continue;
    }

    // --- Special case 2: Point Paper ---
    if (target === 'SPECIAL:point-paper') {
      const torMarker = '/* ── ToR Context Bar ── */';
      const torIdx = content.indexOf(torMarker);

      if (torIdx !== -1) {
        // Find the preceding blank line for a clean split
        let splitIdx = torIdx;
        while (splitIdx > 0 && content[splitIdx - 1] !== '\n') {
          splitIdx--;
        }
        const pointPaperPart = content.substring(0, splitIdx);
        const torContextPart = content.substring(splitIdx);
        appendToFile('pages/point-paper.css', pointPaperPart);
        appendToFile('components/tor-context.css', torContextPart);
      } else {
        // Fallback: everything to point-paper
        appendToFile('pages/point-paper.css', content);
        console.warn('WARNING: Could not find ToR Context Bar marker in Point Paper section');
      }
      continue;
    }

    // Normal section — append to target file
    appendToFile(target, content);
  }

  // --- Phase 3: Write files ---
  let filesWritten = 0;
  let totalLines = 0;

  const sortedPaths = Object.keys(fileContents).sort();

  for (const relPath of sortedPaths) {
    const absPath = path.join(CSS_DIR, relPath);
    const dir = path.dirname(absPath);

    // Create directories if needed
    fs.mkdirSync(dir, { recursive: true });

    // Join all content chunks for this file (preserving source order)
    const combined = fileContents[relPath].join('\n');
    const cleaned = cleanContent(combined);

    fs.writeFileSync(absPath, cleaned, 'utf-8');

    const lineCount = cleaned.split('\n').length;
    totalLines += lineCount;
    filesWritten++;

    console.log(`  ${relPath.padEnd(40)} ${String(lineCount).padStart(5)} lines`);
  }

  // --- Phase 4: Summary ---
  console.log('');
  console.log(`Written: ${filesWritten} files, ${totalLines} total lines`);

  if (unmapped.length > 0) {
    console.log('');
    console.warn(`WARNING: ${unmapped.length} unmapped section(s):`);
    for (const name of unmapped) {
      console.warn(`  - "${name}"`);
    }
  } else {
    console.log('All sections mapped successfully.');
  }
}

main();
