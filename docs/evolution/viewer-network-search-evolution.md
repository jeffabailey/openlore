# Evolution: viewer-network-search (slice-08 network-discovery `/search` view on the read-only viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-network-search/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `slices/`, `deliver/`) and ADR-036..ADR-038 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on three shipped
> slices**: slice-05 (`openlore-appview-search` — the network indexer + query path),
> slice-06 (`htmx-scraper-viewer` — the read-only viewer), and slice-07
> (`viewer-htmx-swaps` — the htmx progressive-enhancement layer). Read those parent
> archives (`docs/evolution/openlore-appview-search-evolution.md`,
> `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`) for the
> surfaces this slice composes.

## Summary

`viewer-network-search` adds a **`GET /search` network-discovery view** to the
`openlore ui` read-only viewer: the **browser UI for `openlore search`** (job
**J-005**). A `/search` route serves a form (pick a dimension + value); on submit the
viewer queries the slice-05 network indexer over HTTP (`OPENLORE_INDEXER_URL`,
`org.openlore.appview.searchClaims`) along **three dimensions** (object/contributor/
subject) and renders **verified + attributed** results as HTML, with an htmx fragment
swap (like `/scrape`). It is the same network discovery J-005 the slice-05 CLI
delivered, now glanceable from the same read-only viewer Maria (P-001) already uses to
inspect her store — she can discover signed claims **across the whole network**, beyond
her own claims and her manually-subscribed peers, without dropping back to the CLI.

The load-bearing thesis: **a network READ on a read-only surface that takes on
authority over nothing**. The viewer signs/writes/persists nothing and holds no signing
key; the new outbound `IndexQueryPort` is the **only** addition — a public-data READ,
distinct from the read-only DuckDB store (slice-06) and the `GithubPort` (`/scrape`,
slice-06). Following a discovered author stays a deliberate CLI action (`openlore peer
add <did>`) — the view shows it as **guidance text**, never an executable control.

The slice ships **ZERO new crates** (workspace stays at **21 members**). It is an
**additive render surface, not a re-architecture**: it REUSES the slice-05 query path
(`IndexQueryPort` + `adapter-index-query`) and the slice-05 pure composition
(`appview-domain::compose_results` + `NetworkResultRow`/`NetworkSearchResult` —
verification + per-author anti-merging, NOT reimplemented), and the slice-06/07 viewer
render pattern (`viewer-domain` maud, the `Shape` fork, page = chrome + fragment, the
vendored offline htmx asset). The new work: extend `viewer-domain` (a pure `SearchState`
ADT + `render_search_*` projecting the `appview-domain` result types — `appview-domain`
becomes a new pure→pure dependency), extend `adapter-http-viewer` (the `/search` handler
+ `Shape` fragment/page fork + a third nav link), wire the read-only index-query client
in the `cli` `ui` verb (still no key), and add 2 `xtask check-arch` deltas.

### What shipped (one paragraph)

A `GET /search` view: a GET form (dimension selector object/contributor/subject + a
value input) → on submit the viewer calls the reused `IndexQueryPort`, maps the outcome
to a `SearchState` ADT (`Form | Results | NoResults | Unavailable`), re-composes flat
rows per-author via the slice-05 `appview-domain::compose_results` (no merge, counter
kept), projects them into HTML, and forks by `Shape::from_request` (the slice-07
`HX-Request` selector) — a full page without the header, the `#search-results` fragment
with it. Every row carries `[verified]` (by construction — the indexer verified
signature + recomputed CID before indexing; the viewer has no second verification path)
+ the author DID + verbatim confidence; `counter_annotation` is SHOWN, never applied;
identical-content-by-two-authors renders as two rows; the page states up front that
discovery indexes only PUBLIC signed claims. An unreachable OR unconfigured indexer
renders a fixed, payload-free `SearchState::Unavailable` notice in BOTH shapes — no HTTP
status, no "connection refused", no raw URL, no stack trace. The bind stays loopback-only
(127.0.0.1); nothing is persisted.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-03 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-04 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-04 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-04 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **13/13 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **24/24 slice-08 acceptance scenarios** GREEN: 20 `viewer_network_search` (the three
  dimensions × the Results/NoResults/Unavailable arms × the fragment/full-page shapes,
  plus the read-only / follow-guidance / counter-shown / public-data-framing scenarios)
  + 4 `viewer_network_search_invariants` (the GOLD guardrails N-INV-ReadOnly /
  N-INV-NoWrite / N-INV-OfflineChrome / N-INV-Verified). Plus **63 `viewer-domain` unit
  tests** (the new `SearchState` projection + `render_search_*` parity + the inherited
  slice-06/07 render properties). The `ViewerServer` harness drives the REAL `openlore
  ui` over HTTP; the indexer is the **only** mocked boundary, and it is a REAL slice-05
  `openlore-indexer serve` spawned by the new `start_with_indexer` /
  `start_with_unreachable_indexer` seams (verified/attributed rows come from the
  production ingest+serve path, not synthetic JSON).
- **Slices 05/06/07 corpora GREEN — zero regression** (the full workspace acceptance
  suite green across all slices; the slice-06 26-scenario + slice-07 30-scenario corpora
  unchanged).
- **NO new crate**: extends `viewer-domain` (PURE) + `adapter-http-viewer` (EFFECT) +
  `cli` (DRIVER) + `xtask` (tooling) in place; REUSES slice-05 `IndexQueryPort` +
  `adapter-index-query` + `appview-domain`. Workspace member count stays **21** (19
  production + 1 test-support + 1 xtask); `cargo xtask check-arch` reports "21 workspace
  members" (with the 2 new deltas).
- **NO new production dependency**: `appview-domain` + `adapter-index-query` (+ reqwest)
  are already in-workspace; `maud`/`hyper` unchanged; no `deny.toml` change.
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production
  functions (**81/81 viable caught, 5 unviable, 0 missed**) — exceeds the ≥80%
  per-feature gate.
- **3 ADRs** (ADR-036..ADR-038) all Accepted/shipped.
- DES integrity: `des-verify-integrity` reports "All 13 steps have complete DES traces"
  (exit 0).
- Adversarial review: **APPROVED**, zero blockers, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members, 2 new deltas). L1-L4 refactor done
  (commit `c2c9dca`).

## Wave-by-wave changelog

### DISCUSS (2026-06-03)

Luna framed the slice as a **brownfield DELTA on slices 05/06/07** (lean mode +
ask-intelligent): the browser UI for the slice-05 J-005 network-discovery job, with NO
new job created (every story traces to J-005). Persona is **P-001 (Maria, the node
operator)** — the viewer's operator wearing the network-discovery hat (slice-05 framed
P-002 as primary for the CLI; the BROWSER viewer's operator is P-001, whose surface this
is, slices 06/07). Four stories: **US-NS-001** (`@infrastructure` — bootstrap the
viewer's indexer-query capability), **US-NS-002** (search by philosophy, attribution
preserved — the walking-skeleton render), **US-NS-003** (contributor/subject dimensions),
**US-NS-004** (trust: verified framing + counter-shown-not-applied + honest degradation +
`peer add` follow guidance). Seven locked decisions (WD-NS-1..7) and nine inherited
invariants (**I-NS-1..9**, all INHERITING the slice-05 AV-* + slice-06 I-VIEW-* +
slice-07 I-HX-* contracts — read-only/no-key, graceful degradation, anti-merging,
verified-by-construction, public-data framing, progressive enhancement, offline chrome,
loopback/zero-persisted, confidence verbatim). slice-08 **REALIZES the existing
KPI-AV-1/2/3/4/5 + KPI-VIEW-2 + KPI-HX-G1/G2/G3 on the browser surface** rather than
minting new KPI IDs (a per-feature `outcome-kpis.md` was intentionally NOT duplicated —
lean). The walking skeleton is US-NS-001 + US-NS-002 (the thinnest end-to-end thread:
viewer → indexer HTTP → verified rows → HTML fragment), validating the riskiest
assumption first — that the read-only viewer can take on a network-query capability while
preserving every cardinal invariant.

### DESIGN (2026-06-04)

Morgan locked slice-08 as an **additive render surface, not a re-architecture** —
**exactly ONE new capability** (an outbound public-data index READ) and ZERO new crates,
ZERO new binary, ZERO new architectural style, ZERO new persisted type, ZERO new
cross-process boundary (the indexer + its XRPC contract already exist from slice-05). The
six open decisions (OD-NS-1..6) were resolved adopting the DISCUSS recommended-leans
verbatim (no deviations), captured in three ADRs:

- **ADR-036** (viewer index-query port + capability boundary): **REUSE** the slice-05
  `IndexQueryPort` + `HttpIndexQueryAdapter` behind the viewer composition root — ONE
  query path, public-data READ only, NO signing/identity/PDS surface (mirrors the
  slice-06 `GithubPort` boundary). Wired as `Option<Arc<dyn IndexQueryPort>>` like
  `GithubPort`; SOFT-probed at startup (an unreachable indexer must NOT block the viewer
  — KPI-5 / I-NS-2). **REUSE** the slice-05 `OPENLORE_INDEXER_URL` config resolution
  (OD-NS-1 + OD-NS-6).
- **ADR-037** (`SearchState` ADT + `viewer-domain` projection + degradation): a **NEW
  pure `viewer-domain` projection** of the `appview-domain` result types into HTML —
  REUSE the composition (`compose_results`, per-author grouping, anti-merging,
  verified/attributed/counter), NOT the CLI stdout text renderer. The degradation is a
  **payload-free `SearchState::Unavailable` UNIT variant** (mirrors `ScrapeState::
  NetworkDown` — structurally cannot leak transport internals), covering unreachable AND
  unconfigured, in both shapes (OD-NS-2 + OD-NS-3).
- **ADR-038** (`GET /search` route + GET form + nav + config reuse): its **OWN route
  `GET /search`** (distinct corpus = the network index, not the local-store tabs), added
  to the nav as a **third link**; a **GET form** (`/search?<dimension>=<value>`) →
  bookmarkable/shareable URL + plain no-JS navigation, htmx fragment fork via `HX-Request`
  + `hx-push-url` (the slice-07 pattern); plus the **2 `xtask check-arch` deltas** (add
  `viewer-domain → appview-domain` to the pure-core dependency allowlist; confirm/extend
  the viewer capability rule admits `IndexQueryPort` read-only while still FORBIDDING any
  signing/identity/PDS + the indexer SERVER/store/ingest crates) (OD-NS-4 + OD-NS-5).

The C4 L2/L3 views, the `GET /search` data-flow, the route/handler design table, the
I-NS-1..9 structural-guarantee table, and the Earned-Trust analysis are in the DESIGN
sections of `feature-delta.md`. The composition root wires NO signing identity and NO
PDS-write surface — `StoreReadPort` + `GithubPort` + `IndexQueryPort`, all read-only by
construction; the `cli` crate MUST NOT link `adapter-xrpc-query-server` /
`adapter-index-store` / `adapter-atproto-ingest` (the indexer-internal crates) into the
viewer surface.

### DISTILL (2026-06-04)

Quinn authored the **24-scenario** executable acceptance corpus (the Reconciliation HARD
GATE passed — 0 contradictions; DESIGN OD-NS-1..6 adopt the DISCUSS recommended-leans
verbatim; ADR-036/037/038 carry I-NS-1..9 forward). Two `[[test]]` targets:

- **`viewer_network_search.rs`** (Tier A — 20 scenarios, `N-` ids): the walking-skeleton
  object-search verified-fragment (N-1), the read-only no-write-surface infra assertion
  (N-1b), object full-page + parity (N-2/3), anti-merging two-rows (N-4), guided empty
  state (N-5), contributor trail + honesty footer + parity (N-6/7), subject N-author-groups
  + parity (N-8/9), absent-contributor no-suggestion empty (N-10), public-data framing
  (N-11), counter-shown-not-applied (N-12), the **Unavailable × {unreachable, unconfigured}
  × {full-page, fragment} matrix** (N-13/14/15/16), and the follow-guidance-text-only row
  (N-17).
- **`viewer_network_search_invariants.rs`** (gold guardrails — 4 scenarios, `N-INV-` ids):
  N-INV-ReadOnly (every dimension + both shapes leaves store row counts unchanged via
  `assert_store_read_only`, Mandate 8), N-INV-NoWrite (no sign/publish/subscribe/
  executable-follow control on any shape), N-INV-OfflineChrome (only local
  `/static/htmx.min.js`, no CDN), N-INV-Verified (every row `[verified]` + attributed
  across all dimensions).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the
indexer is the **only** mocked boundary, and it is a REAL slice-05 `openlore-indexer
serve` — **REUSED, not net-new**: the slice-05 harness already spawns a real indexer
(`seed_network_index` → ingests a fixture corpus into a real `index.duckdb`, spawns a
real serve on an ephemeral port). The **NEW seam** is `ViewerServer::start_with_indexer`
(mirrors `start_with_github`; threads `IndexerHandle::indexer_url()` to the spawned
`openlore ui` via the `OPENLORE_INDEXER_URL` env-var) + `start_with_unreachable_indexer`
(wires a `ClosedIndexerPort` — a freed localhost port, connect-refused by construction).
New shared render assertions: `assert_search_html_every_row_verified_and_attributed`,
`assert_search_html_has_no_merged_consensus_row`, `assert_search_html_leaks_no_transport_
internals`. **Tier B NOT emitted** (the `/search` journey is a single-shot query→render
surface, not a ≥3-chained-scenario stateful journey; the generative exploration of the
pure compose+render core is a DELIVER mutation-testing concern). RED classification: both
targets COMPILE green, all scenarios FAIL via `todo!()` = MISSING_FUNCTIONALITY (correct
RED, not BROKEN).

### DELIVER (2026-06-04)

Executed **13 roadmap steps** via DES-monitored crafter dispatches, each commit carrying
a `Step-ID: NN-NN` trailer. Walking skeleton `facf3a9` (01-01) → final step `80f4208`
(04-04); L1-L4 refactor `c2c9dca`. Per-step SHAs are in `deliver/execution-log.json`.

- **WS / object search (01-xx)**: the `start_with_indexer` seam + the `GET /search` route
  + `Shape::from_request` dispatch + the `SearchState` ADT + `render_search_results_
  fragment` / `render_search_page` parity split + the `IndexQueryPort` wiring in the `ui`
  verb. The north-star walking-skeleton: object-dimension verified-attributed fragment
  rendered in the browser from a reachable real indexer (N-1).
- **Dimensions (02-xx, 03-xx)**: contributor (one author's trail under a single
  `author_did` + the "one developer's reasoning trail, not a community consensus" footer,
  reusing the slice-05 `resolve_contributor_to_did` resolver — see Lessons) and subject (N
  author groups, no consensus row); both forking by `Shape`. The form gained the
  dimension selector here — and the **form-only-had-object-input gap** was caught at 02-03
  (see Lessons / DV-NS-3).
- **Trust + degradation (04-xx)**: the up-front public-data framing banner; the
  `counter_annotation` shown-not-applied render; the payload-free `SearchState::Unavailable`
  notice across the unreachable × unconfigured × full-page × fragment matrix; the
  `openlore peer add <did>` render-only follow-guidance TEXT (no executable control). The
  gold N-INV-* guardrails driving the real binary.

Refactor / review / mutation / integrity outcomes are in the Quality Gates + Mutation
sections below. The DELIVER decisions (including the 02-03 form gap and the 04-04
branch-correction) are recorded in `deliver/wave-decisions.md`.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-NS-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..07 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-NS-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the `SearchState` projection + `render_search_*` renderers), matching slice-02..07 DV-2. The killing properties are kept IN-CRATE (the 63 `viewer-domain` unit tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally (no cross-package cargo-mutants scope detour). 81/81 viable caught, 0 missed. |
| DV-NS-3 | **The `/search` form initially shipped with only the object/value input** (no dimension selector), so contributor/subject could not be reached from the browser form. Caught at step 02-03 when wiring the contributor dimension; the dimension selector (object/contributor/subject) was added so all three dimensions are reachable from one GET form. | Without it, US-NS-003 (contributor/subject) would have been render-reachable only by hand-editing the URL — the form would not have offered the dimensions the AC requires (`/search` form offers philosophy, contributor, AND subject). The gap was a form-completeness miss, not a render bug; the fix made the form match the three-dimension contract (OD-NS-5 / ADR-038). |
| DV-NS-4 | **REUSE the slice-05 `IndexQueryPort` + `adapter-index-query` + `appview-domain::compose_results`** rather than a second query/grouping path (ADR-036/037). ONE outbound query path workspace-wide; the per-author anti-merging composition is consumed, not reimplemented. | A second grouping path is the classic place a "merged consensus" row sneaks back in (KPI-AV-2 is cardinal). Reusing `compose_results` makes anti-merging structural — there is no viewer-side grouping to drift. `appview-domain` becomes a pure→pure dep of `viewer-domain` (the only new dependency edge), enforced by the `xtask` allowlist. |
| DV-NS-5 | **`SearchState::Unavailable` is a payload-free UNIT variant** (mirrors `ScrapeState::NetworkDown`, ADR-037); both unreachable AND unconfigured map to it; one pinned notice constant. | A unit variant has no payload to interpolate, so no HTTP status / URL / "connection refused" / stack-trace CAN leak (I-NS-2 no-leak is STRUCTURAL, not merely intended). The `Unreachable` soft error maps to it; the index-query startup probe is SOFT so an unreachable indexer never blocks the viewer. |
| DV-NS-6 | **`resolve_contributor_to_did` runs viewer-side, reusing the slice-05 pure resolver** (a deliberate viewer-local mirror of the CLI resolution, `github:priya` → `did:plc:priya-test#org.openlore.application`). | One handle→DID convention across the CLI and the browser; reusing the slice-05 PURE resolver (no second resolution path) keeps the contributor-dimension behavior identical to the CLI and keeps the resolver testable in isolation. A deliberate reuse, called out so the viewer-local call site is auditable. |
| DV-NS-7 | **The 04-04 final step was committed on a branch instead of trunk, then corrected** by fast-forwarding `main` to the branch tip per AGENTS.md (trunk-based, no PRs). | AGENTS.md mandates trunk-based development with no PRs and no remote; a feature-branch commit would have left `main` behind the shipped tip and broken the slice-02..07 "every Step-ID commit lands on main" invariant. The slip was caught at close and corrected by fast-forwarding main to the branch tip (no merge commit, linear history preserved); no work was lost. |

## Cardinal release gates + slice-08 invariants (I-NS-1..9)

The cardinal release gates are the inherited KPI guardrails realized on the browser
surface — all release-blocking:

1. **Read-only / no key (KPI-VIEW-2 / KPI-HX-G3 / I-NS-1)** — `/search` is a READ; no new
   write/sign/subscribe route; the web process holds no signing key; the `IndexQueryPort`
   has NO sign/write/publish method (type-level); follow is render-only TEXT. Three-layer:
   TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) +
   BEHAVIORAL (N-1b + N-17 + gold N-INV-NoWrite + the route/key audit).
2. **Anti-merging at network scale (KPI-AV-2 / I-NS-3)** — every row carries one
   non-`Option` `author_did`; `compose_results` is REUSED (no viewer-side grouping);
   identical-content-different-author = two rows; no merged/consensus row; counter shown,
   never applied. BEHAVIORAL (N-4/8 + N-12 + gold N-INV-Verified).
3. **Verified-before-index (KPI-AV-3 / I-NS-4)** — every row `[verified]` by construction
   (the indexer is the verify gate; the viewer has no second verification path).
4. **No-JS no-regression (KPI-HX-G1 / I-NS-6)** — `GET /search` serves a complete full
   page without `HX-Request`; `render_search_page` EMBEDS `render_search_results_fragment`
   (parity by construction); the slice-05/06/07 corpora stay GREEN.
5. **Offline / no-CDN chrome (KPI-HX-G2 / I-NS-7)** — the `/search` page references only
   the vendored local htmx asset; zero off-host references (the SEARCH itself needs the
   network, like `/scrape`, but the chrome stays offline-capable).
6. **Graceful degradation (I-NS-2)** — an unreachable OR unconfigured indexer renders the
   fixed payload-free `Unavailable` notice in BOTH shapes; no leaked transport internals;
   no crash/hang (N-13/14/15/16 + `assert_search_html_leaks_no_transport_internals`).

The full slice-08 invariant set (I-NS-1..9; structural-guarantee detail in the DESIGN
section of `feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-NS-1 | Read-only / no key (search is a READ; no write/sign/subscribe route; no key in the process; the index-query port holds no signing/identity/PDS surface; follow is render-only TEXT). | TYPE (no write method on `IndexQueryPort`) + STRUCTURAL (`xtask check-arch` viewer capability rule — no signing/identity/PDS, no indexer SERVER/store/ingest crates) + BEHAVIORAL (N-1b/N-17/N-INV-NoWrite + route + key audit). Cardinal (KPI-VIEW-2 / KPI-HX-G3). |
| I-NS-2 | Graceful degradation (unreachable/unconfigured → fixed plain-language notice; no crash/hang/blank/stack-trace; no transport leak). | TYPE (payload-free `Unavailable` unit variant) + STRUCTURAL (one pinned notice constant; soft `Unreachable` mapping; soft startup probe) + BEHAVIORAL (N-13/14/15/16 + leaks-no-transport-internals). |
| I-NS-3 | Anti-merging (one non-`Option` `author_did` per row; identical-content-different-author = two rows; no merged/consensus row; counter shown-not-applied). | TYPE (non-`Option` author_did; no merged-row API) + STRUCTURAL (REUSE `compose_results` — no viewer-side grouping) + BEHAVIORAL (N-4/8/12). Cardinal (KPI-AV-2). |
| I-NS-4 | Verified display (every row `[verified]` by construction; the indexer is the verify gate; no second verification path in the viewer). | TYPE (`verified_against` non-empty) + BEHAVIORAL (N-INV-Verified). Cardinal (KPI-AV-3). |
| I-NS-5 | Public-data framing (the `/search` page states up front that discovery indexes only PUBLIC signed claims, verified before indexing). | STRUCTURAL (banner in page chrome) + BEHAVIORAL (N-11). |
| I-NS-6 | Progressive enhancement (full page without `HX-Request`, the same results-region fragment with it; page = chrome + fragment). | STRUCTURAL (`render_search_page` embeds `render_search_results_fragment`) + BEHAVIORAL (N-2/3/7/9 parity). Cardinal (KPI-HX-G1). |
| I-NS-7 | Offline / no-CDN chrome (only the vendored local htmx asset; zero off-host references). | STRUCTURAL (the shared `htmx_script` fn + SHA-256-pinned asset) + BEHAVIORAL (N-INV-OfflineChrome). Cardinal (KPI-HX-G2). |
| I-NS-8 | Zero new persisted types / loopback-only bind (results computed per query; `ViewerServer::bind` still refuses non-loopback). | TYPE (no new persisted type) + STRUCTURAL (no store-write call in the path; loopback guard unchanged) + BEHAVIORAL (gold N-INV-ReadOnly row-count delta). |
| I-NS-9 | Confidence verbatim (rendered through the EXISTING `render_confidence` — `0.90`, never `0.9`/`90%`). | STRUCTURAL (one `render_confidence` site, reused) + BEHAVIORAL (verbatim assertion). |

All slice-08 invariants INHERIT the slice-05 AV-* + slice-06 I-VIEW-1..6 + slice-07
I-HX-1..5 sets; confidence stays shown verbatim (FR-VIEW-8) in both shapes.

## Quality gates — final report

- **Acceptance / integration**: 24/24 slice-08 scenarios GREEN (`viewer_network_search`
  20/20, `viewer_network_search_invariants` 4/4) + 63 `viewer-domain` unit tests; slices
  05/06/07 corpora GREEN — zero regression (the full workspace acceptance suite green
  across all slices). The `ViewerServer` harness drives the REAL `openlore ui` over HTTP;
  the indexer is the only mocked boundary and it is a REAL slice-05 `openlore-indexer
  serve` (the `start_with_indexer` / `start_with_unreachable_indexer` seams).
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate; the **2 new
  deltas** are (1) the `viewer-domain → appview-domain` pure-core dependency allowlist
  entry (pure → pure edge) and (2) the confirmed/extended viewer capability rule admitting
  `IndexQueryPort` (read-only) while still FORBIDDING any signing/identity/PDS + the
  indexer SERVER/store/ingest crates.
- **`cargo xtask check-probes`**: OK — the reused `IndexQueryPort` already carries a
  non-stub `probe()`; `viewer-domain` is pure (no probe required).
- **Refactor (L1-L4)**: commit `c2c9dca` — clippy + check-arch + check-probes clean;
  `viewer-domain` purity intact (no I/O imports; maud + ports + the new `appview-domain`
  pure dep only; the `Shape` dispatch lives in the effect shell, not the pure core).
- **Adversarial review**: APPROVED, zero blockers, zero Testing Theater. The cardinal
  guardrails verified load-bearing; the anti-merging confirmed structural (the viewer
  REUSES `compose_results`, DV-NS-4); the payload-free `Unavailable` confirmed a genuine
  no-leak guarantee (unit variant, DV-NS-5); the form-completeness fix (DV-NS-3) confirmed
  a real gap-closure, not theatre.
- **DES integrity**: PASS — "All 13 steps have complete DES traces" (exit 0).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the `SearchState`
projection + the `render_search_results_fragment` / `render_search_page` parity renderers
+ the inherited slice-06/07 render arithmetic). The slice-04/05 cross-package lesson stays
applied — the 63 `viewer-domain` unit tests pin the production functions IN/against the
crate, so the per-feature mutation measurement reaches the real killing suite without a
cross-package detour.

| Mutant category | Viable | Caught | Missed | Unviable | Kill rate |
|---|---:|---:|---:|---:|---|
| `viewer-domain` production logic (`SearchState` projection + `render_search_*` parity renderers + inherited render arithmetic) | 81 | 81 | 0 | 5 | **100%** (81/81 viable) |

Slice-08 per-feature gate SATISFIED (≥80%; actual 100% on the production scope, 0 missed).
`adapter-http-viewer` is NOT mutated by design (effect shell; covered by the N-INV gold
tests through the real binary); `appview-domain` is REUSED (already mutation-covered at
slice-05). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **The `/search` form initially had only an object/value input (DV-NS-3)**: when the
  walking skeleton shipped object-dimension search, the GET form carried only the object
  input — contributor/subject were render-capable but not reachable from the form. The gap
  surfaced at step 02-03 while wiring the contributor dimension: the dimension selector was
  missing, so the three-dimension contract (OD-NS-5 / US-NS-003 AC) was met only by
  hand-editing the URL. The fix added the object/contributor/subject selector to the one
  GET form. **Institutional lesson: a walking skeleton that ships the thinnest dimension
  first can leave the user-facing FORM behind the render — the render can handle a
  dimension the form does not yet offer; when later dimensions land, re-check that the
  input surface (not just the renderer) exposes them, or an AC ("the form offers all three
  dimensions") passes only via URL-editing.**
- **`resolve_contributor_to_did` is a deliberate viewer-local mirror (DV-NS-6)**: the
  contributor dimension needs `github:priya` → a DID, exactly as the slice-05 CLI does. The
  viewer REUSES the slice-05 PURE `resolve_contributor_to_did` resolver at a viewer-local
  call site rather than passing the handle through to the indexer or minting a second
  resolution. **Lesson: when a browser surface mirrors a CLI behavior that depends on a
  pure helper, reuse the SAME pure helper at the new call site (one convention, one tested
  resolver) and call the reuse out explicitly so the second call site stays auditable —
  the alternative (a parallel resolution path) is where handle→DID conventions silently
  diverge between surfaces.**
- **Reusing `compose_results` makes anti-merging structural (DV-NS-4)**: the cardinal
  KPI-AV-2 (zero merged-consensus rows at network scale) is guaranteed not by a viewer-side
  test but by the ABSENCE of a viewer-side grouping path — the viewer consumes the slice-05
  pure `compose_results` (per-author, no merge API). **Lesson: when a trust guarantee is
  "never merge attribution," the strongest enforcement is to have exactly ONE composition
  function workspace-wide and REUSE it everywhere; a second grouping path on a new surface
  is the classic place a merged row sneaks back in.**
- **The branch-vs-trunk slip, corrected (DV-NS-7)**: the final 04-04 step was committed on
  a feature branch instead of `main`; AGENTS.md mandates trunk-based development (no PRs, no
  remote). The slip was caught at close and corrected by fast-forwarding `main` to the
  branch tip (linear history preserved, no merge commit, no work lost). **Lesson: in a
  trunk-based, no-PR workflow the "every Step-ID commit lands on main" invariant is easy to
  break with a stray branch checkout; verify `git branch --show-current` is the trunk
  before each Step-ID commit, and if a step lands off-trunk, fast-forward main to the tip
  rather than opening a PR (which the workflow forbids).**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | OD-NS-1..6 resolved as DESIGN recommended-leans; ADR-036/037/038 fixed the contracts; field-level shaping (`SearchState`/view-model shapes, the `parse_search_query` grammar, the constructor ergonomics) left to DELIVER. | All adopted verbatim; the `SearchState` arms (`Form`/`Results`/`NoResults`/`Unavailable`), the `parse_search_query` grammar, and the `bind_with_*` constructor materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | The `/search` GET form (OD-NS-5) — DESIGN fixed GET + the three dimensions; the widget (radio vs `<select>`) left to DELIVER. | The dimension selector shipped; the **form initially lacked it** (object-only) and was completed at step 02-03 (DV-NS-3). | Found + fixed within DELIVER; recorded as DV-NS-3. |
| 3 | The contributor handle→DID resolution — DESIGN recommended reusing the slice-05 pure `resolve_contributor_to_did`; DELIVER to confirm. | Reused viewer-side (DV-NS-6). | Confirmed at DELIVER; the recommended reuse adopted. |
| 4 | The `xtask check-arch` rule edits (allowlist entry + capability-rule scope) — ADR-038 fixed the intent; the rule wiring left to DELIVER. | The 2 deltas landed (`viewer-domain → appview-domain` allowlist + the capability-rule scope); `check-arch` reports 21 members. | Resolved at DELIVER. |
| 5 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-NS-2, 100% on production functions, 0 missed). | Recorded. |
| 6 | Trunk-based, no PRs (AGENTS.md). | The final step landed off-trunk and was fast-forwarded onto `main` (DV-NS-7). | Corrected within DELIVER; recorded as DV-NS-7. |

## KPI status at GA (slice-08 — REALIZES the inherited KPI-AV-* / KPI-VIEW-2 / KPI-HX-G* on the browser surface)

slice-08 mints NO new KPI IDs; it realizes the existing contracts on the new `/search`
surface (per the DISCUSS Outcome-KPIs decision). DEVOPS adds viewer-side `/search`
telemetry mirroring the slice-05 CLI events.

| KPI | Type | Status at GA | Note |
|---|---|---|---|
| KPI-AV-1 (non-obvious-author discovery — north star) | north-star | per-feature GREEN; cohort YELLOW | `/search` surfaces verified claims by unfollowed authors in the browser (N-1/N-17 GREEN); the unfollowed-author-hit telemetry + day-30 study is the pending DEVOPS cohort measure. Baseline 0 (no browser network search before slice-08). |
| KPI-AV-2 (anti-merging at network scale) | guardrail | MET (release-blocking) | every row one `author_did`; identical-content-two-authors = two rows; no merged row; `compose_results` REUSED (DV-NS-4); counter shown-not-applied (N-4/8/12). |
| KPI-AV-3 (verified-before-index / every row `[verified]`) | guardrail | MET (release-blocking) | every browser row `[verified]` by construction (the indexer is the verify gate; no second verification path); N-INV-Verified GREEN. |
| KPI-AV-4 (discovery → federation funnel) | leading | per-feature GREEN; cohort YELLOW | the unfollowed-author row shows `openlore peer add <did>` guidance TEXT (run in the CLI); the search→`peer add` funnel telemetry is the pending DEVOPS measure. Follow stays a deliberate CLI action (read-only). |
| KPI-AV-5 (public-data framing comprehension) | leading | per-feature GREEN; cohort YELLOW | the `/search` page states up front it indexes only PUBLIC signed claims verified before indexing (N-11); the day-30 comprehension prompt is the pending DEVOPS measure. |
| KPI-VIEW-2 (read-only — zero write/sign route, zero key) | guardrail | MET (release-blocking) | `/search` adds no write/sign/subscribe route; zero key reads in the viewer process; the `IndexQueryPort` holds no signing/identity/PDS surface; follow is render-only TEXT (N-1b/N-17/N-INV-NoWrite + route/key audit). |
| KPI-HX-G1 (no-JS no-regression) | guardrail | MET (release-blocking) | `GET /search` serves a complete full page without `HX-Request`; the slice-05/06/07 corpora stay GREEN; `render_search_page` embeds the fragment (parity, N-2/3/7/9). |
| KPI-HX-G2 (offline / no-CDN chrome) | guardrail | MET (release-blocking) | the `/search` page references only the vendored local htmx asset; zero off-host references (N-INV-OfflineChrome). The SEARCH itself needs the network (like `/scrape`), by design. |
| KPI-HX-G3 (read-only / no new write surface) | guardrail | MET (release-blocking) | carries KPI-VIEW-2 across the new search surface — no new write/sign route, no key, loopback-only bind. |
| KPI-5 (local-first) | guardrail | MET (release-blocking) | the new outbound READ adds NO dependency to the offline compose/sign flows; an unreachable indexer degrades gracefully (`Unavailable`, never a block); the viewer stays loopback-only. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-network-search/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, slices), `slices/`,
  `deliver/` (wave-decisions, roadmap.json, execution-log.json).
- **Parent slice-05 archive** (the network indexer + query path this slice reuses):
  `docs/evolution/openlore-appview-search-evolution.md`
- **Parent slice-06 archive** (the read-only viewer this slice extends):
  `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Slice-08 ADRs**:
  `docs/adrs/ADR-036-viewer-index-query-port-capability-boundary.md`,
  `docs/adrs/ADR-037-search-state-adt-viewer-domain-projection-degradation.md`,
  `docs/adrs/ADR-038-search-route-get-form-nav-config-reuse.md`
- **Architecture design / component boundaries / C4 / data-flow** (kept in the feature
  workspace): the DESIGN sections of `docs/feature/viewer-network-search/feature-delta.md`
- **DELIVER wave decisions**:
  `docs/feature/viewer-network-search/deliver/wave-decisions.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-network-search/deliver/execution-log.json`,
  `docs/feature/viewer-network-search/deliver/roadmap.json`
- **Outcome KPIs (slice-08 — inherited on a new surface)**: the Outcome-KPIs section of
  `feature-delta.md` (no per-feature `outcome-kpis.md` duplicated, by design — lean)
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_network_search.rs` (20 N-scenarios),
  `tests/acceptance/viewer_network_search_invariants.rs` (4 gold N-INV-scenarios)
- **Reused indexer query path**: `crates/ports::IndexQueryPort`,
  `crates/adapter-index-query` (`HttpIndexQueryAdapter`),
  `crates/appview-domain` (`compose_results` + `NetworkResultRow`/`NetworkSearchResult`)
- **Extended viewer crates**: `crates/viewer-domain` (`SearchState` + `render_search_*`),
  `crates/adapter-http-viewer` (`GET /search` handler + `Shape` fork + third nav link)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
