# Story Map: htmx-scraper-viewer (slice-06)

## User: OpenLore node operator (Maria Santos), on localhost, read-only

## Goal: See what my node holds (persisted claims + peer claims) and what I could add (live scrape proposals) — in a browser, without SQL, with zero risk of writing/signing.

> **DELTA** on slices 01 (claims), 02 (scraper), 03 (peer_claims). Read-only personal
> dashboard. Signing stays in the CLI (inherits I-SCR-1).

---

## Backbone (operator activities, chronological)

| A. Launch viewer | B. See my claims | C. Inspect a claim | D. See peer claims | E. Browse proposals |
|------------------|------------------|--------------------|--------------------|---------------------|
| Start read-only server bound to localhost, against local store | List my signed claims as HTML | Open one claim's full detail + evidence | List federated peer_claims, distinguish from mine | Enter a target, see live candidate proposals (no sign) |

### Ribs (tasks under each activity, most critical at top)

| A. Launch | B. My claims | C. Inspect claim | D. Peer claims | E. Proposals |
|-----------|--------------|------------------|----------------|--------------|
| **A.1** Start server, bind localhost, open store read-only, no key | **B.1** Render `claims` rows (subject/pred/obj/conf/cid) | **C.1** Render one claim detail incl evidence[] | **D.1** Render `peer_claims` rows w/ peer_origin | **E.1** Target form + render live candidates (derived-from, no sign) |
| A.2 Report listen URL + "read-only" banner | B.2 Empty-store guidance state | C.2 CID-not-found guidance | D.2 No-peers-yet guidance | E.2 Zero-candidates guidance |
| A.3 Store-unreadable error guidance | B.3 Pagination for large stores | C.3 No-evidence display | D.3 Pagination for peer claims | E.3 Network-unavailable error (notes store works offline) |
|  | B.4 Sort/filter columns | C.4 Link back to list | D.4 Filter by peer | E.4 Loading/progress state for slow harvest |

---

## Walking Skeleton (thinnest end-to-end thread — one task per *minimum* activity)

The confirmed walking skeleton is **one thin end-to-end thread**: HTTP request → query
local DuckDB → rendered HTML list page viewable in a browser. It spans activities A and B
(the minimum needed for an operator to *see their store in a browser at all*). Activities
C, D, E are deliberately above-the-skeleton enhancements layered in later releases.

- **A.1** — Start the read-only server, bind localhost, open the local DuckDB store, load
  no signing key.
- **B.1** — Serve `GET /claims`: query the local `claims` table and render the rows as a
  single HTML list page the operator can open in a browser.

> Skeleton definition (verbatim target): **start server → request `/claims` → query
> DuckDB → render one HTML list page viewable in a browser.** This is the absolute minimum
> that proves the whole thread works: HTTP in, DuckDB query, HTML out, read-only, offline.
> It is intentionally thin — no pagination, no detail page, no peer view, no scrape.

---

## Release slices (sliced by outcome, not by feature grouping)

### Release 1 — "Operator can see their own store at a glance" (Job 1 core)
Tasks: A.2 (listen URL + read-only banner), A.3 (store-unreadable guidance), B.2
(empty-store guidance), C.1 (claim detail + evidence).
Outcome KPI target: **KPI-VIEW-1** (time-to-see-store-contents < 10s, zero SQL) and
**KPI-VIEW-2** (read-only guardrail: zero write/sign paths).
Rationale: turns the bare skeleton into a trustworthy, self-explanatory store view —
the north-star job. Detail page (C.1) makes a single claim fully legible (evidence).

### Release 2 — "Operator can navigate a large, federated store" (Job 1 depth)
Tasks: B.3 (pagination), D.1 (peer_claims list + peer_origin), D.2 (no-peers guidance).
Outcome KPI target: **KPI-VIEW-3** (operator inspects both own and federated claims;
distinguishes them) + **KPI-VIEW-5** (offline guardrail holds).
Rationale: real stores are large and federated; this makes the view usable beyond a toy
store and surfaces the peer dimension that is unique to a federated node.

### Release 3 — "Operator can triage scrape proposals before signing" (Job 2)
Tasks: E.1 (target form + live candidates + derived-from, no sign), E.2 (zero-candidates),
E.3 (network-unavailable error noting store still works offline).
Outcome KPI target: **KPI-VIEW-4** (operator reviews proposals in browser then signs in
CLI). Reuses the HTTP + render foundation from R1/R2.
Rationale: secondary job (opportunity 7 vs 15); deliberately last so the higher-opportunity
store view ships and stabilizes first.

### Release 4 (Could-have / future) — "Operator works fluidly in a big store"
Tasks: B.4 (sort/filter), D.3 (peer pagination), D.4 (filter by peer), C.2/C.3/C.4 polish,
E.4 (harvest progress). Deferred enhancements; no new job, only friction reduction.

---

## Priority Rationale

Priority order is driven by **opportunity score** (Job 1 = 15, Job 2 = 7; see
`jtbd-opportunity-scores.md`) and **dependency**:

1. **Walking Skeleton first** — validates the riskiest end-to-end assumption: can a
   read-only web process query the same local DuckDB store and render HTML, offline,
   without holding the key? Everything else depends on this thread working.
2. **Release 1 (Job 1 core) next** — Job 1 is the north star (opportunity 15). Making the
   store view trustworthy and self-explanatory is the highest-value behavior change.
3. **Release 2 (Job 1 depth) next** — same job, but required for real (large, federated)
   stores; depends on R1's rendering being solid.
4. **Release 3 (Job 2) after** — secondary job (opportunity 7); functionally already
   served by the CLI, so it is pure scannability gain. Sequenced last among jobs and
   reuses the R1/R2 HTTP+render foundation (efficient ordering).
5. **Release 4 deferred** — friction-reduction polish, no new outcome; ship only if
   capacity allows.

No activity gap in the skeleton's scope (A+B fully covered). Later releases extend into
C/D/E. Every release names the outcome it achieves, not the features it bundles.
