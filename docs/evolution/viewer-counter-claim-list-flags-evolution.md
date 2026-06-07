# Evolution: viewer-counter-claim-list-flags (slice-12 at-a-glance "Countered" presence flag on the read-only `GET /claims` list)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-counter-claim-list-flags/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `deliver/`) and ADR-048 under `docs/adrs/`; this file is
> the post-mortem summary. This slice is a **DELTA on shipped work**: slice-06
> (`htmx-scraper-viewer` — the read-only viewer + the `GET /claims` list route),
> slice-07 (`viewer-htmx-swaps` — the htmx progressive-enhancement layer), and slice-11
> (`viewer-counter-claim-threads` — the `GET /claims/{cid}` counter-thread this slice
> links to). Read those parent archives (`docs/evolution/htmx-scraper-viewer-evolution.md`,
> `viewer-htmx-swaps-evolution.md`, `viewer-counter-claim-threads-evolution.md`) for the
> list route and the thread surface. slice-12 ships the **at-a-glance half of J-003b** —
> the explicitly-deferred slice-11 follow-up (the LIST-row "countered by" flag).

## Summary

`viewer-counter-claim-list-flags` adds a neutral **"Countered"** presence flag to the
read-only **`GET /claims`** list. Each own-claim row that has **≥1 counter** now renders a
render-only `<a href="/claims/{cid}">` one-hop link to the slice-11 counter-thread; rows
with **no** counter render **no flag** (no-noise). This is the *at-a-glance* half of J-003b
— the part slice-11 explicitly deferred (slice-11 shipped the DETAIL-route thread; the
LIST-row flag was deferred to a recommended slice-12). A reader scanning the list now sees,
at a glance, *which claims have been disagreed with* and can click straight through to read
the disagreement, without opening every claim in turn.

The load-bearing thesis: **the flag is ADDITIVE — shown but never applied, and a strict
no-regression on the existing list render**. With the flag markers elided, the `/claims`
render is **byte-identical to slice-06**: same row order (`composed_at DESC, cid`), same
page indicator, same total count, every confidence cell unchanged — the flag never
re-orders, re-pages, re-counts, or re-weights the list. The presence is **presence-only**: a
twice-countered row renders **ONE** neutral marker (via `DISTINCT`), never "disputed by N".
The read is guarded against **N+1**: exactly ONE aggregate query per page, invariant to page
size. The read is **LOCAL and offline** (DB-only — no artifact read, no network).

The slice ships **ZERO new crates** and **ZERO new route** (workspace stays at **21
members**). It is an **additive presence flag on an existing route, not a re-architecture**:
it adds one read-only `StoreReadPort` method `counter_presence_for(&[String]) -> HashSet<String>`
(the ADR-048 batch presence read), one effect-shell projection (`ClaimRowView.is_countered`
set via `from_row_with_presence`), and one pure render arm (`render_list_presence_flag`,
REUSING the slice-11 `COUNTERED_PRESENCE_FLAG`). It REUSES the slice-06 `GET /claims` list
route + render pattern and the slice-11 drill-link target, and adds **NO new xtask edge** (the
dependency graph was already in place).

### What shipped (one paragraph)

The read-only `GET /claims` list now renders a neutral **"Countered"** presence flag on each
own-claim row that has **≥1 counter**. The flag is a render-only `<a href="/claims/{cid}">`
one-hop link to the slice-11 counter-thread on that claim's detail page; un-countered rows
render **no flag** (no-noise). The flag is fed by a **NEW read-only seam**
`counter_presence_for(&[String]) -> HashSet<String>` (ADR-048): **ONE aggregate** `SELECT
DISTINCT referenced_cid` over a `UNION ALL` of `claim_references IN(...) AND
ref_type='counters'` + `peer_claim_references IN(...) AND ref_type='counters'`, bound via
`params_from_iter` double-bind (**NEVER interpolation** — injection-safe), **empty input →
empty set with NO query** (an empty `IN ()` is a SQL error), **ref-tables-only** (no JOIN, no
Step-B artifact read). The `claims_page` handler wires `list_claims → collect CIDs →
counter_presence_for (unwrap_or_default → no flags on error, never 5xx) → project → render`;
the pure `ClaimRowView.is_countered` is set in the effect shell via `from_row_with_presence`
(keeping the render a total fn of `(page, presence)`), and `render_list_presence_flag` REUSES
the slice-11 `COUNTERED_PRESENCE_FLAG`. The **`list_claims` SQL and paging are UNCHANGED** —
the flag is layered on top of the existing list query, not woven into it. The read is **LOCAL
and read-only** (DB-only — no artifact read, no network); nothing is persisted. (The
`/project` + `/philosophy` + `/score` list flags and the `/peer-claims` flag were **deferred
to a recommended slice-13**.)

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-07 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-07 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-07 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-07 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **10/10 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **13 acceptance scenarios** GREEN: **8 `viewer_counter_claim_list_flags`** (LF-1..LF-8,
  incl. the LF-1 walking skeleton — a flagged own-claim row linking to the thread; one WS) +
  **5 GOLD invariants** (`viewer_counter_claim_list_flags_invariants` — read-only, no-write,
  offline-chrome, offline-data, and the CARDINAL **byte-identity** no-regression) + the
  `viewer-domain` unit/property tests (the new `is_countered` projection + the
  `render_list_presence_flag` arm) + the `adapter-duckdb` presence/N+1 tests. The
  `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the store is seeded through
  the REAL ingest path.
- **Slices 06/07/11 corpora GREEN — zero regression** (the full workspace acceptance suite
  green across all slices; the byte-identity gold proves the list render unchanged).
- **NO new crate, NO new route**: extends `viewer-domain` (PURE) + `adapter-http-viewer`
  (EFFECT) + `adapter-duckdb` (EFFECT, read impl) + `ports` in place on the existing
  `GET /claims` route; REUSES the slice-06 list render + the slice-11 drill-link target.
  Workspace member count stays **21** (19 production + 1 test-support + 1 xtask); `cargo xtask
  check-arch` reports "21 workspace members".
- **NO new production dependency**: `maud`/`hyper`/`duckdb` unchanged; no `deny.toml` change;
  **NO new xtask edge** (the dependency graph was already in place).
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production functions
  (**2/2 in-diff viable caught, 0 missed**) — exceeds the ≥80% per-feature gate.
- **1 ADR** (ADR-048) Accepted/shipped.
- DES integrity: 10/10 steps have complete DES traces.
- Adversarial review: **APPROVED** — zero defects, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members, no new allowlist edge).

## Wave-by-wave changelog

### DISCUSS (2026-06-07)

Luna framed the slice as a **brownfield DELTA on slices 06/07/11** that ships the
**at-a-glance half of J-003b** — the LIST-row "countered by" flag slice-11 explicitly
deferred (slice-11 shipped the DETAIL-route thread). Persona is **P-001 (Maria, the node
operator)**, the viewer's operator wearing the disagreement-scanner hat. The load-bearing
DISCUSS decision: **the flag is presence-only and additive** — it tells a reader *which*
claims have been countered so they can click through, but it NEVER re-ranks, filters, or
re-weights the list, and it shows ONE neutral marker per countered row regardless of how many
counters exist. slice-12 **REALIZES the existing viewer KPI contracts on the list surface**
(read-only / offline guardrails, presence-only attribution, no-regression rendering) rather
than minting new KPI IDs. The walking skeleton is the LF-1 flagged row (an own-claim with ≥1
counter → a render-only drill-link to the slice-11 thread), validating the riskiest
assumption first — that a presence flag can be layered onto the existing list render while
holding the byte-identity no-regression and the N+1 guard.

### DESIGN (2026-06-07)

Morgan locked slice-12 as an **additive presence flag on an existing route, not a
re-architecture** — ZERO new crates, ZERO new route, ZERO new architectural style, ZERO new
persisted type, ZERO new xtask edge. The open decisions were resolved adopting the DISCUSS
leans, captured in one ADR:

- **ADR-048** (counter-presence batch read — one aggregate `DISTINCT` over the ref tables):
  a **NEW read-only seam** `counter_presence_for(&[String]) -> HashSet<String>` on the store
  read port, implemented as **ONE aggregate** `SELECT DISTINCT referenced_cid` over a `UNION
  ALL` of `claim_references IN(...) AND ref_type='counters'` + `peer_claim_references IN(...)
  AND ref_type='counters'`. The CID list is bound via `params_from_iter` **double-bind**
  (**NEVER interpolation** — injection-safe), an **empty input returns an empty set with NO
  query** (an empty `IN ()` is a SQL error), and the read is **ref-tables-only** — **no JOIN,
  no Step-B artifact read** (unlike the slice-11 thread read, which needs the reason; the
  list only needs presence). The `ClaimRowView.is_countered` flag is set in the **effect
  shell** via `from_row_with_presence` (keeping the pure render a TOTAL fn of `(page,
  presence)`), and `render_list_presence_flag` REUSES the slice-11 `COUNTERED_PRESENCE_FLAG`.
  The `claims_page` handler wires `list_claims → collect CIDs → counter_presence_for
  (unwrap_or_default → no flags on error, never 5xx) → project → render`; the **`list_claims`
  SQL and paging stay UNCHANGED**. Read-only (no mutation method on the seam), LOCAL (DB-only,
  no artifact read, no network).

The read-only contract stays enforced at THREE layers (a `StoreReadPort` with no mutation
method [TYPE], the `xtask check-arch` viewer capability rule [STRUCTURAL], and behavioral
GOLD invariants [BEHAVIORAL]). The C4 views, the presence-read data-flow, and the
I-LF-1..6 structural-guarantee table are in the DESIGN sections of `feature-delta.md` and
`design/`.

### DISTILL (2026-06-07)

Quinn authored the executable acceptance corpus across two `[[test]]` targets plus the
adapter tests:

- **`viewer_counter_claim_list_flags.rs`** (Tier A — `LF-` ids LF-1..LF-8, 1 WS): the LF-1
  walking skeleton (a flagged own-claim row linking to the slice-11 thread), the no-JS full
  page + fragment/page parity, the neutral **"Countered"** presence flag rendered as a
  `<a href="/claims/{cid}">` one-hop drill-link, the **un-countered row → no flag** no-noise
  assertion, the **presence-only** assertion (a twice-countered row → ONE marker, not
  "disputed by N"), and the at-a-glance scan (a mixed list flags only the countered rows).
- **`viewer_counter_claim_list_flags_invariants.rs`** (gold guardrails — 5 ids): read-only
  (store row counts unchanged across countered/uncountered × page/fragment), no-write (no
  sign/publish/subscribe control on any shape), offline-chrome (only the vendored local htmx
  asset, no CDN), offline-data (the presence read hits the LOCAL DB ref tables, no network),
  and the CARDINAL **byte-identity** (with the flag markers elided, the `/claims` render is
  byte-identical to the slice-06 baseline — same row order `composed_at DESC, cid`, same page
  indicator, same total count, every confidence cell — the no-regression proof).
- **`adapter-duckdb` presence + N+1 tests**: the presence read returns exactly the countered
  CIDs (own arm via a directly-seeded `Counters` ref, peer arm via the peer-counter builder),
  the empty-input → no-query → empty-set path, and the **N+1 behavioral guard** — exactly ONE
  aggregate query at page sizes 1 / N / 5N (invariant to page size), plus the
  `viewer-domain` unit/property tests (the new `is_countered` projection + the
  `render_list_presence_flag` arm).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is
seeded through the REAL ingest path. RED classification: both acceptance targets COMPILE
green, scenarios FAIL via `todo!()` = MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-07)

Executed **10 roadmap steps** via DES-monitored crafter dispatches, each commit carrying a
`Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton (01-01)**: **01-01 is the THICK walking skeleton** —
  the new read-only `counter_presence_for` aggregate seam, the `ClaimRowView.is_countered`
  projection via `from_row_with_presence`, the `render_list_presence_flag` arm (REUSING the
  slice-11 `COUNTERED_PRESENCE_FLAG`), and the `claims_page` wiring (`list_claims → collect
  CIDs → counter_presence_for → project → render`) — the LF-1 flagged own-claim row linking
  to the slice-11 thread. It shipped page = chrome + fragment, so most downstream scenarios
  fell out of the skeleton once the WS landed.
- **Real work (03-02, 03-03)**: **03-02** carried the **byte-identity baseline** — the
  `read_slice06_list_baseline` capture of the recorded slice-06 list ordering + the
  marker-elision comparator, the no-regression gold's load-bearing tactic; **03-03** carried
  the **N+1 guard** — the adapter-level behavioral test at page sizes 1 / N / 5N + the
  empty-input → no-query path.
- **Confirmatory + gold**: most other steps were confirmatory once the thick WS landed
  (parity / no-noise / presence-only / drill-link fell out of the skeleton), and the gold
  invariants (read-only / no-write / offline-chrome / offline-data / byte-identity) flipped
  GREEN last off the confirmatory render path.

The 10-step shape: a **thick WS at 01-01** (the aggregate presence seam + the projection +
the render arm + the wiring) flipped most downstream scenarios green for free; only the
**byte-identity baseline** (03-02) and the **N+1 guard** (03-03) carried genuinely new work;
the gold went last. The L1-L4 refactor needed **no change** (the WS landed clean).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-LF-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..11 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-LF-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the `is_countered` projection + the `render_list_presence_flag` arm), matching slice-02..11 DV-2. The killing properties are kept IN-CRATE (the `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. 2/2 in-diff viable caught, 0 missed. |
| DV-LF-3 | **The flag is ADDITIVE — shown but never applied, byte-identical to slice-06 with the markers elided** (I-LF-2, CARDINAL). | No-regression is the at-a-glance cardinal: a presence flag that re-orders / re-pages / re-counts / re-weights the list would silently change what a reader sees. Guaranteed by leaving `list_claims` SQL + paging UNCHANGED and layering the flag on top; the byte-identity gold (against the recorded slice-06 baseline, markers elided) proves it byte-for-byte. |
| DV-LF-4 | **`counter_presence_for` is ONE aggregate `SELECT DISTINCT referenced_cid` over the ref tables; empty input → empty set with NO query; ref-tables-only, no JOIN, no Step-B artifact read** (ADR-048). | Presence is a set-membership question, not the slice-11 reason read — the list only needs *which* CIDs are countered, so a single `DISTINCT` aggregate over the two ref tables answers the whole page in one query (the N+1 guard) and the `DISTINCT` collapses a twice-countered row to ONE marker (presence-only, never "disputed by N"). Skipping the artifact read keeps it DB-only / offline. |
| DV-LF-5 | **The CID list is bound via `params_from_iter` double-bind, NEVER interpolation; an empty input short-circuits to an empty set with NO query** (ADR-048). | CIDs flow into an `IN(...)` clause across both ref tables; parameter binding (double-bound, once per table) is injection-safe and an empty `IN ()` is a SQL error — short-circuiting the empty case avoids both the error and a needless round-trip. |
| DV-LF-6 | **`counter_presence_for` errors degrade to no flags (`unwrap_or_default`), never 5xx; the list still renders** (I-LF-4). | The list route must never 5xx because the presence read failed — the flag is additive enrichment, so an error degrades to an unflagged list (identical to the slice-06 list), never a failed request. |
| DV-LF-7 | **The exactly-ONE-aggregate-query-per-page guarantee is invariant to page size — pinned structurally AND by an adapter-duckdb behavioral test at sizes 1 / N / 5N + empty→no-query** (I-LF-5). | A per-row presence read would be an N+1 that scales with page size; the batch aggregate keeps the per-page cost at ONE query regardless of page size, and the behavioral test at 1 / N / 5N pins it so a future refactor cannot silently reintroduce the N+1. |

## Cardinal release gates + slice-12 invariants (I-LF-1..6)

The cardinal release gates realized on the list surface — all release-blocking:

1. **Read-only / no key (I-LF-1)** — `GET /claims` (now flagged) is a READ; no
   write/sign/subscribe route; the web process holds no signing key; the presence-read seam
   has NO mutation method (type-level). Three-layer: TYPE (no write method) + STRUCTURAL
   (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-write).
2. **No-regression / shown-never-applied (I-LF-2, CARDINAL)** — with the flag markers elided,
   the `/claims` render is byte-identical to slice-06 (row order `composed_at DESC, cid` +
   page indicator + total count + every confidence cell); the flag never re-orders / re-pages
   / re-counts / re-weights (DV-LF-3 + the byte-identity gold).
3. **Presence-only (I-LF-3)** — a twice-countered row renders ONE neutral marker (via
   `DISTINCT`), never "disputed by N" (DV-LF-4 + the presence-only scenario); un-countered
   rows render no flag (no-noise).
4. **Robust / graceful-degradation (I-LF-4)** — a failed presence read degrades to no flags
   (`unwrap_or_default`), never 5xx; the list still renders (DV-LF-6).
5. **N+1 guard (I-LF-5)** — exactly ONE aggregate query per page, invariant to page size
   (DV-LF-7 + the structural batch seam + the adapter-duckdb behavioral test at sizes 1 / N /
   5N + empty → no-query).
6. **Offline / local-only (I-LF-6)** — the presence read hits the LOCAL DB ref tables only
   (no artifact read, no network — fully offline); the page references only the vendored local
   htmx asset (no CDN); loopback-only bind; nothing persisted (the two offline golds).

The full slice-12 invariant set (I-LF-1..6; structural-guarantee detail in the DESIGN
section of `feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-LF-1 | Read-only / no key (the flagged `GET /claims` is a READ; no write/sign/subscribe route; no key in the process; the presence-read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only/no-write). Cardinal. |
| I-LF-2 | No-regression / shown-never-applied (with the markers elided, the `/claims` render is byte-identical to slice-06 — row order + page indicator + total count + every confidence cell; the flag never re-orders/re-pages/re-counts/re-weights). | STRUCTURAL (`list_claims` SQL + paging UNCHANGED; the flag layered on top, DV-LF-3) + BEHAVIORAL (the byte-identity gold against the recorded slice-06 baseline, markers elided). CARDINAL. |
| I-LF-3 | Presence-only (a twice-countered row = ONE neutral marker via `DISTINCT`, never "disputed by N"; un-countered rows = no flag). | STRUCTURAL (`SELECT DISTINCT referenced_cid`, DV-LF-4) + BEHAVIORAL (presence-only + no-noise scenarios). Cardinal. |
| I-LF-4 | Robust / graceful-degradation (a failed presence read → no flags via `unwrap_or_default`, never 5xx; the list still renders). | STRUCTURAL (the presence read is additive enrichment over `list_claims`, DV-LF-6) + BEHAVIORAL (the degradation path). |
| I-LF-5 | N+1 guard (exactly ONE aggregate query per page, invariant to page size). | STRUCTURAL (the batch `counter_presence_for` aggregate, DV-LF-7) + BEHAVIORAL (the adapter-duckdb N+1 test at sizes 1/N/5N + empty→no-query). Cardinal. |
| I-LF-6 | Offline / local-only (the presence read hits the LOCAL DB ref tables only — no artifact read, no network; no-CDN chrome; loopback-only; nothing persisted). | STRUCTURAL (the ref-tables-only DB read, no Step-B artifact read; the shared `htmx_script` fn + pinned asset; loopback guard unchanged) + BEHAVIORAL (the two offline golds + read-only row-count delta). Cardinal. |

All slice-12 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets
(read-only / no key / human gate / offline + loopback / progressive enhancement / structural
fragment/page parity); confidence stays shown verbatim on every list row.

## Quality gates — final report

- **Acceptance / integration**: 8 `viewer_counter_claim_list_flags` (LF-1..LF-8, the LF-1
  walking skeleton) + 5 GOLD `viewer_counter_claim_list_flags_invariants` GREEN + the
  `adapter-duckdb` presence/N+1 tests + the `viewer-domain` unit/property tests (the new
  `is_countered` projection + the `render_list_presence_flag` arm); slices 06/07/11 corpora
  GREEN — zero regression (the byte-identity gold proves the list render unchanged). The
  `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the store is seeded through
  the REAL ingest path.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, **no
  new allowlist edge** (the dependency graph was already in place) + the confirmed viewer
  capability rule (read-only counter-presence reads; no signing/identity/PDS, no store-write).
- **Refactor (L1-L4)**: clippy + check-arch clean; **no refactor change needed** (the WS
  landed clean); `viewer-domain` purity intact (no I/O imports; maud + ports only; the
  `from_row_with_presence` projection lives in the effect shell, the render stays a total fn
  of `(page, presence)`).
- **Adversarial review**: **APPROVED** — zero defects, zero Testing Theater. The no-regression
  confirmed load-bearing (the byte-identity gold against the recorded slice-06 baseline,
  DV-LF-3); the presence-only confirmed structural (`SELECT DISTINCT`, DV-LF-4); the N+1 guard
  confirmed (one aggregate per page, the adapter test at sizes 1/N/5N, DV-LF-7).
- **DES integrity**: PASS — all 10 steps have complete DES traces (10/10).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the `is_countered`
projection + the `render_list_presence_flag` arm, REUSING the slice-11
`COUNTERED_PRESENCE_FLAG`). The slice-04/05 cross-package lesson stays applied — the
`viewer-domain` unit/property tests pin the production functions IN/against the crate, so the
per-feature mutation measurement reaches the real killing suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`is_countered` projection + `render_list_presence_flag` arm, in-diff) | 2 | 2 | 0 | **100%** (2/2 in-diff viable) |

Slice-12 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0
missed). `adapter-http-viewer` + `adapter-duckdb` are NOT mutated by design (effect shell —
the `counter_presence_for` batch presence read; covered by the GOLD invariants + the adapter
presence/N+1 tests through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **The byte-identity gold = recorded baseline + marker elision, NOT a twin store and NOT a
  no-flag HTTP seam (DV-LF-3)**: the no-regression proof compares the live `/claims` render
  (flag markers elided) against `read_slice06_list_baseline` — the recorded slice-06 list
  ordering. A re-seeded "twin" store gets DIFFERENT CIDs (the CID canonicalizes `composed_at`,
  so a fresh seed yields different content-addresses), and a production no-flag HTTP seam would
  be a test-seam bleeding into production (the `viewer-domain` crate isn't a cli dependency).
  **Lesson: to prove a byte-identical no-regression against a prior slice, record the prior
  slice's output as a fixed baseline and elide the new markers — do NOT re-seed a twin store
  (content-addressed CIDs diverge on a fresh seed) and do NOT add a production "disable the new
  feature" HTTP seam (that is a production test-seam); the baseline-capture + marker-elision
  tactic keeps the comparison exact without touching production.**
- **The own-claim `claim_references` arm needs a directly-seeded ref at the adapter level
  (the self-counter rule blocks own counters end-to-end)**: the ADR-015 self-counter rule
  blocks an author from countering their own claim end-to-end, so own list rows are flagged
  via the PEER arm in practice. The own arm (`claim_references`) is therefore exercised only at
  the adapter unit level — the adapter test seeds a `Counters` ref directly into
  `claim_references` to cover the own arm of the `UNION ALL`. **Lesson: when a production rule
  (the self-counter block) makes one arm of a `UNION ALL` unreachable end-to-end, cover that
  arm at the adapter unit level by seeding the underlying ref directly — the end-to-end path
  exercises the other arm, but the unreachable arm still needs explicit coverage so a
  regression in it is caught.**
- **A thick walking skeleton flipped most scenarios green for free**: the 01-01 WS shipped the
  aggregate presence seam + the `is_countered` projection + the render arm + the `claims_page`
  wiring on day one, so parity / no-noise / presence-only / drill-link fell out of the skeleton
  and the gold invariants flipped green last for free; the real work concentrated into two
  seams — the byte-identity baseline (03-02) and the N+1 guard (03-03) — and the refactor needed
  no change. **Lesson: a walking skeleton that gets the batch read AND the projection-and-render
  right on day one turns the parity/no-noise/presence steps into confirmation — invest in WS
  depth to concentrate the remaining effort onto the few seams (the no-regression baseline, the
  N+1 guard) that carry genuinely new behavior.**
- **A pre-existing cross-binary ephemeral-port flake (`viewer_graph_traversal`) recurs under
  concurrent runs (carry-forward)**: an ephemeral-port bind race in the slice-10
  `viewer_graph_traversal` harness recurs under concurrent test runs; green in isolation.
  **Lesson (carry-forward): a cross-binary ephemeral-port harness can race under concurrent
  runs — it is green in isolation and is not a slice-12 regression; track it for a harness
  hardening pass rather than letting it block an unrelated slice.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-048 fixed the contract; field-level shaping (the `counter_presence_for` signature, the `is_countered` projection, the `render_list_presence_flag` arm, the wiring) left to DELIVER. | All adopted; the `counter_presence_for(&[String]) -> HashSet<String>` seam, the `from_row_with_presence` projection, and the `render_list_presence_flag` arm (REUSING the slice-11 `COUNTERED_PRESENCE_FLAG`) materialized at DELIVER against the render + byte-identity tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-048 fixed the one-aggregate batch-read intent (ref-tables-only, `DISTINCT`, double-bind, empty→no-query). | The batch read landed; the N+1 guard (03-03) pinned the one-aggregate-per-page guarantee behaviorally at sizes 1/N/5N + the empty→no-query path. | Resolved at DELIVER. |
| 3 | The `/project` + `/philosophy` + `/score` list flags and the `/peer-claims` flag were in scope discussion. | **Deferred to a recommended slice-13** — this slice ships the `/claims` own-claim list flag only. | Deferred (recommended slice-13). |
| 4 | Review expected to pass clean. | Review APPROVED — zero defects, zero Testing Theater (no revision needed). | Confirmed at DELIVER. |
| 5 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-LF-2, 100% in-diff 2/2, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-counter-claim-list-flags/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, journey), `design/`
  (architecture-design, component-boundaries, data-models, technology-stack), `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer + the `GET /claims` list route this slice
  flags): `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Parent slice-11 archive** (the `GET /claims/{cid}` counter-thread this slice links to;
  the at-a-glance half deferred from here): `docs/evolution/viewer-counter-claim-threads-evolution.md`
- **Slice-12 ADR**:
  `docs/adrs/ADR-048-counter-presence-batch-read-one-aggregate-distinct-ref-tables.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-counter-claim-list-flags/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-counter-claim-list-flags/deliver/execution-log.json`,
  `docs/feature/viewer-counter-claim-list-flags/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_counter_claim_list_flags.rs` (8 LF-scenarios, the LF-1 walking
  skeleton), `tests/acceptance/viewer_counter_claim_list_flags_invariants.rs` (5 gold
  invariants, incl. the byte-identity no-regression)
- **Reused render constant + drill-link target**: `crates/viewer-domain`
  (`COUNTERED_PRESENCE_FLAG`, slice-11) via `render_list_presence_flag`; the
  `/claims/{cid}` slice-11 thread as the link terminus
- **Extended viewer crates**: `crates/viewer-domain` (`ClaimRowView.is_countered` +
  `from_row_with_presence` + `render_list_presence_flag`), `crates/adapter-http-viewer` (the
  existing `GET /claims` `claims_page` handler + the presence wiring),
  `crates/adapter-duckdb` (the read-only `counter_presence_for` batch presence impl),
  `crates/ports` (the counter-presence read seam)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
