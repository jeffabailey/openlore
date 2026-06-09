# Wave Decisions: viewer-landing-dashboard (slice-17) — DISCUSS

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-09
> Feature type: User-facing · JTBD: YES (J-002 orientation facet) · UX depth: Lightweight · Walking skeleton: brownfield DELTA (thinnest in the series)
> Brownfield DELTA on slices 06 (htmx-scraper-viewer) / 07 (viewer-htmx-swaps) / 15 (viewer-peer-subscriptions). Reuses reads from 06 + 15.

This slice EXTENDS the existing read-only **`GET /`** landing route on the `openlore ui`
viewer: it turns the near-empty front door (today an `<h1>` + `READ_ONLY_NOTICE` + a single
`/claims` link — "queries nothing") into a **navigation hub + at-a-glance LOCAL store
summary**. It threads the read-only store the viewer ALREADY holds into the landing handler
and REUSES three existing `StoreReadPort` reads (`count_claims`, `count_peer_claims`,
`list_active_peer_subscriptions`) to surface own-claims / peer-claims / active-peer counts,
plus links to all 8 shipped entry-point surfaces. It realizes **KPI-VIEW-1** as the front
door and closes the discoverability gap. No new product job — the user-visible story traces
to the already-validated **J-002** orientation facet; the read-wiring story is
`infrastructure-only` with rationale.

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`, `personas/`,
`journeys/`). Proceeded without re-running JTBD; J-002 is validated (slice-04/09/10 DISCUSS,
changelog 2026-05-28/2026-06-05).

## Scope Assessment: PASS — 2 stories (1 infra + 1 user-visible), 1 bounded context (the viewer's `/` surface), 3 integration points, estimated ~0.5–1 day

Carpaccio gate, run BEFORE journey-visualization investment (Phase 1.5):

- **Stories**: 2 (1 infra + 1 user-visible). Well within ≤10. PASS.
- **Bounded contexts**: 1 — the viewer `/` surface. Extends `viewer-domain` (pure —
  `render_landing` gains a summary input) + `adapter-http-viewer` (effect — `landing_page`
  threads the store) + at most `ports` / `adapter-duckdb` IF DESIGN elects a count-only
  active-subs variant. All existing; NO new crate. PASS.
- **Walking-skeleton integration points**: 3 — (1) thread the read-only store into
  `landing_page`, (2) resolve the three counts via the EXISTING reads, (3) the extended
  `render_landing(summary)` + the nav hub from existing URL consts. Within ≤5. PASS.
- **Estimated effort**: ~0.5–1 day. PASS.
- **Multiple independent outcomes**: NO — one outcome ("open the viewer and orient: what's
  here + where to go"). PASS.
- **Verdict**: RIGHT-SIZED — the THINNEST slice in the viewer series. It extends an existing
  route, reuses existing reads (touches no SQL unless DESIGN elects the optional count-only
  variant), and adds a render. The thing that would make it oversized — rendering claim
  content / scores / threads on the front door, a write/compose affordance, or a network
  seam — is explicitly OUT of scope (WD-LD-1 read-only, WD-LD-2 LOCAL-only, WD-LD-6
  content-stays-on-existing-surfaces).

## Locked decisions (WD-LD-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-LD-1 (CARDINAL) | `/` is **READ-ONLY / no key**: it reads three LOCAL counts + renders navigation links only — never mutates, never holds a key, adds no write/compose/sign/subscribe/follow control. Every nav affordance is a plain `<a href>` (no-JS navigable, optionally htmx-enhanced like `render_tab_nav`); none executes a mutation. | The read-only / key-less viewer is cardinal across slices 06–16. Adding LOCAL count reads + nav links preserves it exactly. Load-bearing read-only boundary (KPI-VIEW-2). | LOCKED |
| WD-LD-2 (CARDINAL) | **LOCAL-only / offline + graceful degrade**: the three counts are LOCAL DuckDB aggregate reads; NO network seam. `/` renders fully network-down, references only the vendored `/static/htmx.min.js`. If any count read FAILS, `/` shows the nav hub WITHOUT that number (a missing-number state), never a 5xx, never blank, never a raw stack trace. | The front door is the FIRST surface the operator sees; it must never break because one count couldn't be read. Carries KPI-5 / KPI-VIEW-5 / NFR-VIEW-6. The slice-12 `counter_presence_for(...).unwrap_or_default()` degrade is the precedent. | LOCKED |
| WD-LD-3 | **Extends `GET /` — NO new route**: slice-17 extends the existing `landing_page` / `render_landing`; it adds ZERO new routes. `landing_page` is threaded with `store.as_ref()` (today the only storeless handler). | The route already exists; the gap is that it queries nothing and links only `/claims`. No new route is warranted. | LOCKED |
| WD-LD-4 | **REUSES three existing reads — NO new read method** (baseline): own claims via `count_claims()`, peer claims via `count_peer_claims()`, active peers via the count of `list_active_peer_subscriptions()`. All three already exist on the read-only `StoreReadPort`. | The first viewer slice that adds NO new read method — every read it needs already ships (06: the two count methods; 15: the active-subscriptions list). | LOCKED |
| WD-LD-5 (RESOLVED in DESIGN → count-only variant) | **Active-subs count-read approach**: use `.len()` of the existing `list_active_peer_subscriptions` (zero new port surface; materializes the tiny active set to count it) OR add a tiny count-only `count_active_peer_subscriptions()` (`COUNT(*) … WHERE removed_at IS NULL`, mirrors `count_claims`/`count_peer_claims`). DESIGN decides. The PRODUCT contract is a SINGLE aggregate read for the active-subs count, invariant to store size, either way. If DESIGN adds the variant, it is a read-only method on `StoreReadPort` and `adapter-duckdb` gains ONE `COUNT(*)` impl; workspace stays 21. | Surfaced as an open question per the brief — do not over-specify. The contract (single aggregate read) holds for both; DESIGN picks the cheaper/cleaner. | **RESOLVED (ADR-054 D3) — COUNT-ONLY VARIANT.** Add a read-only `count_active_peer_subscriptions()` (`SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL`). Chosen for SYMMETRY (the other two summary counts are count-only aggregates) + CHEAPNESS (`.len()` would materialize + decode the LEFT JOIN/GROUP BY/per-peer-COUNT rows just to count rows). It is a READ method (no mutation added); `ports` +1 sig, `adapter-duckdb` +1 `COUNT(*)` impl; workspace stays 21. |
| WD-LD-6 | **Counts are aggregates, never merges; content stays on the existing surfaces**: the three numbers are store-level aggregate COUNTS, not a merge of distinct authors' claims into a faceless record. `/` renders NO per-author content, scores, or counter threads — drilling into who-said-what is the existing attributed surfaces (`/claims`, `/peer-claims`, `/score`, `/project`, `/philosophy`, `/peers`). | The anti-merging invariant protects per-author CONTENT rendering; a store-wide count is a legitimate aggregate. The front door orients; the surfaces hold the content. | LOCKED |
| WD-LD-7 | **Navigation completeness + URL-const links**: the hub links ALL 8 shipped entry-point surfaces (`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`); deep/parameterized routes (`/claims/{cid}`, `/score?contributor`, …) are reached THROUGH those. Each link uses the route's URL CONST from `viewer-domain` (`MY_CLAIMS_URL`, `PEERS_URL`, …) — never a hardcoded path. DESIGN decides whether to mint a `SCRAPE_URL` const for parity (the `/scrape` route currently has no const). | Closes the discoverability gap (today only `/claims` reachable from `/`). The URL-const single-source-of-truth prevents link drift. | LOCKED. **Sub-decision RESOLVED (ADR-054 D4): MINT `SCRAPE_URL = "/scrape"`.** `/scrape` is the only entry-point surface lacking a const (the other 7 exist); minting it gives the hub 8 consts so every link is `a href=(CONST)`, no drift (R-LD-4). Migrating the scrape form's `action="/scrape"` literal to the const is OPTIONAL polish, not required. |
| WD-LD-8 | **Missing ≠ zero**: the `LandingSummary` models each count as Option / a total ADT so a FAILED read renders "—" (or omits the number), DISTINCT from a SUCCESSFUL read of 0 ("0 own claims"). | A fabricated 0 on a failed read would mislead "empty store." The distinction is a product AC. | LOCKED |
| WD-LD-9 | **Progressive enhancement + parity**: the summary + nav hub live in the SAME render the full page and (if DESIGN forks the shape) the htmx fragment both embed. The landing is typically a full page; DESIGN confirms the shape handling (full-page-only vs `Shape::from_request` fork). The no-JS full page is the contract; a swap is a nicety. | Reuses the slice-07 `page = chrome + fragment` pattern. The PRODUCT contract is parity — the summary + hub never differ between shapes. | LOCKED. **Sub-decision RESOLVED (ADR-054 D5): FULL-PAGE-ONLY.** `GET /` does NOT fork by `Shape`; `render_landing(summary) -> String` returns a complete document (matching today's `render_landing() -> String`). Rationale: NOTHING targets `/` with an `hx-target`/`hx-get` (the fork exists so a swap lands on a sub-region — `/` has no such region and is never a swap target). Parity is satisfied by CONSTRUCTION — one render means the no-JS page and any htmx request return identical bytes. A speculative fork (mint a fragment fn + swap id with zero consumer) is rejected (simplest-solution-first). |
| WD-LD-10 | **Zero new persisted types; loopback-only bind; no new crate.** The counts are computed per-request, never persisted; the bind stays 127.0.0.1; workspace stays 21 members. | The viewer persists nothing from a read (BR-VIEW-2 / I-VIEW-1 / I-VIEW-4). The slice extends existing crates only. | LOCKED |
| WD-LD-11 | **No new KPI ID**: slice-17 REALIZES inherited KPIs on the `/` facet (KPI-VIEW-1 front-door + discoverability; guardrails KPI-VIEW-2 / KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3). | Matches slice-08–16 (no new KPI per facet slice). Detail in `outcome-kpis.md`. | LOCKED |
| WD-LD-12 | **Persona: P-001 (Maria), NEW orientation hat**. To be appended to `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-09, slice-17). | The browser viewer is P-001's surface (slices 06–16). Front-door orientation is a distinct first-touch behavior from the graph-explorer / counter-claim-scanner / subscription-manager hats, so a new hat is minted. | LOCKED |

## Risks logged

### R-LD-1 (RISK) — No DIVERGE wave for slice-17

No `diverge/` directory for this feature — consistent with all prior OpenLore slices.
Recorded as a NON-BLOCKING risk: the job (J-002 orientation facet) is already validated in
`docs/product/jobs.yaml`; the 8 surfaces being linked are all SHIPPED (slices 06–16); the
three reads already exist on `StoreReadPort`. No design-direction ambiguity — the view
extends the existing front door and reuses the slice-06/07 patterns. No JTBD re-run required.

### R-LD-2 (RISK) — A count read failure 5xxes the whole front door

Mitigated by WD-LD-2 + US-LD-000/001 AC (graceful degrade is a HARD product commitment: a
failed count read renders the hub WITHOUT that number, never a 5xx — NFR-VIEW-6). A
behavioral test seeds an unreadable count and asserts `/` still renders the nav hub at 200.
The slice-12 `unwrap_or_default` degrade is the model.

### R-LD-3 (RISK) — The summary becomes an N+1 (per-claim/per-peer loop)

Mitigated by WD-LD-4/5 + US-LD-000 AC (a FIXED 3 aggregate reads per render, invariant to
store size; `count_*` are aggregate `COUNT(*)`; active-subs is the slice-15 single-aggregate
read via `.len()` or count-only variant). A per-row loop is REJECTED. A behavioral test
asserts read count invariant to store size.

### R-LD-4 (RISK) — A surface link is hardcoded / drifts from its route

Mitigated by WD-LD-7 + US-LD-001 AC (each link uses the route's URL CONST from
`viewer-domain`). DESIGN reuses the consts; a hardcoded path string is REJECTED. The one
const that does not exist yet (`/scrape`) is a DESIGN sub-decision (mint `SCRAPE_URL` vs a
single shared literal).

### R-LD-5 (RISK) — A fabricated 0 on a failed read misleads "empty store"

Mitigated by WD-LD-8 + US-LD-000/001 AC (the `LandingSummary` models each count as Option /
total ADT; a failed read renders "—", DISTINCT from a successful 0). A domain example +
scenario pin the distinction.

### R-LD-6 (RISK) — Open count-read approach (WD-LD-5) under-specified for DESIGN

Mitigated by surfacing it EXPLICITLY as an open DESIGN question (WD-LD-5); the PRODUCT
contract (a single aggregate read for the active-subs count, invariant to store size) holds
either way. NOT a DoR blocker. DESIGN picks the cheaper/cleaner; if it adds the count-only
variant, it is read-only and workspace stays 21.

## DoR verdict: PASSED (9/9 for both stories; Dimension 0 PASS; JTBD PASS)

See `dor-checklist.md`.

## Handoff readiness

DISCUSS artifacts complete (lean set): `feature-delta.md`, `requirements.md`,
`user-stories.md`, `acceptance-criteria.md`, `outcome-kpis.md`, `dor-checklist.md`,
`wave-decisions.md`; persona hat to be appended. Ready for DESIGN (solution-architect) once
peer review approves. No code written; no DESIGN performed. DESIGN inherits two sub-decisions
(WD-LD-5 active-subs count approach; WD-LD-7 `SCRAPE_URL` const; WD-LD-9 shape fork).

## DIVERGE note

No DIVERGE artifacts exist for this slice (`docs/feature/viewer-landing-dashboard/diverge/`
absent) — consistent with all prior OpenLore slices. Journey work is grounded in the
validated J-002 orientation facet and the existing read-only viewer foundation (slice-06
`render_landing` + the I-VIEW invariants).
