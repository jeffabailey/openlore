<!-- markdownlint-disable MD013 -->
# Technology Stack: viewer-counter-claim-threads (slice-11)

> DESIGN · Morgan · 2026-06-06

## Verdict: UNCHANGED

slice-11 introduces NO new dependency, NO new crate, NO new runtime, NO external
integration. It reuses the established slice-06–10 viewer stack in its entirety.

| Concern | Technology | License | Status for slice-11 |
|---|---|---|---|
| HTTP effect shell | `hyper` + `http-body-util` (existing `adapter-http-viewer`) | MIT | reused unchanged |
| Server-rendered HTML | `maud` / `maud_macros` (pure, compile-time) | MIT | reused unchanged |
| Local store | `duckdb` (embedded) | MIT | reused (read-only SELECT + existing artifact read) |
| Progressive enhancement | vendored `/static/htmx.min.js` (no CDN) | BSD-0/MIT | reused unchanged |
| Time / serde | `chrono`, `serde`, `serde_json` | MIT/Apache-2.0 | reused unchanged |
| Paradigm | functional Rust (ADR-007) | — | preserved (pure view-model + render; effect shell at the read edge) |
| Arch enforcement | `cargo xtask check-arch` (in-repo, `syn`/`cargo_metadata`) | MIT/Apache-2.0 | reused; NO rule/allowlist change (see architecture-design.md §6) |

## OSS-first / proprietary check

No proprietary technology introduced or required. Every dependency in play is
permissively licensed (MIT / Apache-2.0 / BSD) and already vetted by the existing
supply-chain policy (ADR-012).

## External integrations

None in this slice. The route is LOCAL-only (no network seam). **No contract-test
annotation is required for the platform-architect handoff** — there is no new external
boundary. (Peer counters were signature-verified at `peer pull` time, KPI-FED-6; the
viewer re-verifies nothing.)

## Architectural enforcement tooling (language-appropriate)

Rust → `cargo xtask check-arch` (already in the repo) enforces the hexagonal
dependency-inversion + viewer capability + anti-merging invariants at CI time. slice-11
requires no new enforcement edge; the existing rules cover the delta.
