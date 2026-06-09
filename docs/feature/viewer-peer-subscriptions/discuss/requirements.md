# Requirements: viewer-peer-subscriptions (slice-15)

> Wave: DISCUSS (lean) · Owner: Luna (nw-product-owner) · 2026-06-09
> Job: **J-003c** (`docs/product/jobs.yaml`) — "Subscription is revocable without residue."
> Brownfield DELTA on slices 03 (federated-read) / 06 (htmx-scraper-viewer) / 07
> (viewer-htmx-swaps) / 08 (viewer-network-search render-only-command) / 10
> (viewer-graph-traversal net-new-route).

## 1. Context

The slice-03 CLI lets the operator subscribe (`openlore peer add <did>`), pull
(`openlore peer pull`), read federated claims (`graph query --federated`), and
unsubscribe (`openlore peer remove <did>` — soft-remove sets `removed_at`; `--purge`
deletes cached `peer_claims`). The read-only `openlore ui` viewer (slices 06–14) lets
the operator GLANCE at their store in the browser. But there is no browser surface that
LISTS who the operator currently follows, with per-peer claim counts, plus the clean
revocation path.

slice-15 adds the **federation-management VIEWING** surface: a net-new read-only
`GET /peers` view realizing the VIEWING side of **J-003c**. The "without residue"
GUARANTEE itself stays the slice-03 CLI; the view makes the subscription set legible and
surfaces the revocation path, and — by reading ONLY active subscriptions — makes the
residue-free promise VISIBLE (a removed peer vanishes from the list).

## 2. Functional Requirements

| ID | Requirement | Rationale / Source |
|---|---|---|
| FR-PS-1 | `GET /peers` lists every ACTIVE subscription (`peer_subscriptions.removed_at IS NULL`), one row per peer, keyed by peer DID. | J-003c VIEWING side; I-PS-2 active-only |
| FR-PS-2 | Each peer row shows the peer's DID rendered VERBATIM (no truncation that loses identity; the bare `did:plc:…` form). | Attribution discipline (J-003a / I-FED-1) |
| FR-PS-3 | Each peer row shows that peer's LOCAL claim count = `COUNT(*) FROM peer_claims WHERE author_did = <this peer DID>` — a PER-PEER count, NEVER a merged total across peers. | J-003a anti-merging / I-PS-3 |
| FR-PS-4 | Each peer row shows the render-only revocation command `openlore peer remove <bare-did>` as TEXT (a `<p>`/`<code>`), never an executable control. | J-003c revocation path; I-PS-1 / I-PS-8; slice-08 `render_follow_guidance` precedent |
| FR-PS-5 | A peer removed via the CLI (soft-remove OR `--purge`) is ABSENT from `/peers` on the next render — the absence IS the residue-free promise rendered. | J-003c "without residue"; I-PS-2 |
| FR-PS-6 | When there are no active subscriptions, `/peers` renders a guided empty state ("You are not subscribed to any peers.") plus the render-only starting command `openlore peer add <did>`. | J-003c onboarding; US-PS-003 |
| FR-PS-7 | `GET /peers` serves a full page (chrome + peers fragment) WITHOUT `HX-Request` and the SAME peers fragment WITH it (`Shape::from_request` fork). The render-only command + empty state live in the SAME fragment fn both shapes embed. | slice-07 `page = chrome + fragment`; I-PS-5 |
| FR-PS-8 | The active-subscription + per-peer-count read is ONE aggregate query per render, invariant to peer count (no N+1). | I-PS-3 / I-PS-4; slice-10/12 single-query discipline |

## 3. Non-Functional Requirements

| ID | NFR | Measurable criterion |
|---|---|---|
| NFR-PS-1 (read-only, CARDINAL) | `/peers` holds `StoreReadPort` only; no mutation method, no signing key, no write/subscribe/unsubscribe control. | Type: the read port declares no mutation method (a `Box<dyn StoreReadPort>` cannot mutate). xtask check-arch viewer-capability rule green. Behavioral gold: no form/`<button>`/mutating `<a>` on `/peers`. |
| NFR-PS-2 (active-only, CARDINAL) | The read filters `removed_at IS NULL`; a soft-removed/purged peer never appears. | Behavioral test: seed a soft-removed peer; assert ABSENT from `/peers`. |
| NFR-PS-3 (local-first / offline) | The read is a LOCAL DuckDB read (`peer_subscriptions` + `peer_claims`); no network seam; `/peers` references only the vendored `/static/htmx.min.js` (no CDN). | Behavioral test: `/peers` renders fully with the network down; no outbound request from the route. |
| NFR-PS-4 (no N+1) | ONE aggregate query per render regardless of peer count. | Behavioral test: query count invariant to number of active subscriptions. |
| NFR-PS-5 (loopback bind / no persistence) | Bind stays 127.0.0.1; the subscription list is computed per-request, never persisted. | Inherited I-VIEW-4 / BR-VIEW-2; no new persisted type. |
| NFR-PS-6 (plain-language errors) | A store-read failure renders a plain-language message, never a raw stack trace. | Inherited NFR-VIEW-6; `StoreReadError` surfaced cleanly. |

## 4. Business Rules

| ID | Rule | Exception / precedence |
|---|---|---|
| BR-PS-1 | A "subscription" shown on `/peers` is an ACTIVE row (`removed_at IS NULL`). A soft-removed row is NOT a subscription for display purposes (it is residue, deliberately invisible). | The slice-03 `UnsubscribedCache` relationship exists for federated READ annotation, but `/peers` never shows it. |
| BR-PS-2 | The per-peer claim count counts `peer_claims` rows attributed to that exact peer DID (`author_did = <peer>`), independent of any other peer's claims. | Mirrors the adapter `count_peer_claims(conn, peer_did)` shape; never a global or merged count. |
| BR-PS-3 | The revocation command is rendered with the BARE DID (any app-identity `#…` fragment stripped — the slice-03 `peer remove` accepted form). | Mirrors slice-08 `render_follow_guidance` bare-DID strip. |
| BR-PS-4 | Mine-vs-peer is never ambiguous on `/peers`: every row is a PEER subscription; the operator's own DID is never listed as a peer (the slice-03 `peer add` rejects self-subscription). | Inherits slice-03 "you are already your own author." |

## 5. Requirements Completeness Check

- **Functional**: FR-PS-1..8 — the list, the attribution, the per-peer count, the
  render-only command, the residue-made-visible absence, the empty state, the
  progressive-enhancement parity, the single-query read. ✓
- **NFR**: NFR-PS-1..6 — read-only, active-only, local/offline, no-N+1, loopback/no-
  persist, plain-language errors, each with a measurable criterion. ✓
- **Business rules**: BR-PS-1..4 — active-only definition, per-peer count, bare-DID
  command, mine-vs-peer. ✓

All three requirement categories present; no completeness gap.

## 6. Domain Language (ubiquitous terms)

| Term | Definition |
|---|---|
| Active subscription | A `peer_subscriptions` row with `removed_at IS NULL` — a peer the operator currently follows. |
| Soft-remove / residue | A `peer_subscriptions` row with `removed_at` set (and, without `--purge`, retained `peer_claims`). Deliberately invisible on `/peers`. |
| Per-peer claim count | `COUNT(*) FROM peer_claims WHERE author_did = <peer DID>` — how many of that peer's claims the operator holds locally. |
| Render-only command | CLI command text (`openlore peer remove <did>` / `openlore peer add <did>`) rendered as non-executable TEXT, mirroring slice-08 `render_follow_guidance`. |
| Residue-made-visible | The property that a removed peer is ABSENT from `/peers` — the J-003c "without residue" guarantee rendered. |
