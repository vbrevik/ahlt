#!/usr/bin/env python3
"""
CSS Modularization Extractor
Extracts static/css/style.css into modular components based on section markers.

Usage:
    python3 scripts/css-extract.py [--verify]

Options:
    --verify    Compare compiled output to original (requires postcss CLI)
"""

import os
import re
import sys
from pathlib import Path

# Section definitions: (start_line, end_line, output_path)
# Lines are 1-indexed in CSS, converted to 0-indexed for Python
SECTIONS = {
    # Base styles
    "base/variables.css": (5, 95),
    "base/reset.css": (96, 122),
    "base/typography.css": (123, 141),
    
    # Layout
    "layout/app.css": (142, 156),
    "layout/page-header.css": (1247, 1260),
    "layout/search-filter.css": (1260, 1305),
    "layout/empty-states.css": (1305, 1335),
    "layout/pagination.css": (1343, 1379),
    "layout/form-layout.css": (3325, 3384),
    
    # Components
    "components/sidebar.css": (156, 192),
    "components/navbar.css": (192, 417),
    "components/buttons.css": (417, 500),
    "components/forms.css": (500, 629),
    "components/tables.css": (629, 680),
    "components/alerts.css": (680, 703),
    "components/cards.css": (703, 1203),
    "components/badges.css": (1203, 1247),
    "components/tab-bar.css": (1548, 1575),
    "components/relation-cards.css": (1725, 1790),
    "components/graph-panel.css": (1843, 1872),
    "components/pipeline-tabs.css": (3020, 3057),
    "components/detail-card.css": (3084, 3115),
    "components/section.css": (3300, 3325),
    "components/tabs.css": (4511, 4549),
    "components/tor-context.css": (5575, 5627),
    
    # Pages
    "pages/error.css": (1379, 1425),
    "pages/login.css": (1425, 1490),
    "pages/ontology.css": (1575, 1725),
    "pages/schema-tables.css": (1790, 1843),
    "pages/graph.css": (1872, 2132),
    "pages/data-browser.css": (2400, 2483),
    "pages/entity-detail.css": (2483, 2619),
    "pages/role-permissions.css": (2690, 2886),
    "pages/menu-builder.css": (2886, 3020),
    "pages/inline-form.css": (3115, 3146),
    "pages/tor-grid.css": (3384, 3507),
    "pages/tor-info-grid.css": (3507, 3551),
    "pages/positions-list.css": (3551, 3629),
    "pages/functions-list.css": (3629, 3675),
    "pages/protocol-steps.css": (3675, 3770),
    "pages/dependency-flow.css": (3770, 3864),
    "pages/governance-cards.css": (3864, 3916),
    "pages/governance-graph.css": (3916, 4132),
    "pages/calendar.css": (4132, 4511),
    "pages/profile.css": (4604, 4685),
    "pages/users-list.css": (4685, 5053),
    "pages/point-paper.css": (5289, 5686),
    "pages/role-list.css": (5686, 5874),
    "pages/role-builder.css": (5874, 6142),
    
    # Utilities
    "utilities/utilities.css": (1490, 1495),
    "utilities/scrollbar.css": (1495, 1514),
    "utilities/animations.css": (1514, 1537),
    "utilities/focus.css": (1537, 1548),
    "utilities/muted-id.css": (1335, 1343),
    "utilities/responsive.css": (2619, 2690),
    "utilities/status-badges.css": (3057, 3084),
    "utilities/warning-styles.css": (3146, 3210),
    "utilities/theme-selection.css": (4549, 4599),
}

INDEX_TEMPLATE = """/* ==========================================
   Ahlt — Design System (Modular)
   Compiled via PostCSS @import
   ========================================== */

@import "base/variables.css";
@import "base/reset.css";
@import "base/typography.css";

@import "layout/app.css";
@import "layout/page-header.css";
@import "layout/search-filter.css";
@import "layout/empty-states.css";
@import "layout/pagination.css";
@import "layout/form-layout.css";

@import "components/sidebar.css";
@import "components/navbar.css";
@import "components/buttons.css";
@import "components/forms.css";
@import "components/tables.css";
@import "components/alerts.css";
@import "components/cards.css";
@import "components/badges.css";
@import "components/tab-bar.css";
@import "components/relation-cards.css";
@import "components/graph-panel.css";
@import "components/pipeline-tabs.css";
@import "components/detail-card.css";
@import "components/section.css";
@import "components/tabs.css";
@import "components/tor-context.css";

@import "pages/error.css";
@import "pages/login.css";
@import "pages/ontology.css";
@import "pages/schema-tables.css";
@import "pages/graph.css";
@import "pages/data-browser.css";
@import "pages/entity-detail.css";
@import "pages/role-permissions.css";
@import "pages/menu-builder.css";
@import "pages/inline-form.css";
@import "pages/tor-grid.css";
@import "pages/tor-info-grid.css";
@import "pages/positions-list.css";
@import "pages/functions-list.css";
@import "pages/protocol-steps.css";
@import "pages/dependency-flow.css";
@import "pages/governance-cards.css";
@import "pages/governance-graph.css";
@import "pages/calendar.css";
@import "pages/profile.css";
@import "pages/users-list.css";
@import "pages/point-paper.css";
@import "pages/role-list.css";
@import "pages/role-builder.css";

@import "utilities/utilities.css";
@import "utilities/scrollbar.css";
@import "utilities/animations.css";
@import "utilities/focus.css";
@import "utilities/muted-id.css";
@import "utilities/responsive.css";
@import "utilities/status-badges.css";
@import "utilities/warning-styles.css";
@import "utilities/theme-selection.css";
"""

def extract_sections(style_css_path="static/css/style.css"):
    """Extract CSS sections into modular files."""
    
    # Read original file
    with open(style_css_path, "r") as f:
        lines = f.readlines()
    
    total_lines = len(lines)
    print(f"Read {total_lines} lines from {style_css_path}\n")
    
    # Create directories and extract sections
    created_files = []
    for output_path, (start_line, end_line) in SECTIONS.items():
        # Convert 1-indexed to 0-indexed
        start_idx = start_line - 1
        end_idx = end_line  # exclusive, so this is end_line-1+1
        
        # Extract content
        section_lines = lines[start_idx:end_idx]
        content = "".join(section_lines)
        
        # Create parent directory
        full_path = Path("static/css") / output_path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        
        # Write file
        full_path.write_text(content)
        created_files.append(str(full_path))
        
        line_count = len(section_lines)
        print(f"✓ {output_path:40} ({line_count:4} lines)")
    
    print(f"\nCreated {len(created_files)} CSS module files\n")
    
    # Create index.css
    index_path = Path("static/css/index.css")
    index_path.write_text(INDEX_TEMPLATE)
    print(f"✓ static/css/index.css (entry point with @import statements)\n")
    
    return created_files

def verify_output(original_css="static/css/style.css", compiled_css="static/css/style.css.compiled"):
    """Verify that compiled output matches original."""
    # Note: This requires postcss CLI to be installed
    # postcss static/css/index.css -o static/css/style.css.compiled
    
    try:
        with open(original_css, "r") as f:
            original = f.read()
        with open(compiled_css, "r") as f:
            compiled = f.read()
        
        if original == compiled:
            print("✓ Compiled CSS is byte-identical to original")
            return True
        else:
            print("✗ Compiled CSS differs from original")
            print(f"  Original: {len(original)} bytes")
            print(f"  Compiled: {len(compiled)} bytes")
            return False
    except FileNotFoundError as e:
        print(f"✗ Cannot verify: {e}")
        return False

if __name__ == "__main__":
    if "--verify" in sys.argv:
        extract_sections()
        print("To verify, run:")
        print("  npm install -D postcss postcss-cli")
        print("  npx postcss static/css/index.css -o static/css/style.css.compiled")
        print("  python3 scripts/css-extract.py  # runs verification\n")
        verify_output()
    else:
        extract_sections()
        print("Next steps:")
        print("1. Install PostCSS: npm install -D postcss postcss-cli")
        print("2. Compile: npx postcss static/css/index.css -o static/css/style.new.css")
        print("3. Verify: diff static/css/style.css static/css/style.new.css")
        print("4. Replace: mv static/css/style.new.css static/css/style.css")
        print("5. Update HTML to link static/css/index.css instead")
