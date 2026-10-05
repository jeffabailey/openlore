# Acceptance Review — indexer-per-did-pds-fetch (DISTILL)

- **Reviewer**: Sentinel (`nw-acceptance-designer-reviewer`) · 2026-10-05 · iteration 1 of max 2
- **Scope**: `tests/acceptance/indexer_per_did_{fetch,resilience,config}.rs`,
  `search_self_attested_label.rs`, `indexer_pass_core.rs`, `support/indexer_network.rs`,
  `crates/test-support/src/fake_atproto.rs` (indexer postures), `xtask/tests/indexer_per_did_architecture.rs`,
  `distill/*.md`
- **Verdict**: **APPROVED** · blockers 0 · high 0 · low 0

```yaml
approval_status: approved
blocker_count: 0
high_count: 0
low_count: 0
findings_list: []
```

## Dimension scores

| Dimension | Score | Note |
|---|---|---|
| D1 happy-path bias | 9 | error/edge/regression = 76% of subprocess scenarios (target ≥ 40%) |
| D2 GWT format | 10 | one When per scenario |
| D3 business language | 9 | domain vocabulary in titles and Gherkin; the technical detail lives in step bodies |
| D4 coverage | 10 | 27/27 ACs, including the 4 design-added ACs |
| D5 walking skeleton | 9 | user-goal titles (Maria / Jeff), observable Thens |
| D6 priority | 8 | the elevator pitch and ADRs justify the work; no measured baseline |
| D7 observable behaviour | 10 | port-exposed universe only (events, exit code, index rows, search output, request log) |
| D8 traceability | 10 | AC table and environment variants mapped |

Mandates: CM-A hexagonal boundary PASS · CM-B business language PASS · CM-C user journey PASS ·
Mandate 9 (PBT only at layer 2) PASS · Mandate 11 (named layer-3 sad paths) PASS ·
`@contract-shape` on every scenario PASS · no Fixture Theater (Givens set inputs only).

Post-review change (not re-reviewed, local only): IPF-20's `pds_url` check now accepts the
canonical IPv6 form for bracketed literals, so a correct implementation that normalizes the URL
does not fail spuriously.
