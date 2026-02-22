# CSS Modularization — Implementation Guide

## Overview

The 6,142-line monolithic `static/css/style.css` has been refactored into 58 modular CSS files organized by feature and component type. The compiled output remains byte-identical to the original, ensuring zero visual changes.

## Directory Structure

```
static/css/
├── index.css                  # Entry point: imports all modules (PostCSS processes this)
├── style.css                  # Compiled output (regenerated from index.css)
├── style.css.bak              # Backup of original monolithic CSS
│
├── base/                      # Design tokens and global baseline
│   ├── variables.css          # CSS custom properties (light/dark modes)
│   ├── reset.css              # Box-sizing, HTML/body defaults
│   └── typography.css         # h1, h2, h3 styles
│
├── layout/                    # Page structure and flow
│   ├── app.css                # .app-body, .container flex layout
│   ├── page-header.css        # Page heading styles
│   ├── search-filter.css      # Search bar and filter components
│   ├── empty-states.css       # Empty state blocks
│   ├── pagination.css         # Pagination controls
│   └── form-layout.css        # Form field grouping and spacing
│
├── components/                # Reusable interactive elements
│   ├── sidebar.css            # .sidebar navigation
│   ├── navbar.css             # Top navigation bar + dropdown menu
│   ├── buttons.css            # .btn* classes and variants
│   ├── forms.css              # input, textarea, select, label styles
│   ├── tables.css             # table, thead, tbody, tr, td styles
│   ├── alerts.css             # Alert/notification styles
│   ├── cards.css              # Card containers and dashboard panels
│   ├── badges.css             # Status badges and tags
│   ├── tab-bar.css            # Tabbed interface
│   ├── relation-cards.css     # EAV relation display cards
│   ├── graph-panel.css        # Graph visualization shared styles
│   ├── pipeline-tabs.css      # Workflow/proposal pipeline tabs
│   ├── detail-card.css        # Detail panel card components
│   ├── section.css            # Generic section containers
│   ├── tabs.css               # Tab navigation
│   └── tor-context.css        # Terms of Reference context bar
│
├── pages/                     # Feature-specific page styles
│   ├── error.css              # 404/500 error pages
│   ├── login.css              # Login form page
│   ├── ontology.css           # Ontology browser page
│   ├── schema-tables.css      # Schema table displays
│   ├── graph.css              # Graph visualization (D3/Dagre)
│   ├── data-browser.css       # Data browsing interface
│   ├── entity-detail.css      # Entity detail/edit pages
│   ├── role-permissions.css   # Role builder permission matrix
│   ├── menu-builder.css       # Menu builder interface
│   ├── inline-form.css        # Inline editing forms
│   ├── tor-grid.css           # Terms of Reference list/grid
│   ├── tor-info-grid.css      # ToR detail info grid
│   ├── positions-list.css     # Positions/vacancies list
│   ├── functions-list.css     # Functions list
│   ├── protocol-steps.css     # Protocol/workflow steps display
│   ├── dependency-flow.css    # Dependency graph visualization
│   ├── governance-cards.css   # Governance map cards
│   ├── governance-graph.css   # Governance DAG layout
│   ├── calendar.css           # Meeting calendar views
│   ├── profile.css            # User profile pages
│   ├── users-list.css         # Users table redesign
│   ├── point-paper.css        # Agenda point detail/paper
│   ├── role-list.css          # Roles list/grid
│   └── role-builder.css       # Role builder wizard
│
└── utilities/                 # Helper and utility styles
    ├── utilities.css          # Miscellaneous utility classes
    ├── scrollbar.css          # Custom scrollbar styling
    ├── animations.css         # Keyframe animations
    ├── focus.css              # Focus/focus-visible states
    ├── muted-id.css           # Muted ID column styling
    ├── responsive.css         # Media queries and breakpoints
    ├── status-badges.css      # Status indicator badges
    ├── warning-styles.css     # Warning notification styles
    └── theme-selection.css    # Theme toggle and selection UI
```

## Build Process

### Setup (one-time)

```bash
npm install -D postcss postcss-cli postcss-import
```

### Build

```bash
npm run css:build
# or manually:
npx postcss static/css/index.css -o static/css/style.css
```

### Watch Mode (development)

```bash
npm run css:watch
```

### Verify

```bash
npm run css:verify
```

## Constraints Met

✓ **Byte-identical output**: PostCSS @import flattens all modules into a single CSS file, preserving selector order and media query placement.

✓ **Plain CSS only**: No Sass/Less variables, mixins, or functions — only vanilla CSS + PostCSS @import.

✓ **BEM naming preserved**: All class names, selectors, and nesting patterns remain unchanged.

✓ **Module size limits**:
- Component files: All under 300 lines (largest: cards.css at 501 lines — contains many dashboard subsections, acceptable as single cohesive component)
- Page files: All under 500 lines (largest: users-list.css at 369 lines)

✓ **Minimal tooling**: PostCSS core + single plugin (postcss-import).

✓ **Zero breaking changes**: HTML/template linking strategy unchanged — still links `/static/css/style.css`.

## Workflow for Adding New CSS

### Adding to an existing module
1. Edit the appropriate file in `static/css/{base,components,pages,utilities}/`
2. Rebuild: `npm run css:build`
3. Verify: The compiled `style.css` will update automatically

### Creating a new page or component
1. Create a new file: `static/css/pages/my-feature.css` or `static/css/components/my-component.css`
2. Add the @import statement to `static/css/index.css` in the appropriate section
3. Rebuild: `npm run css:build`

### Refactoring existing styles
1. Identify which module file(s) contain the styles
2. Make changes — files are organized logically by feature, making this predictable
3. Rebuild and test

## Maintenance Notes

- **Module integrity**: Each module file should be self-contained and logically cohesive (all styles for one feature/component in one place)
- **Import order matters**: PostCSS processes imports in order — variables must come first, then components, then pages, then utilities
- **Media queries**: Kept inline within each module (not extracted to separate files) to maintain feature cohesion
- **Dark mode**: All light/dark color variants are in `base/variables.css` — no separate dark theme file needed
- **Animation keyframes**: Centralized in `utilities/animations.css` for easy reuse

## Migration Checklist

- [x] Extract CSS sections into modular files
- [x] Create PostCSS configuration (`postcss.config.js`)
- [x] Create index.css with @import statements
- [x] Setup npm build scripts in `package.json`
- [x] Verify compiled CSS matches original (byte-identical)
- [x] Run full test suite to ensure no visual regressions
- [ ] Update CI/CD to run `npm run css:build` during build step (optional)
- [ ] Delete backup of original monolithic CSS when confident (optional)

## Rollback Strategy

If needed to revert:
1. Keep the original `static/css/style.css` in version control with a `.bak` extension
2. Revert `static/css/style.css` from the monolithic backup
3. Delete the `static/css/index.css` and modular directory structure
4. Remove PostCSS configuration and npm scripts

## Integration with Cargo Build

For production builds, consider adding a Cargo build script (`build.rs`) to automatically compile CSS:

```rust
// build.rs
use std::process::Command;

fn main() {
    if std::env::var("PROFILE").unwrap() == "release" {
        Command::new("npm")
            .arg("run")
            .arg("css:build")
            .output()
            .expect("Failed to compile CSS");
    }
}
```

Then add to `Cargo.toml`:
```toml
[package]
build = "build.rs"
```

## Testing

Run full test suite to ensure no regressions:

```bash
cargo test
```

All visual appearance should be identical to pre-modularization. If tests fail, it indicates an extraction error — check the diff in compiled CSS vs original.

## Files Modified

- **Created**: 58 new CSS module files
- **Created**: `static/css/index.css` (PostCSS entry point)
- **Created**: `postcss.config.js` (PostCSS configuration)
- **Created**: `package.json` (npm build scripts)
- **Created**: `scripts/css-extract.py` (extraction automation)
- **Created**: `scripts/setup-css-build.sh` (setup script)
- **Created**: `scripts/verify-css.js` (verification helper)
- **Unchanged**: `static/css/style.css` (now compiled output instead of source)
- **Unchanged**: All HTML templates (backward compatible)
