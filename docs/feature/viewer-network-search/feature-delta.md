<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-network-search

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a new READ-ONLY browser view on the `openlore ui` viewer)
> Walking skeleton: Yes, thin (US-NS-001 + US-NS-002)
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07)
> JTBD: YES — every story traces to **J-005** (`docs/product/jobs.yaml`); no new job created
> Brownfield DELTA on: `openlore-appview-search` (slice-05), `htmx-scraper-viewer` (slice-06), `viewer-htmx-swaps` (slice-07)
> Date: 2026-06-04 · Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `viewer-network-search`
(slice-08): a **network-search view** added to the read-only `openlore ui` viewer.
A `/search` route serves a form (pick a dimension + value); on submit the viewer
queries the slice-05 network indexer over HTTP (`org.openlore.appview.searchClaims`)
and renders **verified + attributed** network results as HTML, with an htmx fragment
swap (like `/scrape`). It is the **browser UI for `openlore search`** — the same
network discovery J-005 the slice-05 CLI delivered, now glanceable from the same
read-only viewer Maria already uses to inspect her store.

This is a DELTA. It REUSES the slice-05 verified-attributed search results + client
contract and the slice-06/07 page=chrome+fragment render pattern; it adds exactly
ONE new capability — an indexer-query effect in the viewer process (a network READ,
distinct from the read-only DuckDB store + the slice-06 GithubPort). Tier-1 content
is inlined here (lean); SSOT lives under `docs/product/`; per-slice briefs under
`slices/`; per-journey/registry artifacts under `discuss/`.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME
persona as slices 06/07 (`docs/product/personas/senior-engineer-solo-builder.yaml`).
She lives in a terminal but runs `openlore ui` to GLANCE at her store in a browser
(slice-06) and navigate it without reloads (slice-07). slice-08 extends that same
read-only viewer with a network-discovery surface: she can now discover signed
claims across the network — beyond her own claims and her manually-subscribed peers —
from the browser, without dropping back to the CLI.

slice-05 framed P-002 (Researcher/Tech Lead) as primary for the CLI discovery job;
the BROWSER viewer's operator, though, is P-001 (the viewer is her surface, slices
06/07). She wears the network-discovery hat at her own loopback viewer. UX guardrails
inherited: read-only, never silently mutate, confidence display must never read as
"the system thinks this is true."

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-005**: *When I am orienting a decision around a philosophy or project but do
> NOT already know which developers to follow, I want to discover the signed claims
> that exist across the whole network — verified and attributed — so I can find
> well-evidenced reasoning and the people behind it without first knowing whose DID
> to subscribe to.*

slice-08 is the **browser UI** for J-005 (validated in slice-05; opportunity score
15, `walking_skeleton_for: openlore-appview-search`). No new job. Every story below
traces to J-005 and its sub-jobs:

| Sub-job | Name | Stories |
|---|---|---|
| J-005a | Search by philosophy / subject / contributor at network scale | US-NS-002, US-NS-003 |
| J-005b | Index only signature-verified, attributed public claims | US-NS-001 (inherited — viewer renders already-verified results), US-NS-004 |
| J-005c | Turn a discovery into a follow (discovery feeds federation) | US-NS-004 (guidance text only — follow stays a CLI action) |

---

## Wave: DISCUSS / [REF] Locked Decisions

See `discuss/wave-decisions.md` for full rationale. Summary (WD-NS-*):

| # | Decision | Status |
|---|---|---|
| WD-NS-1 | Sibling feature; brownfield DELTA on slices 05/06/07; US-NS-001 is the thin walking skeleton. | LOCKED |
| WD-NS-2 | Persona = P-001 (Maria, the node operator) — the viewer's operator, wearing the network-discovery hat. | LOCKED |
| WD-NS-3 | Viewer stays **READ-ONLY**: search is a READ; no new write/sign/subscribe route; no key in the process; following stays a CLI action (the view may show `peer add` as guidance text, never execute it). Inherits I-VIEW-1/2/3 / KPI-VIEW-2 / KPI-HX-G3. | LOCKED |
| WD-NS-4 | **Graceful degradation**: an unreachable OR unconfigured indexer renders a fixed plain-language message (mirror the slice-07 `/scrape` `NetworkDown` unit-variant) — never crash/block/leak. Inherits WD-116 / KPI-5 / NFR-VIEW-6/7. | LOCKED |
| WD-NS-5 | **Verified + attributed display**: every row shows `[verified]` + `author_did`; `counter_annotation` SHOWN, never applied (anti-merging); confidence VERBATIM. No faceless consensus row. Inherits WD-103/104 / KPI-AV-2/3 / FR-VIEW-8. | LOCKED |
| WD-NS-6 | **Progressive enhancement**: `/search` serves a full page without `HX-Request`, a fragment of the same results region with it (slice-07 `Shape` fork). htmx stays local/offline for the chrome. Inherits I-HX-1..5 / KPI-HX-G1. | LOCKED |
| WD-NS-7 | **Zero new persisted types; loopback-only bind unchanged.** Results computed per-query, never persisted. Inherits BR-VIEW-2 / I-VIEW-1 / I-VIEW-4. | LOCKED |

---

## Wave: DISCUSS / [REF] Inherited Invariants (I-NS-* inheriting I-VIEW-* / I-HX-* / AV-*)

These are binding inputs to DESIGN; they are NOT relitigated here.

| ID | Inherits | Carries into slice-08 as |
|---|---|---|
| I-NS-1 | I-VIEW-1/2/3 (slice-06) / KPI-VIEW-2 | Read-only preserved: search is a READ; the viewer signs/writes/persists nothing, holds no signing key. The indexer-query port reads only the public index (no signing/identity/PDS surface — mirrors the slice-06 GithubPort capability boundary). |
| I-NS-2 | WD-116 / KPI-5 (slice-05) + NFR-VIEW-6/7 (slice-06) | Graceful degradation: an unreachable/unconfigured indexer renders a fixed plain-language guidance message; never a crash/hang/blank/stack-trace; never leaks transport internals (a payload-free `NetworkDown`-style render). |
| I-NS-3 | WD-103 / KPI-AV-2 (slice-05) / I-FED-1 | Anti-merging at network scale: every result row carries one `author_did` (non-Option, load-bearing); identical-content-different-author = two rows; no merged/consensus row; `counter_annotation` shown, never applied. |
| I-NS-4 | WD-104 / KPI-AV-3 (slice-05) | Verified display: every row carries `[verified]` (driven by `verified_against`), by construction — the indexer verified signature + recomputed CID BEFORE indexing; the viewer renders already-verified results (no second verification path in the viewer). |
| I-NS-5 | WD-105 / KPI-AV-5 (slice-05) | Public-data framing: the `/search` page surfaces, up front, that discovery indexes only PUBLIC signed claims verified before indexing; nothing private is read. |
| I-NS-6 | I-HX-1..5 / KPI-HX-G1 (slice-07) | Progressive enhancement: full page without `HX-Request`, fragment of the same results region with it; page = chrome + fragment; the two shapes agree by construction (the full page embeds the fragment fn). |
| I-NS-7 | I-HX-2 / KPI-HX-G2 (slice-07) | Offline / no-CDN for the chrome: htmx is the vendored, SHA-256-pinned local asset at `/static/htmx.min.js`; zero off-host references. (The search ITSELF needs the network — like `/scrape` — but the page chrome stays offline-capable.) |
| I-NS-8 | I-VIEW-4 (slice-06) / KPI-HX-G3 | Loopback-only bind unchanged (127.0.0.1); zero new persisted types (results computed per query). |
| I-NS-9 | FR-VIEW-8 (slice-06) | Confidence rendered VERBATIM (`0.90`, never `0.9`/`90%`) — the same `render_confidence` contract; confidence must never read as "the system thinks this is true." |

---

## Wave: DISCUSS / [REF] Story Map and Slicing

One journey: **discover-the-network-from-the-browser** (a single coherent surface;
the arc open `/search` → search by a dimension → see verified+attributed results →
trust them → know the next step is `peer add` in the CLI). Visual journey +
shared-artifacts registry under `discuss/` (placement mirrors slice-07).

Emotional arc: **cold-start-curiosity → reassured-by-verification → discovery-joy →
connected-but-grounded** — entry curious-but-wary (cares about a philosophy, follows
nobody who claims it; wary that a browser network view is "just another aggregator"),
through reassured (the public-data framing + `[verified]` markers + visible author
DIDs build trust), the discovery-joy peak (a relevant claim by an unfollowed author
appears in her browser), to connected-but-grounded (she knows the next step is a
deliberate CLI `peer add` — the viewer never silently follows for her).

Slicing (by outcome impact + risk, not feature grouping):

- **Release 1 (walking skeleton)** — `slices/slice-01-walking-skeleton.md`:
  US-NS-001 + US-NS-002. The thinnest end-to-end thread: verified, attributed
  search-by-philosophy results rendered in the browser from a reachable indexer,
  with the fragment swap. Validates the riskiest assumption (the new outbound
  capability works AND the read-only/verified/attributed/PE invariants hold on a
  network-READ surface).
- **Release 2 (dimensions + trust + degradation)** — `slices/slice-02-dimensions-and-trust.md`:
  US-NS-003 + US-NS-004. Completes contributor/subject dimensions; makes the trust
  surface (public-data framing, counter-shown-not-applied) and the failure surface
  (graceful degradation) honest; shows the `peer add` follow path as guidance text.

### Priority Rationale

Release 1 first because it carries the slice's riskiest assumption: that the viewer
can take on a NEW outbound network-query capability while preserving every cardinal
invariant (read-only, verified, attributed, progressive-enhancement). If Release 1
fails, the browser-discovery thesis is disproven and the read-only viewer's trust
model is at risk — everything else is moot. Release 2 completes the dimensions and
hardens trust/failure UX; its failure is survivable (the object dimension alone
still delivers discovery; degradation polish can iterate). Within Release 1,
US-NS-001 (the indexer-query capability) precedes US-NS-002 (the render) because the
render has nothing to show without the capability.

---

## Wave: DISCUSS / [REF] System Constraints (cross-cutting)

These hold across every story (the I-NS-* invariants, restated as build constraints):

- The viewer process holds **no signing key** and exposes **no write/sign/subscribe
  route**. The indexer-query effect reads only the public index (no signing/identity/
  PDS capability). (I-NS-1)
- Search results are **never persisted** and **never normalized**: a rendered field
  matches the author's published, verified record (inherits KPI-4). (I-NS-3/4, WD-NS-7)
- Every result is **`[verified]` by construction** (the indexer is the verify gate;
  the viewer does not re-verify and has no path to render an unverified result).
  (I-NS-4)
- Every result row is **attributed** (`author_did`); **no merged/consensus row**
  exists; `counter_annotation` is shown, never applied. (I-NS-3)
- Every route serves a **complete full page without `HX-Request`** (no-JS no-regression)
  and an offline-capable chrome (vendored htmx). (I-NS-6/7)
- An **unreachable/unconfigured indexer never crashes or leaks**; it renders a fixed
  plain-language message in both shapes. (I-NS-2)
- **Loopback-only bind** unchanged. (I-NS-8)

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

All four stories trace to **J-005**. US-NS-001 is `@infrastructure` (the new
indexer-query capability) with rationale; the slice is NOT 100% `@infrastructure`
(3 user-visible stories). Full UAT below; AC derived per story.

### US-NS-001: Bootstrap the viewer's indexer-query capability (`@infrastructure`)

- **job_id**: `infrastructure-only`
- **infrastructure_rationale**: This story stands up the NEW outbound capability the
  viewer needs to reach the slice-05 indexer — an indexer-query effect in the viewer
  process (a public-data network READ, distinct from the read-only DuckDB store and
  the slice-06 GithubPort). It enables every user-visible story (US-NS-002/003/004)
  but renders no user-facing output on its own; the user decision it serves is made
  in those stories. The capability MUST hold no signing/identity/PDS surface (I-NS-1)
  and MUST degrade gracefully when the indexer is unreachable/unconfigured (I-NS-2).

#### Problem

The read-only viewer (slices 06/07) can read only the LOCAL DuckDB store and (for
`/scrape`) public GitHub. It has no way to reach the slice-05 network index, so the
browser cannot show network discovery at all. The capability must be added without
giving the viewer any write/sign capability or breaking the loopback-only, key-less,
read-only invariants.

#### Solution

Add an indexer-query effect to the viewer process: it reads the configured indexer
URL (`OPENLORE_INDEXER_URL`, the slice-05 env-var seam), queries
`org.openlore.appview.searchClaims` over HTTP along a dimension, and returns the
slice-05 attributed result rows for the render layer to compose. It holds no signing/
identity/PDS capability and, when the indexer is unreachable/unconfigured, returns a
typed unavailable outcome (never a crash). OD-NS-1: DESIGN decides whether this reuses
the slice-05 `adapter-index-query` client or is a new viewer-process port.

#### Domain Examples

1. **Reachable indexer** — `OPENLORE_INDEXER_URL=http://127.0.0.1:9444`; a query for
   object `org.openlore.philosophy.reproducible-builds` returns 12 attributed verified
   rows (9 distinct authors) for the render layer.
2. **Unconfigured indexer** — `OPENLORE_INDEXER_URL` unset; the capability returns the
   typed `unavailable` outcome (no network call attempted) so the render layer shows
   the guidance message — never a crash.
3. **Unreachable indexer** — `OPENLORE_INDEXER_URL` set but the connection is refused;
   the capability returns the same typed `unavailable` outcome (no leaked transport
   error string) — never a hang or stack trace.

#### UAT Scenarios (BDD)

##### Scenario: The browser viewer can query the network index
```
Given the viewer is configured with a reachable indexer URL
When the viewer queries the index for a philosophy along the object dimension
Then it receives the verified, attributed result rows the indexer holds
And the viewer process holds no signing key and exposes no write/sign route
```

##### Scenario: An unreachable or unconfigured indexer never crashes the viewer
```
Given the viewer is configured with no indexer URL (or an unreachable one)
When the viewer attempts a network query
Then it receives a typed "index unavailable" outcome
And no crash, hang, or raw transport error occurs in the viewer process
```

#### Acceptance Criteria

- [ ] The viewer process can query the configured indexer along a search dimension
      and obtain the slice-05 attributed verified result rows.
- [ ] The indexer-query capability holds no signing/identity/PDS surface (no key
      enters the viewer process; no new write/sign route exists).
- [ ] An unset or unreachable `OPENLORE_INDEXER_URL` yields a typed unavailable
      outcome — no crash, no hang, no leaked transport internals.

#### Technical Notes

- REUSE the slice-05 indexer query surface + `appview-domain` result types; do not
  rebuild verification (the indexer is the verify gate — I-NS-4).
- The viewer composition root already owns a tokio runtime (slice-06); the new
  effect rides it. OD-NS-1 / OD-NS-6 are DESIGN's (port shape, config surface).
- Dependencies: slice-05 `openlore-indexer` + `adapter-index-query` + `appview-domain`;
  slice-06 `ViewerServer`.

---

### US-NS-002: Search by philosophy in the browser, attribution preserved

- **job_id**: J-005 (sub-job J-005a)

#### Elevator Pitch

- **Before**: Maria can glance at her own store in the browser (slices 06/07), but to
  discover signed claims by people she does not follow she must drop to the CLI
  (`openlore search`).
- **After**: she opens `http://127.0.0.1:8080/search` in the viewer, picks "philosophy",
  enters `org.openlore.philosophy.reproducible-builds`, and sees a rendered results
  region — per-author groups, each row showing the author DID, the `[verified]` marker,
  and the verbatim confidence — e.g. *"12 signed claims across 9 distinct authors — all
  verified"*, with no merged "the network thinks X" row.
- **Decision enabled**: she decides which well-evidenced, verified reasoning (and which
  unfamiliar author) is worth pursuing — without first knowing whose DID to follow.

#### Problem

Maria cares about a philosophy but follows nobody who has claimed it. Her browser
viewer shows only her own + her subscribed peers' claims; a great signed claim by an
unknown author is invisible there. She wants to search the network by philosophy from
the same read-only browser surface she already trusts.

#### Who

- P-001 (Maria), node operator | at her loopback `openlore ui` viewer | wants
  network discovery without leaving the browser or compromising local-first.

#### Solution

A `/search` route: a form with a dimension selector (philosophy/object for this story)
and a value input. On submit, the viewer queries the indexer (US-NS-001 capability)
and renders the slice-05 per-author-attributed verified rows as HTML. Served as a full
page without `HX-Request`, and as a results-region fragment swap with it.

#### Domain Examples

1. **Happy path** — Maria searches object `org.openlore.philosophy.reproducible-builds`;
   the results region shows 3 author groups (`did:plc:priya-test`,
   `did:plc:bjorn-test`, `did:plc:maria` — her own), each row with `[verified]`,
   subject, and verbatim confidence `0.85`.
2. **Edge: identical content, two authors** — two different authors each claim
   `nixos/nixpkgs` embodies reproducible-builds at `0.90`; the view renders TWO rows
   under two author groups — never one merged row.
3. **Edge: htmx swap** — Maria (on a JS-enabled browser) re-submits; only the
   `#search-results` region swaps, the form stays put; the rows are identical to the
   full-page render.
4. **Boundary: no results / typo** — Maria searches `org.openlore.philosophy.reprducible`
   (typo); the results region shows a guided "no claims found for that philosophy"
   state (the slice-05 near-match suggestion is a DESIGN nicety, OD-NS-3), never a blank
   region or a crash.

#### UAT Scenarios (BDD)

##### Scenario: Discover verified claims by philosophy in the browser
```
Given Maria opens /search in her read-only viewer with a reachable indexer
And she selects the philosophy dimension and enters "org.openlore.philosophy.reproducible-builds"
When she submits the search
Then the results region shows the matching claims grouped by author
And each row shows the author DID, a [verified] marker, and the verbatim confidence
And no merged "network consensus" row appears
```

##### Scenario: The same results region swaps in place under htmx
```
Given Maria has JavaScript enabled in her browser
And she has run a philosophy search
When she submits a new philosophy search
Then only the search-results region updates (the form is preserved)
And the swapped rows are identical to the full-page render of the same search
```

##### Scenario: A philosophy with no network claims shows a guided empty state
```
Given Maria searches a philosophy that no indexed author has claimed
When she submits the search
Then the results region shows a plain-language "no claims found" guidance
And the viewer does not crash or show a blank region
```

#### Acceptance Criteria

- [ ] `GET /search` (no `HX-Request`) serves a full page with the dimension form.
- [ ] A philosophy submit renders per-author groups; each row shows `author_did`,
      `[verified]`, and verbatim confidence (`0.85`, not `0.9`/`90%`).
- [ ] Identical-content-different-author renders as two rows (no merge).
- [ ] The same submit with `HX-Request` returns only the results-region fragment,
      structurally identical to the full page's results region.
- [ ] A no-results search renders a guided empty state, not a blank region or crash.

#### Outcome KPIs

- **Who**: P-001 viewer operators · **Does what**: discover ≥1 verified claim by an
  unfollowed author from the browser `/search` · **By how much**: realizes KPI-AV-1
  on the browser surface (the slice's behavioral hypothesis) · **Measured by**: viewer
  search telemetry (unfollowed-author hits) · **Baseline**: 0 (no browser network search
  before slice-08).

#### Technical Notes

- REUSE `appview-domain::compose_results` + the slice-05 `NetworkResultRow` render;
  REUSE the slice-07 `Shape` fork + page=chrome+fragment pattern.
- OD-NS-2 (reuse `appview-domain` rendering vs a new viewer-domain fragment),
  OD-NS-4 (own route vs nav tab), OD-NS-5 (form UI) are DESIGN's.
- Dependencies: US-NS-001.

---

### US-NS-003: Search by contributor or subject in the browser

- **job_id**: J-005 (sub-job J-005a)

#### Elevator Pitch

- **Before**: the browser `/search` (US-NS-002) discovers by philosophy only; to survey
  a project or a developer's network trail Maria must use the CLI.
- **After**: she picks "contributor" and enters `github:priya` (or "subject" and
  `github:bazelbuild/bazel`) and sees the rendered results — for a contributor, one
  developer's verified trail under a single author DID with the footer *"one developer's
  reasoning trail, not a community consensus"*; for a subject, the project's claims
  grouped BY AUTHOR with no merged row.
- **Decision enabled**: she decides whether a specific developer's trail is worth following,
  or whether a project is broadly claimed by many authors — at network scale, from the browser.

#### Problem

Discovery by philosophy alone is one of three orienting surfaces. Maria also thinks in
terms of "what does THIS developer claim?" and "what is claimed about THIS project?" —
the contributor and subject dimensions slice-05 ships in the CLI but the browser does not.

#### Who

- P-001 (Maria), node operator | at her loopback viewer | thinks in contributor/subject
  terms as well as philosophy terms.

#### Solution

Extend the `/search` form's dimension selector with contributor and subject. The
contributor query resolves a handle/DID (the slice-05 convention) and renders the trail
under one author DID with the honesty footer; the subject query renders the project's
claims grouped by author (no consensus row).

#### Domain Examples

1. **Happy path (contributor)** — Maria searches contributor `github:priya`; the view
   shows 8 verified claims under `did:plc:priya-test#org.openlore.application`, with the
   "one developer's reasoning trail, not a community consensus" footer.
2. **Happy path (subject)** — Maria searches subject `github:bazelbuild/bazel`; the view
   shows 5 author groups (5 distinct authors who claimed something about bazel), each row
   `[verified]` — no "bazel: the network thinks X" merged row.
3. **Edge: absent contributor** — Maria searches contributor `github:nobody-here`; the
   results region names the queried handle with a plain-language "no claims for that
   contributor" state and NO near-match suggestion (an absent contributor is not a typo).

#### UAT Scenarios (BDD)

##### Scenario: Survey one developer's verified trail in the browser
```
Given Maria opens /search and selects the contributor dimension
And she enters "github:priya" with a reachable indexer
When she submits the search
Then the results region shows priya's verified claims under a single author DID
And a footer states this is one developer's reasoning trail, not a community consensus
```

##### Scenario: Survey a project at network scale, attribution preserved
```
Given Maria selects the subject dimension and enters "github:bazelbuild/bazel"
When she submits the search
Then the results region shows the claims grouped by their distinct authors
And no merged "the network thinks X about bazel" consensus row appears
```

##### Scenario: A contributor with no network claims shows a no-suggestion empty state
```
Given Maria searches a contributor handle that no indexed author matches
When she submits the search
Then the results region names the queried handle with a plain-language empty state
And offers no near-match suggestion (an absent contributor is not a typo)
```

#### Acceptance Criteria

- [ ] The `/search` form offers philosophy, contributor, and subject dimensions.
- [ ] A contributor search renders one author's trail under a single `author_did` with
      the "one developer's reasoning trail, not a community consensus" footer.
- [ ] A subject search renders N authors' rows grouped BY AUTHOR (no consensus row).
- [ ] An absent contributor renders a named, no-suggestion empty state.
- [ ] Both dimensions fork by `Shape` (fragment under `HX-Request`, full page without).

#### Outcome KPIs

- **Who**: P-001 viewer operators · **Does what**: survey a contributor's trail or a
  project at network scale from the browser · **By how much**: contributor/subject
  searches available in the browser surface (parity with CLI dimensions) · **Measured by**:
  viewer search telemetry by dimension · **Baseline**: 0 (object-only before this story).

#### Technical Notes

- REUSE the slice-05 handle→DID resolution + the contributor/subject query surface +
  the `EmptyPolicy::NoSuggestion` precedent.
- Dependencies: US-NS-002.

---

### US-NS-004: Trust a browser discovery — verified framing, honest degradation, follow guidance

- **job_id**: J-005 (sub-jobs J-005b display + J-005c follow-guidance)

#### Elevator Pitch

- **Before**: Maria sees rows in the browser but is unsure whether to trust them ("is this
  just another aggregator? is this tampered?"), and a flaky indexer could show her a crash
  or a leaked transport error.
- **After**: the `/search` page states up front that discovery indexes only PUBLIC signed
  claims verified before indexing; every row carries `[verified]` + the author DID; a
  countered claim SHOWS its counter-annotation (never silently applied); and when the
  indexer is unreachable she sees a calm plain-language message ("the network index is
  unavailable; your local store views still work") instead of a stack trace — plus, for an
  unfollowed author, the guidance text `openlore peer add <did>` she can run in the CLI.
- **Decision enabled**: she decides to ACT on a browser discovery (cite it, or follow the
  author via the CLI) with the same confidence she has in a self-pulled peer claim — instead
  of dismissing the browser view as aggregator noise.

#### Problem

A network-discovery view is only useful if it is trustworthy and fails honestly. Maria
needs the verified/public-data framing visible, the anti-merging guarantee visible
(counter shown not applied), and a degradation path that never crashes, never leaks, and
never silently follows on her behalf.

#### Who

- P-001 (Maria), node operator | at her loopback viewer | will not act on data she cannot
  trust, and will not tolerate a surface that crashes or follows without her say-so.

#### Solution

Add the up-front public-data framing to the `/search` page; render the `counter_annotation`
on a row when present (shown, never applied); render the fixed plain-language guidance when
the indexer is unreachable/unconfigured (mirror the slice-07 `/scrape` `NetworkDown`
unit-variant — no leaked transport internals — in both shapes); and show, for an unfollowed
author, the `openlore peer add <did>` guidance text as plain text (never an executable
control — the viewer stays read-only).

#### Domain Examples

1. **Happy path (trust framing)** — Maria opens `/search`; before any results she reads
   "Discovery indexes only PUBLIC signed claims, verified before indexing. Nothing private
   is read." Every result row carries `[verified]` + the author DID.
2. **Edge (counter shown, not applied)** — a result row for a claim Bjorn later countered
   shows the counter-annotation ("countered by did:plc:maria") inline; the claim is still
   shown verbatim and is NOT merged/over-ridden.
3. **Error (graceful degradation)** — Maria's indexer is down; she submits a search and the
   results region shows "The network index is unavailable, so no network results could be
   fetched. Your local store views still work." — no HTTP status, no "connection refused",
   no raw URL, no stack trace.
4. **Edge (follow is CLI-only)** — a result by `did:plc:priya-test` whom Maria does not
   follow shows the guidance text `openlore peer add did:plc:priya-test` (run it in the CLI)
   and NO clickable follow/subscribe control.

#### UAT Scenarios (BDD)

##### Scenario: The search page states what it indexes, up front
```
Given Maria opens /search in her read-only viewer
Then she sees, before any results, that discovery indexes only public signed claims verified before indexing
And every result row she later sees carries a verified marker and the author DID
```

##### Scenario: A countered claim is shown but never silently applied
```
Given a network result includes a claim that another author has countered
When the results render
Then the row shows the counter-annotation inline
And the original claim is still shown verbatim, not merged or over-ridden
```

##### Scenario: An unavailable index degrades to calm guidance, not a crash
```
Given Maria's configured indexer is unreachable (or unconfigured)
When she submits a network search
Then the results region shows a plain-language "index unavailable; your local store views still work" message
And no HTTP status, connection error, raw URL, or stack trace is shown
And the viewer does not crash or hang
```

##### Scenario: Following a discovered author stays a deliberate CLI action
```
Given a network result is by an author Maria does not yet follow
When the row renders
Then it shows the "openlore peer add <did>" guidance text to run in the CLI
And it shows no clickable follow or subscribe control (the viewer is read-only)
```

#### Acceptance Criteria

- [ ] The `/search` page shows the public-data framing before results (I-NS-5).
- [ ] Every result row carries `[verified]` + the author DID (I-NS-3/4).
- [ ] A row with a counter-annotation SHOWS it; the claim is not applied/merged (I-NS-3).
- [ ] An unreachable/unconfigured indexer renders the fixed plain-language guidance in
      BOTH fragment and full-page shapes; no leaked transport internals; no crash (I-NS-2).
- [ ] An unfollowed-author row shows `openlore peer add <did>` guidance TEXT and no
      executable follow control (WD-NS-3 / I-NS-1).

#### Outcome KPIs

- **Who**: P-001 viewer operators · **Does what**: act on a browser discovery (cite it or
  follow via CLI) instead of dismissing it as aggregator noise · **By how much**: realizes
  KPI-AV-3 (verified framing) + KPI-AV-5 (public-data comprehension) + the discovery→
  federation funnel (KPI-AV-4) on the browser surface · **Measured by**: viewer telemetry
  (search→`peer add` funnel) + day-30 comprehension prompt · **Baseline**: 0 (no browser
  discovery before slice-08).

#### Technical Notes

- REUSE the slice-07 `NetworkDown` ADT discipline (a payload-free variant that structurally
  cannot leak the raw error). The follow affordance is render-only guidance (mirrors the
  slice-05 `peer add` render-only hint; no parallel subscription state).
- OD-NS-3 (unreachable/unconfigured degradation UX wording placement) is DESIGN's.
- Dependencies: US-NS-002 (and US-NS-003 for the contributor/subject degradation pointers).

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-08 is a NEW SURFACE for slice-05's job; it REALIZES the existing KPI-AV-* on the
browser surface rather than minting new KPIs. Per-story KPIs above; cross-feature SSOT
in `docs/product/kpi-contracts.yaml` (KPI-AV-1..6, KPI-VIEW-2, KPI-HX-G1/2/3).

- **North star (inherited)**: KPI-AV-1 — ≥60% of discovery sessions surface ≥1 verified
  claim by an unfollowed author — now reachable from the browser viewer.
- **Guardrails (inherited, all release-blocking)**: KPI-AV-2 (anti-merging at network scale),
  KPI-AV-3 (verified-before-index / every row `[verified]`), KPI-VIEW-2 (read-only — zero
  write/sign route, zero key in the process), KPI-HX-G1 (no-JS no-regression — every route a
  full page without `HX-Request`), KPI-HX-G2 (offline/no-CDN chrome), KPI-5 (local-first).
- **Leading (inherited)**: KPI-AV-4 (discovery→federation funnel via CLI `peer add`),
  KPI-AV-5 (public-data framing comprehension).

A per-feature `outcome-kpis.md` is intentionally NOT duplicated (lean): the KPIs are the
inherited KPI-AV-* / KPI-VIEW-* / KPI-HX-* on a new surface. DEVOPS adds viewer-side
`/search` telemetry (dimension, unfollowed-author hits, search→`peer add` funnel) mirroring
the slice-05 CLI events; no new KPI IDs are minted unless dogfood reveals a browser-specific
behavior the CLI events miss.

---

## Wave: DISCUSS / [REF] Out of Scope

- **Any write/sign/subscribe affordance in the viewer** — the viewer stays read-only;
  following stays a deliberate CLI `openlore peer add` (the view shows it as guidance text).
- **A standalone web AppView application** — slice-08 is a render surface on the existing
  `openlore ui`, not a new app (the slice-05 OD-AV-6 "web AppView OUT of scope" line holds).
- **A browser shareable-link** equivalent to the CLI `--share` — deferred (CLI `--share`
  already realizes KPI-AV-6).
- **Persisting search results or relationship state** — computed per query (WD-NS-7).
- **Re-verifying claims in the viewer** — the indexer is the verify gate (I-NS-4); the
  viewer renders already-verified results.
- **Cross-user scoring / Firehose / retraction-aware search filters** — inherited
  deferrals from slice-05 (WD-108, OD-AV-7).

---

## Wave: DISCUSS / [REF] Walking Skeleton Strategy

US-NS-001 + US-NS-002 form the thin walking skeleton: from a reachable indexer,
`/search?object=<nsid>` WITH `HX-Request` renders the verified network result-rows
fragment in the browser. It is the thinnest end-to-end thread — viewer → indexer HTTP →
verified rows → HTML — touching exactly 4 integration points, 3 of which are REUSES:

1. the new `/search` route in the viewer (net-new),
2. the indexer-query effect (reuse the slice-05 client? — OD-NS-1),
3. the slice-05 `appview-domain` result composition (reuse),
4. the slice-07 fragment-render fork (reuse).

This validates the riskiest assumption first: that the read-only viewer can take on a
network-query capability while preserving read-only/verified/attributed/PE invariants.

---

## Wave: DISCUSS / [REF] Driving Ports (for DESIGN)

The viewer's `/search` surface is driven through (names indicative; DESIGN owns shapes):

- **An indexer-query effect port** (new outbound capability; OD-NS-1): viewer process →
  indexer HTTP (`org.openlore.appview.searchClaims`); returns the slice-05 attributed
  result rows or a typed `unavailable` outcome. Holds NO signing/identity/PDS surface
  (mirror the slice-06 GithubPort capability boundary). Ships a `probe()` (ADR-009 / I-4).
- **The existing `StoreReadPort`** (slice-06): unchanged; the relationship-label projection
  (subscribed-peer vs unfollowed) reads the local subscriptions — DESIGN decides whether the
  viewer surfaces that label and from which read.
- **The pure `appview-domain` composition** (slice-05, reused): `compose_results` (per-author
  grouping, no merge) + the `NetworkResultRow` / `NetworkSearchResult` types.
- **The pure render layer** (OD-NS-2: reuse the slice-05 render vs a new `viewer-domain`
  fragment): the `/search` full page (chrome + form + results region) and the
  results-region fragment, forked by `Shape::from_request` (slice-07).

---

## Wave: DISCUSS / [REF] Pre-requisites and Open Decisions for DESIGN

Pre-requisites (all SHIPPED, inherited):

- slice-05 `openlore-indexer` + `adapter-index-query` + `appview-domain` + the
  `org.openlore.appview.searchClaims` query surface + `OPENLORE_INDEXER_URL` seam.
- slice-06 `adapter-http-viewer` (`ViewerServer`, route table, `html_ok`, loopback bind) +
  `viewer-domain` render pattern + the `/scrape` GithubPort capability-boundary precedent.
- slice-07 `Shape::from_request` fork + page=chrome+fragment + vendored htmx asset.

Open Decisions (OD-NS-*) — DESIGN owns:

| ID | Decision | Default lean |
|---|---|---|
| OD-NS-1 | Indexer-query port shape: a NEW viewer-process effect port, vs reuse the slice-05 `adapter-index-query` client directly. | Recommend REUSE the slice-05 client behind a thin viewer-process port (no second query path); it MUST hold no signing/identity/PDS surface. |
| OD-NS-2 | Result rendering: reuse `appview-domain`'s `NetworkResultRow` rendering, vs a new `viewer-domain` HTML fragment. | Recommend a new `viewer-domain` fragment that PROJECTS the `appview-domain` result types (the CLI render is stdout text; the browser needs HTML) — reusing the composition, not the text renderer. |
| OD-NS-3 | Unreachable/unconfigured degradation UX: wording + placement (results-region message vs page-level banner). | Recommend a fixed results-region `NetworkDown`-style message (mirror slice-07 `/scrape`); no leaked transport internals; both shapes. |
| OD-NS-4 | `/search` as its own route, vs a tab in the existing My/Peer nav. | Recommend its OWN route `/search` (the network corpus is distinct from the local-store tabs); DESIGN may add it to the nav as a third link. |
| OD-NS-5 | The search form UI: dimension selector (radio/select) + value input; GET-query vs POST-form. | Recommend a GET form (`/search?<dim>=<value>`) so a search is bookmarkable/shareable as a URL and the no-JS path is a plain navigation (consistent with slice-07 `hx-push-url`). |
| OD-NS-6 | Indexer URL config surface in the viewer: env-var only (`OPENLORE_INDEXER_URL`), vs `[appview] indexer_url` config, vs a viewer flag. | Recommend reuse the slice-05 resolution (config `[appview] indexer_url` with the env-var seam) — one source of truth for "where is the index". |

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-NS-001 | US-NS-002 | US-NS-003 | US-NS-004 |
|---|---|---|---|---|
| 1. Problem statement clear, domain language | PASS (infra rationale) | PASS | PASS | PASS |
| 2. Persona with specific characteristics | n/a (infra) | PASS (P-001 Maria) | PASS (P-001) | PASS (P-001) |
| 3. ≥3 domain examples with real data | PASS (3) | PASS (4) | PASS (3) | PASS (4) |
| 4. UAT in Given/When/Then (3-7) | PASS (2 — narrow infra surface) | PASS (3) | PASS (3) | PASS (4) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (1.5d, 2) | PASS (2d, 3) | PASS (2d, 3) | PASS (1.5d, 4) |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (slice-05/06 shipped) | PASS (US-NS-001) | PASS (US-NS-002) | PASS (US-NS-002/003) |
| 9. Outcome KPIs defined with measurable targets | n/a — supports KPI-AV-1..5 | PASS (KPI-AV-1) | PASS (KPI-AV-1 by dimension) | PASS (KPI-AV-3/4/5) |

**Overall DoR status: PASSED** for all stories.

Notes:
- US-NS-001 ships 2 composite scenarios (narrow infra surface) — same pattern as the
  infra story in every prior slice (US-AV-001 etc.). Flagged for reviewer judgment; PASS.
- US-NS-001 is `infrastructure-only` with `infrastructure_rationale`; the slice is NOT
  100% `@infrastructure` (3 user-visible stories) — passes Dimension 0 §5.

### Elevator Pitch verification (BLOCKING per Dimension 0)

| Story | Section present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-NS-001 | n/a (@infrastructure with rationale) | n/a | n/a | n/a (`infrastructure-only`) | PASS via rationale |
| US-NS-002 | YES (Before/After/Decision) | YES (`GET http://127.0.0.1:8080/search?object=...` in the browser) | YES (rendered per-author groups + `[verified]` + verbatim confidence, no merged row) | YES (decide which verified reasoning/unfollowed author to pursue) | PASS |
| US-NS-003 | YES | YES (`/search?contributor=github:priya` / `?subject=github:bazelbuild/bazel`) | YES (one author's trail + "not a community consensus" footer / N-author subject survey) | YES (decide whether a developer's trail or a project is worth pursuing) | PASS |
| US-NS-004 | YES | YES (the `/search` page framing + a result row + the degradation message) | YES (public-data framing + `[verified]` + counter shown-not-applied + plain-language degradation + `peer add` guidance text) | YES (act on a browser discovery with peer-claim confidence, or dismiss aggregator noise) | PASS |

Slice-level Elevator Pitch check (Dimension 0 §5): 3 user-visible stories + 1 infra. Slice
is NOT 100% `@infrastructure`. PASS.

---

## Wave: DISCUSS / [REF] Definition of Done (9-item, for DISTILL→DELIVER)

PO defines; acceptance-designer enforces at DISTILL→DELIVER.

1. All UAT scenarios (US-NS-001..004) pass green as executable acceptance tests.
2. All supporting unit/integration/component tests pass (incl. the render fork + the
   indexer-query port `probe()`).
3. **Read-only enforced** (KPI-VIEW-2 / I-NS-1): route inventory shows no new write/sign
   route; key-access audit shows zero key reads in the viewer process; the indexer-query
   port holds no signing/identity/PDS surface.
4. **No-JS no-regression** (KPI-HX-G1 / I-NS-6): `/search` serves a complete full page
   without `HX-Request`; the slice-06/07 corpus stays green.
5. **Offline/no-CDN chrome** (KPI-HX-G2 / I-NS-7): the `/search` page references only the
   vendored local htmx asset; zero off-host references.
6. **Verified + attributed** (KPI-AV-2/3 / I-NS-3/4): every rendered row carries `[verified]`
   + `author_did`; identical-content-different-author = two rows; no merged row; counter
   shown-not-applied.
7. **Graceful degradation** (I-NS-2): an unreachable/unconfigured indexer renders the fixed
   plain-language guidance in both shapes; no leaked internals; no crash.
8. **Loopback-only bind + zero new persisted types** (I-NS-8 / WD-NS-7) verified.
9. Code refactored, reviewed, merged to main, demonstrable in a single session (open
   `openlore ui`, search a philosophy in the browser, see verified attributed rows).

---

## Wave: DISCUSS / [REF] Handoff

### To DESIGN (nw-solution-architect)

- Read: this `feature-delta.md`; `discuss/wave-decisions.md`;
  `slices/slice-01-walking-skeleton.md`; `slices/slice-02-dimensions-and-trust.md`;
  `docs/product/jobs.yaml` (J-005); the slice-05 design (`docs/feature/openlore-appview-search/design/`)
  for the indexer query surface + `appview-domain` types; the slice-06/07 viewer crates
  (`adapter-http-viewer`, `viewer-domain`) for the route table + `Shape` fork.
- Decide: OD-NS-1..6 (indexer-query port shape, render reuse vs new fragment, degradation UX,
  route vs tab, form UI/GET-vs-POST, config surface). Likely a new ADR for the viewer's
  outbound indexer-query capability (after the latest viewer ADR).
- Constraints inherited (DO NOT relitigate without returning to PO): WD-NS-3 (read-only,
  follow stays CLI), WD-NS-4 (graceful degradation), WD-NS-5 (verified+attributed,
  counter-shown-not-applied), WD-NS-6 (progressive enhancement), WD-NS-7 (nothing persisted,
  loopback bind) + the I-NS-1..9 invariants.

### To DEVOPS (nw-platform-architect, parallel)

- Read: this file's Outcome KPIs section + the slice-05 `kpi-instrumentation.md`.
- Deliver: viewer-side `/search` telemetry mirroring the slice-05 CLI events (search by
  dimension, unfollowed-author hits for KPI-AV-1, search→`peer add` funnel for KPI-AV-4) —
  privacy-preserving (structural counts + DIDs the user already saw, never claim contents).
  Confirm the new outbound indexer-query adds no dependency to the local-first flows
  (offline compose/sign unchanged) and the viewer stays loopback-only.

### To DISTILL (nw-acceptance-designer)

- Read: this file's User Stories section (UAT per story) + `discuss/wave-decisions.md` +
  the two slice briefs.
- Build executable acceptance tests: browser search by philosophy/contributor/subject
  (attributed, `[verified]`, no merged row); fragment-vs-full-page parity under `HX-Request`;
  graceful degradation (unreachable/unconfigured → fixed message, no leak, no crash);
  read-only invariants (no write/sign route, no key, no executable follow control); counter
  shown-not-applied. Reuse the slice-05 verified/attributed fixtures + the slice-07 htmx
  shape-fork harness.
