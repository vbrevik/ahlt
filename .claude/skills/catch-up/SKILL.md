---
name: catch-up
description: Start-of-session context loading. Run at the beginning of a new conversation to understand project state, recent changes, and what to work on next.
user_invocable: true
---

Run this checklist sequentially at the start of a new session. The goal is to rebuild working context so you can continue development as if the previous session never ended.

## Step 1: Read Project Documentation

Read these files in parallel — they are the source of truth:

- `docs/BACKLOG.md` — vision, completed work, remaining backlog, implementation order, architecture decisions
- Auto-memory `MEMORY.md` — stack, lessons learned, gotchas, patterns
- Auto-memory `rust-web-patterns.md` — project structure, auth/template/CRUD/DB patterns

Absorb silently. Do NOT summarize each file individually.

## Step 2: Check Recent Git History

Run `git log --oneline -15` to see recent commits. This tells you:
- What was done in the last session(s)
- Whether there's uncommitted or in-progress work

Also run `git status` (never use `-uall`) to check for any uncommitted changes or work-in-progress.

## Step 3: Quick Project Health Check

Run `cargo build` to verify the project compiles. Note any errors — these may indicate interrupted work from the last session.

Do NOT fix issues at this step — just observe.

## Step 4: Synthesize & Present

Present a concise briefing to the user:

```
### Project Status

**App:** [app name] — [one-line description from backlog vision]
**Stack:** [from memory]
**Branch:** [current branch] | **Build:** [pass/fail]
**Last commit:** [hash] [message] ([time ago])

### Recently Completed
- [2-4 bullets from backlog "Completed Work" bottom + recent commits]

### Current State
- [Any uncommitted changes or WIP noted]
- [Any build issues found]

### Next Up (from backlog)
1. **[task id] — [task name]** — [priority] | [effort]
   [one-line description]
2. **[task id] — [task name]** — [priority] | [effort]
   [one-line description]

### Ready
What would you like to work on?
```

## Rules

- Keep the briefing **short** — this is a launchpad, not a report. The user already knows the project.
- Do NOT start working on anything. Wait for the user to pick a task or give instructions.
- If uncommitted changes exist, mention them prominently — they may be interrupted work.
- If the build fails, flag it as the likely first thing to address.
- If backlog has a clear "NEXT" item, highlight it but don't assume the user wants it.

ARGUMENTS: None. This skill takes no arguments.
