# Definition of Ready: indexer-per-did-pds-fetch

This is the 9-item hard gate, plus two project checks: **job_id traceability** (Decision 1,
2026-04-28) and **Elevator Pitch present** (every story except @infrastructure).

## Summary

| Story | Scen. | Est. | 1 Prob | 2 Persona | 3 Ex | 4 UAT | 5 AC | 6 Size | 7 Tech | 8 Deps | 9 KPI | job_id | Pitch | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| US-IPF-001 | 5 | 2–3 d | PASS | PASS | PASS (4) | PASS | PASS | PASS | PASS | PASS | PASS | J-005 | PASS | READY |
| US-IPF-002 | 6 | 2 d | PASS | PASS | PASS (4) | PASS | PASS | PASS* | PASS | PASS (001) | PASS | J-005 | PASS | READY* |
| US-IPF-003 | 3 | 1 d | PASS | PASS | PASS (3) | PASS | PASS | PASS | PASS | PASS (001, 002) | PASS | J-005 | PASS | READY |
| US-IPF-004 | 4 | 1 d | PASS | PASS | PASS (4) | PASS | PASS | PASS | PASS | PASS (001) | PASS | infra + rationale | n/a (@infra) | READY |
| US-IPF-005 | 3 | 1 d | PASS | PASS | PASS (3) | PASS | PASS | PASS | PASS | PASS (001) | PASS | J-005 | PASS | READY |

\* US-IPF-002 has 6 scenarios, which is the top of the range. Split option, if DESIGN
estimates more than 3 days: 002a isolation and reasons, 002b time and concurrency bounds.

Slice check: the single release contains 4 user-visible stories and 1 @infrastructure story.
PASS.

## Evidence per item

| Item | Evidence |
|---|---|
| 1 Problem | Each story opens with domain pain. Example: "Priya's records look relay-origin and are refused, so Maria's search is missing exactly the new authors." |
| 2 Persona | Maria (searcher), Priya Raman (bsky.social, `did:plc:priyaraman7x2k`), Dmitri Volkov (`pds.volkov.dev`), Jeff Bailey (operator, 14 DIDs). See `requirements.md`. |
| 3 Examples | 18 examples in total, using real-shaped DIDs and hosts (morel.us-east.host.bsky.network, pds.priyaraman.dev, did:plc:ghost0000, did:plc:mallory4k1z). |
| 4 UAT | 21 Given/When/Then scenarios with business-outcome titles. A lightweight `.feature` file and the journey YAML carry the key ones. |
| 5 AC | AC-001.1 through AC-005.3, each derived from a scenario. See the trace table in `acceptance-criteria.md`. |
| 6 Size | 1–3 d each, 3–6 scenarios. The total is about 7–9 d. |
| 7 Technical notes | Brownfield pointers (`run.rs` `ingest`, `origin_for`, `SearchResultDto`). No migration. |
| 8 Dependencies | Inside the feature: 001 → 002 → 003. External: none. Open questions are tracked (OQ-IPF-1..6) and are DESIGN decisions, not blockers. |
| 9 KPIs | KPI-IPF-1..7 with targets, baselines and measurement. See `outcome-kpis.md`. |

## Open questions for DESIGN (tracked, non-blocking)

- **OQ-IPF-1** Startup probe semantics now that there is no single "only source". What does the
  probe exercise? Does an unreachable fallback degrade or refuse?
- **OQ-IPF-2** `did:web` resolution. `IdentityLookup` is built from the PLC endpoint only. Are
  `did:web` repo DIDs in scope, or reported `did_unresolvable` for now?
- **OQ-IPF-3** Make "fallback is relay-origin" a type-level guarantee rather than a URL
  comparison. Consider the edge case where the fallback URL happens to equal a DID's PDS: that
  DID resolved, so the fallback is not used for it.
- **OQ-IPF-4** Exact event names and shape for skip events and the pass summary (ADR-024
  `source_skipped`), plus whether `stats` exposes the counts.
- **OQ-IPF-5** Defaults for `${max_concurrent_fetches}` and `${per_did_time_budget}`. Is an
  index-store upsert failure still pass-fatal?
- **OQ-IPF-6** ADR housekeeping: amend ADR-071 §4 (the indexer origin now comes from per-DID
  resolution) and ADR-024 (source discovery, fallback relay semantics).
