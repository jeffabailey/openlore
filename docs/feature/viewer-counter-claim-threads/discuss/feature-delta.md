<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-claim-threads

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a DELTA on the existing read-only `GET /claims/{cid}` view of the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-CT-002 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07)
> JTBD: YES — every story traces to **J-003b** (`docs/product/jobs.yaml`, sub-job of J-003); no new job created
> Brownfield DELTA on: `htmx-scraper-viewer` (slice-06), `viewer-htmx-swaps` (slice-07), reusing the slice-03 counter-claim model (ADR-015) + the slice-08 "countered by" annotation precedent
> Date: 2026-06-06 · Owner: Luna (nw-product-owner)
> Slice: slice-11

This file is the canonical DISCUSS-wave delta for `viewer-counter-claim-threads`
(slice-11): a **counter-claim thread** surface added to the read-only `openlore ui`
viewer. The existing `GET /claims/{cid}` route is extended so that, BENEATH the
original claim, ALL counter-claims targeting that CID are rendered as a thread —
each with its own author DID, its own CID, and its verbatim free-text `--reason`.
The original claim is rendered VERBATIM with its original confidence; the counters
are SHOWN, never applied. It is the BROWSER VIEWING side of **J-003b**
("counter-claim authoring as first-class disagreement") — authoring already ships
EXCLUSIVELY via the CLI `claim counter --reason <REASON> <CID>` (slice-03).

This is a DELTA. It REUSES the slice-03 counter-claim domain model (a counter is an
ordinary signed claim with `references[].type == counters` + a mandatory `reason`,
ADR-015), the slice-06/07 page=chrome+fragment render pattern, and the slice-08
"countered by" annotation precedent. It adds exactly ONE new read capability — a
read-only `query_counter_claims(target_cid)` method on `StoreReadPort` over the
LOCAL `claims ∪ peer_claims` tables (no network on this route). Tier-1 content is
inlined here (lean); SSOT lives under `docs/product/`; per-journey/registry
artifacts under `discuss/`.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME
persona as slices 06/07/08/09/10 (`docs/product/personas/senior-engineer-solo-builder.yaml`).
She lives in a terminal but runs `openlore ui` to GLANCE at her store in a browser
(slice-06), navigate it without reloads (slice-07), search the network (slice-08),
read transparent scores (slice-09), and traverse the graph (slice-10). slice-11
extends that same read-only viewer with a counter-claim-reading hat: when she opens
a claim, she now sees the DISAGREEMENT around it — who countered it, with what CID,
and exactly why (the verbatim reason) — rendered as a thread beneath the claim.

slice-03 framed P-002 (Researcher/Tech Lead, federation-reader hat) as primary for
the CLI counter-claim AUTHORING job. The BROWSER viewer's operator, though, is P-001
(the viewer is her surface, slices 06–10). She wears a counter-claim-reader hat at
her own loopback viewer. UX guardrails inherited verbatim: read-only, never silently
mutate, attribution always visible (no merged consensus), confidence display must
NEVER read as "the system thinks this is true."

### Counter-claim-reader hat (NEW — slice-11)

P-001 wearing the counter-claim-reader hat is reading a claim in the browser viewer
and wants to instantly see whether anyone has disagreed with it — and if so, who,
with what reasoning — WITHOUT the disagreement ever silently changing the claim she
is reading. The thread makes disagreement legible; the claim stays sovereign.

- **Load-bearing anxieties**: "Am I reading a claim that someone has already
  refuted, and the viewer is hiding it from me?" · "Will the counter overwrite or
  re-weight the original, picking a winner for me?" · "Can I tell whose counter this
  is and read their actual reasoning, or just a faceless 'disputed' badge?"
- **Load-bearing signals of success**: "The countered claim is clearly FLAGGED as
  disputed, AND its original confidence/text is untouched." · "Each counter shows
  its author DID, its own CID, and the full verbatim reason." · "An un-countered
  claim looks exactly like it does today — no empty 'no disagreement' noise."

> A new hat entry is appended to `docs/product/personas/senior-engineer-solo-builder.yaml`
> by this DISCUSS wave (changelog 2026-06-06).

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003b**: *When a peer publishes a claim I disagree with, I want to publish a
> counter-claim that stands on its own — not a reply on their record — so
> disagreement is a public structured artifact rather than a thread.*
> (`docs/product/jobs.yaml`, sub-job of **J-003**, opportunity score 15,
> `walking_skeleton_for: openlore-federated-read`.)

slice-11 realizes the **VIEWING / LEGIBILITY half** of J-003b. The AUTHORING half
("publish a counter-claim that stands on its own") already shipped in slice-03 via
the CLI `claim counter`. But a disagreement artifact that is only barely visible
(slice-08's one-line "countered by <author> (<cid>)" annotation on the network
/search view) is not yet fully LEGIBLE: you cannot, on the LOCAL viewer, drill into
a claim and READ the disagreement — the author DID, the CID, and the verbatim
reason, side by side with the original. slice-11 closes that gap.

No new job. No new sub-job. Every story below traces to J-003b (with the J-003a /
J-003c boundaries made explicit in the JTBD-trace section).

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Every story → J-003b. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | J-003a (attribute every peer claim without merging) is REINFORCED — each counter is attributed to its own author DID + CID, never merged. J-003c (revocable subscription) is untouched — purging a peer removes its counters from the thread by construction (they live in `peer_claims`); the operator's OWN counters (in `claims`) persist, which is correct (they are her claims, not the peer's). |
| No contradiction with cardinal invariants? | PASS | Shown-never-applied (OD-AV-7 / I-NS-3 / ADR-015) is HONORED — the countered claim is rendered verbatim with original confidence; counters never filter/merge/re-weight it. Read-only (KPI-VIEW-2), anti-merging (KPI-AV-2 / KPI-GRAPH-2 / KPI-FED-1), verbatim confidence (KPI-4), local-first (KPI-5) all carry forward. |
| Authoring NOT re-introduced on the viewer? | PASS | This slice adds ZERO write/sign/counter controls. Authoring stays EXCLUSIVELY in the CLI (I-VIEW-3). The viewer never offers a "reply / counter" button; it only RENDERS counters that already exist. |
| Job already fully served? | NO (gap is real) | The local viewer cannot today show a claim's counter thread. The slice-08 annotation is network-/search-only and one-line; the local `/claims/{cid}` detail shows the claim alone. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting DELTA.

---

## Wave: DISCUSS / [REF] JTBD trace (every story → J-003b, with boundaries)

| Story | Title | job_id | Sub-job realized | Boundary note |
|---|---|---|---|---|
| US-CT-001 | Read-only counter-claim thread READ capability in the viewer process | `infrastructure-only` | (enables US-CT-002/003) | `infrastructure_rationale` below. NOT a J-003a/c story. |
| US-CT-002 | See the counter-claim thread beneath a countered claim | J-003b | J-003b (VIEW half) + J-003a (each counter attributed to its own DID + CID, never merged) | NOT J-003c (no subscription change); NOT the authoring half of J-003b (that is the slice-03 CLI). |
| US-CT-003 | An un-countered claim renders exactly as today (no empty-thread noise) + a countered claim is flagged | J-003b | J-003b (the legibility flag + the no-noise discipline) | NOT J-002c (no scoring); the flag is a presence marker, never a weight or verdict. |

**J-003a / J-003b / J-003c boundary statement (explicit per the brief):**

- **J-003a** (attribute every peer claim without merging) is the cardinal
  anti-merging invariant. slice-11 INHERITS and REINFORCES it: every counter in a
  thread carries its own author DID + CID; two counters are never collapsed; the
  countered claim and its counters are never merged into a "consensus" or
  "net verdict" row. slice-11 mints NO J-003a story — it carries the invariant.
- **J-003b** (counter-claim authoring as first-class disagreement) is THIS slice's
  job — specifically the VIEWING half. The AUTHORING half (the `claim counter`
  CLI verb) shipped in slice-03 and is explicitly OUT of scope here.
- **J-003c** (subscription revocable without residue) is untouched. slice-11 adds
  no subscription surface. Purge semantics are inherited unchanged: a purged peer's
  counters vanish from threads because they lived in `peer_claims`; the operator's
  own counters persist because they are her own claims.

### Infrastructure rationale (US-CT-001)

US-CT-001 carries `job_id: infrastructure-only` with this rationale: it adds the
read-only `query_counter_claims(target_cid)` capability to `StoreReadPort` (+ its
`adapter-duckdb` read impl) and the pure `viewer-domain` thread view-model — the
plumbing US-CT-002/003 consume. It produces no user-visible output on its own (no
new route, no rendered page), so it enables a user decision only THROUGH US-CT-002.
The slice contains TWO non-infrastructure, user-visible stories (US-CT-002, US-CT-003),
so the slice has release value (Dimension-0 slice-level check passes).

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

These are RESTATED as binding commitments for slice-11 (inherited, not re-litigated):

| ID | Commitment | Source |
|---|---|---|
| I-CT-1 (= I-VIEW-1/2/3) | **Read-only**: the counter-thread route holds `StoreReadPort` only — no mutation method, no signing key in the viewer process, no write/sign/counter control on any rendered surface. Authoring stays EXCLUSIVELY in the CLI. Enforced 3 layers (type system: `query_counter_claims` is read-only on a no-mutation trait + xtask check-arch viewer capability rule + behavioral gold). | KPI-VIEW-2, slice-06/07 |
| I-CT-2 (= OD-AV-7 / I-NS-3) | **Shown, never applied**: a counter is an ANNOTATION/thread item. The countered claim is rendered VERBATIM with its ORIGINAL confidence — never overwritten, filtered out, merged, down-weighted, or re-ranked by the existence of a counter. The thread adds context BELOW; it changes nothing ABOVE. | ADR-015, slice-08 I-NS-3 |
| I-CT-3 (= I-FED-1 / KPI-AV-2 / KPI-GRAPH-2) | **Attribution without merging**: every counter is attributed to its OWN `author_did` + its OWN `cid`; the verbatim `reason` is shown as-authored. Two counters render as two thread items, never a merged "disputed by N people" aggregate that hides the individuals. | KPI-FED-1/2, slice-03/04/08 |
| I-CT-4 (= KPI-4 / FR-VIEW-8) | **Verbatim confidence**: any confidence shown (the original claim's; never a counter-derived re-score) renders as `0.90`, never `0.9` or `90%`, via the single `render_confidence` site. | KPI-4, slice-06 |
| I-CT-5 (= KPI-5 / KPI-VIEW-5) | **LOCAL-only / offline**: the route reads the local DuckDB store (`claims ∪ peer_claims` — a counter can be the operator's OWN or a peer's). NO network seam on this route. The thread renders fully with the network down. Only the vendored local `/static/htmx.min.js` is referenced (no CDN). | KPI-5, slice-06/07 KPI-HX-G2 |
| I-CT-6 (= I-HX-1/4/5) | **Progressive enhancement**: an `HX-Request` returns the detail fragment (claim + thread); a no-JS / bookmark / direct-URL request returns the full page = chrome + the SAME fragment (structural parity via `Shape::from_request`). A swap is a nicety, never a requirement. | slice-07 KPI-HX-G1/G2/G3 |
| I-CT-7 | **No new crates**: extend the PURE `viewer-domain` + EFFECT `adapter-http-viewer` + `adapter-duckdb` read impl + `ports` + `cli` (composition root) + `xtask`. Workspace stays 21 members. Functional paradigm (ADR-007). | slice-06–10 precedent |

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-11 does NOT, under any circumstance:

- **Author or compose a counter-claim on the viewer.** No "counter / reply / dispute"
  button, form, or control. Authoring stays EXCLUSIVELY in the CLI `claim counter`
  verb (I-VIEW-3 / I-CT-1). The viewer only RENDERS counters that already exist in
  the store.
- **Apply, filter, hide, merge, re-weight, or re-rank a countered claim.** The
  countered claim is rendered verbatim with its original confidence (I-CT-2). The
  thread is additive context, never a transformation.
- **Compute or show a "net verdict", "consensus", "disputed score", or "X people
  disagree" aggregate.** Every counter is shown individually with its author + CID
  (I-CT-3). No faceless aggregate.
- **Add any network seam to this route.** Counters are read from the LOCAL store
  only (`claims ∪ peer_claims`). No PDS fetch, no indexer call, no live verification
  (peer counters were already signature-verified at `peer pull` time per KPI-FED-6;
  the viewer re-verifies nothing, mirroring slice-08). (I-CT-5)
- **Re-verify signatures or recompute CIDs.** The viewer trusts the store's
  verified-at-write-time contract (slice-03 pull-time verification). No second
  verification path in the viewer process.
- **Resolve, fetch, or render the counter's own evidence detail thread.** A counter
  is shown with its author DID, CID, and reason; drilling into the counter's OWN
  detail (and ITS counters, recursively) reuses the existing `/claims/{cid}` link —
  it is not a new nested-thread render. (Deep nesting is a non-goal; the thread is
  one level deep: the claim and its direct counters.)
- **Add a "countered by" annotation to the network `/search` view** — that already
  exists (slice-08, `SEARCH_COUNTERED_BY_PREFIX`). slice-11 does not touch /search.

### Deferred (recommend split if it would push the slice >1 day)

- **The `/claims` LIST-row "countered by" annotation** (so the operator can SEE
  which claims have disagreement before drilling in). This is genuinely nice but is
  NOT the core of the slice — and rendering it correctly requires a SECOND read
  shape (a per-CID counter-presence lookup across the whole list page, with its own
  anti-merging + verbatim discipline + an N+1-query risk to manage). **Recommendation:
  DEFER to a follow-up slice-12 (`viewer-counter-claim-list-flags`)** unless DESIGN
  finds the presence lookup is a trivial single aggregate read that fits inside the
  1-day budget. The thread-on-detail (US-CT-002/003) is the load-bearing deliverable
  and stands alone. See "Scope assessment" below.

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 3 (1 infra + 2 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending: `viewer-domain` (pure), `adapter-http-viewer` (effect), `adapter-duckdb` (read impl), `ports`, `cli`, `xtask` — all existing | No (single context: the viewer; the rest are its existing edges) |
| Integration points (new) | 1 (the new `query_counter_claims` read method over the existing shared connection) | No (≤5) |
| Estimated effort | ~1 day (one read method + one render path on an EXISTING route; the detail page, fragment fork, and anti-merging discipline all already exist and are REUSED) | No (≤2 weeks) |
| Independent user outcomes | 1 (see disagreement on a claim) | No |

**## Scope Assessment: PASS — 3 stories, 1 context, estimated ~1 day.**

The list-row annotation is explicitly carved OUT (deferred) precisely to KEEP this
at ~1 day. If DESIGN determines the thread render alone would exceed 1 day, split
US-CT-003 (the flag + no-noise discipline) into a follow-up — but US-CT-002 (the
thread itself) is the irreducible core and must ship as one slice.

---

## Wave: DISCUSS / [REF] Proposed route(s) + read method

- **Route**: EXTEND the existing `GET /claims/{cid}` (`claim_detail_page` in
  `adapter-http-viewer`). NO new route. The detail page/fragment now renders the
  claim (as today) PLUS the counter thread beneath it.
- **Read method (new, read-only)**: `StoreReadPort::query_counter_claims(target_cid: &str) -> Result<Vec<CounterClaimRow>, StoreReadError>`.
  Reads the LOCAL `claims ∪ peer_claims` (UNION ALL, explicit `author_did` + `cid`,
  no merging JOIN/GROUP BY/AVG — anti-merging by construction, mirroring slice-10's
  `query_*_survey`) for every signed claim that carries a `references[]` entry of
  type `counters` whose `cid == target_cid`, returning each counter's `author_did`,
  its own `cid`, its `reason` (verbatim), its `confidence` (DOUBLE, rendered
  verbatim), `composed_at` (ordering/tiebreak only), and its `origin`
  (`PeerOrigin`: `Own` vs a pulled peer). Returns an EMPTY vec for an un-countered
  claim (the renderer then shows the claim alone, no thread — US-CT-003).
  > DESIGN owns the exact SQL shape for matching a `counters` reference whose target
  > CID equals `{cid}` (the `references[]` storage shape is an existing
  > slice-03 concern). The PRODUCT contract is: read-only, LOCAL, attributed,
  > anti-merging, verbatim, empty-vec-when-none.
- **Pure view-model (new, in `viewer-domain`)**: a `CounterThread` ADT
  (`None` → render the claim alone; `Countered { counters: Vec<CounterClaimView> }`
  → render the flag + the thread). `render_claim_detail` / `_fragment` are extended
  to compose the thread BELOW the existing `render_claim_fields` + evidence section.
  > DESIGN owns the exact ADT/render shape; the PRODUCT contract is the AC below.

---

## User Stories

See `user-stories.md` (combined file, one section per story).

| ID | One-line | job_id |
|---|---|---|
| US-CT-001 | Read-only counter-claim thread READ capability in the viewer process (`query_counter_claims`) | infrastructure-only |
| US-CT-002 | See the counter-claim thread beneath a countered claim — each counter attributed (DID + CID) with its verbatim reason; original claim untouched | J-003b |
| US-CT-003 | An un-countered claim renders exactly as today (no empty-thread noise); a countered claim is clearly flagged as disputed | J-003b |

---

## Outcome KPIs

slice-11 mints **NO new KPI ID**. Like slice-08/09/10 it REALIZES inherited KPIs on
a new surface (the counter-thread on `/claims/{cid}`). The relevant inherited KPIs:

- **KPI-FED-3** (`Counter-claim publication rate` — J-003b disagreement as
  first-class artifact, north-star): slice-11 STRENGTHENS the READ side of the J-003b
  loop. The disprover for KPI-FED-3 was "< 10% counter-claim rate at day-30 forces UX
  re-investigation". A plausible cause of low counter-rate is that counters, once
  authored, are barely VISIBLE — so authoring feels like shouting into a void.
  slice-11's thread makes a counter LEGIBLE on the local viewer, closing the
  feedback loop (author a counter → see it land in the thread). Per-feature: GREEN
  (the thread renders own + peer counters); cohort: YELLOW (pending the inherited
  opt-in telemetry endpoint, ADR-010).
- **KPI-VIEW-1** (`Time-to-see-store-contents` — legibility north-star): EXTENDED
  into the disagreement dimension (the operator can now SEE disagreement around a
  claim in the browser, zero SQL).
- **KPI-VIEW-2** (read-only, guardrail): MET — no write/sign/counter route, no key
  read in the viewer process. Release-blocking.
- **KPI-AV-2 / KPI-GRAPH-2 / KPI-FED-1/2** (anti-merging, guardrails): MET — every
  counter attributed to its own DID + CID; no merged "disputed" aggregate;
  `query_counter_claims` UNION-ALL projects `author_did` + `cid` explicitly
  (no merging JOIN). Release-blocking.
- **KPI-4** (verbatim confidence, guardrail): MET — the original claim's confidence
  renders verbatim; no counter-derived re-score exists. Release-blocking.
- **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS
  no-regression / read-only, guardrails): MET — the thread reads the local store
  only, renders offline, references only the vendored htmx asset, serves a full page
  without HX-Request, and adds no write surface. Release-blocking.

A new product hypothesis specific to this slice (a leading indicator OF KPI-FED-3,
not a new KPI ID):

> **Hypothesis**: We believe that making counter-claims LEGIBLE on the local viewer
> (P-001, counter-claim-reader hat) will increase the share of dogfood users who
> author at least one counter-claim within 30 days (KPI-FED-3), because seeing a
> counter land in a thread closes the disagreement feedback loop. We will know this
> is true when KPI-FED-3 moves above its 30% target after slice-11 ships, with the
> local viewer cited as the surface where users confirm their counter landed.

> Detail rationale is inlined here (lean — no separate `outcome-kpis.md`, matching
> the slice-08 precedent). The cross-feature SSOT is `docs/product/kpi-contracts.yaml`.

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the
`GET /claims/{cid}` route, the read-only store port, the page=chrome+fragment render
pattern, and the counter-claim domain model all already exist (slices 03/06/07). The
thinnest end-to-end slice IS US-CT-002 (the thread render on the existing route),
backed by US-CT-001 (the read method). US-CT-003 (no-noise + flag) is a thin
discipline layer on the same render. Delivery sequence: US-CT-001 → US-CT-002 →
US-CT-003. Each is demonstrable in a single session against the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Definition of Ready

See `definition-of-ready.md`. Verdict: **PASS (9/9)**.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Journey (visual + emotional arc + TUI/HTML mockups): `journey-counter-claim-thread-visual.md`
- Journey schema (Gherkin embedded per step): `journey-counter-claim-thread.yaml`
- Shared-artifact registry: `shared-artifacts-registry.md`

---

## Wave: DISTILL / [REF] Reconciliation result

**Reconciliation passed — 0 contradictions.** Checked DISCUSS (this feature-delta +
`journey-counter-claim-thread.yaml`) against DESIGN (`architecture-design.md`,
`component-boundaries.md`, `data-models.md`, `technology-stack.md`) + ADR-046/047. No
standalone `wave-decisions.md` files exist for this feature (lean model — decisions are
inlined). DESIGN realizes the DISCUSS contract verbatim: extend `GET /claims/{cid}`,
read-only, anti-merging (UNION-ALL explicit author_did + cid, no merge), shown-never-
applied (claim verbatim/unchanged), depth-1 no-recursion, empty-reason → "no reason
provided", LOCAL/offline 2-step read (indexed ref lookup + `read_artifact_at`), no new
crate/route/network/KPI. The list-row annotation is consistently OUT of scope (deferred
to slice-12) in both waves.

## Wave: DISTILL / [REF] Scenario list with tags

Tier A (Gojko-style, production composition root, example-only — Mandate 10). NO Tier B:
the journey is a single detail-route render (not a ≥3-chained-scenario state machine);
generative exploration of the pure projection/render belongs to layer-1/2 `viewer-domain`
units in DELIVER (Mandate 9). Layer 3/5 subprocess + real-I/O; sad paths example-based
(Mandate 11).

`tests/acceptance/viewer_counter_claim_threads.rs` (US-CT-002/003 story scenarios):

| ID | Scenario | US | Invariant | Tags |
|---|---|---|---|---|
| CT-1 | `open_a_countered_claim_with_htmx_returns_only_the_detail_fragment_with_the_thread` | US-CT-002 | I-CT-2/3/6 | `@walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment @happy` |
| CT-2 | `a_countered_claim_renders_identically_under_htmx_and_no_js` | US-CT-002 | I-CT-6 | `@driving_port @real-io @no-js @full-page @parity @happy` |
| CT-3 | `the_countered_claim_is_shown_verbatim_never_re_weighted_by_its_counter` | US-CT-002 | I-CT-2/4 | `@driving_port @real-io @shown-never-applied @gold` |
| CT-4 | `two_counters_by_distinct_authors_render_as_two_attributed_items_never_merged` | US-CT-002 | I-CT-3 | `@driving_port @real-io @anti-merging @kpi-av-2 @gold` |
| CT-5 | `the_counter_reason_renders_verbatim_and_its_cid_is_a_one_hop_drill_link` | US-CT-002 | I-CT-3/4 | `@driving_port @real-io @verbatim @drill-link @happy` |
| CT-6 | `a_counter_with_no_reason_renders_an_explicit_no_reason_provided_state` | US-CT-002 | ADR-047 | `@driving_port @real-io @empty-reason @edge` |
| CT-7 | `an_un_countered_claim_shows_no_counter_section_and_no_empty_noise` | US-CT-003 | I-CT-2 | `@driving_port @real-io @no-noise @happy` |
| CT-8 | `a_countered_claim_shows_a_neutral_countered_presence_flag_not_a_verdict` | US-CT-003 | I-CT-3 | `@driving_port @real-io @presence-flag @happy` |
| CT-9 | `an_unknown_cid_keeps_the_existing_guided_not_found_with_no_thread_added` | US-CT-003 | (slice-06 V-7 no-regression) | `@driving_port @real-io @not-found @boundary @no-regression @error` |

`tests/acceptance/viewer_counter_claim_threads_invariants.rs` (GOLD / guardrails):

| ID | Scenario | US | Invariant | Tags |
|---|---|---|---|---|
| CT-INV-ReadOnly | `every_detail_route_with_counters_leaves_the_store_read_only` | US-CT-002/003 | I-CT-1 | `@property @driving_port @real-io @read-only @gold` |
| CT-INV-NoWrite | `no_detail_response_with_counters_adds_a_write_or_sign_control` | US-CT-002/003 | I-CT-1 | `@property @driving_port @real-io @read-only @gold` |
| CT-INV-OfflineChrome | `the_counter_thread_page_chrome_stays_offline_no_cdn` | US-CT-002 | I-CT-5 | `@property @driving_port @real-io @offline @no-cdn @gold` |
| CT-INV-Offline | `the_counter_thread_renders_fully_offline` | US-CT-002 | I-CT-5 | `@property @driving_port @real-io @offline @local-first @kpi-5 @gold` |
| CT-INV-ShownNeverApplied | `the_countered_claim_confidence_is_byte_identical_with_and_without_counters` | US-CT-002/003 | I-CT-2 | `@property @driving_port @real-io @shown-never-applied @gold` |

US coverage: US-CT-002 → CT-1..6 + all 5 gold; US-CT-003 → CT-7/8/9 + read-only/no-write/
shown-never-applied gold; US-CT-001 (infra `query_counter_claims`) → exercised THROUGH the
route by every scenario (its observable contract — attributed/empty/no-merge/local — is
asserted on the rendered surface, port-to-port; no direct-call test, per Mandate 1).

Error/edge ratio: 14 total; CT-3/4/6/9 + all 5 gold are error/edge/guardrail = 9/14 ≈ 64%
(≥ 40% target met — the slice is invariant-heavy by nature: shown-never-applied,
anti-merging, read-only, offline are all failure-mode guards).

## Wave: DISTILL / [REF] WS strategy

Per the Architecture of Reference (port-class → treatment, project-level): driving port
`GET /claims/{cid}` = REAL adapter (the `openlore ui` subprocess via `ViewerServer`);
driven-internal `StoreReadPort` (DuckDB + artifacts) = REAL via the project Infrastructure
Policy (the operator's REAL DuckDB, seeded through production write paths); no
driven-external/non-deterministic port on this route (LOCAL read, no network/clock/LLM —
nothing faked). CT-1 is the `@walking_skeleton @driving_port` scenario: the thinnest
end-to-end thread (viewer → LOCAL 2-step read → pure projection → HTML fragment) that a
stakeholder confirms "yes — I can read the disagreement around a claim." Brownfield DELTA:
no Feature-0 skeleton (the viewer, route, store port, page=chrome+fragment, and counter
model all pre-exist — slices 03/06/07).

## Wave: DISTILL / [REF] Adapter coverage table

| Driven adapter | Real-I/O scenario | Covered by |
|---|---|---|
| `DuckDbStoreReadAdapter::query_counter_claims` (indexed ref lookup, Step A) | YES | every CT-* + every gold (REAL DuckDB seeded via production `claim counter` / `peer pull`) |
| `read_artifact_at` (on-disk SignedClaim `reason` decode, Step B) | YES | CT-1/3/4/5 (verbatim reason rendered) + CT-6 (empty-reason `None` decode) + CT-INV-Offline (LOCAL artifact fs read, network-down) |

No NEW external boundary in this slice (DESIGN §5 — no network/PDS/indexer seam on this
route), so no `@requires_external` contract-smoke is needed. Both driven reads are LOCAL
and exercised with REAL I/O by the scenarios above.

## Wave: DISTILL / [REF] Driving-adapter coverage

`GET /claims/{cid}` (the ONLY driving port in DESIGN §7) is exercised via its real
protocol — the `openlore ui` subprocess + in-test HTTP — in BOTH shapes (no-header full
page via `get`, `HX-Request` fragment via `get_htmx`) across countered/un-countered/404
postures. Verified per scenario: HTTP status (200 / existing 404), output format
(text/html; fragment-vs-full-page shape via `is_fragment()`/`is_full_page()`), and the
rendered counter-thread surface. No new route added (the route is extended, ADR-046).

## Wave: DISTILL / [REF] Scaffolds (RED-ready, `todo!()` per ADR-025)

- `tests/acceptance/viewer_counter_claim_threads.rs` — 9 CT-* scenarios, all `todo!()`.
- `tests/acceptance/viewer_counter_claim_threads_invariants.rs` — 5 CT-INV-* gold, all
  `todo!()`.
- `tests/acceptance/support/mod.rs` — NEW seams (all `todo!()` / SCAFFOLD: true):
  - seeds: `seed_claim_with_counter`, `seed_claim_two_counters_distinct_authors`,
    `seed_counter_empty_reason`, `seed_uncountered_claim` (drive the production CLI
    counter path: own counter via `claim counter`; peer counter via `peer add`+`peer
    pull`).
  - asserts: `assert_counter_thread_renders_attributed_verbatim`,
    `assert_counter_thread_two_attributed_no_merge`,
    `assert_counter_thread_empty_reason_state`,
    `assert_counter_thread_presence_flag_is_neutral`, `assert_no_counter_thread_noise`,
    `assert_detail_html_has_no_write_or_sign_control`,
    `assert_counter_claim_verbatim_unchanged`.
  - types/consts: `SeededCounterThread`, `SeededCounter`, `COUNTER_TARGET_AUTHOR_RACHEL`,
    `COUNTER_AUTHOR_TOBIAS`, `COUNTER_REASON_VERBATIM`, `CLAIM_DETAIL_REGION_ID`.
- `crates/cli/Cargo.toml` — two new `[[test]]` targets registered (`viewer_counter_
  claim_threads`, `viewer_counter_claim_threads_invariants`).

RED gate verified: `cargo build -p cli --tests` compiles (no errors; pre-existing
unused-import warnings only); each scenario body panics at `todo!()` →
MISSING_FUNCTIONALITY (correct RED), never ImportError/collection error (BROKEN). DELIVER
unskips them one at a time and replaces the `todo!()` seam bodies with production-path
implementations.

## Wave: DISTILL / [REF] Test placement

`tests/acceptance/` (workspace-level integration tests compiled by `crates/cli` via
explicit `[[test]]` targets) — the EXACT placement + harness of the slice-06..10 viewer
corpus (`viewer_store.rs`, `viewer_graph_traversal{,_invariants}.rs`). Story scenarios +
GOLD invariants split into a `_invariants.rs` sibling, mirroring slice-10. Reuses the
shared `support/mod.rs` harness (`ViewerServer`, `get`/`get_htmx`, `is_fragment`/
`is_full_page`/`references_external_cdn`, `capture_store_row_count_universe`/
`assert_store_read_only`, the production seeding seams) — NO new test framework.

## Wave: DISTILL / [REF] Mandate compliance evidence

- CM-A (hexagonal): every scenario enters through `GET /claims/{cid}` (the `openlore ui`
  driving port via `ViewerServer`); zero direct calls to `viewer-domain` render fns.
- CM-B (business language): scenario names + tags use domain terms (counter, thread,
  reason, countered, attributed); technical detail lives in step/assert bodies only.
- CM-C (journey completeness): CT-1 walking skeleton = the demo-able user goal (read the
  disagreement around a claim); 8 focused + 5 gold cover the boundaries/guardrails.
- CM-E/F (Mandate 8/9): read-only gold uses `assert_store_read_only` (universe = the two
  port-exposed row counts, all `unchanged`); all layer-3+ scenarios are example-only — no
  PBT machinery imported (the `@property` tag marks universal invariants for the reader +
  DELIVER, not Hypothesis at this layer, Mandate 11).
- CM-G (Mandate 10): Tier B intentionally absent (single detail-route render, not a rich
  ≥3-chained-scenario state machine — documented above).

## Changelog

- 2026-06-06 — slice-11 (`viewer-counter-claim-threads`) DISCUSS. Traces to J-003b
  (VIEW half; authoring stays the slice-03 CLI). 3 stories (1 infra + 2 user-visible).
  New read-only `StoreReadPort::query_counter_claims`. No new crates, no new KPI ID,
  no new route (extends `GET /claims/{cid}`). List-row annotation DEFERRED to a
  recommended slice-12. Scope PASS (~1 day). DoR PASS (9/9).
- 2026-06-06 — slice-11 DISTILL (Quinn, nw-acceptance-designer). Reconciliation passed
  (0 contradictions). 14 RED Tier-A acceptance scaffolds authored (9 CT-* story + 5
  CT-INV-* gold), all `todo!()` per ADR-025; error/edge ratio 64%. New `tests/acceptance/
  viewer_counter_claim_threads{,_invariants}.rs` + 7 stubbed support seams (4 seed via the
  production CLI counter path, 7 assert) + 2 `[[test]]` targets. `cargo build -p cli
  --tests` compiles; scenarios classify RED (panic at `todo!()`). Tier B intentionally
  absent (single detail-route render). NOT proceeding to DELIVER.
