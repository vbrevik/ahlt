# Efficiency Log

Tracks resource consumption per delivered requirement, weighted by effectiveness.

**Resource Efficiency** = Baseline_cost / Cost_per_requirement (rolling median baseline)
**Path Efficiency** = Ideal_observations / Actual_observations
**Combined** = avg(Resource, Path) x Effectiveness

| Date | Task | Work Tokens | Obs | Time | Reqs Met | Cost/Req | Resource Eff | Path Eff | Combined | Top Overhead |
|------|------|-------------|-----|------|----------|----------|-------------|----------|----------|-------------|
| 2026-02-21 | Point Paper Redesign | 122,683 | 23 | 1-2h | 13 | 9,437 | building | 0.57 | 0.57 | Tangential (Playwright 43k) |
| 2026-02-23 | CA4.4 REST API Coverage Expansion | 32,226 | 9 | 30-60m | 20 | 1,611 | building | 0.67 | 0.67 | Rework (warning test status_at) |
