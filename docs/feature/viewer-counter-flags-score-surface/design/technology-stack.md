# Technology Stack — viewer-counter-flags-score-surface (slice-14)

> Wave: DESIGN · Date: 2026-06-08

## Verdict: UNCHANGED

slice-14 introduces NO new technology, dependency, crate, table, read method, render fn, SQL,
or external integration. It reuses the slice-06–13 stack verbatim — and notably REUSES the
slice-12 `counter_presence_for` read AND the slice-13 `render_countered_link` SSOT with NO
modification:

| Layer | Technology | License | Role in slice-14 | Change |
|---|---|---|---|---|
| Language | Rust (2021) | MIT/Apache-2.0 | the whole slice | none |
| Storage | DuckDB via `duckdb-rs` | MIT | the REUSED slice-12 indexed ref-table presence read | reuse (NO new query) |
| HTTP | `hyper` | MIT | the `GET /score` effect shell | reuse |
| HTML | `maud` | MIT/Apache-2.0 | the pure `render_score_breakdown` flag arm + the `SCORE_COUNTER_LEGEND` render site | reuse |
| Scoring core | `scoring` crate (internal) | workspace | `WeightedView`/`WeightedPairing`/`Contribution` — PROJECTED, UNCHANGED, never recomputed | reuse (NO new math, NO new field) |
| Trait/ports | `ports` crate (internal) | workspace | `counter_presence_for` — UNCHANGED, REUSED | reuse (NO new method) |
| Flag render | `viewer-domain::render_countered_link` (slice-13 SSOT) | workspace | the contribution-row marker | reuse (NO new render fn) |
| Arch enforcement | `xtask check-arch` (`syn`-based) | workspace | viewer capability + anti-merging rules — unchanged, no new rule | reuse |

## Notes

- **`std::collections::HashSet`** for the presence set — std, no dependency. Threaded as a
  `&presence` parameter through the score render chain (`render_score_results_fragment` →
  `render_score_result` → `render_score_pairing` → `render_score_breakdown`).
- **No new SQL, no new read method** — the slice-12 `counter_presence_for` (ADR-048) is called
  as-is from the `/score` handler (one more call site, flattened across all pairings).
- **No new render fn, no new flag string** — the slice-13 `render_countered_link` +
  `COUNTERED_PRESENCE_FLAG = "Countered"` are reused verbatim. The ONLY new string is the
  `SCORE_COUNTER_LEGEND` anti-misread constant (a render-only pure constant; ADR-051).
- **No external integration** → no contract-test annotation for the DEVOPS handoff.
- **Functional paradigm (ADR-007)** preserved: pure `viewer-domain` core (the render is a total
  function of `(ScoreState, presence)`), pure `scoring` core (PROJECTED, never recomputed),
  effect shell at the `adapter-http-viewer` edge (the one handler wiring + the
  `score_counter_presence` flatten helper). The REUSED read is the only effect; the render +
  scoring projection are pure.

## OSS preference

All dependencies remain permissively licensed (MIT / Apache-2.0). No proprietary technology
introduced or required.

## Workspace member count

**21 members — UNCHANGED.** No new crate.
</content>
