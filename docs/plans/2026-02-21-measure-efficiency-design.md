# Measure Efficiency Skill — Design

**Date**: 2026-02-21
**Status**: Approved

## Problem

We measure whether the team builds the right thing (effectiveness), but not how many resources it costs to get there. Without efficiency measurement, we can't identify wasteful patterns — over-research, rework loops, unnecessary exploration — or track improvement over time.

## Solution

A skill (`measure-efficiency`) that runs at the end of each task (after `measure-effectiveness`), computes two efficiency scores from resource usage data, and weights them by the effectiveness score so that wrong work is penalized.

## Two Scores

### Score 1 — Resource Efficiency (cost per delivered requirement)

```
Cost_per_requirement = Total_work_tokens / Requirements_met
Efficiency_raw = Baseline_cost / Cost_per_requirement
Resource_efficiency = clamp(Efficiency_raw, 0, 1.5)
```

- Baseline = rolling median of last 10 measurements
- First 5 measurements: no baseline, store raw cost_per_requirement only
- Clamped at 1.5 to avoid runaway scores on trivially small tasks

### Score 2 — Path Efficiency (ideal vs actual path)

After reviewing the session's work, Claude estimates the minimum path:

```
Path_efficiency = Ideal_observations / Actual_observations
```

Presented qualitatively with the top 2-3 sources of overhead.

### Combined Score

```
Efficiency = ((Resource_efficiency + Path_efficiency) / 2) × Effectiveness_score
```

Weighted by effectiveness so fast-but-wrong work scores poorly.

## Inputs (Hybrid Collection)

| Metric | Source | Method |
|--------|--------|--------|
| Work tokens | Context index | Sum "Work" column from session observations |
| Read tokens | Context index | Sum "Read" column |
| Observation count | Context index | Count rows |
| Wall-clock time | Auto + ask | Session timestamps, confirm with human |
| Requirements met | Effectiveness score | From just-run `/measure-effectiveness` |
| Effectiveness score | Effectiveness score | From just-run `/measure-effectiveness` |

## Prompt Contract Integration

Requirements come from the same prompt contract used by the effectiveness skill. The efficiency skill reuses the requirement count and met/partial/missed classification — it does not re-extract them.

## Storage

Same dual-persist as effectiveness:
- **claude-mem**: structured record in `im-ctrl-metrics` project
- **docs/metrics/efficiency-log.md**: append row to human-readable table

## Scope Boundaries

- **Does**: Measure resource consumption relative to delivered value
- **Does NOT**: Re-measure effectiveness, run tests, or optimize anything
- **Prerequisite**: `/measure-effectiveness` must have been run first in the same session
- **Trigger**: Manual via `/measure-efficiency` or invoked by wrap-up workflow
