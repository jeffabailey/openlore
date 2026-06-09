# Component Boundaries: viewer-landing-dashboard (slice-17)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-09 · ADR: ADR-054
> Functional paradigm (ADR-007): the boundary is pure render core vs effect shell.
> NO new crate · NO new route · workspace stays 21 members.

## 1. Crate touch map (what changes, what does not)

| Crate | Role | Change in slice-17 |
|---|---|---|
| `viewer-domain` (PURE) | The render core | `render_landing()` gains a `&LandingSummary` param and returns the full document (summary + nav hub). Add `pub struct LandingSummary` (3 × `Option<usize>`). Add `pub const SCRAPE_URL = "/scrape"`. Add a missing-number marker const (e.g. `MISSING_COUNT_MARKER = "—"`). |
| `adapter-http-viewer` (EFFECT) | The HTTP shell | `landing_page` gains `store: &dyn StoreReadPort`; resolves 3 counts via `.ok()`; builds `LandingSummary`; calls `render_landing(&summary)`. Route arm `"/" => Ok(landing_page(store.as_ref()))`. |
| `ports` (TRAIT) | `StoreReadPort` | Add ONE read-only method: `count_active_peer_subscriptions(&self) -> Result<usize, StoreReadError>` (WD-LD-5 → count-only variant, ADR-054 D3). NO mutation method added. |
| `adapter-duckdb` (ADAPTER) | DuckDB impl | Implement `count_active_peer_subscriptions`: `SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL`. One aggregate read. |
| `xtask` (ARCH RULES) | check-arch | UNCHANGED. `check_viewer_capability_boundary` is a crate-dependency-graph rule; a read-only count method does not touch it (the viewer still depends on no write adapter). |
| All other crates | — | UNCHANGED. No new crate. Workspace stays 21. |

## 2. The pure / effect boundary (ADR-007)

```
                 EFFECT SHELL                          PURE CORE
   adapter-http-viewer::landing_page          viewer-domain::render_landing
   ────────────────────────────────          ────────────────────────────
   reads 3 counts (I/O, Result)               total fn of &LandingSummary
   maps each Result→Option via .ok()   ──►    Some(n) → the number
   builds LandingSummary                      None    → MISSING_COUNT_MARKER
   wraps html_ok (200)                        renders <h1> + READ_ONLY_NOTICE
                                              + 3-count summary + nav hub
```

- **The I/O (the three reads + the Result→Option mapping) lives ONLY in the shell.**
  The pure core never sees a `Result`, a `StoreReadError`, or the store — it receives
  an already-resolved `LandingSummary` and renders totally. This is the slice-09/10/
  15/16 pure-render discipline unchanged.
- **The degrade decision (`.ok()`) is an effect-shell decision** (it is about how to
  treat an I/O failure), mirroring slice-12's `unwrap_or_default()` (ADR-048). The
  pure render only knows "this count is `Some` or `None`" and renders each totally.

## 3. The reused reads (no new orientation read invented)

| Count | Read | Status | Source |
|---|---|---|---|
| own claims | `count_claims()` | EXISTING | slice-06, `ports` ~296, `adapter-duckdb` ~281 (`SELECT COUNT(*) FROM claims`) |
| peer claims | `count_peer_claims()` | EXISTING | slice-06, `ports` ~316, `adapter-duckdb` ~488 |
| active peers | `count_active_peer_subscriptions()` | **NEW (count-only, read-only)** | ADR-054 D3; mirrors the two above; `WHERE removed_at IS NULL` reuses the slice-15 active-only definition (BR-LD-2) |

The active-peer count is the ONLY net-new surface. The DISCUSS baseline (WD-LD-4)
said "reuse the three existing reads, possibly `.len()` `list_active_peer_subscriptions`";
DESIGN chose the count-only sibling (ADR-054 D3) for symmetry + cheapness. It is a
READ method: `StoreReadPort` still declares no mutation method (I-VIEW-1), so the
read-only invariant is preserved by construction.

## 4. The count-variant's effect on `ports` / `adapter-duckdb`

- `ports`: +1 trait method signature (`count_active_peer_subscriptions`). The
  `ports`-async-trait-only xtask rule is unaffected (this is a sync read like
  `count_claims`/`count_peer_claims`/`list_active_peer_subscriptions`).
- `adapter-duckdb`: +1 impl — `let conn = self.lock_conn()?; conn.query_row("SELECT
  COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL", ...)` mapping a failure
  to `StoreReadError::Unreadable`, mirroring `count_claims` (~281). Touches ONLY
  `peer_subscriptions` (the active-only definition); does NOT join `claims` (so the
  `no_cross_table_join_elides_author` rule stays GREEN — there is no JOIN at all).
- Any OTHER `StoreReadPort` impl (test doubles) gains the same method; trivially a
  count, no mutation.

## 5. xtask boundary unchanged

`check_viewer_capability_boundary` (xtask/src/check_arch.rs ~694) is a transitive-
crate-dependency rule: the viewer crate must not depend on write/sign adapters.
Adding a read-only count method to `StoreReadPort` changes no crate dependency edge,
so the rule stays GREEN. The read-only invariant's THREE enforcement layers
(ADR-054 Enforcement) are: (1) type — the trait has no mutation method; (2) xtask —
this capability rule; (3) behavioral gold — `/` has no mutating control + store
unchanged after N requests. All three hold unchanged.

## 6. What stays out (boundary guards)

- NO new route — `GET /` exists; the route arm only gains `store.as_ref()`.
- NO new crate — workspace stays 21.
- NO mutation method, NO signing key, NO write/compose/sign/subscribe/follow control.
- NO network seam — three LOCAL reads only.
- NO per-author content / scores / threads / merge on `/` (BR-LD-1 / WD-LD-6) — the
  three numbers are aggregates; content stays on the existing attributed surfaces.
- NO deep/parameterized route as a top-level hub link (FR-LD-5).
- NO `Shape` fork on `/` (ADR-054 D5 — full-page-only; nothing targets `/`).
