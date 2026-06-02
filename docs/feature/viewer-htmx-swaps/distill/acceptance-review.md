# DISTILL Acceptance Self-Review: viewer-htmx-swaps (slice-07)

Self-review against `nw-ad-critique-dimensions` (Dims 1-9) + the DISTILL DoD. Reviewer
output is ephemeral; this is the committed self-check the DELIVER crafter reads.

## Phase 0 / 1.5 log (audit trail)

- `[lang-mode] rust` — `Cargo.toml` marker; canonical Rust adapter row applies.
- `[policy-mode] inherit` — `docs/architecture/atdd-infrastructure-policy.md` exists;
  applied as-is. NO new port in scope (slice-07 is a response-SHAPE delta over the slice-06
  routes) → no new policy row needed. Driving = `openlore` CLI `ui` subprocess; Driven
  internal = real DuckDB; Driven external = `FakeGithub` (only `/scrape`).
- `[port-mode] inherit` — `tests/common/state_delta.rs` present (slice-01 bootstrap); the
  read-only universe reuses `assert_store_read_only` / `capture_store_row_count_universe`.
- **Wave-Decision Reconciliation HARD GATE**: no `wave-decisions.md` exists in discuss/
  design/devops (WARN per graceful-degradation — none authored for this slice). DISCUSS
  user-stories, DESIGN architecture-design, component-boundaries, and ADR-031..035 are
  mutually consistent (HX-Request selector, `/static/htmx.min.js`, swap ids `#claims-table`
  / `#scrape-results` / `#claim-detail` / `#view-panel`, no new data route). **Reconciliation
  passed — 0 contradictions.**

## Story × (htmx / no-JS / parity) coverage matrix

Every value-producing interaction story (US-HX-001/002/003/004/006) carries all THREE
scenario kinds; US-HX-005 is `@infrastructure` (asset/offline guarantee, no htmx/no-JS/parity
trio).

| Story | htmx fragment | no-JS full page | parity | extra (boundary/error) |
|---|---|---|---|---|
| US-HX-001 (WS) | H-1a `@walking_skeleton` | H-1b | H-1c | H-1d over-the-end clamp (both shapes) |
| US-HX-002 | H-2a | H-2b | H-2c | H-2d unknown origin in fragment |
| US-HX-003 | H-3a | H-3d | H-3e | H-3b zero candidates · H-3c network-down no-leak |
| US-HX-004 | H-4a | H-4b | H-4e | H-4c unknown CID (both) · H-4d no evidence (both) |
| US-HX-006 | H-6a | H-6b | H-6d | H-6c bookmark/reload re-enters full page |
| US-HX-005 | — (asset/offline) | — | — | H-5a asset 200 · H-5b no-CDN · H-5c no-write |

Every story has ≥1 scenario → Dim 4 (Coverage) + Dim 8 Check A (story-to-scenario
traceability) PASS, zero untraceable stories.

## Invariant coverage matrix

| Invariant | Scenario(s) |
|---|---|
| I-HX-1 progressive enhancement (header → shape; full page when absent) | H-1a/b, H-2a/b, H-3a/d, H-4a/b, H-6a/b + H-INV-NoReg |
| I-HX-2 htmx served locally / offline / no-CDN | H-5a, H-5b, H-5c |
| I-HX-3 read-only preserved (no write/sign surface) | H-INV-ReadOnly, H-INV-NoWrite, H-5c |
| I-HX-4 no regression (non-htmx byte-equivalent) | H-1b, H-2b, H-3d, H-4b, H-6b, H-INV-NoReg + slice-06 26-suite (companion gate) |
| I-HX-5 fragment/full-page parity | H-1c, H-2c, H-3e, H-4e, H-6d |
| BR-HX-4 / I-SCR-1 no sign control, nothing persisted | H-3a, H-INV-ReadOnly, H-INV-NoWrite |
| BR-HX-5 derived-from only on /scrape candidates | H-3a, H-3e (present); slice-06 V-INV-2 (absent on persisted views) |
| FR-VIEW-8 confidence verbatim | H-1a, H-1c, H-4a, H-4e |
| KPI-VIEW-3 peer origin separable | H-2a, H-2c, H-2d, H-6a, H-6d |

## Failure-mode coverage (journey YAML `failure_modes` → scenario)

| journey failure_mode | Scenario guarding it |
|---|---|
| No-JS Next gets a fragment instead of full page (I-HX-1) | H-1b, H-2b, H-3d, H-4b, H-6b |
| Fragment table diverges from full page (I-HX-5) | H-1c, H-2c, H-3e, H-4e, H-6d |
| Over-the-end ?page returns blank (regress DV-5) | H-1d |
| Sign control leaks into the scrape fragment (I-VIEW-3) | H-3a, H-INV-NoWrite |
| A candidate persisted on submit (BR-VIEW-2) | H-INV-ReadOnly |
| Network-down fragment drops the offline guidance | H-3c |
| Unknown CID broken/empty fragment vs guided not-found | H-4c |
| Confidence reformatted in fragment but verbatim in page (FR-VIEW-8/I-HX-5) | H-1c, H-4a, H-4e |
| Tab swap does not update URL / breaks bookmark | H-6b, H-6c (server-side URL contract; hx-push-url client-side per ADR-035) |
| Peer rows lose origin in the fragment (KPI-VIEW-3) | H-2a, H-2d, H-6a |
| htmx loaded from a CDN — swaps break offline (I-HX-2) | H-5b |
| Two copies of the asset drift (single-source) | H-5a (single local route) + H-5b (one local src) |

Every enumerated `failure_modes` entry is covered → Dim 1 (error-path) PASS.

## Error/edge ratio (Dim 1)

Of 28 scenarios: error/edge/boundary/guardrail = H-1d, H-2d, H-3b, H-3c, H-4c, H-4d, H-6c +
all 6 guardrail/gold (H-5a/b/c, H-INV-NoReg/ReadOnly/NoWrite) = **13 / 28 ≈ 46%** — above the
40% target. Happy/primary = 15 (the htmx-fragment + no-JS + parity trios).

## Dimension-by-dimension

| Dim | Verdict | Note |
|---|---|---|
| 1 Happy-path bias | PASS | 46% error/edge/guardrail; every failure_mode mapped. |
| 2 GWT compliance | PASS | Each scenario: one Given context, one When action (with/without header), Then observable outcome. The `*_in_both_shapes` scenarios assert the SAME outcome across two shapes — one behavior, not two When actions. |
| 3 Business language | PASS | Scenario fn names + doc-comment GWT are domain language ("paging the claims list", "opening a claim", "switching to peer claims"). `HX-Request` / `#claims-table` / route paths appear only in technical notes + step bodies, never as the behavioral claim. |
| 4 Coverage | PASS | Every US-HX-001..006 covered; AC trios complete. |
| 5 WS user-centricity | PASS | H-1 titled as a user goal; Then = observable rendered text; demo-able (see walking-skeleton.md litmus). |
| 6 Priority | PASS | WS = US-HX-001 (P1, the thinnest PE thread); the largest paging pain (US-HX-002 peer, 1840 rows) is P2 next. Matches story-map. |
| 7 Observable assertions | PASS | Every Then asserts on `ViewerResponse` status / `body_contains` (rendered HTML the operator sees) / `is_fragment`/`is_full_page` (observable shape) / `assert_store_read_only` (port-exposed row counts). No internal struct fields, no method-call counts. |
| 8 Traceability | PASS | Check A: every story tagged. Check B (environments): the viewer has one environment posture (initialized localhost store); no DEVOPS env matrix → default applies; the read-only/offline/no-CDN guardrails cover the offline-machine posture (H-5b, H-INV-ReadOnly). |
| 9 WS boundary proof | PASS | WS strategy = real CLI subprocess + real DuckDB (Architecture of Reference: driving=real, driven-internal=real); H-1a is `@real-io`, NOT `@in-memory`. The only fake is `FakeGithub` (driven-external, not touched by the H-1 paging skeleton). The asset/read-only adapters are exercised by real I/O (H-5a fetches the real route; H-INV-ReadOnly snapshots the real store). |

## Mandate compliance evidence

- **CM-A (Mandate 1, hexagonal)**: every scenario imports only `support::*` and drives via
  `ViewerServer` (the CLI driving port + HTTP). Zero `viewer-domain` / `adapter-http-viewer`
  internal imports. `grep "use " tests/acceptance/viewer_htmx*.rs` → only `use support::*`.
- **CM-B (Mandate 2, business language)**: scenario fn names + GWT comments are domain terms;
  technical detail (header, ids, routes) lives in the step bodies / technical notes.
- **CM-C (Mandate 3, journeys)**: 1 walking skeleton (H-1a) + 27 focused scenarios; each is a
  complete user journey (Given context → When action → Then observable value).
- **CM-E (Mandate 8, universe)**: the read-only guardrails (H-INV-ReadOnly, H-5c) use the
  universe-bound `assert_store_read_only` (universe = `claims.row_count` /
  `peer_claims.row_count`, port-exposed names, all `unchanged`). Layers 4+ shape/parity
  scenarios use traditional `body_contains` assertions (permitted at this layer per Mandate 8).
- **CM-F (Mandate 9, PBT mode)**: NO PBT machinery imported at this layer-3/5; all scenarios
  example-only. The `@property` tag marks invariants for the reader, not generative tests.
- **CM-G (Mandate 10, Tier B)**: NOT applicable. The journeys are 1-2 chained scenarios per
  interaction over a config-shaped input space (page numbers, a CID, a scrape target) — Tier A
  example coverage suffices; Tier B (state-machine PBT over a domain-rich input space) is
  correctly SKIPPED.
- **CM-H (Mandate 11, sad paths example-based)**: H-1d, H-2d, H-3b, H-3c, H-4c, H-4d are named
  example-based sad/edge scenarios; no PBT machinery.

## RED classification confirmation (pre-DELIVER gate)

- Both `[[test]]` targets COMPILE: `cargo test -p cli --test viewer_htmx --test
  viewer_htmx_invariants --no-run` → Finished.
- The walking-skeleton scenario was run: classified **RED / MISSING_FUNCTIONALITY** — panics
  at `not yet implemented` (the `todo!()` macro) inside the test body, reaching the
  assertions; NOT ImportError / FixtureBroken / SetupFailure. The `get_htmx` /
  `post_form_htmx` seam compiles (only adds a header to the existing reqwest call).
- The slice-06 suites (`viewer_store` / `viewer_scrape` / `viewer_invariants`) still COMPILE
  after the harness additions (no-regression on the harness API).

## One-at-a-time DELIVER step mapping

| Step | Scenario | What it forces into existence |
|---|---|---|
| 1 | H-1a (WS) | `render_claims_table_fragment` + `Shape::from_request` + claims handler fork + chrome `<script src>` line |
| 2 | H-1b | `render_claims_page` re-composed as chrome + the fragment fn (no-JS arm) |
| 3 | H-1c | parity wiring (page EMBEDS the fragment fn — structural) |
| 4 | H-1d | clamp preserved across both shapes |
| 5-8 | H-2a..d | peer handler `?page=N` threading + peer-table fragment (reuse pattern) |
| 9-13 | H-3a..e | `render_scrape_results_fragment` + scrape handler fork (reuse FakeGithub) |
| 14-18 | H-4a..e | `render_claim_detail_fragment` + `render_claim_not_found_fragment` + detail handler fork |
| 19-22 | H-6a..d | `#view-panel` fragment + tab anchors (`hx-push-url` + href) |
| 23 | H-5a | `GET /static/htmx.min.js` route (`include_str!` vendored htmx 2.0.4) |
| 24 | H-5b | no-CDN: chrome `<script src>` points to the local route only |
| 25 | H-5c | asset route GET-only, no write surface |
| 26 | H-INV-NoReg | non-htmx byte-equivalence + slice-06 26-suite green (release gate) |
| 27 | H-INV-ReadOnly | read-only across every htmx fragment route |
| 28 | H-INV-NoWrite | no sign control on any fragment |

## Not groundable in the existing harness (handed to DELIVER)

- **Browser-side in-place feel (no flash / no scroll reset)** — the HTTP harness sends raw
  HTTP and runs NO JS engine (ADR-035 §consequences). It pins the SERVER contract that
  enables the feel (header→shape, ids present, parity, no-CDN); the no-flash/no-scroll UX is a
  manual/visual check, out of the AT harness scope. Documented, not blocked.
- **`hx-push-url` URL/history update** — client-side (ADR-034). H-6a/b/c assert the SERVER
  contract (each URL serves the right fragment/page); the actual browser URL push + Back is a
  manual check.
- **Exact byte-equivalence vs a recorded slice-06 baseline** — H-INV-NoReg pins full-page
  SHAPE + no-CDN + content; the byte-for-byte diff vs a golden baseline (the page body delta
  is bounded to the `<div id>` wrapper + the `<script src>` line per ADR-032) is DELIVER's
  tightening, with the slice-06 26-scenario suite as the load-bearing companion gate.
- **htmx asset content-type assertion** — H-5a pins 200 + non-empty + looks-like-htmx; the
  `application/javascript` content-type assertion is a DELIVER detail (the `ViewerResponse`
  currently exposes status + body; a header accessor can be added if DELIVER wants to pin it).
