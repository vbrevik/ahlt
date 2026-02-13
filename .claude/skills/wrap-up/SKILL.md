---
name: wrap-up
description: End-of-task wrap-up routine. Run after completing a feature or fix to verify, review learnings, update backlog, commit+push, and suggest the next task.
user_invocable: true
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
- **Should auto-memory be updated?** — If a lesson is likely to recur, update `MEMORY.md` or a topic file in the memory directory. If unsure, skip it.

Present this as a short bullet list (3-6 bullets), not a wall of text.

## Step 3: Update Backlog

- Read `docs/BACKLOG.md`.
- Move the completed task from "Remaining Backlog" to "Completed Work" (or update its status).
- If the task revealed new work items, add them to the appropriate section.
- If the implementation order diagram needs updating, update it.
- Summarize what changed in the backlog.

## Step 4: Commit & Push

- Run `git status` and `git diff --stat` to see what changed.
- Stage relevant files (be specific, avoid `git add .`).
- Write a concise commit message that focuses on what was added/changed and why.
- Commit and push to the current branch.
- Show the commit hash and push result.

## Step 5: Suggest Next Task

- Re-read the "Implementation Order" and "Remaining Backlog" sections of `docs/BACKLOG.md`.
- Recommend the single highest-priority next task with a 1-2 sentence rationale.
- Ask the user if they want to proceed with it.

## Output Format

Use this structure so the user can scan quickly:

```
### Verify
[build/test summary]

### Learnings
- bullet 1
- bullet 2
- ...

### Backlog Updated
[what moved/changed]

### Committed
[commit hash] [message]
Pushed to [branch]

### Next Up
[task name] — [rationale]
Proceed?
```

ARGUMENTS: Optionally pass a short description of what was just completed, e.g. `/wrap-up added graph toolbar to concepts tab`. If no argument is given, infer from recent conversation context.
