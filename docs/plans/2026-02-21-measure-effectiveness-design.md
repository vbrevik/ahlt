# Measure Effectiveness Skill — Design

**Date**: 2026-02-21
**Status**: Approved

## Problem

We have no systematic way to measure how effectively the human+Claude team translates requirements into working code. Without measurement, we can't identify patterns in what causes gaps or track improvement over time.

## Solution

A skill (`measure-effectiveness`) that runs at the end of each task, traces each requirement from the task's **prompt contract** against the delivered code, and produces a scored record.

## Why Prompt Contracts

Prompt contracts have 4 structured components that map directly to verifiable requirements:

| Contract Component | Effectiveness Check |
|---|---|
| **GOAL** | Was the stated objective achieved? |
| **CONSTRAINTS** | Were all boundaries respected? |
| **FORMAT** | Does output match expected shape? |
| **FAILURE CONDITIONS** | Did any failure condition occur? |

Each bullet point within these sections becomes a discrete, checkable requirement.

## Process

### Step 1 — Locate Contract
Find the prompt contract for the just-completed task. Sources (in priority order):
1. Prompt contract in conversation context (most common)
2. Contract embedded in plan doc (`docs/plans/`)
3. If neither exists: ask the human to state what was built

### Step 2 — Extract Requirements
Parse each contract component into numbered requirements. Present to the human for confirmation: "These are the requirements I'm scoring against — correct?"

### Step 3 — Trace Each Requirement
For each requirement, verify against delivered code using:
- `git diff` of task commits
- `cargo check` / `cargo test` results
- Template/handler inspection where relevant

Classify each as:
- **Met** — implemented and verified
- **Partial** — started but incomplete or has known gaps
- **Missed** — not addressed

### Step 4 — Quick Retrospective
Two questions:
1. "Anything delivered that wasn't in the contract?" (scope creep detection)
2. "What caused any partial/missed items?" (root cause: unclear spec, wrong assumption, technical blocker, time constraint)

### Step 5 — Score & Persist

```
Effectiveness = (Met + 0.5 × Partial) / Total
```

Persist to two locations:
- **claude-mem**: structured record (searchable across sessions)
- **docs/metrics/effectiveness-log.md**: append row to human-readable table

## Output Format

### claude-mem record
```
Task: [task name]
Date: YYYY-MM-DD
Contract: GOAL(n) CONSTRAINTS(n) FORMAT(n) FAILURE_CONDITIONS(n)
Results: Met=X Partial=Y Missed=Z
Score: 0.XX
Gaps: [root cause notes]
```

### Markdown log row
```markdown
| Date | Task | Total | Met | Partial | Missed | Score | Root Cause |
```

## Scope Boundaries

- **Does**: Measure requirement completion against prompt contracts
- **Does NOT**: Run tests (that's CI/wrap-up), measure efficiency (separate skill), assign blame
- **Trigger**: Manual via `/measure-effectiveness` or invoked by wrap-up workflow
