# Wave Decisions: viewer-network-search (slice-08, DISCUSS)

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · Date: 2026-06-04
> Feature type: User-facing · JTBD: YES (J-005) · UX depth: Lightweight · Walking skeleton: YES (thin)
> Brownfield DELTA on slices 05 (appview-search) / 06 (htmx-scraper-viewer) / 07 (viewer-htmx-swaps).

This slice adds a **network-search view** to the read-only `openlore ui` viewer: a
`/search` route (form → query the slice-05 indexer over HTTP → render verified +
attributed network results as HTML, with an htmx fragment swap like `/scrape`). It
is the **browser UI for `openlore search`** (J-005). No new product job is created —
every story traces to the already-validated **J-005** ("Discover signed claims
across the network without knowing who to follow first").

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`,
`personas/`). Proceeded without re-running JTBD; J-005 is validated (slice-05).

## Scope Assessment: PASS — 5 stories (4 user-visible + 1 infra), 1 bounded context (the viewer's `/search` surface), estimated ~7 days

Carpaccio gate (5 taste tests):

- **Stories**: 5 (1 infra walking-skeleton + 4 user-visible). Well within <=10. PASS.
- **Bounded contexts**: 1 — the viewer `/search` surface. It adds ONE new outbound
  capability (an indexer-query effect port) to the existing read-only viewer; it
  reuses the slice-05 `appview-domain` result types + the slice-05 search client
  contract and the slice-06/07 page=chrome+fragment render pattern. PASS.
- **Walking-skeleton integration points**: US-NS-001 needs (1) the new `/search`
  route in the viewer, (2) an indexer-query effect (reuse the slice-05 client?),
  (3) the slice-05 `appview-domain` result composition, (4) the slice-06/07
  fragment-render fork. That is 4 — within <=5. PASS (3 of the 4 are REUSES, not
  net-new, which keeps it tractable).
- **Estimated effort**: ~7 days (within <=2 weeks). PASS.
- **Multiple independent outcomes**: NO — all stories serve the single outcome
  "see verified, attributed network discovery in the browser." PASS.
- **Verdict**: RIGHT-SIZED. Single slice = single sibling feature. The thing that
  would make it oversized — building a write/sign/subscribe affordance into the
  viewer, or a standalone web AppView app — is explicitly OUT of scope (the viewer
  stays read-only; following stays a CLI action).

## Locked decisions (WD-NS-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-NS-1 | slice-08 ships as a SIBLING feature `viewer-network-search`; it is a brownfield DELTA on slices 05/06/07. US-NS-001 is the (thin) walking skeleton. | Inherits WD-9 carpaccio split. One slice = one feature. | LOCKED |
| WD-NS-2 | Persona = **P-001 Senior Engineer Solo Builder ("Maria", the node operator)** — the SAME persona as slices 06/07. She wears the network-discovery hat at her own loopback viewer. | The viewer is P-001's surface (slices 06/07). slice-05 framed P-002 as primary for the CLI discovery job, but the BROWSER viewer's operator is P-001 — she now discovers the network from the same read-only UI she already uses to glance at her store. | LOCKED |
| WD-NS-3 | The viewer stays **READ-ONLY**: searching the network is a READ. No new write/sign/subscribe route; the viewer holds no signing key (inherits I-VIEW-1/2/3 / KPI-VIEW-2 / KPI-HX-G3). Following a discovered author stays a CLI action (`openlore peer add`); the view may DISPLAY the affordance as guidance text, never execute it. | The read-only invariant is cardinal across slices 06/07. Adding an indexer-query (a public-data network READ) preserves it exactly as the slice-06 `/scrape` GithubPort did. | LOCKED |
| WD-NS-4 | **Graceful degradation** (inherits slice-05 WD-116/KPI-5 + slice-06/07 NetworkDown pattern): an unreachable OR unconfigured indexer (`OPENLORE_INDEXER_URL` unset / connection fails) renders a plain-language guidance message — never a crash, never a block, never a leaked transport internal (mirror the slice-07 `/scrape` `NetworkDown` unit-variant render). | KPI-5 / KPI-VIEW-5 discipline: the viewer never shows a stack trace. The `/scrape` `NetworkDown` ADT arm is the precedent — a payload-free variant that structurally cannot leak the raw error. | LOCKED |
| WD-NS-5 | **Verified + attributed display** (inherits slice-05 AV invariants WD-103/104 → KPI-AV-2/3): every result row shows the `[verified]` marker + `author_did` attribution; `counter_annotation` is SHOWN but NOT applied (anti-merging — the network never silently merges/over-rides); confidence rendered VERBATIM (inherits FR-VIEW-8). No faceless "network consensus" row anywhere. | Carries the slice-05 cardinal trust guarantees into the browser surface. The `NetworkResultRow.author_did` is non-Option (load-bearing); `verified_against` drives `[verified]`. | LOCKED |
| WD-NS-6 | **Progressive enhancement** (inherits I-HX-1..5 / KPI-HX-G1): `/search` serves a complete full page WITHOUT `HX-Request` and a fragment of the SAME results region WITH it (the slice-07 page=chrome+fragment pattern, keyed on `Shape::from_request`). htmx stays local/offline for the chrome (the search itself needs the network — exactly like `/scrape`). | Reuses the slice-07 `Shape` fork verbatim. A swap is a nicety, never a requirement; the no-JS full page is the contract. | LOCKED |
| WD-NS-7 | **Zero new persisted types; loopback-only bind unchanged.** Search results are computed per-query and never persisted (same as the CLI `search` and the live `/scrape`). The bind stays 127.0.0.1-only (inherits I-VIEW-4). | The viewer persists nothing from a network read (BR-VIEW-2 / I-VIEW-1). Discovery is a per-query read surface. | LOCKED |

## Risks logged

- **Index coverage (inherited KPI-AV-1 risk)**: discovery surfaces nothing new if
  the index is sparse. Same mitigation as slice-05 (coverage dashboard handed to
  DEVOPS; the disprover triggers a coverage/UX re-investigation). The viewer view
  does not change index coverage — it is a render surface over the slice-05 index.
- **Indexer-query port shape (OD-NS-1)**: whether the viewer reuses the slice-05
  `adapter-index-query` client or a new viewer-process port is a DESIGN call. The
  viewer process is read-only + key-less; the new port must hold NO signing/identity
  capability (mirror the slice-06 GithubPort capability boundary).
- **DISCOVER + DIVERGE skipped** (same as all prior slices). J-005 is the validated
  source job; the four-forces for J-005 were done in slice-05's DISCUSS. No prior
  validation interviews specific to the BROWSER surface — mitigated by the inherited
  KPI-AV / KPI-VIEW behavioral hypotheses + the day-30 studies.
- **Scope creep into a standalone web AppView**: held off by WD-NS-3 (read-only,
  follow stays CLI) + WD-NS-7 (nothing persisted). The `/search` view is a render
  surface over the existing indexer, not a new app.

## DIVERGE note

No DIVERGE artifacts exist for this slice (`docs/feature/viewer-network-search/diverge/`
absent) — consistent with all prior OpenLore slices. Journey work is grounded in
the validated J-005 job statement (slice-05) and the slice-06/07 viewer journey.
