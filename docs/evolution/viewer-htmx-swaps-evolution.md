# Evolution: viewer-htmx-swaps (slice-07 htmx partial-swaps on the read-only viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-htmx-swaps/`
> (discuss/ design/ distill/ deliver/) and ADR-031..ADR-035 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on slice-06
> (`htmx-scraper-viewer`)** — read that parent evolution archive
> (`docs/evolution/htmx-scraper-viewer-evolution.md`) for the read-only viewer it
> builds on.

## Summary

`viewer-htmx-swaps` layers **htmx partial-swaps as progressive enhancement** onto
the slice-06 `openlore ui` read-only localhost viewer (job **J-001**: make the node
operator's node LEGIBLE). It removes the per-click full-page-reload jolt from the
viewer's interactions so navigating the local store feels like **moving around one
steady place** — paging, scraping, opening a claim, and switching views update *in
place*, not by reloading the whole document — **without sacrificing any slice-06
promise**: the viewer stays read-only, works offline (no CDN), and works with
JavaScript off. The thesis is **in-place without authority and without a JS
dependency**: a swap is a nicety, never a requirement.

The slice enhances **four interactions plus a tab switch**, all on the EXISTING
slice-06 routes (zero new write routes, zero new persisted types):

1. **Claims pagination** — `GET /claims` Prev/Next returns a **table-only fragment**
   under `HX-Request` (north star KPI-HX-1); the full page when the header is absent.
2. **Peer-claims pagination** — `GET /peer-claims` Prev/Next, same fragment/page fork.
3. **Live-scrape results** — `POST /scrape` returns a **results-only fragment** (the
   form preserved) under `HX-Request`; the full page otherwise (KPI-HX-2). Still no
   sign control, still nothing persisted in either shape.
4. **Claim-detail inline** — `GET /claims/{cid}` returns a **detail-panel fragment**
   under `HX-Request` (the list preserved); the full detail page otherwise (KPI-HX-3).
5. **My ↔ Peer tab switching** — returns a **view-panel fragment** AND updates the
   URL via `hx-push-url` so the view stays bookmarkable and Back works (KPI-HX-4).

The load-bearing design move: **the same route serves a FULL page when `HX-Request`
is absent (no-JS / bookmark / curl) and a FRAGMENT of the same content when it is
present** — progressive enhancement by construction. Every `viewer-domain` region
gained a pure `render_*_fragment()`; every `render_*_page()` composes the SAME
fragment (page = chrome + fragment), so the fragment and the full page are
**structurally one source** (invariant I-HX-5). The effect shell `adapter-http-viewer`
reads `HX-Request` ONCE (`Shape::from_request`) and forks fragment-vs-page at the
render call; the pure core stays header-unaware (ADR-033). htmx itself is **vendored**
(2.0.4, 0BSD) at `crates/adapter-http-viewer/assets/htmx.min.js`, `include_str!`-embedded
and served from one cached `GET /static/htmx.min.js` route with a pinned SHA-256
integrity test — **no CDN, offline by construction** (ADR-031 / KPI-HX-G2).

The CLI + signed claims REMAIN the source of truth; the swaps ride GET + the existing
POST `/scrape` — **no new write/sign route, no signing key in the web process, bind
stays loopback-only** (KPI-HX-G3, inheriting slice-06 KPI-VIEW-2). **NO new crate**:
the slice extends `viewer-domain` (fragment renderers + `Shape`) and
`adapter-http-viewer` (dispatch + static route) in place, plus a vendored text asset.
Workspace stays at **21 members**.

### Wave timeline

| Wave    | Date       | Owner                              |
|---------|------------|------------------------------------|
| DISCUSS | 2026-06-01 | Luna (nw-product-owner)            |
| DESIGN  | 2026-06-01 | Morgan (nw-solution-architect)     |
| DISTILL | 2026-06-01 | Quinn (nw-acceptance-designer)     |
| DELIVER | 2026-06-02 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **15/15 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **30/30 slice-07 acceptance scenarios** GREEN: 24 `viewer_htmx` (the four
  interaction families + tab-switch, fragment-vs-page across each route) + 6
  `viewer_htmx_invariants` (the GOLD guardrails H-INV-* for progressive
  enhancement / offline / read-only / no-regression / structural parity). Plus
  **50 `viewer-domain` unit/property tests** (the new `render_*_fragment` /
  `render_*_page` parity + `Shape` dispatch + the inherited slice-06 render /
  pagination properties). The `ViewerServer` harness drives the REAL `openlore ui`
  over HTTP with the new `get_htmx` / `post_form_htmx` seams (set `HX-Request`) and
  the `is_fragment` / `is_full_page` / `references_external_cdn` assertions
  (ADR-035).
- **Slice-06 26-scenario corpus GREEN — zero regression** (viewer_store 15/15,
  viewer_scrape 5/5, viewer_invariants 6/6); the no-htmx full-page path stays
  byte-equivalent beyond the bounded chrome delta (KPI-HX-G1).
- **NO new crate**: extends `viewer-domain` (PURE) + `adapter-http-viewer` (EFFECT)
  in place; adds the vendored `assets/htmx.min.js` text asset (NOT a crate). Workspace
  member count stays **21** (19 production + 1 test-support + 1 xtask); `cargo xtask
  check-arch` reports "21 workspace members".
- **NO new crate dependency**: htmx is a text asset embedded via `include_str!`
  (`sha2` is dev-only, for the integrity test); no `deny.toml` change; no `check-arch`
  rule change.
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production
  functions (72/72 viable caught, 5 unviable, 0 missed) — exceeds the ≥80% per-feature
  gate.
- **5 ADRs** (ADR-031..ADR-035) all Accepted/shipped.
- DES integrity: `des-verify-integrity` reports "All 15 steps have complete DES
  traces" (exit 0).
- Adversarial review: **APPROVED**, zero blockers, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members). L1-L4 refactor done
  (commit `7f78fc1`).

## Wave-by-wave changelog

### DISCUSS (2026-06-01)

Framed the slice as a **DELTA on slice-06**: remove the full-page-reload jolt from
the four viewer interactions while keeping the slice-06 guarantees intact. Authored
four outcome KPIs (KPI-HX-1..4) measuring the **observable behavior of the response
shape** (fragment vs full page, keyed on `HX-Request`) against the real `openlore
ui`, with **KPI-HX-1** (north star: paging `/claims` + `/peer-claims` returns a
table-only fragment under htmx, full page without it, fragment bytes a fraction of
the full page) as the heart — paging is the most-used, highest-pain interaction on a
real-sized store and is the walking-skeleton outcome. Three leading indicators:
KPI-HX-2 (scrape in place), KPI-HX-3 (detail inline), KPI-HX-4 (tab switch in place,
URL updated / bookmarkable). Three release-blocking guardrails encode the load-bearing
contracts: **KPI-HX-G1** (no-JS no-regression — every route serves a complete
slice-06 full page when `HX-Request` is absent; non-htmx responses byte-equivalent to
slice-06; the 26-scenario corpus GREEN), **KPI-HX-G2** (offline — every store view AND
every swap works with the network down; zero CDN references), and **KPI-HX-G3**
(read-only / no new write surface — zero new write/sign routes, zero key reads,
loopback only; carries slice-06 KPI-VIEW-2). The baseline is slice-06: every
interaction is a full-page reload today; the full-page byte sizes are the KPI-HX-1
comparison baseline. No new persisted data, no user PII, loopback only.

### DESIGN (2026-06-01)

Morgan locked the slice-07 invariants I-HX-1..5 and authored five ADRs, all framed as
brownfield deltas on the shipped slice-06 viewer. The headline decisions:

- **ADR-031** (htmx asset delivery): htmx **vendored** (2.0.4, 0BSD), `include_str!`-
  embedded, served from one cached `GET /static/htmx.min.js` route with a pinned
  SHA-256 integrity test (`sha256
  e209dda5c8235479f3166defc7750e1dbcd5a5c1808b7792fc2e6733768fb447`). No CDN — offline
  by construction (KPI-HX-G2). The mechanism (vendored static route) was the user
  decision; the ADR records its provenance + rejected alternatives (CDN `<script src>`
  rejected: breaks offline; npm/bundler rejected: no JS toolchain in a Rust workspace).
- **ADR-032** (fragment / full-page split): each region gets a pure
  `render_*_fragment()`; each `render_*_page()` composes the SAME fragment (page =
  chrome + fragment), giving **structural parity** (I-HX-5) — the fragment and the page
  are one source, so they cannot drift.
- **ADR-033** (`HX-Request` shape dispatch in the effect shell): the header is read
  ONCE per route in `adapter-http-viewer` (`Shape::from_request`) and forks
  fragment-vs-page at the render call; the pure core stays header-unaware. One decision
  point per route — no header sniffing scattered through the renderers.
- **ADR-034** (tab-switch history): tabs use `hx-push-url` to keep the REAL URLs, so
  the htmx path converges with the no-JS path (the swapped view is bookmarkable and
  Back works). `#view-panel` wraps `#claims-table` so the tab-swap and the
  pagination-swap coexist on the same page.
- **ADR-035** (acceptance-harness seam): specifies the `get_htmx` / `post_form_htmx`
  seams (set `HX-Request`) + the `is_fragment` / `is_full_page` /
  `references_external_cdn` assertions for driving BOTH shapes through the real binary
  (materialized in DISTILL/DELIVER).

The slice-07 invariants I-HX-1..5: progressive enhancement (swap is a nicety, never a
requirement); offline/no-CDN (htmx local); read-only/no-key (swaps ride GET + the
existing POST `/scrape`; no new write route); no-regression (non-htmx byte-equivalent
beyond the bounded chrome delta); structural fragment/page parity. All INHERIT the
slice-06 I-VIEW-* invariants (read-only, no key in the web process, human gate,
offline store views, loopback-only).

### DISTILL (2026-06-01)

Quinn authored the 30-scenario executable acceptance corpus across two targets:
`viewer_htmx` (24 — the four interaction families + tab-switch, each asserting the
fragment shape under `HX-Request` and the full slice-06 page without it: claims
Prev/Next table fragment, peer-claims Prev/Next, `POST /scrape` results-only fragment
with the form preserved + no sign control + nothing persisted, `GET /claims/{cid}`
detail-panel fragment with the list preserved, and the My↔Peer view-panel fragment +
`hx-push-url` URL update) and `viewer_htmx_invariants` (6 — the GOLD guardrails:
progressive-enhancement no-JS full page, offline + no-CDN, read-only/no-new-write,
no-regression byte-equivalence beyond the chrome delta, structural fragment/page
parity). Built the harness seams per ADR-035 (`get_htmx` / `post_form_htmx` set
`HX-Request`; `is_fragment` / `is_full_page` / `references_external_cdn` probe the
shape) on top of the slice-06 `ViewerServer`.

### DELIVER (2026-06-02)

Executed 15 roadmap steps via DES-monitored crafter dispatches, each commit carrying
a `Step-ID: NN-NN` trailer. Walking skeleton `d53cfe1` (01-01) → final step `14b8f15`
(06-04); the `/scrape` htmx-script defect fix `bcf9007`; L1-L4 refactor `7f78fc1`. Key
per-step SHAs are in `deliver/execution-log.json`.

- **WS / claims paging (01-xx)**: the static route + `Shape::from_request` dispatch +
  the `render_claims_table_fragment()` / `render_claims_page()` parity split; the
  north-star `/claims` Prev/Next swap. The walking-skeleton end-to-end demo.
- **Peer-claims paging (02-xx)**: `render_peer_claims_table_fragment()` /
  `render_peer_claims_page()`, preserving the slice-06 origin (`author_did` +
  `fetched_from_pds`) display in both shapes.
- **Live scrape (03-xx)**: `render_scrape_result_fragment()` — the `POST /scrape`
  results-only fragment (form preserved), reusing the slice-06 `render_scrape_result`
  inside both fragment + page; no sign control, nothing persisted, the slice-06
  `NetworkDown`/`ZeroCandidates` renders carried through.
- **Claim detail (04-xx)**: `render_claim_detail_fragment()` / `render_claim_detail_page()`
  — the detail-panel fragment under htmx, the full detail page without; unknown-CID
  guided render carried through both shapes (ADR-033).
- **Tab switching (05-xx)**: the `#view-panel` wrapper + `render_*_view_panel_fragment()`
  + `render_tab_nav()` with `hx-push-url`; the My↔Peer swap keeps the real URL.
- **Gold invariants + refactor (06-xx)**: the H-INV-* gold guardrails
  (progressive-enhancement / offline-no-CDN / read-only / no-regression / structural
  parity) driving the real binary; the shared `page_head()` / `htmx_script()` helper
  extracted so every page loads the local asset from ONE source (the refactor that also
  fixed the `/scrape` defect — see Lessons).

Refactor / review / mutation / integrity outcomes are in the Quality Gates +
Mutation sections below.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..06 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions, matching slice-02..06 DV-2. The killing properties are kept IN-CRATE (the 50 `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally (no cross-package cargo-mutants scope detour). |
| DV-3 | **htmx VENDORED + `include_str!`-embedded + served from `GET /static/htmx.min.js`, NOT a CDN `<script src>`** (ADR-031). Pinned `sha256 e209dda...8fb447` with a SHA-256 integrity test. | A CDN reference would break the slice-06 offline guarantee (KPI-HX-G2 / I-VIEW-6). Vendoring keeps htmx local; the integrity test makes a silent asset swap a test failure. htmx is a TEXT asset, not a crate — no new crate, no new prod dependency (`sha2` dev-only). |
| DV-4 | **Page composes the same pure fragment function** (ADR-032 / I-HX-5): `render_*_page()` = chrome + `render_*_fragment()`. | Structural parity by construction — the fragment served under htmx and the full page served without it are ONE source, so they cannot drift (no two-renderer divergence bug class). Pinned by the in-crate `*_page_embeds_the_fragment` properties (01-02 RED_UNIT). |
| DV-5 | **`HX-Request` read ONCE in the effect shell** (`Shape::from_request`, ADR-033); the pure core stays header-unaware. | One dispatch point per route keeps the pure renderers header-free (testable in isolation, no HTTP-header coupling smuggled into `viewer-domain`) and keeps the fork auditable in the effect shell. |
| DV-6 | **Fixed a REAL defect: the `/scrape` page was missing the local htmx `<script src>`** so its form swap would not work in a browser. Caught closing Phase 06; fixed in commit `bcf9007`; the test accommodation was then REMOVED. | The `/scrape` page rendered its own head without the shared htmx script tag, so `POST /scrape` would have full-page-reloaded in a real browser instead of swapping (KPI-HX-2 silently broken in the browser, though the harness — which sets `HX-Request` directly — still passed). The fix extracted a shared `page_head()` / `htmx_script()` helper so EVERY page loads the local asset from one source; the temporary test accommodation that masked it was removed. See Lessons. |
| DV-7 | `hx-push-url` on tab switching (ADR-034) + a `#view-panel` wrapping `#claims-table`. | Keeps the REAL URLs after a tab swap (bookmarkable / Back works), converging the htmx path with the no-JS path; the nested wrapper lets the tab-swap and the pagination-swap coexist on one page without target collision. |

## Cardinal release gates + slice-07 invariants

The cardinal release gates are the three KPI-HX guardrails — all release-blocking,
all inheriting the slice-06 contracts:

1. **No-JS no-regression (KPI-HX-G1 / I-HX-4)** — every route serves a complete
   slice-06 full page when `HX-Request` is absent; non-htmx responses byte-equivalent
   to slice-06 beyond the bounded chrome delta (the added htmx script tag + swap
   attributes). Enforced by the slice-06 26-scenario corpus staying GREEN + the
   no-regression gold test.
2. **Offline / no-CDN (KPI-HX-G2 / I-HX-2)** — every store view AND every swap works
   with the network down; zero CDN references. Enforced by the offline harness + the
   `references_external_cdn` HTML scan + the vendored asset + its SHA-256 integrity
   pin (DV-3).
3. **Read-only / no new write surface (KPI-HX-G3 / I-HX-3)** — no swap adds a
   write/sign route; the web process holds no signing key; bind stays loopback-only.
   Carries slice-06 KPI-VIEW-2 (three-layer: `StoreReadPort` no-mutation type + xtask
   viewer capability rule + behavioral gold).

The full slice-07 invariant set (I-HX-1..5; detail in
`docs/feature/viewer-htmx-swaps/design/component-boundaries.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-HX-1 | Progressive enhancement (a swap is a nicety, never a requirement; every interaction works with JavaScript off via the full-page path). | STRUCTURAL (same route serves a full page when `HX-Request` absent) / BEHAVIORAL (H-INV progressive-enhancement gold). |
| I-HX-2 | Offline / no-CDN (htmx vendored + served locally; every store view AND swap works network-down; zero off-host htmx URL). | STRUCTURAL (vendored `include_str!` asset + SHA-256 pin, ADR-031) / BEHAVIORAL (offline harness + `references_external_cdn` scan). Guardrail (KPI-HX-G2). |
| I-HX-3 | Read-only / no new write surface (swaps ride GET + the existing POST `/scrape`; no new write/sign route; no key in the web process; loopback only). | Inherits slice-06 I-VIEW-1/2 (three-layer). Cardinal (KPI-HX-G3 / KPI-VIEW-2). |
| I-HX-4 | No-regression (the non-htmx full page is byte-equivalent to slice-06 beyond the bounded chrome delta; the slice-06 26-scenario corpus stays GREEN). | BEHAVIORAL (slice-06 corpus GREEN + no-regression gold). Guardrail (KPI-HX-G1). |
| I-HX-5 | Structural fragment/page parity (`render_*_page()` composes the SAME pure `render_*_fragment()`; the two shapes are one source and cannot drift). | TYPE/STRUCTURAL (page = chrome + fragment, ADR-032) / property (`*_page_embeds_the_fragment`). |

All slice-07 invariants INHERIT the slice-06 I-VIEW-1..6 set (read-only / no key /
human gate / derived-from honesty / same-store / offline + loopback); confidence stays
shown verbatim (FR-VIEW-8) in both the fragment and the full page.

## Quality gates — final report

- **Acceptance / integration**: 30/30 slice-07 scenarios GREEN (viewer_htmx 24/24,
  viewer_htmx_invariants 6/6); slice-06 26-scenario corpus GREEN — zero regression
  (viewer_store 15/15, viewer_scrape 5/5, viewer_invariants 6/6). Plus 50
  `viewer-domain` unit/property tests. The `ViewerServer` harness drives the REAL
  `openlore ui` over HTTP with the `get_htmx` / `post_form_htmx` + `is_fragment` /
  `is_full_page` / `references_external_cdn` seams (ADR-035).
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new
  capability rule; the slice-06 `viewer-domain` pure-core allowlist (maud) + viewer
  capability rule remain load-bearing and unchanged.
- **Refactor (L1-L4)**: commit `7f78fc1` — clippy + check-arch + check-probes clean;
  `viewer-domain` purity intact (no I/O imports; maud + ports only; the `Shape`
  dispatch lives in the effect shell, not the pure core). The shared `page_head()` /
  `htmx_script()` helper was extracted here (single source for the local asset script
  tag — the move that fixed the `/scrape` defect, DV-6).
- **Adversarial review**: APPROVED, zero blockers, zero Testing Theater. The three
  cardinal guardrails verified load-bearing; the fragment/page parity (I-HX-5)
  confirmed structural (page composes the fragment, DV-4); the `/scrape` htmx-script
  fix (DV-6) confirmed a REAL bug-fix with the test accommodation removed (not theatre).
- **DES integrity**: PASS — "All 15 steps have complete DES traces" (exit 0).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the
`render_*_fragment` / `render_*_page` parity renderers + the `Shape` projection + the
inherited slice-06 render / pagination arithmetic). The slice-04/05 cross-package
lesson stays applied — the 50 `viewer-domain` properties pin the production functions
IN/against the crate, so the per-feature mutation measurement reaches the real killing
suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Unviable | Kill rate |
|---|---:|---:|---:|---:|---|
| `viewer-domain` production logic (fragment/page parity renderers + Shape projection + inherited render/pagination arithmetic) | 72 | 72 | 0 | 5 | **100%** (72/72 viable) |

Slice-07 per-feature gate SATISFIED (≥80%; actual 100% on the production scope, 0
missed). `adapter-http-viewer` is NOT mutated by design (effect shell; covered by the
H-INV gold tests through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **A header-direct harness can hide a browser-only break (DV-6)**: the `/scrape` page
  shipped without the local htmx `<script src>`, so a REAL browser would have
  full-page-reloaded on `POST /scrape` instead of swapping (KPI-HX-2 silently broken in
  the browser). The acceptance harness still passed because it sets `HX-Request`
  DIRECTLY — it never needs the page to load htmx to exercise the fragment path. The
  break was caught closing Phase 06, fixed in `bcf9007` by extracting a shared
  `page_head()` / `htmx_script()` helper (every page now loads the local asset from ONE
  source), and the test accommodation that masked it was removed. **Institutional
  lesson: when a harness simulates the client capability directly (here, the
  `HX-Request` header), it can pass while the real client path is broken; for
  progressive enhancement, at least one gold check must assert the page actually SERVES
  the enhancement asset (the script tag), not just that the server honors the header.**
- **Page-composes-fragment kills the two-renderer drift class (DV-4 / I-HX-5)**: making
  `render_*_page()` literally compose the pure `render_*_fragment()` (page = chrome +
  fragment) means the swapped fragment and the full page are ONE source — there is no
  second renderer to drift out of parity. Lesson: when two output shapes must agree,
  make one structurally CONTAIN the other rather than maintaining two parallel
  renderers kept in sync by tests.
- **Read the dispatch header once, in the shell (DV-5 / ADR-033)**: reading `HX-Request`
  exactly once per route in the effect shell (`Shape::from_request`) kept every pure
  renderer header-unaware and testable in isolation, and kept the fork auditable in one
  place. Lesson: a transport concern (a request header) belongs at the effect edge; do
  not let it leak into the pure core as a render-time parameter scattered across
  functions.
- **Vendoring beats a CDN for an offline-first local tool (DV-3 / ADR-031)**: the
  offline guarantee (KPI-HX-G2, inherited from slice-06 I-VIEW-6) made a CDN `<script
  src>` a non-starter; `include_str!`-embedding htmx + a SHA-256 integrity pin keeps the
  asset local with no new crate and no new production dependency, and turns a silent
  asset swap into a test failure. For a single-file local-first tool this is a net
  simplification, not a cost — the same instinct that picked `hyper` over `axum` in
  slice-05/06.

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-031 left the exact htmx version/pin to materialization. | htmx 2.0.4 (0BSD) vendored; `sha256 e209dda...8fb447` pinned with an integrity test. | Resolved at DELIVER; recorded as DV-3. |
| 2 | ADR-035 specified the harness seam; materialization deferred to DISTILL/DELIVER. | `get_htmx` / `post_form_htmx` + `is_fragment` / `is_full_page` / `references_external_cdn` materialized on the slice-06 `ViewerServer`. | Resolved across DISTILL/DELIVER; ADR-035 status updated to shipped. |
| 3 | DESIGN assumed every page already loaded the htmx asset uniformly. | The `/scrape` page was missing the local htmx `<script src>` (browser-only break); fixed by a shared `page_head()` / `htmx_script()` helper. | Found + fixed within DELIVER; recorded as DV-6; test accommodation removed. |
| 4 | `architecture-design.md` noted `hx-push-url` on paging as a possible FUTURE extension. | Shipped `hx-push-url` on the TAB switch (ADR-034); paging `hx-push-url` remains a documented future option (not built this slice). | Intentional scope; the forward-looking note is left in place. |
| 5 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-2, 100% on production functions, 0 missed). | Recorded. |

## KPI status at GA (slice-07)

| KPI | Type | Status at GA | Note |
|---|---|---|---|
| KPI-HX-1 (north star: paging in-place swap) | leading | per-feature GREEN; bytes-transferred comparison YELLOW | `/claims` + `/peer-claims` Prev/Next return a table-only fragment under `HX-Request` and the full page without it (viewer_htmx GREEN); the fragment-vs-full-page bytes-transferred per-release comparison is the pending DEVOPS leading measure. |
| KPI-HX-2 (scrape in place) | leading | MET (per-feature GREEN) | `POST /scrape` returns a results-only fragment (form preserved) under htmx, full page without; no sign control, nothing persisted (route audit). The browser-path script defect was fixed (DV-6). |
| KPI-HX-3 (detail inline) | leading (secondary) | per-feature GREEN | `GET /claims/{cid}` returns a detail-panel fragment under htmx (list preserved), full detail page without. |
| KPI-HX-4 (tab switch in place, URL updated) | leading (secondary) | per-feature GREEN | My↔Peer returns a view-panel fragment AND updates the URL via `hx-push-url` (bookmarkable / Back works, ADR-034). |
| KPI-HX-G1 (no-JS no-regression) | guardrail | MET (release-blocking) | the slice-06 26-scenario corpus stays GREEN; non-htmx responses byte-equivalent beyond the bounded chrome delta. |
| KPI-HX-G2 (offline / no-CDN) | guardrail | MET (release-blocking) | htmx vendored + served locally with a SHA-256 pin; offline harness + `references_external_cdn` scan green (0 off-host htmx URLs). |
| KPI-HX-G3 (read-only / no new write surface) | guardrail | MET (release-blocking) | zero new write/sign routes; zero key reads; loopback-only bind (carries slice-06 KPI-VIEW-2, three-layer). |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-htmx-swaps/` (discuss/ design/ distill/ deliver/)
- **Parent slice-06 archive** (the read-only viewer this slice enhances):
  `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Slice-07 ADRs**:
  `docs/adrs/ADR-031-htmx-asset-vendored-static-route.md`,
  `docs/adrs/ADR-032-fragment-page-rendering-split.md`,
  `docs/adrs/ADR-033-hx-request-dispatch-in-effect-shell.md`,
  `docs/adrs/ADR-034-tab-switch-hx-push-url-history.md`,
  `docs/adrs/ADR-035-acceptance-harness-hx-request-seam.md`
- **Architecture design / component boundaries / data models / tech stack**
  (kept in the feature workspace): `docs/feature/viewer-htmx-swaps/design/`
- **DELIVER wave decisions**:
  `docs/feature/viewer-htmx-swaps/deliver/wave-decisions.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-htmx-swaps/deliver/execution-log.json`,
  `docs/feature/viewer-htmx-swaps/deliver/roadmap.json`
- **Outcome KPIs (slice-07 rationale)**:
  `docs/feature/viewer-htmx-swaps/discuss/outcome-kpis.md`
- **Vendored htmx asset + static route**:
  `crates/adapter-http-viewer/assets/htmx.min.js` (htmx 2.0.4, 0BSD;
  `sha256 e209dda5c8235479f3166defc7750e1dbcd5a5c1808b7792fc2e6733768fb447`)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
</content>
</invoke>
