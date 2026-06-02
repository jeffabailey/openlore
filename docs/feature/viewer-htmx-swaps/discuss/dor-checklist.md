# Definition of Ready: viewer-htmx-swaps (slice-07)

> 9-item hard gate per story, with evidence. Stories: US-HX-001 (walking skeleton),
> US-HX-002, US-HX-003, US-HX-004, US-HX-005 (@infrastructure), US-HX-006. The 9th item
> (JTBD traceability) is the mandatory `job_id` check (Decision 1, 2026-04-28).
>
> **Job mapping note**: the viewer is a **read-only view surface over existing signed
> claims**, so value-producing stories trace to **J-002** in `docs/product/jobs.yaml` ("Read
> /query claims and SEE who claims what" — the orient/inspect job), NOT J-001 (authoring/
> signing, which the read-only viewer explicitly does not do).

## Item 0 (BLOCKING, checked first): Elevator Pitch / JTBD traceability

| Story | `job_id` | Elevator Pitch | Status |
|-------|----------|----------------|--------|
| US-HX-001 | J-002 (navigate-without-reloads) | Present (Before/After/Decision; After names Next on `http://127.0.0.1:8788/claims`, the real route; output = table swap; decision = scan store without losing place) | PASS |
| US-HX-002 | J-002 | Present (After names `/peer-claims` Next; output = peer table swap; decision = judge federation set) | PASS |
| US-HX-003 | J-002 | Present (After names `POST /scrape` submit; output = results swap below form; decision = triage which candidates to sign in CLI) | PASS |
| US-HX-004 | J-002 | Present (After names clicking a claim → inline detail; output = detail panel; decision = verify evidence) | PASS |
| US-HX-005 | infrastructure-only (+ `infrastructure_rationale`) | EXEMPT (@infrastructure; rationale present; value realized via US-HX-001..004/006) | PASS |
| US-HX-006 | J-002 | Present (After names the tab switch → view-panel swap + URL; output = panel + bookmarkable URL; decision = compare own vs federated) | PASS |

> Slice-level value check (review Dimension 0.5): US-HX-001..004 and US-HX-006 are all
> user-visible. US-HX-005 is the only @infrastructure story and is correctly tagged with a
> rationale — the slice is NOT infrastructure-only, so it has release value. PASS.

## Per-story 8-item DoR

### US-HX-001 — Pagination swaps the claims table in place (Walking Skeleton)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear (domain language) | PASS | "every Next/Prev on `/claims` triggers a full-page reload that jumps her scroll to the top and flashes" — operator pain, no solution prescribed. |
| User/persona with specific characteristics | PASS | "Node operator on localhost, paging her own `claims` list" — Maria Santos, 312 claims. |
| 3+ domain examples with real data | PASS | htmx swap (312 claims, `tokio-rs/tokio` 0.95, "51–100 of 312"); no-JS direct URL; over-the-end clamp `?page=99` → 301–312 of 312. |
| UAT in Given/When/Then (3-7) | PASS | 4 scenarios (htmx swap, no-JS full page, clamp, read-only). |
| AC derived from UAT | PASS | 6 AC each trace to a scenario (fragment shape, full-page shape, clamp, verbatim conf, no write surface). |
| Right-sized (1-3 days, 3-7 scenarios) | PASS | 4 scenarios; one route's GET fragment+branch reusing slice-06 PageView — thin skeleton. |
| Technical notes (constraints/deps) | PASS | Read-only GET; OD-HX-2/3/5/1 noted; depends on slice-06 `/claims` + minimal htmx ref. |
| Dependencies resolved or tracked | PASS | slice-06 `/claims` + PageView exist; minimal htmx reference (hardened in US-HX-005) tracked. |

**DoR Status: PASSED**

### US-HX-002 — Pagination swaps the peer-claims table in place

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | "Paging this large list is where the full-reload jolt hurts most" — 1,840 peer rows. |
| User/persona | PASS | "Federated node operator paging `/peer-claims`" — 1,840 from 4 peers. |
| 3+ domain examples | PASS | htmx swap (`axum/axum` 0.88 origin peer-A); no-JS; unknown-origin "unknown". |
| UAT (3-7) | PASS | 3 scenarios (swap, no-JS, unknown origin). |
| AC derived from UAT | PASS | 5 AC trace to scenarios (fragment, full page, origin preserved, unknown origin, clamp). |
| Right-sized | PASS | 3 scenarios; repeats US-HX-001 pattern on the second list. |
| Technical notes | PASS | Reuses `/peer-claims` + PageView; OD-HX-2; dep on US-HX-001. |
| Dependencies | PASS | US-HX-001 (pattern), slice-06 `/peer-claims` exist. |

**DoR Status: PASSED**

### US-HX-003 — Live scrape swaps results below the form

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | "submitting a target reloads the whole `/scrape` page, flashing the document and losing the form's place." |
| User/persona | PASS | "Node operator triaging scrape candidates, network available." |
| 3+ domain examples | PASS | `tokio-rs/tokio` 7 candidates 0.95 "LICENSE @ HEAD"; `some-org/empty-repo` zero; offline network-down. |
| UAT (3-7) | PASS | 4 scenarios (swap+no-sign+no-persist, zero candidates, network down, no-JS). |
| AC derived from UAT | PASS | 6 AC trace to scenarios incl. no sign control, no persist, derived-from honesty, no-leak. |
| Right-sized | PASS | 4 scenarios; reuses `POST /scrape` + GithubPort; results fragment. |
| Technical notes | PASS | No persist/no sign (I-SCR-1); DV-4 no-leak pattern; OD-HX-2/3; dep on US-HX-001. |
| Dependencies | PASS | US-HX-001 (pattern), slice-06 `/scrape` + GithubPort exist. |

**DoR Status: PASSED**

### US-HX-004 — Claim detail loads inline

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | "opening a claim navigates away to its detail page; ... she repeatedly leaves and returns, losing her scroll position." |
| User/persona | PASS | "Node operator reviewing several claims in a sitting." |
| 3+ domain examples | PASS | `bafyrei...1` two evidence URLs 0.90; no-JS direct URL; unknown CID `bafyrei...zzz`; (+ no-evidence edge in AC). |
| UAT (3-7) | PASS | 3 scenarios (inline swap, no-JS full page, unknown CID both shapes). |
| AC derived from UAT | PASS | 5 AC trace to scenarios incl. list preserved, verbatim conf, no-evidence, full-page fallback. |
| Right-sized | PASS | 3 scenarios; GET detail fragment reusing the pattern. |
| Technical notes | PASS | Reuses `get_claim` + evidence; FR-VIEW-8 verbatim; OD-HX-2/3; dep on US-HX-001. |
| Dependencies | PASS | US-HX-001 (pattern), slice-06 detail route exists. |

**DoR Status: PASSED**

### US-HX-005 — htmx served locally so swaps work offline (@infrastructure)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | "if that library were loaded from a CDN, every swap would break the moment Maria's machine is offline — silently breaking the offline guarantee." |
| User/persona | PASS | "Node operator on an offline / air-gapped machine relying on the dashboard working with no network." |
| 3+ domain examples | PASS | offline swaps work; view-source no CDN; single-source no drift. |
| UAT (3-7) | PASS | 4 scenarios (offline swaps, no CDN, no write surface, no-JS unaffected). |
| AC derived from UAT | PASS | 5 AC trace to scenarios (local serve, offline, no CDN property, single source, no write surface). |
| Right-sized | PASS | 4 scenarios; one asset-delivery concern (OD-HX-1). |
| Technical notes | PASS | OD-HX-1 mechanism; loopback (I-VIEW-4); used by all swap stories. |
| Dependencies | PASS | OD-HX-1 (DESIGN); skeleton carries a minimal local reference this hardens. |

**DoR Status: PASSED** (Elevator Pitch correctly EXEMPT via @infrastructure + rationale.)

### US-HX-006 — Switch My Claims ↔ Peer Claims in place

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | "switching tabs reloads the whole page each toggle, flashing the document — friction on a comparison she makes repeatedly." |
| User/persona | PASS | "Federated node operator comparing own vs federated claims." |
| 3+ domain examples | PASS | swap to Peer Claims (1,840 from 4 peers, URL → /peer-claims); bookmark/Back; no-JS plain link. |
| UAT (3-7) | PASS | 3 scenarios (panel swap + URL, bookmark/Back, no-JS). |
| AC derived from UAT | PASS | 5 AC trace to scenarios incl. URL update, bookmarkable, reload → full page. |
| Right-sized | PASS | 3 scenarios; adds the only new sub-mechanism (URL/history, OD-HX-4). |
| Technical notes | PASS | OD-HX-4 history strategy; converges with no-JS real URLs; dep on US-HX-001/002. |
| Dependencies | PASS | US-HX-001/002 (lists), OD-HX-4 (DESIGN), slice-06 routes exist. |

**DoR Status: PASSED**

## Feature-level DoR summary

| Story | DoR | Notes |
|-------|-----|-------|
| US-HX-001 | PASSED | Walking skeleton; proves the contract. |
| US-HX-002 | PASSED | Second list; same pattern. |
| US-HX-003 | PASSED | Read-only-sensitive (no sign/no persist) in fragment shape. |
| US-HX-004 | PASSED | Inline detail. |
| US-HX-005 | PASSED | @infrastructure (offline asset hardening); Elevator Pitch exempt with rationale. |
| US-HX-006 | PASSED | Adds URL/history sub-mechanism (OD-HX-4). |

**Feature DoR Status: PASSED** — all 6 stories pass all 9 items. Residual items are DESIGN
decisions (OD-HX-1..6), correctly deferred, not DoR gaps. Ready for peer review then DESIGN
handoff.
