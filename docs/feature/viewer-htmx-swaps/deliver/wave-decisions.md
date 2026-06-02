# Wave Decisions — DELIVER — viewer-htmx-swaps (slice-07)

- **Wave**: DELIVER
- **Date**: 2026-06-02
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 15 steps, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD; review + L1-L4 refactor + per-feature mutation enabled; models inherit.

## Execution summary

All 15 roadmap steps executed via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Walking skeleton `d53cfe1` (01-01) → final
step `14b8f15` (06-04); the `/scrape` htmx-script defect fix `bcf9007`; L1-L4
refactor `7f78fc1`. All 30 slice-07 acceptance scenarios GREEN (viewer_htmx 24/24,
viewer_htmx_invariants 6/6) plus 50 `viewer-domain` unit/property tests; the
`ViewerServer` harness drives the REAL `openlore ui` over HTTP with the new
`get_htmx` / `post_form_htmx` + `is_fragment` / `is_full_page` /
`references_external_cdn` seams (ADR-035). Slice-06 26-scenario corpus GREEN — zero
regression. **NO new crate**: extends `viewer-domain` (PURE) + `adapter-http-viewer`
(EFFECT) in place + a vendored `assets/htmx.min.js` text asset; workspace stays at
21 members.

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|---|---|
| DV-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..06 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions, matching slice-02..06 DV-2. The killing properties are kept IN-CRATE (the 50 `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally without a cross-package cargo-mutants scope detour. |
| DV-3 | **htmx VENDORED + `include_str!`-embedded + served from `GET /static/htmx.min.js`, NOT a CDN `<script src>`** (ADR-031). Pinned `sha256 e209dda5c8235479f3166defc7750e1dbcd5a5c1808b7792fc2e6733768fb447` with a SHA-256 integrity test. | A CDN reference would break the slice-06 offline guarantee (KPI-HX-G2 / I-VIEW-6). Vendoring keeps htmx local; the integrity test makes a silent asset swap a test failure. htmx is a TEXT asset, not a crate — no new crate, no new prod dependency (`sha2` dev-only). |
| DV-4 | **Page composes the same pure fragment function** (ADR-032 / I-HX-5): `render_*_page()` = chrome + `render_*_fragment()`. | Structural parity by construction — the fragment served under htmx and the full page served without it are ONE source, so they cannot drift. Pinned by the in-crate `*_page_embeds_the_fragment` properties (01-02). |
| DV-5 | **`HX-Request` read ONCE in the effect shell** (`Shape::from_request`, ADR-033); the pure core stays header-unaware. | One dispatch point per route keeps the pure renderers header-free (testable in isolation) and keeps the fragment-vs-page fork auditable in the effect shell. |
| DV-6 | **Found + fixed a REAL defect: the `/scrape` page was missing the local htmx `<script src>`** so its form swap would not work in a browser. Caught closing Phase 06; fixed in `bcf9007`; the test accommodation that masked it was then REMOVED. | The `/scrape` page rendered its own head without the shared htmx script tag, so `POST /scrape` would have full-page-reloaded in a real browser instead of swapping (KPI-HX-2 silently broken in the browser, though the harness — which sets `HX-Request` directly — still passed). The fix extracted a shared `page_head()` / `htmx_script()` helper so EVERY page loads the local asset from one source; the temporary accommodation was removed. |
| DV-7 | `hx-push-url` on tab switching (ADR-034) + a `#view-panel` wrapping `#claims-table`. | Keeps the REAL URLs after a tab swap (bookmarkable / Back works), converging the htmx path with the no-JS path; the nested wrapper lets the tab-swap and the pagination-swap coexist on one page without target collision. |

## Demo Evidence — 2026-06-02

The slice-07 demos require a LIVE viewer; the `ViewerServer` acceptance harness
stands one up (a real `openlore ui` serving the operator's local DuckDB store over
HTTP) and drives BOTH shapes via `get_htmx` / `post_form_htmx` (set `HX-Request`)
vs the plain `get` / `post_form` (full page). The **walking-skeleton acceptance
tests are the executable end-to-end demos**, all GREEN:

| Demo | What it proves end-to-end (green) |
|---|---|
| Claims-paging swap walking skeleton (north star) | `openlore ui` serves `GET /claims` Prev/Next as a TABLE-ONLY fragment under `HX-Request` and as a FULL slice-06 page without it — wire→probe→use through the real binary; the static `GET /static/htmx.min.js` route serves the vendored asset. The in-place paging end-to-end demo (KPI-HX-1). |
| Live-scrape swap walking skeleton | `POST /scrape` returns a RESULTS-ONLY fragment (form preserved) under htmx and the full page without; no sign control, nothing persisted in either shape. The propose-side in-place demo (KPI-HX-2). |

The remaining user-visible capabilities are demonstrated by their GREEN acceptance
scenarios driving the real `openlore ui` binary:

| Capability | Demo coverage (green acceptance scenario, real binary over HTTP) |
|---|---|
| Claims pagination swap | viewer_htmx (`GET /claims` Prev/Next: table fragment under htmx, full page without; KPI-HX-1) |
| Peer-claims pagination swap | viewer_htmx (`GET /peer-claims` Prev/Next: table fragment, origin preserved in both shapes) |
| Live-scrape results swap | viewer_htmx (`POST /scrape`: results-only fragment, form preserved, no sign control, nothing persisted; KPI-HX-2) |
| Claim-detail inline swap | viewer_htmx (`GET /claims/{cid}`: detail-panel fragment under htmx, full detail page without; KPI-HX-3) |
| My↔Peer tab switch | viewer_htmx (view-panel fragment + `hx-push-url` URL update; bookmarkable / Back works; KPI-HX-4) |
| Progressive-enhancement / offline / read-only / no-regression / parity (gold) | viewer_htmx_invariants H-INV-* (no-JS full page, offline + no-CDN, no new write/sign route + no key, slice-06 byte-equivalence, structural fragment/page parity) |

Cardinal guardrails end-to-end verified: every route serves a complete slice-06 full
page when `HX-Request` is absent (KPI-HX-G1, no regression); every store view AND
swap works network-down with zero CDN references (KPI-HX-G2, offline); no swap adds a
write/sign route, the web process holds no key, the bind stays loopback-only
(KPI-HX-G3, inheriting slice-06 KPI-VIEW-2); and the `/scrape` page now actually
serves the local htmx asset so the browser swap works (DV-6).

## Post-Merge Integration Gate — PASS

- Full slice-07 acceptance suite GREEN (viewer_htmx 24/24, viewer_htmx_invariants
  6/6) + 50 `viewer-domain` unit/property tests; slice-06 26-scenario corpus zero
  regression (viewer_store 15/15, viewer_scrape 5/5, viewer_invariants 6/6); the
  full workspace acceptance suite green across all slices. xtask guards green (no new
  crate, no new capability rule; the slice-06 `viewer-domain` maud pure-core allowlist
  + viewer capability rule remain load-bearing).
- Environment matrix: slice-07 acceptance is hermetic (the `ViewerServer` harness
  stands up a live `openlore ui` over a loopback port against a seeded local store +
  a `tempfile` HOME); the htmx asset is `include_str!`-embedded so the static route is
  self-contained (no per-environment asset fetch). The default matrix is satisfied by
  the hermetic design (same rationale as slice-02..06; DEVOPS graceful-degrade default).
- Known harness flake (NOT a slice-07 regression): the `adapter-system-clock`
  `now_utc_*` env-var contention under full-workspace PARALLEL lib-test runs (carried
  from slice-01/03/04/05/06; serialized fix landed in commit `2629e56`); the
  acceptance targets pass single-threaded / in isolation.

## Quality gates

- `cargo xtask check-arch`: OK (21 workspace members) — no new crate, no new
  capability rule; the `Shape` dispatch lives in the effect shell, not the pure core;
  `viewer-domain` purity intact (maud + ports only, no I/O imports).
- `cargo xtask check-probes`: OK — the `adapter-http-viewer` probe stays real and
  non-stub; `viewer-domain` correctly requires no `probe()` (pure crate).
- Per-phase L1-L4 refactor / adversarial review / mutation / integrity outcomes
  recorded below.

## L1-L4 refactoring

@nw-functional-software-crafter cleared the deferred DELIVER debt (commit `7f78fc1`):
clippy + check-arch + check-probes clean; pure-core purity intact (no I/O imports in
`viewer-domain`; `maud` + `ports` only; the `HX-Request` dispatch isolated in the
effect shell, ADR-033). The shared `page_head()` / `htmx_script()` helper was
extracted here so every page loads the local asset from ONE source — the move that
fixed the `/scrape` htmx-script defect (DV-6) and removed the masking test
accommodation.

## Adversarial review — APPROVED (zero blockers)

@nw-software-crafter-reviewer verdict APPROVED. Zero blockers; Testing Theater clean
across all 15 steps. The three cardinal guardrails (KPI-HX-G1/G2/G3) verified
load-bearing; the fragment/page structural parity (I-HX-5) confirmed real (the page
composes the pure fragment, DV-4); the `/scrape` htmx-script fix (DV-6) confirmed a
genuine browser-path bug-fix with the test accommodation removed (not theatre); the
offline / no-CDN guarantee confirmed structural (vendored asset + SHA-256 pin, DV-3).

## Mutation testing (per-feature 100% on `viewer-domain` production functions): PASS

Scope: the new + extended pure `viewer-domain` production functions (the
`render_*_fragment` / `render_*_page` parity renderers + the `Shape` projection + the
inherited slice-06 render / pagination arithmetic). The slice-04/05 cross-package
lesson stays applied (the 50 `viewer-domain` properties pin the production functions
IN/against the crate, so the per-feature measurement reaches the real killing suite
without a cross-package detour).

| Mutant category | Viable | Caught | Missed | Unviable | Kill rate |
|---|---:|---:|---:|---:|---|
| `viewer-domain` production logic (fragment/page parity renderers + Shape projection + inherited render/pagination arithmetic) | 72 | 72 | 0 | 5 | **100%** (72/72 viable) |

Gate SATISFIED (≥80%; actual 100% on the production scope, 0 missed).
`adapter-http-viewer` is NOT mutated by design (effect shell; covered by the H-INV
gold tests through the real binary). DEVOPS sweep is the ongoing backstop.

## Deliver integrity verification: PASS

`des-verify-integrity docs/feature/viewer-htmx-swaps/deliver/` → "All 15 steps have
complete DES traces" (exit 0).
</content>
