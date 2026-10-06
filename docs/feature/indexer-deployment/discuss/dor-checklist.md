# Definition of Ready: indexer-deployment

| # | DoR item | 001 | 002 | 003 | 004 | 005 | 006 | Evidence |
|---|---|---|---|---|---|---|---|---|
| 1 | Problem statement clear, in domain language | PASS | PASS | PASS | PASS | PASS | PASS | Each story's Problem section (Maria / Jeff pain) |
| 2 | User/persona with specific characteristics | PASS | PASS | PASS | PASS | PASS | PASS | Maria, Jeff, Priya, Dmitri, Tomás with DIDs (`requirements.md`) |
| 3 | 3+ domain examples with real data | PASS (4) | PASS (4) | PASS (4) | PASS (4) | PASS (3) | PASS (4) | Real DIDs, hosts, times, counts |
| 4 | UAT in Given/When/Then (3-7) | 5 | 5 | 4 | 4 | 3 | 6 | `user-stories.md` |
| 5 | AC derived from UAT | PASS | PASS | PASS | PASS | PASS | PASS | `acceptance-criteria.md` traces |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | 2-3 d | 2-3 d | 1-2 d | 1 d | 1 d | 2 d | `prioritization.md` |
| 7 | Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS | PASS | System Constraints C-1..C-6 and per-story notes |
| 8 | Dependencies resolved or tracked | PASS | PASS | PASS | PASS | PASS | PASS | DEP-IXD-1..5. Sequencing in `wave-decisions.md`. OQ-IXD-1..9 assigned to DESIGN/DEVOPS. |
| 9 | Outcome KPIs with measurable targets | PASS | PASS | PASS | PASS | PASS | PASS | KPI-IXD-1..6 (`outcome-kpis.md`) |
| + | `job_id` present (Decision 1) | J-005 | J-005 | J-005 | infra + rationale | infra + rationale | infra + rationale | Each release slice has at least one J-005 story |
| + | Elevator Pitch (non-infra required) | PASS | PASS | PASS | present | present | present | Real entry points: `openlore search`, `aws ssm put-parameter`, `deploy.sh` |

## Verdict

DoR PASSED for all 6 stories. The open questions are design choices with stated requirements
and do not block DESIGN. OQ-IXD-4 (dead-timer or serve-down alert) needs a user decision before
DEVOPS finalizes the alarms.
