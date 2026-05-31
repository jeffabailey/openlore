# Wave Decisions — DELIVER — htmx-scraper-viewer (slice-06)

- **Wave**: DELIVER
- **Date**: 2026-05-30..31
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 20 steps, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD; review + L1-L4 refactor + per-feature mutation enabled; models inherit.

## Execution summary

All 20 roadmap steps executed via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Walking skeleton `47984fa` → final step
`1c4e4ff`; L1-L4 refactor `a5ddb22`. All 26 slice-06 acceptance scenarios GREEN
(viewer_store 15/15, viewer_scrape 5/5, viewer_invariants 6/6) plus 40
`viewer-domain` unit/property tests; the `ViewerServer` harness spawns the REAL
`openlore ui` over HTTP. slice-01/02/03/04/05 suites show zero regression. TWO NEW
crates shipped: 1 pure (`viewer-domain`) + 1 effect (`adapter-http-viewer`); crate
count 19 → 21.

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|---|---|
| DV-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02/03/04/05 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-2 | Mutation = per-feature 100% on the new PURE `viewer-domain` production functions, matching slice-02/03/04/05 DV-2. The killing properties are kept IN-CRATE (the 40 `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally without a cross-package cargo-mutants scope detour. |
| DV-3 | **`hyper` 1.x, NOT `axum`/`actix`, for the viewer listener** (`adapter-http-viewer`). | `axum`/`actix` are `deny.toml`-banned (transitive supply-chain narrowing); a hand-rolled `hyper` handler serves the six read-only routes with no new banned dependency — mirroring the slice-05 DV-3 `hyper`-over-`axum` resolution for the XRPC server. |
| DV-4 | The `/scrape` `NetworkDown` render resolved as a **unit ADT variant that discards the raw error** — structurally cannot leak transport internals. | A unit variant carries no payload, so no `reqwest`/transport detail can reach the rendered page. Read-only + no-leak by type, not by string-scrubbing. |
| DV-5 | Found + fixed a REAL pagination **clamp gap**: `?page` beyond the last page previously overshot to an empty page; now clamps to the LAST page. | A direct over-the-end `?page` URL would have shown a blank list instead of the last claims — a legibility bug for the north-star (KPI-VIEW-1) view. Fixed in the pure pagination arithmetic + pinned by an in-crate property + 100% mutation on the boundary. |
| DV-6 | Closed an **xtask capability-rule coverage gap**: the `adapter-atproto-pds` exclusion was not independently unit-pinned. | The viewer capability rule's exclusion set was correct but under-tested; an independent unit pin prevents a future edit from silently widening the web-process capability surface. |
| DV-7 | Added a roadmap-format `criteria` field per step to satisfy the DES integrity validator. | `des-verify-integrity` requires a per-step `criteria` field; adding it brought the slice-06 roadmap into spec without altering executed step semantics. |

## Demo Evidence — 2026-05-30..31

The slice-06 viewer demos require a LIVE viewer; the `ViewerServer` acceptance
harness stands one up (a real `openlore ui` serving the operator's local DuckDB
store over HTTP, probed via the live routes). The **walking-skeleton acceptance
tests are the executable end-to-end demos**, all GREEN:

| Demo | What it proves end-to-end (green) |
|---|---|
| Store-view walking skeleton (V-1/V-2) | `openlore ui` serves the read-only landing + the paginated My Claims (size 50) over HTTP — wire→probe→use through the real binary against the real shared DuckDB connection. The store-view end-to-end demo (north-star KPI-VIEW-1: persisted claims in a browser, zero SQL). |
| Live scrape walking skeleton (V-S1) | The `GET/POST /scrape` view runs a live ephemeral GitHub propose reusing the slice-02 `GithubPort` + `derive_candidates`, rendering candidates with NO sign control. The propose-side end-to-end demo (KPI-VIEW-4). |

The remaining user-visible capabilities are demonstrated by their GREEN acceptance
scenarios driving the real `openlore ui` binary:

| Capability | Demo coverage (green acceptance scenario, real binary over HTTP) |
|---|---|
| Read-only landing | V-* (`GET /`: no write/sign affordance) |
| My Claims (paginated) | V-* (`GET /claims`: paginated size 50; pagination clamp DV-5) |
| Claim detail + evidence | V-* (`GET /claims/{cid}`: detail + evidence + verbatim confidence, FR-VIEW-8) |
| Peer Claims (federated origin) | V-* (`GET /peer-claims`: origin = author_did + fetched_from_pds, separable from own; KPI-VIEW-3) |
| Live Scrape (ephemeral propose) | V-S1/V-S3/V-S4 (`GET/POST /scrape`: candidate render, NetworkDown render DV-4, no sign control) |
| Read-only / no-key / honesty / loopback (gold) | V-INV-1 (no write/sign route), V-INV-2 (no key in process), V-INV-3 (derived-from honesty), V-INV-4 (loopback-only bind) |

Cardinal trust invariants end-to-end verified: no route writes or signs (V-INV-1),
the web process holds no signing key (V-INV-2), only `CandidateRowView` carries
`derived_from` — persisted view-models have no such slot (V-INV-3, WD-62), the
viewer binds 127.0.0.1 only (V-INV-4), the store views read the SAME shared
`Arc<Mutex<Connection>>` (zero new persisted types / table / CID path), and the
`/scrape` `NetworkDown` render cannot leak transport internals (DV-4).

## Post-Merge Integration Gate — PASS

- Full slice-06 acceptance suite GREEN (viewer_store 15/15, viewer_scrape 5/5,
  viewer_invariants 6/6) + 40 `viewer-domain` unit/property tests;
  slice-01/02/03/04/05 suites zero regression (the full workspace acceptance suite
  green across all slices). xtask guards green (the viewer capability rule + the
  `maud` pure-core allowlist + the pure-core arm active; the capability-rule
  exclusion set independently unit-pinned, DV-6).
- Environment matrix: slice-06 acceptance is hermetic (the `ViewerServer` harness
  stands up a live `openlore ui` over a loopback port against a seeded local store
  + a `tempfile` HOME) and does NOT depend on a per-environment cross-product; the
  default matrix is satisfied by the hermetic design (same rationale as
  slice-02/03/04/05; DEVOPS graceful-degrade default).
- Known harness flake (NOT a slice-06 regression): the `adapter-system-clock`
  `now_utc_*` env-var contention under full-workspace PARALLEL lib-test runs
  (carried from slice-01/03/04/05; serialized fix landed in commit `2629e56`); the
  acceptance targets pass single-threaded / in isolation.

## Quality gates

- `cargo xtask check-arch`: OK (21 workspace members) — `viewer-domain` pure-core
  allowlist (with the `maud` whitelist) + the viewer capability rule (the web
  process may not link signing / mutation; exclusion set independently
  unit-pinned, DV-6) + the pure-core arm.
- `cargo xtask check-probes`: OK — the `adapter-http-viewer` probe is real and
  non-stub; `viewer-domain` correctly requires no `probe()` (pure crate).
- Per-phase L1-L4 refactor / adversarial review / mutation / integrity outcomes
  recorded below.

## L1-L4 refactoring

@nw-functional-software-crafter cleared the deferred DELIVER debt (commit
`a5ddb22`): clippy + check-arch + check-probes clean; pure-core purity intact (no
I/O imports in `viewer-domain`; `maud` + `ports` only; ADTs make illegal states
unrepresentable — `StoreReadError` choice type, no `Option` smuggling of a write
capability; the `/scrape` `NetworkDown` modeled as a payload-free unit variant,
DV-4). The pagination-clamp fix (DV-5) and the capability-rule exclusion pin
(DV-6) landed in this window.

## Adversarial review — APPROVED (zero blockers)

@nw-software-crafter-reviewer verdict APPROVED. Zero blockers; Testing Theater
clean across all 20 steps. The cardinal read-only gate verified load-bearing
across all three layers (StoreReadPort no-mutation type + the xtask viewer
capability rule + the V-INV-1/2 gold behavioral tests through the real binary);
the `/scrape` `NetworkDown` no-leak confirmed structural (a unit variant, DV-4);
the pagination clamp fix (DV-5) confirmed a real bug-fix, not theatre;
derived-from honesty (V-INV-3) and loopback-only bind (V-INV-4) PASS.

## Mutation testing (per-feature 100% on `viewer-domain` production functions): PASS

Scope: the new pure `viewer-domain` production functions (the view-model render
functions + the pure pagination arithmetic). The slice-04/05 cross-package lesson
was applied from the start (the 40 `viewer-domain` properties pin the production
functions IN/against the crate, so the per-feature measurement reaches the real
killing suite without a cross-package detour).

| Mutant category | Viable | Caught | Missed | Unviable | Kill rate |
|---|---:|---:|---:|---:|---|
| `viewer-domain` production logic (render + pagination arithmetic, incl. the DV-5 pagination-clamp boundary) | 62 | 62 | 0 | 5 | **100%** (62/62 viable) |

Gate SATISFIED (≥80%; actual 100% on the production scope, 0 missed).
`adapter-http-viewer` is NOT mutated by design (effect shell; covered by the
V-INV gold tests through the real binary). DEVOPS sweep is the ongoing backstop.

## Deliver integrity verification: PASS

`des-verify-integrity docs/feature/htmx-scraper-viewer/deliver/` → "All 20 steps
have complete DES traces" (exit 0).
</content>
