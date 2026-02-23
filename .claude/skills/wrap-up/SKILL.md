---
name: wrap-up
description: End-of-task wrap-up routine. Run after completing a feature or fix to verify, review learnings, update backlog, commit+push, and suggest the next task.
user-invokable: true
---

Run this checklist sequentially after completing a task. Each step must finish before the next begins.

## Step 1: Verify & Test

- Run `cargo build` and confirm it compiles cleanly (warnings are OK, errors are not).
- If the task touched UI, take a Playwright screenshot of the affected page(s) to visually verify.
- If there are automated tests, run them. If no tests exist for the changed code yet, note that as a gap but do NOT write tests unless the user asks.
- Summarize: **Build status**, **Test status**, any issues found.

## Step 2: Learnings Review

Reflect on the task just completed. Be honest and specific, not generic. Cover:

- **What went well?** — approaches that saved time or produced clean results.
- **What should have been done differently?** — wasted effort, wrong assumptions, dead ends.
- **Patterns worth remembering** — new gotchas, Askama quirks, Actix-web behaviors, CSS tricks, D3.js lessons, or other reusable knowledge.

Present this as a short bullet list (3-6 bullets), not a wall of text.

**Action required:** Don't just list learnings — persist them. For each learning, apply the highest-matching tier:

| Tier | Destination | Bar |
|------|-------------|-----|
| **1. CLAUDE.md** | `CLAUDE.md` Gotchas or Key Patterns section | New framework quirk, workflow pattern, or architectural rule that would save ≥1 hour for *any* future developer/session in this repo. High bar — only framework-level or project-wide gotchas belong here. |
| **2. Auto-memory** | `MEMORY.md` or a topic file | Session-recurrent patterns, project-specific conventions, or architectural insights that are more specific to how this project is structured and how we work. |
| **3. Backlog** | `docs/BACKLOG.md` Architecture Decisions | Project-level design decisions worth tracking but not broad enough for CLAUDE.md. |
| **4. Skip** | — | Truly session-specific; won't help future work. |

When writing to **CLAUDE.md**: place the learning in the most relevant existing section (Gotchas & Quirks, Key Patterns, etc.). If no section fits, add a minimal new subsection. Keep it concise — one short example or one-liner is enough.

State which tier was applied for each learning.

## Step 3: Scan for Long Files & Technical Debt

### Long File Scan

Scan for files that have grown too large and should be split:

- **CSS**: Any `.css` file over 2,000 lines → backlog item to split into feature modules
- **Rust source**: Any `.rs` file over 500 lines → backlog item to split into submodules
- **Templates**: Any `.html` template over 300 lines → backlog item to extract partials
- **JavaScript**: Any inline `<script>` block or `.js` file over 200 lines → backlog item to extract

Use `wc -l` on relevant file types to check. Report files exceeding thresholds.

### Technical Debt Scan

Scan for other indicators of accumulated technical debt:

- **Dead code**: Check `cargo build` warnings for `dead_code`, `unused_imports`, `unused_variables`
- **TODO/FIXME/HACK comments**: `grep -rn 'TODO\|FIXME\|HACK' src/ templates/` — count and list locations
- **Duplicate code patterns**: Note any patterns you observed during the task where similar logic exists in multiple places
- **Missing tests**: Handler files with no corresponding test file in `tests/`
- **Stale documentation**: Plan docs in `docs/plans/` that reference completed work but haven't been updated

For each issue found, create a concise backlog entry with:
- What the issue is
- Where it is (file:line or file pattern)
- Suggested fix approach (1 sentence)

**Priority**: Long files and dead code go at the TOP of the "Remaining Backlog" as the next items to address.

## Step 4: Update Backlog

- Read `docs/BACKLOG.md`.
- Move the completed task from "Remaining Backlog" to "Completed Work" (or update its status).
- If the task revealed new work items, add them to the appropriate section.
- **Add any long-file or tech-debt items found in Step 4 as the next items in the backlog.**
- If the implementation order diagram needs updating, update it.
- Summarize what changed in the backlog.

## Step 5: Commit & Push

- Run `git status` and `git diff --stat` to see what changed.
- Stage relevant files (be specific, avoid `git add .`).
- Write a concise commit message that focuses on what was added/changed and why.
- Commit and push to the current branch.
- Show the commit hash and push result.

## Step 6: Suggest Next Task

- Re-read the "Implementation Order" and "Remaining Backlog" sections of `docs/BACKLOG.md`.
- If tech debt items were added in Step 4, consider whether they should be the immediate next task (especially long files blocking further feature work).
- Recommend the single highest-priority next task with a 1-2 sentence rationale.
- Ask the user if they want to proceed with it.

## Step 7: Clear Context Window

- Inform the user that you're clearing the context window to start fresh.
- Use the `/clear` command to clear the conversation context.
- This ensures a clean slate for the next task without accumulated context weight.

## Step 8: Run Catch-Up

- Invoke the `catch-up` skill to initialize fresh context for the next session.
- This loads current project state, recent changes, and prepares for the next task.
- The catch-up skill will read the updated backlog and recent commits to understand what was just completed.

## Output Format

Use this structure so the user can scan quickly:

```
### Verify
[build/test summary]

### Learnings
- bullet 1 → [CLAUDE.md | memory | backlog | skip]
- bullet 2 → [CLAUDE.md | memory | backlog | skip]
- ...

### Technical Health
**Long files:** [n found / none]
- [file:lines — action needed]

**Tech debt:** [n items / clean]
- [item — location — action]

### Backlog Updated
[what moved/changed, including any tech debt items added]

### Committed
[commit hash] [message]
Pushed to [branch]

### Next Up
[task name] — [rationale]
Proceed?

### Clearing Context
Clearing context window for a fresh start...
/clear

### Starting Fresh
[Running catch-up skill to initialize next session]
```

ARGUMENTS: Optionally pass a short description of what was just completed, e.g. `/wrap-up added graph toolbar to concepts tab`. If no argument is given, infer from recent conversation context.
