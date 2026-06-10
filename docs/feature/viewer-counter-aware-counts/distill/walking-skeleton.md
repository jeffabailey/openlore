# Walking Skeleton — viewer-counter-aware-counts (slice-18) · DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-09 · ADR-055

## Strategy (Architecture of Reference + Project Infrastructure Policy)

**Brownfield DELTA — no Feature-0 walking skeleton.** The `openlore ui` viewer, the
read-only `StoreReadPort`, the indexed counter-reference tables (slice-12), the slice-17
`LandingSummary` + `render_landing` + `MISSING_COUNT_MARKER`, and the slice-06 `/claims`
header all already exist. Per the Architecture of Reference, the port treatments are
fixed by the project policy (`docs/architecture/atdd-infrastructure-policy.md`):

| Port class | Port | Treatment (this slice) |
|---|---|---|
| Driving | `GET /` + `GET /claims` (HTTP) | REAL `openlore ui` subprocess (`ViewerServer::start`) + in-test HTTP GET |
| Driven internal | `StoreReadPort::count_countered_own_claims` (DuckDB) | REAL local DuckDB, seeded via production `claim add` + `peer add` + `peer pull` |
| Driven external / non-deterministic | (none — the count is a LOCAL aggregate, no network edge) | n/a — offline-STRONGER than `/search`/`/scrape` |

`[policy-mode] inherit` · `[port-mode] inherit` (`tests/common/state_delta.rs` already
present; the acceptance layer here is layer-3 subprocess + real-I/O, so it uses traditional
HTTP-body assertions per Mandate 8 layer-4+ allowance — the universe is the rendered surface).

## The one walking-skeleton scenario

`CC-WS` — **the_front_door_shows_how_many_own_claims_are_countered** (US-CC-001 Theme 1),
tagged `@walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy`.

The thinnest complete thread the slice can demo end-to-end:

```
viewer (real `openlore ui` subprocess)
  → count_countered_own_claims() — one LOCAL COUNT(DISTINCT) aggregate over the slice-12
    counter-reference tables (claim_references ∪ peer_claim_references, ref_type='counters')
  → the extended LandingSummary { …, countered_own_claims: Some(3) }
  → render_landing → render_countered(Some(3)) → "(3 countered)"
  → the GET / full-page HTML the operator's browser shows: "12 own claims (3 countered)"
```

```gherkin
@us-cc-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy
Scenario: The front door shows how many of my own claims are countered
  Given Maria's store has 12 own claims, 3 of which have ≥1 counter
    (one countered by both Rachel and Tobias)
  When she opens GET / in the openlore ui viewer
  Then the landing summary shows "12 own claims" with "(3 countered)" beside it
  And the own-claims count "12" is unchanged by the presence of the countered count
```

### Litmus test (Mandate 5 / Dim 5 user-centricity)

1. **Title is a user goal** — "shows how many of my own claims are countered" (orientation),
   not "the route threads a 4th Option field through the render" (technical flow). PASS.
2. **Given/When are user actions** — "Maria's store has 12 own claims, 3 countered" + "she
   opens GET /", not "the LandingSummary struct gains a field". PASS.
3. **Then is a user observation** — "the landing summary shows '12 own claims (3 countered)'",
   not "render_countered returns the string". PASS.
4. **Non-technical stakeholder confirms "yes, that's what users need"** — a node operator
   immediately sees how much of her own work has been disputed at the front door. PASS.

### Why this is the riskiest-assumption thread

It exercises the WHOLE new vertical at once: the new LOCAL `count_countered_own_claims`
aggregate (proven genuine by the seed's direct ADR-055 `COUNT(DISTINCT)` oracle returning
exactly 3 — including the presence-once collapse of the twice-countered claim), the 4th
additive `LandingSummary` field, the shared `render_countered` helper, and the additive
render beside the unchanged own-claims line. If this thread is green, the count is read,
threaded, and rendered correctly on the headline surface.

### Demo-ability

A node operator opens `http://127.0.0.1:<port>/` and sees "12 own claims (3 countered)".
No SQL, no CLI flags, no technical framing — the disputed-claim awareness count is right
there beside the own-claims count, read-only and offline.

## RED confirmation

The WS scenario COMPILES and FAILS for the right reason (MISSING_FUNCTIONALITY): the seed
runs the full production write path successfully (12 own claims via `claim add`; the 3 peer
counters via `peer add` + `peer pull`; the direct ADR-055 oracle confirms the seeded
countered count is exactly 3), the viewer spawns, `GET /` returns a 200 full page showing
"12 own claims" — and the test fails at `assert_landing_countered_count` because
"(3 countered)" is ABSENT (the production `render_landing` does not render the countered
count yet, and `count_countered_own_claims` / `render_countered` / the 4th field do not
exist). NOT a setup/import/fixture error. The ATs drive `GET /` via subprocess HTTP (never
the Rust `render_landing` signature), so the DELIVER signature change does not affect AT
compilation.
