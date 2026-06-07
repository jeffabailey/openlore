# Technology Stack — viewer-counter-claim-list-flags (slice-12)

> Wave: DESIGN · Date: 2026-06-07

## Verdict: UNCHANGED

slice-12 introduces NO new technology, dependency, crate, table, or external integration.
It reuses the slice-06–11 stack verbatim:

| Layer | Technology | License | Role in slice-12 | Change |
|---|---|---|---|---|
| Language | Rust (2021) | MIT/Apache-2.0 | the whole slice | none |
| Storage | DuckDB via `duckdb-rs` | MIT | the indexed ref-table presence read; `params_from_iter` for the `IN (...)` bind | reuse (new query, no new dep) |
| HTTP | `hyper` | MIT | the `GET /claims` effect shell | reuse |
| HTML | `maud` | MIT/Apache-2.0 | the pure `render_claim_row` flag | reuse |
| Trait/ports | `ports` crate (internal) | workspace | the new `counter_presence_for` method | reuse (new method on existing trait) |
| Arch enforcement | `xtask check-arch` (`syn`-based) | workspace | viewer capability + anti-merging rules — unchanged, no new rule needed | reuse |

## Notes

- **`params_from_iter`** is part of `duckdb-rs` (already a workspace dependency); it is the
  variable-length parameter-binding API for the `IN (...)` list. No new crate.
- **`std::collections::HashSet`** for the presence set — std, no dependency.
- **No external integration** → no contract-test annotation for the DEVOPS handoff.
- **Functional paradigm (ADR-007)** preserved: pure `viewer-domain` core (the render is a
  total function of `(page, presence)`), effect shell at the `adapter-http-viewer` /
  `adapter-duckdb` edges. The new read is an effect; the projection + render are pure.

## OSS preference

All dependencies remain permissively licensed (MIT / Apache-2.0). No proprietary
technology introduced or required.
