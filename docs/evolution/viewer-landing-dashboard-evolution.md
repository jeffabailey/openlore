# Evolution: viewer-landing-dashboard (slice-17 read-only `GET /` landing page — navigation hub + at-a-glance LOCAL store summary on the viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-landing-dashboard/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `distill/`, `deliver/`) and ADR-054 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-06 (`htmx-scraper-viewer` — the read-only viewer foundation this slice extends,
> and the home of the previously-storeless `landing_page` handler), the slices that
> stood up the count reads this slice REUSES (`count_claims` + `count_peer_claims`),
> slice-15 (`viewer-peer-subscriptions` — the source of the **active-only** subscription
> definition `removed_at IS NULL` the new `count_active_peer_subscriptions` read mirrors),
> and slice-16 (`viewer-search-follow-state` — the source of the **`#[cfg(debug_assertions)]`
> test-only fault-seam + the `xtask check-arch` seam guard** pattern this slice reuses and
> extends). Read those parent archives
> (`docs/evolution/htmx-scraper-viewer-evolution.md`,
> `viewer-peer-subscriptions-evolution.md`,
> `viewer-search-follow-state-evolution.md`) for the surfaces this slice composes.
> slice-17 realizes **KPI-VIEW-1** (time-to-see-store-contents) **as the front door** —
> turning the previously-bare `GET /` into the viewer's entry point and closing the
> discoverability gap (only `/claims` was reachable from `/` before).

## Summary

`viewer-landing-dashboard` turns the `openlore ui` read-only viewer's **`GET /`** from a bare
shell into a **navigation hub + at-a-glance LOCAL store summary**. It renders three LOCAL counts
— **own claims**, **peer claims**, and **active peer subscriptions** — and links to **ALL 8
top-level viewer surfaces** (`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`,
`/search`, `/scrape`, `/peers`) via URL constants. Before this slice the 11-surface viewer was
**un-navigable from its own front door** — only `/claims` was reachable from `/`; the landing
page was the only handler that did not even touch the store. slice-17 closes that discoverability
gap and **realizes KPI-VIEW-1 (time-to-see-store-contents) as the front door**, making the viewer
coherent from its entry point: the operator opens `/`, sees what is in their store, and reaches
any surface from one place. It REUSES `count_claims` + `count_peer_claims` and adds ONE new
read-only count, `count_active_peer_subscriptions`.

The load-bearing thesis: **the viewer's front door becomes a coherent entry point that surfaces
LOCAL store state and routes to every surface, while taking on authority over nothing**. `GET /`
reads the LOCAL store read-only and renders plain `<a href>` navigation — no write, no compose,
no sign, no subscribe/follow control. The CARDINAL concerns are four: (1) **read-only / no-key** —
counts + plain nav links only; no executable control of any kind; (2) **LOCAL / offline** — three
LOCAL aggregate reads, no network, vendored htmx only; (3) **missing≠zero** — a failed count
renders the MISSING marker `—`, NEVER a fabricated `0`, and a genuinely-empty store renders an
honest `0`; (4) **no-N+1** — three fixed aggregate reads per render, invariant to store size.
Read-only is enforced at **three layers** (a `StoreReadPort` with no mutation method [TYPE], the
`xtask check-arch` viewer capability rule [STRUCTURAL], and a behavioral GOLD invariant
[BEHAVIORAL]).

The slice ships **ZERO new crates** (workspace stays at **21 members**) and **ZERO new routes**
(it thickens the existing `GET /`). It is an **additive enrichment of an existing handler, not a
re-architecture**: it extends `viewer-domain` (the pure `render_landing` over a new
`LandingSummary` ADT + `render_count` + the `LANDING_HUB_SURFACES` table), `adapter-http-viewer`
(threading the store into the previously-storeless `landing_page` handler + the `.ok()`-degrade
count resolution), the `adapter-duckdb` read impl (ONE new count query), the `ports` (one read
seam + the `LandingSummary` DTO), and the `cli` (`ui` wiring, still no key). The one new read
method is **read-only on the existing `StoreReadPort`**. The defining design choice is the
**Option-shaped `LandingSummary`** — `{ own_claims, peer_claims, active_peers: Option<usize> }` —
which makes a fabricated `0` **unrepresentable by type**: `Some(0)` is an honest empty store,
`None` is a failed read rendered as the missing marker.

### What shipped (one paragraph)

One enriched view — **`GET /`** — that on request resolves three LOCAL counts (`count_claims`,
`count_peer_claims`, and the NEW `count_active_peer_subscriptions`) into the Option-shaped
`LandingSummary` ADT via **independent `.ok()` degrade** (each count degrades on its own; one
failed read never sinks the page), then renders the page with the pure
`render_landing(&LandingSummary)`. Each count is projected via `render_count(Option<usize>)` —
`Some(n)` → the number (a genuine empty store renders `0`), `None` → the `MISSING_COUNT_MARKER`
(`—`) — so a fabricated `0` is unrepresentable. The navigation hub is rendered from a
`LANDING_HUB_SURFACES` table of `(label, URL-const)` pairs covering all 8 top-level surfaces
(`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`);
this slice **minted `SCRAPE_URL`**, the only surface previously lacking a URL const. `GET /` is
**full-page-only** — it does NOT fork by `Shape` (nothing htmx-targets `/`), so page/fragment
parity holds by construction (there is no fragment). The new
`count_active_peer_subscriptions = SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS
NULL` is **count-only** (chosen over `.len()` of the slice-15 active-subscription read for
symmetry with the other two counts + cheapness), read-only on the existing port, and reuses the
slice-15 active-only definition (`removed_at IS NULL`). The store read is **LOCAL and read-only**
(offline, no network); the page references only the vendored local htmx asset; the bind stays
loopback-only; nothing is persisted; the viewer holds no key.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-09 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-09 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-09 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-09 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **8/8 roadmap steps** done across **3 phases** (all COMMIT/PASS in
  `deliver/execution-log.json`).
- **Acceptance scenarios GREEN**: the `viewer_landing_dashboard` corpus (LD- ids —
  including the **thick walking skeleton** at 01-01 driving the summary + 8-surface hub) + the
  GOLD invariants (`viewer_landing_dashboard_invariants` — read-only / no-write, LOCAL / offline,
  N+1-free). Plus the new `adapter-http-viewer` in-crate unit tests (landing render, the route
  `GET /` dispatch, the fault-seam pass-through + Err-injection) and the `viewer-domain`
  unit/property tests (the `LandingSummary` projection + `render_count` + the hub table). The
  `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the store is seeded through the
  REAL verbs.
- **Slices 06/15/16 corpora GREEN — zero regression** (the full workspace acceptance suite green
  across all slices).
- **NO new crate, NO new route**: extends `viewer-domain` (PURE) + `adapter-http-viewer`
  (EFFECT — the previously-storeless `landing_page` handler) + `adapter-duckdb` (EFFECT, read
  impl) + `ports` (the read seam + the `LandingSummary` DTO) + `cli` (DRIVER) in place. Workspace
  member count stays **21**; `cargo xtask check-arch` reports "21 workspace members".
- **NO new production dependency**: `maud`/`hyper` unchanged; no `deny.toml` change.
- **100% mutation kill rate on the genuinely-viable in-diff** (`viewer-domain` 4/4 caught;
  `adapter-http-viewer` 5/5 viable caught after adding 4 in-crate unit tests) — exceeds the ≥80%
  per-feature gate. The 2 remaining cargo-mutants "missed" are the `#[cfg(not(debug_assertions))]`
  release identity sibling of the fault seam (a cfg-dead-branch artifact, same class as slice-16's
  lone survivor), independently guarded.
- **1 ADR** (ADR-054) Accepted/shipped.
- DES integrity: **8/8** steps have complete DES traces.
- Adversarial review: **APPROVED**, 0 defects, zero Testing Theater (the em-dash assertion
  verified falsifiable; the fault seam confirmed release-gated in 3 layers).
- Gates: DoR 9/9, DESIGN APPROVED (genuine value confirmed by a hard-pushed reviewer — the
  11-surface viewer was un-navigable from its front door), DISTILL APPROVED 9.6/10.
- **Release build verified seam-free**: the `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` token is
  ABSENT from the release rlib.
- `cargo xtask check-arch`: OK (21 workspace members; the refactored `scan_viewer_fail_seam_guard`
  now iterates a `VIEWER_FAIL_SEAM_TOKENS` set covering BOTH the slice-16 and slice-17 tokens —
  an ungated read of either fails check-arch).

## Wave-by-wave changelog

### DISCUSS (2026-06-09)

Luna framed the slice as a **brownfield DELTA on slice-06 (the viewer foundation) + the existing
count reads + slices 15/16** that realizes **KPI-VIEW-1 (time-to-see-store-contents) AS THE FRONT
DOOR**. Persona is **P-001 (the node operator)** opening the viewer to answer "what's in my
store?" the moment they land on `/`. The load-bearing DISCUSS decision: **`GET /` is a read-only
NAVIGATION HUB + at-a-glance LOCAL store summary, not a control surface** — it surfaces three
LOCAL counts (own claims, peer claims, active peer subscriptions) and links to all 8 top-level
surfaces, but holds no key and performs no mutation. The CARDINAL framing insight: **before this
slice the 11-surface viewer was un-navigable from its own front door** — only `/claims` was
reachable from `/`, and `landing_page` was the only handler that never touched the store; making
`/` the coherent entry point closes the discoverability gap and is the FRONT-DOOR realization of
the inherited KPI-VIEW-1 north star. The walking skeleton is the thick `/` thread (the store
threaded into `landing_page` + the `LandingSummary` ADT + the three count reads + the
`render_landing` projection + the 8-surface hub + the `ui` wiring), validating the riskiest
assumption first — that the front door can surface LOCAL store state AND route to every surface
while staying read-only and key-free.

### DESIGN (2026-06-09)

Morgan locked slice-17 as an **additive enrichment of an existing handler, not a
re-architecture** — ZERO new crates, ZERO new routes, ZERO new binary, ZERO new architectural
style, ZERO new persisted type. The open decisions were resolved adopting the DISCUSS leans,
captured in one ADR:

- **ADR-054** (landing dashboard — Option-shaped `LandingSummary`, count-only active-subs read,
  nav hub of URL consts, full-page-only): **thread the store into `landing_page`** (the only
  previously-storeless handler), **resolve the three counts via independent `.ok()`** into the
  **Option-shaped `LandingSummary { own_claims, peer_claims, active_peers: Option<usize> }`** — the
  shape makes a fabricated `0` UNREPRESENTABLE (`Some(0)` = honest empty store, `None` = failed
  read). The view is a **NEW pure `viewer-domain` projection** — `render_landing(&LandingSummary)`
  rendering each count via `render_count(Option<usize>)` (`Some` → number / `None` →
  `MISSING_COUNT_MARKER` `—`) and the nav hub via a **`LANDING_HUB_SURFACES` table of
  `(label, URL-const)` pairs** — and **minted `SCRAPE_URL`** (the only surface previously lacking
  a const). `GET /` is **full-page-only** — it does NOT fork by `Shape` (nothing htmx-targets `/`),
  so parity holds by construction (no fragment exists). The NEW read
  **`count_active_peer_subscriptions = SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS
  NULL`** is **count-only** (chosen over `.len()` of the slice-15 active-subscription read for
  symmetry with `count_claims`/`count_peer_claims` + cheapness), read-only on the existing
  `StoreReadPort`, reusing the slice-15 active-only definition. The **2nd fault seam — LD-DEGRADE**
  (a failed peer-claims-count read → `—`, no 5xx) is exercised via a **TEST-ONLY**
  `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` env seam honored ONLY by a `#[cfg(debug_assertions)]`
  function (the `#[cfg(not(debug_assertions))]` release sibling is the identity function, no env
  read compiled in), mirroring ADR-026 / slice-16; the slice-16
  `scan_viewer_fail_seam_guard` xtask guard is **refactored to iterate a `VIEWER_FAIL_SEAM_TOKENS`
  set** covering BOTH the slice-16 (active-set) and slice-17 (peer-claims-count) tokens.

The read-only contract is enforced at THREE layers (a `StoreReadPort` with no mutation method, the
`xtask check-arch` viewer capability rule, and a behavioral GOLD invariant). The C4 views, the `/`
data-flow, and the I-LD-1..n structural-guarantee table are in the DESIGN sections of
`feature-delta.md` and `design/`. DISTILL closed at **APPROVED 9.6/10**.

### DISTILL (2026-06-09)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_landing_dashboard.rs`** (Tier A — `LD-` ids): the **thick walking skeleton** (**LD-WS**
  — the `/` page threading the store into `landing_page`, rendering the three counts via
  `render_landing` + the 8-surface hub via `LANDING_HUB_SURFACES`), the **discoverability /
  URL-const hub** (**LD-DISCOVER / LD-URLCONST** — all 8 top-level surfaces linked from `/` via URL
  consts, `SCRAPE_URL` minted), the **honest-zero** (**LD-ZEROS** — a genuinely-empty store renders
  `0`, not the missing marker), the **read-only / no-write** (**LD-READONLY** — counts + plain
  `<a href>` only; no write/compose/sign/subscribe/follow control), the **missing≠zero degrade**
  (**LD-DEGRADE** — a failed peer-claims-count read renders the `MISSING_COUNT_MARKER` `—`, NEVER a
  fabricated `0`, NO 5xx — driven by the TEST-ONLY `#[cfg(debug_assertions)]` fault seam), the
  **LOCAL / offline aggregate** (**LD-OFFLINE / LD-AGGREGATE** — three LOCAL aggregate reads, no
  network, vendored htmx), and **soft-removed peers excluded** (**LD-SOFTREMOVED** — the active-subs
  count honors `removed_at IS NULL`, a CLI-removed peer is not counted).
- **`viewer_landing_dashboard_invariants.rs`** (gold guardrails — 7 GOLD invariants): **read-only /
  no-write** (no write/compose/sign/subscribe/follow EXECUTABLE control on `/`; store row counts
  unchanged across rich/empty/degraded renders), **LOCAL / offline** (the three counts read the
  LOCAL store with no network; the page references only the vendored local htmx asset, no CDN;
  loopback-only), and **N+1-free** (three FIXED aggregate reads per render, invariant to store size
  — never one-per-row).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is
seeded through the REAL verbs; the LD-DEGRADE fault is driven by the TEST-ONLY
`OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` env seam (honored only under `debug_assertions`). RED
classification: both targets COMPILE green, scenarios FAIL via `todo!()` / unimplemented seam =
MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-09)

Executed **8 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton + discoverability + honest-zero + read-only (01-xx)**:
  **01-01 is the THICK walking skeleton** (**LD-WS**) — the store threaded into `landing_page` +
  the `LandingSummary` ADT + the three count reads (`count_claims` + `count_peer_claims` + the new
  `count_active_peer_subscriptions`) + the `render_landing` projection + the 8-surface
  `LANDING_HUB_SURFACES` hub + the `ui` wiring. **The thick WS drove the summary + hub into
  existence.** **01-02 (LD-DISCOVER / LD-URLCONST)** confirmed all 8 surfaces linked via URL consts
  (`SCRAPE_URL` minted); **01-03 (LD-ZEROS)** the honest empty-store `0`; **01-04 (LD-READONLY)**
  the read-only / no-write surface (counts + plain `<a href>` only).
- **Phase 02 — the genuinely-new fault seam + offline + soft-removed (02-xx)**: **02-01** the
  **LD-DEGRADE** fault seam — **the real implementation work of the slice** (the
  `#[cfg(debug_assertions)]` `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` env seam + the release
  identity sibling + the refactored `scan_viewer_fail_seam_guard` xtask guard now iterating the
  `VIEWER_FAIL_SEAM_TOKENS` set; a failed peer-claims-count read → `—`, no 5xx); **02-02** the
  **LD-OFFLINE / LD-AGGREGATE** (three LOCAL aggregate reads, no network, vendored htmx); **02-03**
  the **LD-SOFTREMOVED** (the active-subs count honors `removed_at IS NULL`, a CLI-removed peer not
  counted).
- **Phase 03 — gold (03-xx)**: **03-01** the **7 GOLD invariants** (read-only / no-write, LOCAL /
  offline, N+1-free). They flipped GREEN off the confirmatory render path.

The 8-step shape: a **thorough WS at 01-01** drove the whole summary + hub thread into existence
for free (the three fixed aggregate reads make N+1-free structural; the `LANDING_HUB_SURFACES`
table makes discoverability structural; full-page-only makes parity structural). The rest were
**unskipped per step** (the scaffolds were `#[ignore]`d) with **LD-DEGRADE — the fault seam + the
xtask guard — the real new implementation work**. A **latent em-dash assertion bug** was caught and
fixed at DELIVER: the LD-DEGRADE missing-marker assertion (`—`) collided with the page-chrome
title "OpenLore — Viewer"; the fix narrowed the assertion to scan the COUNT position rather than
the whole page. **Phase-3 refactor: none needed** — `render_count` was already factored, and the
fault-seam token unification was **correctly declined** to keep each token literal individually
`cfg`-gated for the guard (collapsing them would have defeated the per-token classification).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-LD-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..16 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-LD-2 | Mutation = per-feature 100% on the genuinely-viable in-diff (`viewer-domain` 4/4 caught; `adapter-http-viewer` 5/5 viable caught after adding 4 in-crate unit tests), matching slice-02..16 DV-2. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. The 2 remaining cargo-mutants "missed" are cfg-dead-branch artifacts (see Mutation note), not viable survivors; ≥80%-of-viable gate MET. |
| DV-LD-3 | **Thread the store into `landing_page` — the ONLY previously-storeless handler** (ADR-054). | The bare front door was the one viewer handler that never touched the store; threading it is what makes `/` an at-a-glance store summary AND the prerequisite for surfacing any LOCAL state at the entry point. |
| DV-LD-4 | **Option-shaped `LandingSummary { own_claims, peer_claims, active_peers: Option<usize> }` — a fabricated `0` is UNREPRESENTABLE by type** (`Some(0)` = honest empty store, `None` = failed read → `—`) (ADR-054). | The whole missing≠zero cardinal rides on the TYPE: an `Option<usize>` per count forces the render to distinguish "honest empty store (`0`)" from "failed read (`—`)" — there is no way to type a failed read as `0`. |
| DV-LD-5 | **Per-count INDEPENDENT `.ok()` degrade — one failed count never sinks the page; `GET /` never 5xx** (ADR-054). | A front door that 500s because one of three counts failed is worse than an honest partial summary; degrading each count independently to `—` keeps the entry point resilient and the operator oriented (LD-DEGRADE). |
| DV-LD-6 | **`render_count(Option<usize>)` projects `Some(n)` → number / `None` → `MISSING_COUNT_MARKER` (`—`); a genuine empty store renders `0`** (ADR-054). | One render fn owns the missing≠zero distinction at the projection site — `0` and `—` are never conflated, and the marker is a single SSOT constant rather than a scattered literal. |
| DV-LD-7 | **The nav hub is rendered from a `LANDING_HUB_SURFACES` table of `(label, URL-const)` pairs covering ALL 8 top-level surfaces; `SCRAPE_URL` was minted** (ADR-054). | Driving the hub off a table of URL CONSTS (not hand-written hrefs) makes the discoverability guarantee structural — every top-level surface is reachable from `/`, and a new surface is one table row; minting `SCRAPE_URL` closed the one surface lacking a const. |
| DV-LD-8 | **`GET /` is full-page-only — it does NOT fork by `Shape`** (nothing htmx-targets `/`) (ADR-054). | Forking a route that nothing htmx-targets would add a phantom fragment + a parity obligation for no benefit; full-page-only keeps `/` simple and makes page/fragment parity hold by construction (there is no fragment). |
| DV-LD-9 | **`count_active_peer_subscriptions = SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL` — count-only, read-only on the existing port, reusing the slice-15 active-only definition** (chosen over `.len()` of the slice-15 active-subscription read, for symmetry + cheapness) (ADR-054). | The other two summary cells are counts; a count-only read keeps the three symmetric and cheap (no row materialization), and reusing `removed_at IS NULL` keeps ONE active-subscription definition workspace-wide (slice-15) — a CLI-removed peer is not counted (LD-SOFTREMOVED). |
| DV-LD-10 | **The 2nd fault seam (LD-DEGRADE) is TEST-ONLY: the `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` token is honored ONLY by a `#[cfg(debug_assertions)]` function; the `#[cfg(not(debug_assertions))]` release sibling is the identity function (NO env read compiled in)** — release build verified seam-free, token ABSENT from the rlib (ADR-054, mirroring ADR-026 / slice-16). | LD-DEGRADE needs a deterministic mid-request count failure, but a fault hook compiled into release is a production liability; gating the env read behind `debug_assertions` keeps the release binary seam-free while the debug profile drives the degrade scenario. |
| DV-LD-11 | **The slice-16 `scan_viewer_fail_seam_guard` xtask guard was REFACTORED to iterate a `VIEWER_FAIL_SEAM_TOKENS` set covering BOTH the slice-16 (active-set) and slice-17 (peer-claims-count) tokens** — an ungated read of EITHER fails check-arch (ADR-054). | A second `#[cfg]`-gated seam must be structurally guarded too, or the second token could be read outside its gate undetected; generalizing the guard to a TOKEN SET keeps the cfg-gate enforcement in ONE place and extends to every future seam by adding a set entry. |
| DV-LD-12 | **The fault-seam degrade is independently pinned at three layers**: the debug seam's pass-through + Err-injection is pinned by the new `adapter-http-viewer` unit tests; the release identity sibling is pinned by the xtask seam guard + the release-build seam-free check (ADR-054). | The 2 cargo-mutants "missed" land on the release identity sibling (not compiled under the debug test profile, so neither reachable nor genuinely viable); the debug twin is killed by the in-crate tests, and the release sibling is structurally pinned — so the cfg-dead-branch artifact is covered without theatre. |
| DV-LD-13 | **Phase-3 refactor: none needed — `render_count` was already factored; the fault-seam token unification was DECLINED to keep each token literal individually `cfg`-gated for the guard.** | Collapsing the two seam tokens into one shared abstraction would have defeated the per-token classification the `VIEWER_FAIL_SEAM_TOKENS` guard relies on; declining the merge keeps each literal individually guardable — the right call, not a missed refactor. |
| DV-LD-14 | **A latent em-dash assertion bug was caught + fixed**: the LD-DEGRADE `—` assertion collided with the page-chrome title "OpenLore — Viewer"; narrowed to scan the COUNT position. | An assertion that matches the missing marker anywhere on the page passes even when the count is fine (the chrome always contains `—`) — narrowing it to the count position makes the test genuinely falsifiable (review confirmed). |

## Cardinal release gates + slice-17 invariants (I-LD-1..n)

The cardinal release gates realized on the landing surface — all release-blocking:

1. **Read-only / no key (CARDINAL, I-LD-1)** — `GET /` is a READ; counts + plain `<a href>` nav
   only; no write/compose/sign/subscribe/follow EXECUTABLE control; the web process holds no
   signing key; the read seam has NO mutation method (type-level). Three-layer: TYPE (no write
   method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only /
   no-write).
2. **Missing≠zero (CARDINAL, I-LD-2)** — the Option-shaped `LandingSummary` makes a fabricated `0`
   unrepresentable: `Some(0)` (honest empty store) renders `0`, `None` (failed read) renders the
   `MISSING_COUNT_MARKER` `—`; each count degrades independently via `.ok()`, `GET /` never 5xx
   (LD-DEGRADE + LD-ZEROS).
3. **Discoverability / URL-const hub (CARDINAL, I-LD-3)** — all 8 top-level surfaces (`/claims`,
   `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`) are linked
   from `/` via the `LANDING_HUB_SURFACES` table of URL consts (`SCRAPE_URL` minted); the front
   door is the navigable entry point (LD-DISCOVER / LD-URLCONST).
4. **No-N+1 (CARDINAL, I-LD-4)** — three FIXED aggregate reads per render, invariant to store size,
   never one-per-row (the N+1-free gold).
5. **LOCAL / offline (I-LD-5)** — the three counts read the LOCAL store with no network (fully
   offline); the page references only the vendored local htmx asset (no CDN); loopback-only bind;
   nothing persisted (the offline gold).
6. **Active-only counted (I-LD-6)** — `count_active_peer_subscriptions` honors `removed_at IS NULL`
   (reusing the slice-15 active-only definition); a CLI-removed peer is not counted (LD-SOFTREMOVED).
7. **Full-page-only / parity by construction (I-LD-7)** — `GET /` does NOT fork by `Shape` (nothing
   htmx-targets `/`); there is no fragment, so page/fragment parity holds by construction.
8. **Fault seam release-gated (I-LD-8)** — the LD-DEGRADE fault trigger
   (`OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`) is honored ONLY by a `#[cfg(debug_assertions)]`
   function; the release sibling is the identity function (no env read compiled in); release build
   verified seam-free (token absent from the rlib); the `VIEWER_FAIL_SEAM_TOKENS` xtask guard fails
   any ungated read.

| # | Invariant | Enforcement |
|---|---|---|
| I-LD-1 | Read-only / no key (`GET /` is a READ; counts + plain `<a href>` nav only; no executable write/compose/sign/subscribe/follow control; no key in the process; the read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-write, DV-LD-3). Cardinal. |
| I-LD-2 | Missing≠zero (Option-shaped `LandingSummary` makes a fabricated `0` unrepresentable; `Some(0)` → `0`, `None` → `—`; per-count independent `.ok()` degrade; never 5xx). | TYPE (`active_peers: Option<usize>`, the per-count Option shape, DV-LD-4) + STRUCTURAL (`render_count` `Some`/`None` split + the `MISSING_COUNT_MARKER` SSOT, DV-LD-5/6) + BEHAVIORAL (LD-DEGRADE + LD-ZEROS). Cardinal. |
| I-LD-3 | Discoverability / URL-const hub (all 8 top-level surfaces linked from `/` via the `LANDING_HUB_SURFACES` table of URL consts; `SCRAPE_URL` minted). | STRUCTURAL (the `(label, URL-const)` table, DV-LD-7) + BEHAVIORAL (LD-DISCOVER / LD-URLCONST). Cardinal. |
| I-LD-4 | No-N+1 (three FIXED aggregate reads per render, invariant to store size, never one-per-row). | STRUCTURAL (the three fixed count reads, DV-LD-9) + BEHAVIORAL (N+1-free gold). Cardinal. |
| I-LD-5 | LOCAL / offline (the three counts read the LOCAL store with no network; no-CDN chrome; loopback-only; nothing persisted). | STRUCTURAL (the read-only local aggregate counts; the shared `htmx_script` fn + pinned asset; loopback guard unchanged) + BEHAVIORAL (LD-OFFLINE / LD-AGGREGATE gold). |
| I-LD-6 | Active-only counted (`count_active_peer_subscriptions` honors `removed_at IS NULL`; a CLI-removed peer is not counted). | STRUCTURAL (the `WHERE removed_at IS NULL` filter, reusing the slice-15 active-only definition, DV-LD-9) + BEHAVIORAL (LD-SOFTREMOVED). |
| I-LD-7 | Full-page-only / parity by construction (`GET /` does not fork by `Shape`; no fragment exists; nothing htmx-targets `/`). | STRUCTURAL (the single full-page render, no `Shape` fork, DV-LD-8). |
| I-LD-8 | Fault seam release-gated (LD-DEGRADE trigger honored ONLY by `#[cfg(debug_assertions)]`; release sibling = identity; release build seam-free; the `VIEWER_FAIL_SEAM_TOKENS` xtask guard fails any ungated read). | TYPE/COMPILE (the `#[cfg(debug_assertions)]` gate; the release identity sibling, DV-LD-10) + STRUCTURAL (the refactored `scan_viewer_fail_seam_guard` over the token set, DV-LD-11; release-build seam-free check) + BEHAVIORAL (the in-crate seam pass-through + Err-injection tests, DV-LD-12). Cardinal. |

All slice-17 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets (read-only /
no key / human gate / offline + loopback / progressive enhancement / structural fragment/page
parity); each count is shown verbatim through the single full-page render.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_landing_dashboard` corpus (LD-WS, LD-DISCOVER /
  LD-URLCONST, LD-ZEROS, LD-READONLY, LD-DEGRADE, LD-OFFLINE / LD-AGGREGATE, LD-SOFTREMOVED — the
  thick walking skeleton at LD-WS) + the GOLD `viewer_landing_dashboard_invariants` (read-only /
  no-write, LOCAL / offline, N+1-free) GREEN + the `viewer-domain` unit/property tests (the
  `LandingSummary` projection + `render_count` + the hub table) + the new `adapter-http-viewer`
  in-crate unit tests (landing render, route `GET /` dispatch via a loopback wiring test, the
  fault-seam pass-through + Err-injection); slices 06/15/16 corpora GREEN — zero regression. The
  `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the LD-DEGRADE fault is driven by
  the TEST-ONLY `#[cfg(debug_assertions)]` env seam.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, no new
  allowlist edge: `render_landing` is a total fn of the flat `LandingSummary` DTO (no
  `viewer-domain → claim-domain` reachout). The viewer capability rule is unchanged (read-only
  counts; no signing/identity/PDS, no store-write). The refactored `scan_viewer_fail_seam_guard`
  now iterates `VIEWER_FAIL_SEAM_TOKENS` (slice-16 active-set + slice-17 peer-claims-count); an
  ungated read of EITHER fails check-arch.
- **Refactor (L1-L4)**: clippy + check-arch clean; **Phase-3 refactor: none needed** (`render_count`
  already factored; the fault-seam token unification was correctly DECLINED to keep each token
  literal individually `cfg`-gated for the guard, DV-LD-13); `viewer-domain` purity intact (no I/O
  imports; maud + ports only; the store threading + `.ok()` degrade live in the effect shell; NO
  `claim-domain` reachout).
- **Release-build seam check**: the `#[cfg(not(debug_assertions))]` release build was verified
  **seam-free** — the `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` token is NOT compiled into the
  release rlib (the release sibling is the identity function, no env read).
- **Adversarial review**: **APPROVED**, **0 defects, zero Testing Theater**. The genuine value
  confirmed by a hard-pushed reviewer (the 11-surface viewer was un-navigable from its front door —
  not padding); the **em-dash assertion verified falsifiable** (narrowed to the count position
  after the page-chrome collision fix, DV-LD-14); the **fault seam confirmed release-gated in 3
  layers** (the `#[cfg(debug_assertions)]` gate + the xtask token-set guard + the release-build
  seam-free check); the missing≠zero confirmed type-load-bearing (the Option shape, DV-LD-4); the
  no-N+1 confirmed structural (three fixed aggregate reads, DV-LD-9).
- **DES integrity**: PASS — all 8 steps have complete DES traces (8/8).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the `LandingSummary` projection +
`render_count` + the `LANDING_HUB_SURFACES` hub render) AND the `adapter-http-viewer` slice-17
logic (the store-threaded `landing_page` handler + the `.ok()` count resolution + the fault-seam
pass-through). The slice-04/05 cross-package lesson stays applied — the killing properties are kept
IN-CRATE.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`LandingSummary` projection + `render_count` + hub render, in-diff) | 4 | 4 | 0 | **100%** (4/4 in-diff viable) |
| `adapter-http-viewer` slice-17 logic (store-threaded `landing_page` + `.ok()` resolution + fault-seam pass-through, in-diff) | 5 | 5 | 0 | **100%** (5/5 viable, after the 4 in-crate unit tests) |

Slice-17 per-feature gate SATISFIED (≥80%; actual **100% on the genuinely-viable in-diff**, 0
viable missed). **The 2 remaining cargo-mutants "missed" are both the `#[cfg(not(debug_assertions))]`
release identity sibling of the fault seam** — a **cfg-dead-branch artifact** NOT compiled under the
debug test profile (neither reachable nor genuinely viable), the **same class as slice-16's lone
survivor**. They are **independently pinned** by the `VIEWER_FAIL_SEAM_TOKENS` xtask guard + the
release-build seam-free check (DV-LD-12) — covered without theatre.

**Closing the package-scoped harness gap**: the slice-17 `adapter-http-viewer` logic is
acceptance-covered in the `cli` package and therefore invisible to `cargo mutants -p
adapter-http-viewer` (the package-scoped harness cannot see cross-package acceptance tests). Four
**in-crate `adapter-http-viewer` unit tests** were added (the `landing_page` render, the route
`GET /` dispatch via a loopback wiring test, the fault-seam pass-through + the Err-injection) to
close that gap — **mirroring the slice-16 adapter-unit-test precedent** — so the 5 viable adapter
mutants are caught locally. `adapter-duckdb` is NOT mutated by design (effect shell; covered by the
GOLD invariants through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **A thorough walking skeleton drove the summary + hub into existence for free**: the LD-WS WS
  threaded the store into `landing_page`, resolved the three counts into the Option-shaped
  `LandingSummary`, and rendered the 8-surface hub on day one — so N+1-free (three fixed reads),
  discoverability (the URL-const table), and full-page parity (no fragment) became structural, and
  the rest of the slice (honest-zero, read-only, offline, soft-removed, gold) was confirmatory.
  **Lesson: when a slice's cardinals are SHAPE decisions (an Option-typed summary, a fixed set of
  aggregate reads, a table-driven hub), get the shape right inside the walking skeleton and most
  downstream scenarios become confirmation of the structure rather than new work.**
- **An Option-shaped summary makes the missing≠zero cardinal a TYPE guarantee, not a convention
  (DV-LD-4)**: typing each count as `Option<usize>` (and routing through `render_count`) makes a
  fabricated `0` UNREPRESENTABLE — `Some(0)` is an honest empty store, `None` is a failed read
  rendered `—`, and there is no third way to type a failure as zero. **Lesson: when "missing must
  not read as zero" is a cardinal, encode the absence in the TYPE (`Option`) at the port boundary
  rather than relying on a sentinel value or a render-site convention — the type makes the
  fabricated zero impossible to write.**
- **A table of `(label, URL-const)` pairs makes discoverability structural and minted the one
  missing const (DV-LD-7)**: driving the hub off `LANDING_HUB_SURFACES` (URL consts, not
  hand-written hrefs) means every top-level surface is reachable from `/` by construction, a new
  surface is one table row, and the audit surfaced `SCRAPE_URL` as the only surface lacking a
  const. **Lesson: when "every surface must be reachable from the entry point" is the value, render
  the hub from a TABLE of typed URL consts — discoverability becomes a structural property and the
  table audit catches any surface missing a const.**
- **A second `#[cfg]`-gated fault seam means GENERALIZING the xtask guard to a token SET, not
  copying it (DV-LD-10/11)**: LD-DEGRADE reused the slice-16 `#[cfg(debug_assertions)]` seam pattern
  but added a second token (`OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`); the slice-16
  `scan_viewer_fail_seam_guard` was refactored to iterate `VIEWER_FAIL_SEAM_TOKENS` so an ungated
  read of EITHER fails check-arch, and the release build was verified seam-free. **Lesson: when a
  second test-only seam joins an existing one, generalize the structural guard to a TOKEN SET so it
  extends to every future seam by one entry — and verify the release binary is seam-free, don't just
  trust the cfg gate.**
- **Declining a refactor is a decision, not a gap (DV-LD-13)**: the obvious "DRY" move was to unify
  the two fault-seam tokens behind one abstraction, but that would have defeated the per-token
  classification the `VIEWER_FAIL_SEAM_TOKENS` guard relies on; keeping each literal individually
  `cfg`-gated was the right call. **Lesson: a refactor that would collapse the very distinction a
  structural guard depends on is one to DECLINE explicitly — record the non-refactor as a decision
  so it doesn't look like an oversight.**
- **An assertion that matches the missing marker anywhere on the page is a non-falsifiable bug
  (DV-LD-14)**: the LD-DEGRADE `—` assertion collided with the page-chrome title "OpenLore — Viewer"
  — it passed even when the count rendered fine. Narrowing it to scan the COUNT position made it
  genuinely falsifiable. **Lesson: when asserting a marker that ALSO appears in shared chrome,
  anchor the assertion to the specific position under test — a page-wide substring match on a
  marker the chrome already contains is testing theatre.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-054 fixed the contracts; field-level shaping (the `LandingSummary` Option shape, `render_count`, the `LANDING_HUB_SURFACES` table, the full-page-only render) left to DELIVER. | All adopted; the Option-shaped `LandingSummary`, `render_count` (`Some`→number / `None`→`—`), the 8-surface URL-const hub, and the full-page-only render materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-054 fixed the new count-only read `count_active_peer_subscriptions = SELECT COUNT(*) ... WHERE removed_at IS NULL` (over `.len()` of the slice-15 read). | Shipped exactly — count-only, read-only on the existing port, reusing the slice-15 active-only definition; a CLI-removed peer not counted (LD-SOFTREMOVED green). | Resolved at DELIVER. |
| 3 | ADR-054 fixed the 2nd fault seam as TEST-ONLY (`#[cfg(debug_assertions)]`), the release sibling = identity, and the xtask guard generalized to a token set. | The `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` seam landed `#[cfg(debug_assertions)]`-only; `scan_viewer_fail_seam_guard` refactored to iterate `VIEWER_FAIL_SEAM_TOKENS`; release build verified seam-free (token absent from the rlib). | Resolved at DELIVER. |
| 4 | ADR-054 fixed "no new pure-core edge; check-arch unchanged (member count, allowlist)." | `render_landing` is a total fn of the flat `LandingSummary` DTO; `check-arch` reports 21 members, no new allowlist edge, no new route. | Resolved at DELIVER. |
| 5 | `SCRAPE_URL` expected to be minted as the one missing surface const. | `SCRAPE_URL` minted; the `LANDING_HUB_SURFACES` table covers all 8 surfaces via URL consts. | Resolved at DELIVER. |
| 6 | Phase-3 refactor anticipated (per the slice-15 precedent). | NONE needed — `render_count` already factored; the fault-seam token unification was DECLINED to keep each literal individually `cfg`-gated (DV-LD-13). | No refactor at DELIVER (a deliberate non-refactor). |
| 7 | The LD-DEGRADE missing-marker assertion expected to assert the `—` marker. | A latent em-dash assertion bug (collision with the page-chrome title "OpenLore — Viewer") was caught + fixed to scan the COUNT position. | Fixed at DELIVER (DV-LD-14); assertion now falsifiable. |
| 8 | `adapter-http-viewer` mutation coverage expected via the cli-package acceptance suite. | The package-scoped harness gap was closed with 4 in-crate `adapter-http-viewer` unit tests (mirroring slice-16); 5/5 viable adapter mutants caught. | Closed at DELIVER (DV-LD-12). |
| 9 | Review expected to pass clean. | Review APPROVED, 0 defects, zero Testing Theater; genuine value confirmed by a hard-pushed reviewer. | Confirmed at DELIVER. |
| 10 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-LD-2, 100% viable in-diff, 0 viable missed; the 2 "missed" are cfg-dead-branch artifacts). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-landing-dashboard/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, requirements, user-stories,
  acceptance-criteria, outcome-kpis, dor-checklist), `design/`, `distill/`, `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer foundation this slice extends, home of the
  previously-storeless `landing_page` handler): `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-15 archive** (the active-only `removed_at IS NULL` subscription definition the new
  count reuses): `docs/evolution/viewer-peer-subscriptions-evolution.md`
- **Parent slice-16 archive** (the `#[cfg(debug_assertions)]` test-only fault-seam + the
  `scan_viewer_fail_seam_guard` xtask guard pattern this slice reuses + generalizes):
  `docs/evolution/viewer-search-follow-state-evolution.md`
- **Slice-17 ADR**:
  `docs/adrs/ADR-054-landing-dashboard-option-shaped-landingsummary-count-only-active-subs-read-nav-hub-of-url-consts-full-page-only.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-landing-dashboard/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-landing-dashboard/deliver/execution-log.json`,
  `docs/feature/viewer-landing-dashboard/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_landing_dashboard.rs` (LD-WS, LD-DISCOVER / LD-URLCONST, LD-ZEROS,
  LD-READONLY, LD-DEGRADE, LD-OFFLINE / LD-AGGREGATE, LD-SOFTREMOVED — the thick walking skeleton at
  LD-WS), `tests/acceptance/viewer_landing_dashboard_invariants.rs` (the 7 gold invariants —
  read-only / no-write, LOCAL / offline, N+1-free)
- **Reused fault-seam + xtask-guard pattern**: `xtask` (`scan_viewer_fail_seam_guard`, refactored to
  iterate `VIEWER_FAIL_SEAM_TOKENS` — slice-16 active-set + slice-17 peer-claims-count); the
  ADR-026 pubkey-seam release-gate pattern (`classify_cfg_gated_token`)
- **Extended viewer crates**: `crates/viewer-domain` (the `LandingSummary` projection +
  `render_landing` + `render_count` + the `MISSING_COUNT_MARKER` constant + the
  `LANDING_HUB_SURFACES` URL-const table + the minted `SCRAPE_URL`), `crates/adapter-http-viewer`
  (the store threaded into `landing_page` + the per-count `.ok()` degrade resolution + the
  `#[cfg(debug_assertions)]` fault seam + the release identity sibling + the 4 in-crate unit tests),
  `crates/adapter-duckdb` (the read-only `count_active_peer_subscriptions` impl — `SELECT COUNT(*)
  FROM peer_subscriptions WHERE removed_at IS NULL`), `crates/ports` (the read seam + the
  `LandingSummary` DTO)
- **Reused count reads (NOT re-implemented)**: `count_claims` + `count_peer_claims` on the existing
  `StoreReadPort`
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml` — KPI-VIEW-1
  (time-to-see-store-contents), realized as the front door + the discoverability funnel
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-list-flags-evolution.md`,
  `viewer-counter-claim-threads-evolution.md`, `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`, `viewer-peer-subscriptions-evolution.md`,
  `viewer-search-follow-state-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS 48e7677 → DESIGN 9cef22d → DISTILL 6781bae → roadmap (post-6781bae) → 01-01 9d73529 →
01-02 bcc5783 → 01-03 e7ece11 → 01-04 d2beddd → 02-01 9697117 → 02-02 7a24691 → 02-03 fd09cd0 →
03-01 4a4d00e → mutation-gate unit tests ceb8473.
