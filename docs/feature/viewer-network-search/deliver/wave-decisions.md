# Wave Decisions — DELIVER — viewer-network-search (slice-08)

- **Wave**: DELIVER
- **Date**: 2026-06-04
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 13 steps, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD; review + L1-L4 refactor + per-feature mutation enabled; models inherit.

## Execution summary

All 13 roadmap steps executed via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Walking skeleton `facf3a9` (01-01) → final step
`80f4208` (04-04); L1-L4 refactor `c2c9dca`. All 24 slice-08 acceptance scenarios
GREEN (`viewer_network_search` 20/20, `viewer_network_search_invariants` 4/4) plus 63
`viewer-domain` unit tests; the `ViewerServer` harness drives the REAL `openlore ui`
over HTTP and the indexer is the only mocked boundary — a REAL slice-05
`openlore-indexer serve` spawned via the new `start_with_indexer` /
`start_with_unreachable_indexer` seams (the `OPENLORE_INDEXER_URL` thread, OD-NS-6).
Slices 05/06/07 corpora GREEN — zero regression. **NO new crate**: extends
`viewer-domain` (PURE) + `adapter-http-viewer` (EFFECT) + `cli` (DRIVER) + `xtask`
(tooling) in place; REUSES the slice-05 `IndexQueryPort` + `adapter-index-query` +
`appview-domain::compose_results`. Workspace stays at **21 members** (with the 2 new
`check-arch` deltas).

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|----------|-----------|
| DV-NS-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..07 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-NS-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the `SearchState` projection + `render_search_*` renderers), matching slice-02..07 DV-2. The killing properties are kept IN-CRATE (the 63 `viewer-domain` unit tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally without a cross-package cargo-mutants scope detour. 81/81 viable caught, 0 missed. |
| DV-NS-3 | **The `/search` form initially shipped with only the object/value input** (no dimension selector). Caught at step 02-03 while wiring the contributor dimension; the object/contributor/subject dimension selector was added so all three dimensions are reachable from one GET form. | Without it, US-NS-003 (contributor/subject) was render-reachable only by hand-editing the URL — the form would not have offered the dimensions the AC requires ("the `/search` form offers philosophy, contributor, AND subject"). A form-completeness miss, not a render bug; the fix made the form match the three-dimension contract (OD-NS-5 / ADR-038). |
| DV-NS-4 | **REUSE the slice-05 `IndexQueryPort` + `adapter-index-query` + `appview-domain::compose_results`** rather than a second query/grouping path (ADR-036/037). ONE outbound query path workspace-wide; the per-author anti-merging composition is consumed, not reimplemented. | A second grouping path is the classic place a "merged consensus" row sneaks back in (KPI-AV-2 is cardinal). Reusing `compose_results` makes anti-merging STRUCTURAL — there is no viewer-side grouping to drift. `appview-domain` becomes a pure→pure dep of `viewer-domain` (the only new dependency edge), enforced by the `xtask` allowlist. |
| DV-NS-5 | **`SearchState::Unavailable` is a payload-free UNIT variant** (mirrors `ScrapeState::NetworkDown`, ADR-037); both unreachable AND unconfigured map to it; one pinned notice constant. | A unit variant has no payload to interpolate, so no HTTP status / URL / "connection refused" / stack-trace CAN leak (I-NS-2 no-leak is STRUCTURAL, not merely intended). The soft `Unreachable` error maps to it; the index-query startup probe is SOFT so an unreachable indexer never blocks the viewer (KPI-5 / I-NS-2). |
| DV-NS-6 | **`resolve_contributor_to_did` runs viewer-side, reusing the slice-05 pure resolver** (a deliberate viewer-local mirror of the CLI resolution, `github:priya` → `did:plc:priya-test#org.openlore.application`). | One handle→DID convention across the CLI and the browser; reusing the slice-05 PURE resolver (no second resolution path) keeps the contributor-dimension behavior identical to the CLI and keeps the resolver testable in isolation. A deliberate reuse, called out so the viewer-local call site is auditable. |
| DV-NS-7 | **The 04-04 final step was committed on a branch instead of trunk, then corrected** by fast-forwarding `main` to the branch tip per AGENTS.md (trunk-based, no PRs, no remote). | A feature-branch commit would have left `main` behind the shipped tip and broken the slice-02..07 "every Step-ID commit lands on main" invariant. The slip was caught at close and corrected by fast-forwarding main to the branch tip (no merge commit, linear history preserved); no work was lost. |

## Demo Evidence — 2026-06-04

The slice-08 demos require a LIVE viewer AND a LIVE indexer; the `ViewerServer`
acceptance harness stands BOTH up (a real `openlore ui` serving the operator's local
DuckDB store over HTTP, and a real `openlore-indexer serve` over an ephemeral loopback
port) via `start_with_indexer` / `start_with_unreachable_indexer`, and drives BOTH
shapes via `get_htmx` (set `HX-Request`) vs the plain `get` (full page). The
**walking-skeleton acceptance tests are the executable end-to-end demos**, all GREEN:

| Demo | What it proves end-to-end (green) |
|---|---|
| Object-search verified-fragment walking skeleton (north star) | `openlore ui` serves `GET /search?object=...` as a VERIFIED, ATTRIBUTED `#search-results` fragment under `HX-Request` and as a FULL page without it — wire→(soft-)probe→use through the real binary; the rows come from a REAL slice-05 `openlore-indexer serve` (production ingest+serve path, not synthetic JSON). The browser network-discovery end-to-end demo (KPI-AV-1 on the browser surface). |
| Graceful-degradation walking skeleton | `GET /search` against an unreachable OR unconfigured indexer renders the fixed payload-free `Unavailable` notice in BOTH shapes — no HTTP status, no "connection refused", no raw URL, no stack trace, no crash/hang (I-NS-2 / KPI-5). |

The remaining user-visible capabilities are demonstrated by their GREEN acceptance
scenarios driving the real `openlore ui` over HTTP against the real indexer:

| Capability | Demo coverage (green acceptance scenario, real binary over HTTP) |
|---|---|
| Search by object/philosophy | viewer_network_search N-1/N-2/N-3 (verified-attributed fragment under htmx, full page without, parity; KPI-AV-1) |
| Anti-merging at network scale | viewer_network_search N-4 (identical-content-two-authors = two rows) + N-8 (subject N-author-groups, no consensus row); KPI-AV-2 |
| Search by contributor | viewer_network_search N-6/N-7 (one author's trail under one `author_did` + the "not a community consensus" footer, parity) |
| Search by subject | viewer_network_search N-8/N-9 (N author groups, no consensus row, parity) |
| Guided empty states | viewer_network_search N-5 (typo'd object) + N-10 (absent contributor, no-suggestion) |
| Trust framing + counter-shown | viewer_network_search N-11 (public-data framing up front) + N-12 (counter shown-not-applied); KPI-AV-3/5 |
| Graceful degradation | viewer_network_search N-13/14/15/16 (unreachable × unconfigured × full-page × fragment → fixed notice, no leak, no crash); I-NS-2 |
| Follow-guidance (read-only) | viewer_network_search N-17 (`openlore peer add <did>` TEXT only, no executable control); I-NS-1 |
| Read-only / no-write / offline / verified (gold) | viewer_network_search_invariants N-INV-ReadOnly / N-INV-NoWrite / N-INV-OfflineChrome / N-INV-Verified |

Cardinal guardrails end-to-end verified: every row `[verified]` + attributed with no
merged row (KPI-AV-2/3); `/search` adds no write/sign/subscribe route, the web process
holds no key, the bind stays loopback-only (KPI-VIEW-2 / KPI-HX-G3); `GET /search`
serves a complete full page without `HX-Request` (KPI-HX-G1) referencing only the
vendored local htmx asset (KPI-HX-G2); the unreachable/unconfigured indexer degrades to
a payload-free notice (I-NS-2 / KPI-5).

## Post-Merge Integration Gate — PASS

- Full slice-08 acceptance suite GREEN (`viewer_network_search` 20/20,
  `viewer_network_search_invariants` 4/4) + 63 `viewer-domain` unit tests; slices
  05/06/07 corpora zero regression; the full workspace acceptance suite green across all
  slices. xtask guards green (no new crate; the 2 new `check-arch` deltas — the
  `viewer-domain → appview-domain` pure-core allowlist entry + the confirmed/extended
  viewer capability rule admitting `IndexQueryPort` read-only).
- Environment matrix: slice-08 acceptance is hermetic (the `ViewerServer` harness stands
  up a live `openlore ui` AND a live `openlore-indexer serve` over loopback ports against
  a seeded local store + a seeded `index.duckdb` + a `tempfile` HOME). Build-before-run:
  the run `cargo build`s BOTH the `openlore` (viewer) AND `openlore-indexer` bins before
  running these ATs so `start_with_indexer` spawns the CURRENT viewer over a CURRENT
  indexer (mirrors the slice-05/06/07 viewer + indexer AT precedent). The default matrix
  is satisfied by the hermetic design (DEVOPS graceful-degrade default).
- Known harness flake (NOT a slice-08 regression): the `adapter-system-clock`
  `now_utc_*` env-var contention under full-workspace PARALLEL lib-test runs (carried
  from slice-01/03/04/05/06/07; serialized fix landed in commit `2629e56`); the
  acceptance targets pass single-threaded / in isolation.

## Quality gates

- `cargo xtask check-arch`: OK (21 workspace members) — no new crate; 2 new deltas (the
  `viewer-domain → appview-domain` pure-core dependency allowlist entry [pure → pure] +
  the confirmed/extended viewer capability rule admitting `IndexQueryPort` read-only while
  still FORBIDDING any signing/identity/PDS + the indexer SERVER/store/ingest crates).
  `viewer-domain` purity intact (maud + ports + the new `appview-domain` pure dep only, no
  I/O imports); the `Shape` dispatch lives in the effect shell, not the pure core.
- `cargo xtask check-probes`: OK — the reused `IndexQueryPort` already carries a non-stub
  `probe()`; `viewer-domain` correctly requires no `probe()` (pure crate).
- Per-phase L1-L4 refactor / adversarial review / mutation / integrity outcomes recorded
  below.

## L1-L4 refactoring

@nw-functional-software-crafter cleared the deferred DELIVER debt (commit `c2c9dca`):
clippy + check-arch + check-probes clean; pure-core purity intact (no I/O imports in
`viewer-domain`; `maud` + `ports` + the new `appview-domain` pure dep only; the
`HX-Request` dispatch isolated in the effect shell, ADR-033). The `SearchState`
projection + `render_search_*` renderers consume the slice-05 `compose_results`
verbatim (no viewer-side grouping path, DV-NS-4).

## Adversarial review — APPROVED (zero blockers)

@nw-software-crafter-reviewer verdict APPROVED. Zero blockers; Testing Theater clean
across all 13 steps. The cardinal guardrails verified load-bearing; the anti-merging
confirmed STRUCTURAL (the viewer REUSES `compose_results`, DV-NS-4 — no second grouping
path); the payload-free `Unavailable` confirmed a genuine no-leak guarantee (unit
variant, DV-NS-5); the form-completeness fix (DV-NS-3) confirmed a real gap-closure (the
GET form now offers all three dimensions), not theatre.

## Mutation testing (per-feature 100% on `viewer-domain` production functions): PASS

Scope: the new + extended pure `viewer-domain` production functions (the `SearchState`
projection + the `render_search_results_fragment` / `render_search_page` parity renderers
+ the inherited slice-06/07 render arithmetic). The slice-04/05 cross-package lesson stays
applied (the 63 `viewer-domain` unit tests pin the production functions IN/against the
crate, so the per-feature measurement reaches the real killing suite without a
cross-package detour).

| Mutant category | Viable | Caught | Missed | Unviable | Kill rate |
|---|---:|---:|---:|---:|---|
| `viewer-domain` production logic (`SearchState` projection + `render_search_*` parity renderers + inherited render arithmetic) | 81 | 81 | 0 | 5 | **100%** (81/81 viable) |

Gate SATISFIED (≥80%; actual 100% on the production scope, 0 missed).
`adapter-http-viewer` is NOT mutated by design (effect shell; covered by the N-INV gold
tests through the real binary); `appview-domain` is REUSED (already mutation-covered at
slice-05). DEVOPS sweep is the ongoing backstop.

## Deliver integrity verification: PASS

`des-verify-integrity docs/feature/viewer-network-search/deliver/` → "All 13 steps have
complete DES traces" (exit 0).
