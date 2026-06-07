# Technology Stack — viewer-counter-flags-graph-surfaces (slice-13)

> Wave: DESIGN · Date: 2026-06-07

## Verdict: UNCHANGED

slice-13 introduces NO new technology, dependency, crate, table, read method, SQL, or
external integration. It reuses the slice-06–12 stack verbatim — and notably REUSES the
slice-12 `counter_presence_for` read with NO modification:

| Layer | Technology | License | Role in slice-13 | Change |
|---|---|---|---|---|
| Language | Rust (2021) | MIT/Apache-2.0 | the whole slice | none |
| Storage | DuckDB via `duckdb-rs` | MIT | the REUSED slice-12 indexed ref-table presence read | reuse (NO new query) |
| HTTP | `hyper` | MIT | the `GET /peer-claims` / `/project` / `/philosophy` effect shells | reuse |
| HTML | `maud` | MIT/Apache-2.0 | the pure `render_peer_claim_row` + shared `render_edge_row` flags | reuse |
| Trait/ports | `ports` crate (internal) | workspace | `counter_presence_for` — UNCHANGED, REUSED | reuse (NO new method) |
| Arch enforcement | `xtask check-arch` (`syn`-based) | workspace | viewer capability + anti-merging rules — unchanged, no new rule | reuse |

## Notes

- **`std::collections::HashSet`** for the presence set — std, no dependency. Added as a
  parameter to the two new projection constructors + the widened grouper.
- **No new SQL, no new read method** — the slice-12 `counter_presence_for` (ADR-048,
  `ports/store_read.rs:380`) is called as-is from three more handlers.
- **No external integration** → no contract-test annotation for the DEVOPS handoff.
- **Functional paradigm (ADR-007)** preserved: pure `viewer-domain` core (the two
  projections + renders are total functions of `(rows, presence)`; the widened `group_by`
  stays pure), effect shell at the `adapter-http-viewer` edge (the three handler wirings).
  The REUSED read is the only effect; the projections + renders are pure.

## OSS preference

All dependencies remain permissively licensed (MIT / Apache-2.0). No proprietary
technology introduced or required.

## Workspace member count

**21 members — UNCHANGED.** No new crate.
