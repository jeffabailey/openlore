# ADR-054: Landing Dashboard — Option-Shaped `LandingSummary` View-Model (Per-Count Independent Degrade), a Count-Only `count_active_peer_subscriptions` Read, a Navigation Hub of URL Consts (mint `SCRAPE_URL`), and a Full-Page-Only `GET /`

- **Status**: Accepted
- **Date**: 2026-06-09
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-landing-dashboard` (slice-17)
- **Feature**: viewer-landing-dashboard (slice-17)
- **Extends**: ADR-052 (the slice-15 active-subscription survey + `list_active_peer_subscriptions`; this ADR adds a SIBLING count-only read on the SAME `peer_subscriptions WHERE removed_at IS NULL` active-set definition), ADR-053 (the slice-16 read-only `/search` follow-state — the most recent net-new-read DESIGN, mirrored for structure), ADR-048 (slice-12 `counter_presence_for(...).unwrap_or_default()` — the graceful-degrade precedent generalized here to per-count `Option`), ADR-034 (the URL-const single-source-of-truth for nav links: `MY_CLAIMS_URL`, `PEER_CLAIMS_URL`, …), ADR-033 (the `Shape::from_request` fragment-vs-full-page fork — explicitly NOT applied to `/` here), ADR-030 (the read-only `StoreReadPort` with NO mutation method — the count-only variant is a read), ADR-031 (offline-first vendored htmx, no CDN), ADR-009 (Hexagonal + Modular Monolith), ADR-007 (functional paradigm — pure render core, effect shell at the I/O edge)

## Context

The `openlore ui` viewer has shipped 11 surfaces (slices 06–16): `/claims`,
`/claims/{cid}`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`,
`/scrape`, `/peers`. But the landing `GET /` (`render_landing`,
`crates/viewer-domain/src/lib.rs` ~554) renders ONLY an `<h1>`, the
`READ_ONLY_NOTICE`, and a SINGLE hardcoded `<a href="/claims">` link — it "queries
nothing" (`landing_page`, `crates/adapter-http-viewer/src/lib.rs` ~414, is the ONLY
handler that takes no store). The other 8 surfaces are cross-linked only WITHIN
features, so a user opening `/` cannot discover `/peers`, `/search`, `/score`,
`/project`, `/philosophy`, `/scrape`, `/peer-claims`. The 11-slice viewer is not
navigable as a coherent app from its own front door, and the landing surfaces NO
store state — despite `count_claims` (~296), `count_peer_claims` (~316), and
`list_active_peer_subscriptions` (~445) all already existing on the read-only
`StoreReadPort`.

DISCUSS is APPROVED (DoR 9/9). The validated job is **J-002** (the ORIENTATION /
FRONT-DOOR facet). slice-17 turns `GET /` into a read-only navigation hub +
at-a-glance LOCAL store summary (three counts: own claims, peer claims, active
peers). It extends the existing route (no new route), threads the read-only store
the viewer already holds into `landing_page`, and reuses the existing reads.

DISCUSS handed DESIGN three open sub-decisions, each with a real downside if
decided wrong:

1. **WD-LD-5 — the active-subs count-read approach.** `.len()` of the existing
   `list_active_peer_subscriptions` (zero new port surface, but materializes the
   full active set — a `peer_subscriptions LEFT JOIN peer_claims … GROUP BY peer_did
   … COUNT(pc.cid)` — and decodes every row just to count rows) vs a tiny count-only
   `count_active_peer_subscriptions()` (`SELECT COUNT(*) FROM peer_subscriptions
   WHERE removed_at IS NULL`, mirroring `count_claims`/`count_peer_claims`).
2. **WD-LD-7 — the `/scrape` link.** `/scrape` is the ONLY entry-point surface
   lacking a URL const (it uses an `action="/scrape"` literal in the scrape form,
   `viewer-domain` ~1359). The hub needs a const-or-literal decision (R-LD-4: a
   hardcoded path drifts from its route).
3. **WD-LD-9 — the shape of `GET /`.** Does `/` fork by `Shape` (htmx fragment vs
   full page) like the other surfaces, or stay full-page-only? (R parity, C-5.)

## Decision

### D1 — `LandingSummary`: a flat view-model of three INDEPENDENT `Option<usize>` counts

The pure render takes a flat `LandingSummary` whose three fields each model a
single count's read outcome as `Option<usize>`:

```
pub struct LandingSummary {
    pub own_claims: Option<usize>,
    pub peer_claims: Option<usize>,
    pub active_peers: Option<usize>,
}
```

- `Some(n)` = a SUCCESSFUL read of `n` (including `Some(0)` — an honest empty
  store). `None` = that count's read FAILED (a missing-number state). This makes
  `0 ≠ missing` a TYPE-LEVEL distinction (WD-LD-8 / BR-LD-3 / C-7) — a fabricated 0
  on a failed read is unrepresentable: the only way to get `Some(0)` is a read that
  returned 0.
- The three counts degrade **independently**: one failed read leaves the other two
  `Some(_)`. There is NO all-or-nothing summary state (rejected — see Alternatives),
  because a single transient `count_peer_claims` failure must not blank the
  own-claims and active-peer numbers the operator can legitimately see (domain
  example 3, US-LD-000/001).
- A flat struct of three `Option`s is chosen over a richer ADT (e.g. one enum per
  count, or a `SummaryState` sum type) because the three counts have IDENTICAL
  shape and IDENTICAL degrade semantics — three parallel `Option<usize>` is the
  total, exhaustive, mutation-resistant model with the least ceremony. The pure
  render is a **total function of `LandingSummary`** (every field combination
  renders; no panic, no I/O).

### D2 — Per-count degrade in the EFFECT shell via `.ok()` (the slice-12 precedent generalized)

`landing_page` resolves each count by calling the read and mapping
`Result<usize, StoreReadError>` to `Option<usize>` with `.ok()` — INDEPENDENTLY,
one `.ok()` per count. This is the slice-12 `counter_presence_for(...)
.unwrap_or_default()` graceful-degrade precedent (ADR-048) generalized: there, a
failed presence read degraded to "no flags"; here, a failed count read degrades to
`None` (a missing-number marker). A failed read NEVER propagates a 5xx, NEVER
echoes a `StoreReadError` cause, NEVER fabricates 0. The shell builds the
`LandingSummary` from the three `.ok()` results and passes it to the pure render
(I-LD-2 / NFR-VIEW-6). The route always returns a 200 full page.

### D3 — Add a count-only `count_active_peer_subscriptions()` read (WD-LD-5 RESOLVED → count-only variant)

A new read method is added to the read-only `StoreReadPort`:

```
fn count_active_peer_subscriptions(&self) -> Result<usize, StoreReadError>;
```

`adapter-duckdb` implements it as a single aggregate
`SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL` — the SAME
active-only definition as ADR-052's `list_active_peer_subscriptions` (a
soft-removed row, `removed_at` set, is residue and excluded; I-PS-2 / BR-LD-2),
expressed as one COUNT rather than a materialized LEFT JOIN + GROUP BY +
per-peer `COUNT(pc.cid)`.

**Rationale (count-only chosen over `.len()`):**
- **Symmetry.** The other two summary counts ARE count-only aggregates
  (`count_claims`, `count_peer_claims`); a third count-only sibling makes the
  three-count summary three structurally identical `COUNT(*)` reads — the cleanest
  read shape for the slice's "three aggregate reads, invariant to store size"
  contract (C-4 / I-LD-4).
- **Cheapness / honest no-N+1.** `.len()` of `list_active_peer_subscriptions`
  executes a `LEFT JOIN peer_claims … GROUP BY peer_did … COUNT(pc.cid) ORDER BY`
  and DECODES every `PeerSubscriptionSummary` row (peer_did, handle, subscribed_at,
  per-peer claim count) — work the count throws away. `SELECT COUNT(*) … WHERE
  removed_at IS NULL` touches only `peer_subscriptions` and decodes ONE scalar. The
  count is invariant to store size either way, but the count-only read does the
  minimum work for the number the front door actually needs.
- **Cost is one tiny read method.** The downside (a new port surface) is one
  read-only method mirroring two that already exist, plus one `COUNT(*)` impl in
  `adapter-duckdb`. It adds NO mutation method (read-only by construction; the trait
  still declares no `write_*`/`sign`), NO new crate (workspace stays 21), and stays
  within the `check_viewer_capability_boundary` rule (a crate-dependency-graph rule
  the count satisfies trivially — the viewer depends on no write adapter).

### D4 — Mint `SCRAPE_URL = "/scrape"` (WD-LD-7 RESOLVED → mint the const)

A new URL const is added to `viewer-domain`:

```
pub const SCRAPE_URL: &str = "/scrape";
```

`/scrape` is the only entry-point surface lacking a const (the other 7 —
`MY_CLAIMS_URL`, `PEER_CLAIMS_URL`, `SEARCH_URL`, `SCORE_URL`, `PROJECT_URL`,
`PHILOSOPHY_URL`, `PEERS_URL` — already exist). Minting `SCRAPE_URL` gives the hub
EIGHT URL consts, one per surface, so EVERY hub link is `a href=(CONST)` and no link
is a hardcoded literal that could drift (C-3 / FR-LD-4 / R-LD-4). The scrape form's
existing `action="/scrape"` literal (`viewer-domain` ~1359) and the adapter route
arm `"/scrape" =>` (~401) MAY be migrated to the const for full single-source-of-
truth, but that migration is OPTIONAL polish, not required by this slice (the slice's
contract is that the HUB links use consts). The const is the canonical path for the
nav hub.

### D5 — `GET /` stays FULL-PAGE-ONLY; `render_landing` returns a complete document (WD-LD-9 RESOLVED → full-page-only)

`GET /` does NOT fork by `Shape`. `render_landing(summary)` returns a complete HTML
document (`String`, as today) — the existing chrome (`page_head`, `<!DOCTYPE>`,
`<h1>`, `READ_ONLY_NOTICE`) plus the summary + nav hub. `landing_page` ignores
`shape` for `/` (the route arm stays `"/" => Ok(landing_page(store.as_ref()))`).

**Rationale.** The landing is the ENTRY full page — the surface a browser opens
fresh, bookmarks, or reloads. NOTHING targets `/` with an `hx-target`/`hx-get`: the
fragment fork (ADR-033) exists so a swap LANDS on a sub-region (`#view-panel`,
`#claims-table`, `#traversal-results`, `#peers`), but `/` has no such swap region
and is never the target of one — the nav-hub links point AT the other surfaces
(which fork), not at `/`. Adding a `Shape` fork to `/` would mint a fragment fn and
a swap-target id with zero consumer — speculative complexity rejected by
simplest-solution-first. Parity (C-5 / NFR-LD-6) is satisfied trivially and by
construction: there is ONE render, so the no-JS full page and any (non-existent)
htmx request return the SAME bytes — they cannot differ because there is only one
shape. The full-page-only choice MATCHES the current `render_landing() -> String`
shape (slice-06), so the change is purely additive (gains the `summary` param + the
hub markup; keeps the return type).

## Alternatives Considered

### For D1/D2 (degrade model)

- **One all-or-nothing `Result<Summary, _>` for the whole summary** — a single
  failed count would blank ALL three numbers (or 5xx). REJECTED: violates the
  per-count-independent-degrade AC (domain example 3 — own-claims and active-peer
  numbers must still show when peer-claims fails) and risks the 5xx the cardinal
  I-LD-2 forbids.
- **Fabricate 0 on a failed read (`unwrap_or(0)`)** — REJECTED: misleads "empty
  store" (R-LD-5 / BR-LD-3). The whole point of `Option` is to make
  `0 ≠ missing` representable.
- **A richer per-count ADT (`enum CountState { Read(usize), Unreadable }`)** —
  semantically equivalent to `Option<usize>` but heavier. REJECTED for this thin
  slice: `Option` IS the two-state total model; a bespoke enum adds names without
  adding states. (If a future count needed a THIRD state — e.g. "not applicable" —
  the ADT would earn its place; today it does not.)

### For D3 (count-read approach, WD-LD-5)

- **`.len()` of `list_active_peer_subscriptions`** — zero new port surface, but
  materializes + decodes the full active-subscription set (LEFT JOIN + GROUP BY +
  per-peer COUNT) just to count rows. REJECTED: asymmetric with the other two
  count-only reads, and does throwaway work. Viable and correct (the active set is
  tiny at dogfood scale), but the count-only variant is cleaner and cheaper for one
  trivial read method.

### For D4 (`/scrape` link, WD-LD-7)

- **A single shared `/scrape` literal in `viewer-domain`** — REJECTED: a const IS
  the single-source-of-truth mechanism the other 7 surfaces use; a bare literal
  (even shared) is inconsistent and invites drift.
- **Inline `"/scrape"` at the hub** — REJECTED outright (R-LD-4: hardcoded path
  drift; violates C-3 / FR-LD-4).

### For D5 (shape, WD-LD-9)

- **Fork `/` by `Shape` (mint a landing fragment + swap-target id)** — REJECTED:
  no consumer targets `/`; speculative complexity (simplest-solution-first). Parity
  is better served by a SINGLE render than by two renders kept in sync.

## Consequences

### Positive
- `0 ≠ missing` is a TYPE-LEVEL invariant (`Option<usize>` per count); a fabricated
  0 on failure is unrepresentable.
- Per-count independent degrade: any one count can fail without blanking the others
  or 5xxing the front door (cardinal I-LD-2 honored).
- The three-count summary is three structurally identical `COUNT(*)` aggregate
  reads, invariant to store size (C-4 / no N+1), with the count-only active-subs
  read doing the minimum work.
- All 8 hub links use URL consts (8th minted: `SCRAPE_URL`); no hardcoded path can
  drift (C-3 / R-LD-4).
- One render for `/` (full-page-only) → parity by construction; the change is
  purely additive over the current `render_landing() -> String`.
- Read-only preserved: no mutation method added (the count-only read is a read);
  workspace stays 21; no new crate; no new route; loopback bind unchanged; nothing
  persisted.

### Negative / trade-offs
- One new read method on `StoreReadPort` (`count_active_peer_subscriptions`) + one
  `COUNT(*)` impl in `adapter-duckdb` (vs zero new surface for `.len()`). Accepted:
  one trivial read-only method buys symmetry + cheapness; it is the only ports/
  adapter-duckdb touch in the slice.
- `/` gains no htmx fragment, so a future need to htmx-swap INTO `/` would require a
  follow-up shape fork. Accepted: no such consumer exists; YAGNI.

## Enforcement (Earned Trust — how the design proves it honors its contract)

- **Read-only (3 layers, cardinal C-1):** (a) TYPE — `StoreReadPort` declares no
  mutation method; the added `count_active_peer_subscriptions` is a read returning
  `Result<usize, _>`, so a `Box<dyn StoreReadPort>` remains structurally incapable
  of mutating. (b) xtask `check_viewer_capability_boundary` (crate-dependency-graph
  rule) stays GREEN — the viewer depends on no write adapter. (c) BEHAVIORAL gold —
  a port-to-port test asserts `/` contains no `<form>`/`<button>`/mutating control,
  only navigation `<a href>`, and that store contents are byte-identical before and
  after N `GET /` requests (acceptance-criteria.md Theme 3 `@property`).
- **Missing ≠ zero (BR-LD-3):** a behavioral test seeds an unreadable count and
  asserts `/` renders the missing-number marker for THAT count (distinct from a
  seeded real 0), the hub still renders, and the route returns 200 — never a 5xx,
  never a raw stack trace (Theme 4). This is Earned Trust applied to the store-read
  dependency: the design exercises the substrate LYING (a transient
  `StoreReadError`) and proves the front door survives it.
- **No N+1 (C-4):** a `@property` behavioral test asserts the read count is
  invariant to store size (N own claims, M peer claims, K active peers — the read
  count does not grow with N/M/K; Theme 5).
- **No network (C-2):** a behavioral test asserts `/` renders fully network-down,
  makes no outbound request, and references only the vendored `/static/htmx.min.js`
  (Theme 5).
- **URL-const links (C-3):** a behavioral test asserts each hub link's `href`
  equals the route's const and no link is a drifting literal (Theme 2).
