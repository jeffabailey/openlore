# Wave Decisions: viewer-peer-subscriptions (slice-15) — DISCUSS

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-09
> Feature type: User-facing · JTBD: YES (J-003c) · UX depth: Lightweight · Walking skeleton: brownfield DELTA (thin)
> Brownfield DELTA on slices 03 (federated-read) / 06 (htmx-scraper-viewer) / 07 (viewer-htmx-swaps) / 08 (viewer-network-search) / 10 (viewer-graph-traversal).

This slice adds a net-new read-only **`GET /peers`** view to the `openlore ui` viewer:
the federation-management VIEWING surface realizing the VIEWING side of **J-003c**
("Subscription is revocable without residue"). It lists the operator's currently-
subscribed peers (active `peer_subscriptions` rows, `removed_at IS NULL`) — each peer's
DID + its local claim count — plus a render-only `openlore peer remove <did>` revocation
command per peer; a guided empty state when there are none. No new product job — every
non-`@infrastructure` story traces to the already-validated **J-003c**.

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`, `personas/`,
`journeys/`). Proceeded without re-running JTBD; J-003c is validated (slice-03 DISCUSS,
changelog 2026-05-27).

## Scope Assessment: PASS — 3 stories (1 infra + 2 user-visible), 1 bounded context (the viewer's `/peers` surface), 4 integration points, estimated ~1 day

Carpaccio gate, run BEFORE journey-visualization investment (Phase 1.5):

- **Stories**: 3 (1 infra + 2 user-visible). Well within ≤10. PASS.
- **Bounded contexts**: 1 — the viewer `/peers` surface. Extends `viewer-domain` (pure),
  `adapter-http-viewer` (effect), `ports` (ONE new read method), `adapter-duckdb` (ONE
  new read impl + SQL), `cli`, `xtask`. All existing; NO new crate. PASS.
- **Walking-skeleton integration points**: 4 — (1) the new `GET /peers` route, (2) the new
  `StoreReadPort` method, (3) the new `adapter-duckdb` read impl + active-only/per-peer-
  count SQL, (4) the new `render_peers_fragment` + `render_remove_guidance` (mirrors
  slice-08). Within ≤5. PASS.
- **Estimated effort**: ~1 day. PASS.
- **Multiple independent outcomes**: NO — all stories serve the single outcome "see who I
  follow + the clean revocation path in the browser" (the empty state is a facet, not a
  separate outcome). PASS.
- **Verdict**: RIGHT-SIZED. Slightly bigger than the flag slices (slice-12/13) — a NEW
  route + a NEW read + a NEW render — but still a thin ~1-day vertical slice. The thing
  that would make it oversized — a write/subscribe/unsubscribe affordance in the viewer,
  a `peer pull` / DID-re-resolution network seam, or a per-peer claims drill-in — is
  explicitly OUT of scope (WD-PS-1 read-only, WD-PS-4 LOCAL-only). If DESIGN finds the
  read + render exceed 1 day, US-PS-003 (empty state) splits from US-PS-002.

## Locked decisions (WD-PS-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-PS-1 (CARDINAL) | `/peers` is **READ-ONLY / no key**: it renders the subscription set + the revocation COMMAND TEXT only — never mutates, never holds a key, adds no write/subscribe/unsubscribe control. The unsubscribe is render-only `openlore peer remove <did>` TEXT (mirroring the slice-08 render-only `peer add` follow guidance). Subscribe/unsubscribe stays EXCLUSIVELY the slice-03 CLI. | The read-only / key-less viewer is cardinal across slices 06–14. Adding a LOCAL subscription read preserves it exactly as the slice-10 survey reads did. The unsubscribe must stay a deliberate CLI action; the viewer has no key. Load-bearing read-only boundary (KPI-VIEW-2). | LOCKED |
| WD-PS-2 (CARDINAL) | **Active-only / residue made visible**: `/peers` lists ONLY active subscriptions (`removed_at IS NULL`). A peer removed via the CLI (soft-remove OR `--purge`) VANISHES from `/peers` — that absence IS the J-003c "revocable without residue" promise rendered. The read NEVER shows soft-removed (`UnsubscribedCache`) rows. | The defining product property of the slice: the residue-free guarantee made VISIBLE, not merely trusted. The slice-03 `soft_remove` sets `removed_at`; the filter is the rendering of the promise. | LOCKED |
| WD-PS-3 (= J-003a) | **Per-peer, never merged**: each peer is its own attributed row; the claim count is PER-PEER (`COUNT(*) FROM peer_claims WHERE author_did = <peer>`), NEVER a merged total across peers, never a "consensus peer" row. | Anti-merging extends to the subscription view (J-003a / I-FED-1). The existing adapter `count_peer_claims(conn, peer_did)` confirms the per-peer shape. | LOCKED |
| WD-PS-4 | **LOCAL-only / offline**: the subscription list + per-peer counts are a LOCAL DuckDB read (`peer_subscriptions` + `peer_claims`); NO network seam. `/peers` renders fully network-down; references only the vendored `/static/htmx.min.js`. | Distinct from `/search` (indexer) and `/scrape` (GitHub). Carries KPI-5 / KPI-VIEW-5. `/peers` shows the LOCAL state as-is; no `peer pull`, no DID re-resolution. | LOCKED |
| WD-PS-5 | **NEW read-only `StoreReadPort` method + NEW SQL (adapter-duckdb)** — the first slice since slice-10 to add a read method. Returns active subscriptions (`removed_at IS NULL`) each with its per-peer claim count, in ONE aggregate query (no N+1). | The viewer reads through the read-only `StoreReadPort`; no existing method lists active subscriptions with counts (the write-side `PeerStoragePort::list_active_subscriptions()` carries no counts). The single-aggregate-query discipline mirrors slice-10/12; a per-peer `count_peer_claims` fold is REJECTED. | LOCKED |
| WD-PS-6 | **Render-only revocation command, single source of truth**: a `PEER_REMOVE_GUIDANCE_PREFIX` const + a `render_remove_guidance(bare_did)` fn (mirroring slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX` / `render_follow_guidance`) emit `openlore peer remove <bare-did>` as render-only `<p>`/`<code>` TEXT — never an `<a>` that executes, never a form. Bare-DID strip mirrors `render_follow_guidance`. | REUSES the slice-08 render-only-CLI-command precedent verbatim in shape; one mutation site keeps the command text consistent with the slice-03 verb. | LOCKED |
| WD-PS-7 | **Progressive enhancement + parity**: `/peers` serves a full page (chrome + fragment) WITHOUT `HX-Request` and the SAME fragment WITH it (slice-07 `Shape::from_request` fork). The render-only command + empty state live in the SAME fragment fn both shapes embed. | Reuses the slice-07 / slice-10 `page = chrome + fragment` pattern verbatim. A swap is a nicety, never a requirement; the no-JS full page is the contract. | LOCKED |
| WD-PS-8 | **Zero new persisted types; loopback-only bind; no new crate.** The subscription list is computed per-request, never persisted; the bind stays 127.0.0.1; workspace stays 21 members. | The viewer persists nothing from a read (BR-VIEW-2 / I-VIEW-1 / I-VIEW-4). The slice extends existing crates only. | LOCKED |
| WD-PS-9 | **No new KPI ID**: slice-15 REALIZES inherited KPIs on the `/peers` facet (KPI-FED-4 read side; KPI-VIEW-1; guardrails KPI-VIEW-2 / KPI-AV-2 / KPI-FED-1/2 / KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3). | Matches slice-08–14 (no new KPI per facet slice). Detail in `outcome-kpis.md`. | LOCKED |
| WD-PS-10 | **Persona: P-001 (Maria), NEW subscription-manager hat**. Appended to `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-09, slice-15). | The browser viewer is P-001's surface (slices 06–14). Federation-management is a distinct scanning behavior from the counter-claim-scanner / graph-explorer hats, so a new hat is minted (not extended). | LOCKED |

## Risks logged

### R-PS-1 (RISK) — No DIVERGE wave for slice-15

There is no `diverge/` directory for this feature — consistent with all prior OpenLore
slices. Recorded as a NON-BLOCKING risk: the job (J-003c) is already validated in
`docs/product/jobs.yaml`, and the journey is the slice-03 `subscribe-and-read-federated.yaml`
(step 4, `peer remove`), grounded verbatim. No design-direction ambiguity — the view
mirrors the slice-08 render-only-command + slice-10 net-new-route patterns. No JTBD re-run
required.

### R-PS-2 (RISK) — New read becomes N+1 (one count query per peer)

Mitigated by WD-PS-5 + US-PS-001 AC (the single-aggregate-query-per-render is a HARD
product commitment: active-subscriptions joined to a per-peer `COUNT(*)`, ONE query). A
behavioral test asserts query count invariant to peer count. DESIGN owns the exact SQL
(correlated subquery vs `LEFT JOIN … GROUP BY`); a per-peer `count_peer_claims` fold is
explicitly REJECTED. Tracked into DESIGN/DISTILL.

### R-PS-3 (RISK) — A soft-removed peer leaks into `/peers` (residue-made-visible broken)

Mitigated by WD-PS-2 + US-PS-002/003 AC (the read filters `removed_at IS NULL`); a
behavioral test seeds a soft-removed peer and asserts it is ABSENT from `/peers`. The
slice-03 `soft_remove` sets `removed_at` — the precondition for the filter.

### R-PS-4 (RISK) — Render-only command misread as a button / executed

Mitigated by WD-PS-6 (REUSES the slice-08 `render_follow_guidance` shape, already vetted
as render-only `<p>`/`<code>` TEXT); the viewer holds no key (WD-PS-1). A behavioral gold
asserts no form/`<button>`/mutating `<a>` on `/peers`.

### R-PS-5 (RISK) — Per-peer count merged into a total (anti-merging broken)

Mitigated by WD-PS-3 + US-PS-002 AC (the count is PER-PEER, `WHERE author_did = <this
peer>`); the existing `count_peer_claims(conn, peer_did)` confirms the per-peer shape; a
domain example with two peers of distinct counts (5 vs 3, never 8) pins it.

## DoR verdict: PASSED (9/9 for all 3 stories; Dimension 0 PASS; JTBD PASS)

See `dor-checklist.md`.

## Handoff readiness

DISCUSS artifacts complete (lean set): `feature-delta.md`, `requirements.md`,
`user-stories.md`, `acceptance-criteria.md`, `outcome-kpis.md`, `dor-checklist.md`,
`wave-decisions.md`; persona hat to be appended. Ready for DESIGN (solution-architect)
once peer review approves. No code written; no DESIGN performed.

## DIVERGE note

No DIVERGE artifacts exist for this slice (`docs/feature/viewer-peer-subscriptions/diverge/`
absent) — consistent with all prior OpenLore slices. Journey work is grounded in the
validated J-003c job statement (slice-03) and the slice-03
`subscribe-and-read-federated.yaml` journey (step 4, `peer remove`).

---

# Wave Decisions: viewer-peer-subscriptions (slice-15) — DESIGN

> Wave: DESIGN · Owner: Morgan (nw-solution-architect) · 2026-06-09
> Mirrors the slice-10 (`viewer-graph-traversal`) DESIGN shape: a net-new
> read-only route + a net-new read method + a new pure render, reuse-first.
> Artifacts: `design/architecture-design.md`, `design/component-boundaries.md`,
> `design/data-models.md`, `design/technology-stack.md`, **ADR-052**.

## Architecture style + paradigm (UNCHANGED)

Hexagonal + Modular Monolith (ADR-009); functional (ADR-007 — pure render/ADTs in
`viewer-domain`, effect shell at the I/O edge in `adapter-http-viewer`, function
signatures as ports). Default modular-monolith reaffirmed (team-of-one dogfood,
single bounded context); no microservice/event-sourcing/CQRS warranted.

## Locked DESIGN decisions (DD-PS-*)

| # | Decision | WD trace | ADR |
|---|---|---|---|
| DD-PS-1 | **ONE aggregate query** for the active-subscription survey: `peer_subscriptions LEFT JOIN peer_claims ON author_did = peer_did`, `WHERE removed_at IS NULL`, `GROUP BY` the subscription identity, `COUNT(pc.cid)` — invariant to peer count, no N+1. A per-peer `count_peer_claims` fold is REJECTED. | WD-PS-5 / WD-PS-8 / R-PS-2 | ADR-052 D1 |
| DD-PS-2 | **`LEFT JOIN` + `COUNT(pc.cid)`** (not inner JOIN / `COUNT(*)`) so a subscribed-but-never-pulled peer stays in the result at count 0 (US-PS-002 Ex 2). Correlated subquery is an equivalent drop-in (same contract); JOIN form chosen for single-scan readability. | WD-PS-3 | ADR-052 D1 |
| DD-PS-3 | **`list_active_peer_subscriptions(&self) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>`** on the read-only `StoreReadPort` — NO mutation method added (read-only by construction, I-PS-1). | WD-PS-1 / WD-PS-5 | ADR-052 D1 |
| DD-PS-4 | **`PeerSubscriptionSummary` DTO lives in `ports`** beside `ClaimRow`/`PeerClaimRow`/`SurveyRow` — `{ peer_did (non-Option), peer_handle, subscribed_at, local_claim_count: u64 }`. | WD-PS-3 | ADR-052 D2 |
| DD-PS-5 | **`PeersView` ADT** = `Subscriptions { peers } \| NoSubscriptions` (pure render input; total match). The empty / read-error case degrades to `NoSubscriptions` in the shell BEFORE the ADT is built. | WD-PS-2 / WD-PS-7 | ADR-052 D2 |
| DD-PS-6 | **Render-only revocation command reuses the slice-08 pattern**: `PEER_REMOVE_GUIDANCE_PREFIX` + `render_remove_guidance(peer_did)` (bare-DID strip, render-only `<p>`/`<code>` TEXT, never executable); empty-state `PEER_ADD_GUIDANCE_PREFIX`. One mutation site per command. | WD-PS-6 | ADR-052 D3 |
| DD-PS-7 | **`GET /peers` handler** (effect shell): read → map to `PeersView` → `Shape` fork → render. NO query param (lists the whole active set). NO new `ViewerServer` field (reads the store it already holds, mirrors `/peer-claims`). Read failure degrades gracefully (guided message, never 5xx). Add the `/peers` nav link. | WD-PS-1 / WD-PS-7 | ADR-052 |
| DD-PS-8 | **Parity by construction**: `render_peers_page` EMBEDS `render_peers_fragment`; both shapes embed the SAME `NoSubscriptions` arm. | WD-PS-7 | ADR-052 |
| DD-PS-9 | **xtask check-arch UNCHANGED**: no pure-core allowlist edge (the render is a total fn of the flat DTO — NO new dependency edge), no capability-rule change, anti-merging SQL rule GREEN by construction (SQL names `peer_subscriptions` + `peer_claims`, not the standalone `claims` table). | WD-PS-1 / WD-PS-3 | ADR-052 D4 |
| DD-PS-10 | **No new crate (21 members), no new external dependency, no new persisted type, loopback-only bind** — all UNCHANGED. | WD-PS-8 | ADR-052 |

## Quality gates (DESIGN)

- [x] Requirements traced to components (FR-PS-1..8 / NFR-PS-1..6 → ports/adapter-duckdb/viewer-domain/adapter-http-viewer; see architecture-design.md §5).
- [x] Component boundaries with clear responsibilities (component-boundaries.md).
- [x] Technology choices in ADRs with alternatives (ADR-052; stack UNCHANGED, technology-stack.md).
- [x] Quality attributes addressed (ISO 25010 table; perf = ONE aggregate query; reliability = graceful degrade; security = read-only/no-key/loopback).
- [x] Dependency-inversion compliance (ports/adapters, dependencies inward; NO new edge).
- [x] C4 diagrams (L1 + L2 Mermaid; L3 not warranted — 5 crates, thin additions).
- [x] Integration patterns specified (driving = GET /peers; driven = StoreReadPort; sync in-process).
- [x] OSS preference validated (no proprietary; all in-workspace / already pinned).
- [x] AC behavioral, not implementation-coupled (the DTO/ADT/SQL are the seam; the AC drive HTTP).
- [x] External integrations annotated — NONE on this route (LOCAL-only); no contract test required.
- [x] Architectural enforcement tooling recommended (cargo xtask check-arch — UNCHANGED this slice; import-linter rejection reaffirmed).
- [ ] Peer review completed and approved (solution-architect-reviewer — pending).

## DESIGN handoff readiness

DESIGN artifacts complete (lean set mirroring slice-10): `architecture-design.md`,
`component-boundaries.md`, `data-models.md`, `technology-stack.md`, ADR-052.
Ready for DISTILL (acceptance-designer) once peer review approves. The driving
surface is `GET /peers` (port-to-port through the real `openlore ui` subprocess);
the four cardinals (read-only / active-only / per-peer / no-N+1) each have a named
structural enforcement point (architecture-design.md §5–6).
