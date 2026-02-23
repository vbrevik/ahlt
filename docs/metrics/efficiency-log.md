# Efficiency Log

Tracks resource consumption per delivered requirement, weighted by effectiveness.

**Resource Efficiency** = Baseline_cost / Cost_per_requirement (rolling median baseline)
**Path Efficiency** = Ideal_observations / Actual_observations
**Combined** = avg(Resource, Path) x Effectiveness

| Date | Task | Work Tokens | Obs | Time | Reqs Met | Cost/Req | Resource Eff | Path Eff | Combined | Top Overhead |
|------|------|-------------|-----|------|----------|----------|-------------|----------|----------|-------------|
| 2026-02-21 | Point Paper Redesign | 122,683 | 23 | 1-2h | 13 | 9,437 | building | 0.57 | 0.57 | Tangential (Playwright 43k) |
| 2026-02-23 | CA4.4 REST API Coverage Expansion | 32,226 | 9 | 30-60m | 20 | 1,611 | building | 0.67 | 0.67 | Rework (warning test status_at) |
| 2026-02-23 | CA4.5 Day View Overlapping Events | 80,000 | 14 | 15-30m | 15 | 5,333 | building | 0.57 | 0.57 | Rework (column-vs-index bug) |
| 2026-02-23 | CA4.8 E2E Suite CI Integration | 25,000 | 7 | 15-30m | 16 | 1,563 | building | 1.00 | 1.00 | None |
| 2026-02-23 | Role Builder Redesign (Accordion UX) | 180,000 | 45 | 30-60m | 14 | 12,857 | building | 0.82 | 0.82 | Rework (chevron, accordion timing) |
| 2026-02-23 | TD.1 CSS Monolith Split | 50,000 | 10 | 30-60m | 12 | 4,167 | building | 0.90 | 0.90 | Minor rework (data-manager CSS split) |
| 2026-02-23 | TD.2 Template Partial Extraction | 585,000 | 24 | 30-60m | 12 | 48,750 | 0.11 | 0.87 | 0.49 | Subagent context overhead + unnecessary Phase 1 spec review |
