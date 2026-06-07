# Evolution: viewer-counter-claim-threads (slice-11 counter-claim threading on the read-only `GET /claims/{cid}` detail route)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-counter-claim-threads/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `deliver/`) and ADR-046/ADR-047 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-06 (`htmx-scraper-viewer` — the read-only viewer + the `GET /claims/{cid}`
> detail route), slice-07 (`viewer-htmx-swaps` — the htmx progressive-enhancement layer),
> and the federated-read store (`claims ∪ peer_claims`, the self-counter rule of ADR-015).
> Read those parent archives (`docs/evolution/htmx-scraper-viewer-evolution.md`,
> `viewer-htmx-swaps-evolution.md`, `openlore-federated-read-evolution.md`) for the
> detail route and the store this slice threads against. slice-11 **realizes J-003b**
> (counter-claim as first-class disagreement, the VIEWING side; authoring stays CLI).

## Summary

`viewer-counter-claim-threads` threads **every counter-claim targeting a CID beneath the
verbatim claim** on the existing read-only **`GET /claims/{cid}`** detail route. Given a
claim CID, the detail page now renders, beneath the byte-identical claim region, an
attributed list of all counter-claims pointing at it — each counter naming its `author_did`
+ `cid` + verbatim `--reason`, a neutral **"Countered"** presence flag, and a one-hop
`/claims/{counter_cid}` drill-link (depth-1, no recursion). An empty reason renders **"no
reason provided"**. This is the J-003b job — counter-claim as first-class disagreement,
surfaced on the browser VIEWING side (authoring a counter stays a CLI operation) — so a
reader sees not just a claim but *who disagreed with it and why*, in their own words.

The load-bearing thesis: **counter-claims are shown but NEVER applied — they are additive
context, never a re-weight, filter, or merge of the claim they target**. The countered
claim renders verbatim, at its original confidence, in a byte-identical claim region; the
counter thread is appended below it (I-CT-2 shown-never-applied, CARDINAL). Two distinct
authors countering the same CID render as **two attributed entries**, never a "disputed by
N" consensus row (I-CT-3 anti-merging, via UNION-ALL + a no-aggregate ADT). The read is
**LOCAL and offline** — a DB ref lookup plus a local artifact filesystem read, no network
(I-CT-5). Read-only stays enforced at **three layers** (the `StoreReadPort` with no mutation
method [TYPE], the `xtask check-arch` viewer capability rule [STRUCTURAL], the behavioral
GOLD invariants [BEHAVIORAL], I-CT-1).

The slice ships **ZERO new crates** and **ZERO new route** (workspace stays at **21
members**). It is an **additive thread on an existing route, not a re-architecture**: it
extends `viewer-domain` (a pure `CounterThread` ADT + `from_rows` + render woven into the
existing `render_claim_detail_fragment`), `adapter-http-viewer` (the existing detail handler,
`get_claim` extended to fall back to `peer_claims`), the `adapter-duckdb` read impl (the new
read-only `query_counter_claims` two-step read), and the `ports` (the read seam). It REUSES
the slice-06 `GET /claims/{cid}` detail route + render pattern (`viewer-domain` maud, the
`Shape` fork, page = chrome + fragment), and adds **NO new xtask edge** (the dependency graph
was already in place).

### What shipped (one paragraph)

The read-only `GET /claims/{cid}` detail route now threads **all** counter-claims targeting
that CID beneath the verbatim claim. On request the viewer reads the claim (with the new
`peer_claims` fallback — the ADR-015 self-counter rule makes a countered claim a peer's), runs
the new read-only `query_counter_claims(target_cid)`, maps the rows to the pure `CounterThread`
ADT (`None | Countered{counters: Vec<CounterEntry>}`) via `from_rows`, and renders the thread
woven into `render_claim_detail_fragment` (so fragment/page parity holds by construction). Each
counter entry carries `author_did` + `cid` + verbatim `--reason`, a neutral **"Countered"**
presence flag, and a one-hop `/claims/{counter_cid}` drill-link (depth-1, **no recursion** — a
counter's own counters are not threaded on this page). An empty reason renders **"no reason
provided"**. The countered claim itself renders **VERBATIM** — original confidence,
byte-identical claim region — with the thread appended as additive context. The
`query_counter_claims` read is **two-step** (ADR-046): **Step A** is an indexed UNION-ALL ref
lookup over `claims ∪ peer_claims` (JOIN `claim_references` / `peer_claim_references` on
`ref_type='counters'`, explicit `author_did` + `cid`, **no cross-store merge**); **Step B** is
a per-row `read_artifact_at → unsigned.reason` (the reason is **NOT a DB column** — it lives in
the signed-claim JSON artifact). The read is **LOCAL** (DB + local artifact fs read, no
network); nothing is persisted. (The `/claims` LIST-row "countered by" flag was **deferred to
a recommended slice-12**.)

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-06 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-06 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-06 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-06 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **11/11 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **15 acceptance scenarios** GREEN: **9 `viewer_counter_claim_threads`** (CT-1..CT-9,
  including the CT-1 walking skeleton — the threaded detail fragment) + **5 GOLD invariants**
  (`viewer_counter_claim_threads_invariants` — read-only, no-write, offline-chrome,
  offline-data, and the CARDINAL shown-never-applied) + a **missing-artifact degradation
  test** + an **adapter-level two-author `query_counter_claims` test** + the `viewer-domain`
  unit/property tests (the new `CounterThread` projection). The `ViewerServer` harness drives
  the REAL `openlore ui` over HTTP; the store is seeded through the REAL ingest path.
- **Slices 06/07 + federated-read corpora GREEN — zero regression** (the full workspace
  acceptance suite green across all slices).
- **NO new crate, NO new route**: extends `viewer-domain` (PURE) + `adapter-http-viewer`
  (EFFECT) + `adapter-duckdb` (EFFECT, read impl) + `ports` in place on the existing
  `GET /claims/{cid}` route; REUSES the slice-06 detail render pattern. Workspace member count
  stays **21** (19 production + 1 test-support + 1 xtask); `cargo xtask check-arch` reports
  "21 workspace members".
- **NO new production dependency**: `maud`/`hyper`/`duckdb` unchanged; no `deny.toml` change;
  **NO new xtask edge** (the dependency graph was already in place).
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production functions
  (**8/8 in-diff viable caught, 0 missed**) — exceeds the ≥80% per-feature gate.
- **2 ADRs** (ADR-046, ADR-047) all Accepted/shipped.
- DES integrity: 11/11 steps have complete DES traces.
- Adversarial review: **APPROVED after one revision** (D2 graceful-degradation fix +
  D1/D5 test strengthening, fixed in one pass).
- `cargo xtask check-arch`: OK (21 workspace members, no new allowlist edge).

## Wave-by-wave changelog

### DISCUSS (2026-06-06)

Luna framed the slice as a **brownfield DELTA on slices 06/07 + the federated-read store**
that **realizes J-003b**: counter-claim as first-class disagreement, the VIEWING side —
authoring a counter stays a CLI operation; this slice surfaces it in the browser. Persona is
**P-001 (Maria, the node operator)**, the viewer's operator wearing the disagreement-reader
hat. The load-bearing DISCUSS decision: **counter-claims are SHOWN but NEVER APPLIED** — the
countered claim renders verbatim, at its original confidence, and the counters are additive
context appended below, never a re-weight / filter / merge / consensus. slice-11 **REALIZES
the existing viewer KPI contracts on the detail surface** (read-only / offline guardrails,
anti-merging attribution, verbatim rendering) rather than minting new KPI IDs. The walking
skeleton is the CT-1 thread (claim CID → read-only ref lookup → pure `CounterThread` →
attributed counter-list HTML fragment beneath the verbatim claim), validating the riskiest
assumption first — that the existing detail route can thread the full attributed counter set
at depth-1 while preserving shown-never-applied and anti-merging.

### DESIGN (2026-06-06)

Morgan locked slice-11 as an **additive thread on an existing route, not a re-architecture**
— ZERO new crates, ZERO new route, ZERO new architectural style, ZERO new persisted type,
ZERO new xtask edge. The open decisions were resolved adopting the DISCUSS leans, captured in
two ADRs:

- **ADR-046** (counter-claim thread read — indexed ref lookup + artifact reason, two-step):
  a **NEW read-only seam** `query_counter_claims(target_cid)` on the store read port,
  returning a `CounterClaimRow` DTO, implemented as a **two-step read**. **Step A** is an
  **indexed UNION-ALL ref lookup** over `claims ∪ peer_claims` (JOIN
  `claim_references` / `peer_claim_references` on `ref_type='counters'`, explicit `author_did`
  + `cid`, **no cross-store merge**); **Step B** is a per-row `read_artifact_at →
  unsigned.reason` — the reason is **NOT a DB column** (it lives in the signed-claim JSON
  artifact), so it is read best-effort from the local artifact filesystem. `get_claim` is
  extended to **fall back to `peer_claims`** (the ADR-015 self-counter rule makes a countered
  claim a peer's). The read is LOCAL (DB + local artifact fs, no network), read-only (no
  mutation method on the seam).
- **ADR-047** (`CounterThread` ADT — depth-1, no recursion, empty-reason display): a **NEW
  pure `viewer-domain` projection** — a `CounterThread` ADT (`None | Countered{counters:
  Vec<CounterEntry>}`; `CounterEntry{author_did, cid, reason: Option<...>}`) + `from_rows`
  mapping the `CounterClaimRow`s, with render woven into the existing
  `render_claim_detail_fragment` (parity by construction). Depth-1 only — the page threads a
  claim's direct counters, **NOT** a counter's own counters (no recursion); the counter `cid`
  is a one-hop `/claims/{counter_cid}` drill-link. An empty/absent reason renders **"no reason
  provided"**; the presence flag is a neutral **"Countered"** (no consensus / "disputed by N").

The read-only contract stays enforced at THREE layers (a `StoreReadPort` with no mutation
method, the `xtask check-arch` viewer capability rule, and behavioral GOLD invariants). The
C4 views, the two-step counter-read data-flow, and the I-CT-1..5 structural-guarantee table
are in the DESIGN sections of `feature-delta.md` and `design/`.

### DISTILL (2026-06-06)

Quinn authored the executable acceptance corpus across two `[[test]]` targets plus the
adapter and degradation tests:

- **`viewer_counter_claim_threads.rs`** (Tier A — `CT-` ids CT-1..CT-9): the CT-1 walking
  skeleton (the threaded detail fragment — a counter rendered beneath the verbatim claim), the
  no-JS full page + fragment/page parity, the attributed counter entry naming `author_did` +
  `cid` + verbatim `--reason`, the neutral **"Countered"** presence flag, the
  `/claims/{counter_cid}` one-hop drill-link (depth-1, no recursion), the **anti-merging
  two-author entries** (two distinct authors countering the same CID = two attributed entries,
  fed by the new peer-counter builder), the **empty-reason → "no reason provided"** render, and
  the verbatim-claim-region assertion (the countered claim renders byte-identical, original
  confidence).
- **`viewer_counter_claim_threads_invariants.rs`** (gold guardrails — 5 ids): read-only
  (store row counts unchanged across countered/uncountered × page/fragment), no-write (no
  sign/publish/subscribe control on any shape), offline-chrome (only the vendored local htmx
  asset, no CDN), offline-data (the two-step read hits the LOCAL DB + local artifact fs, no
  network), and the CARDINAL **shown-never-applied** (the countered claim region is
  byte-identical and the confidence unchanged whether or not counters exist).
- **A missing-artifact degradation test** (an unreadable / undeserializable counter artifact
  degrades to `reason: None` → "no reason provided", never 5xx) and an **adapter-level
  two-author `query_counter_claims` test** (the UNION-ALL ref lookup returns both authors'
  rows, no cross-store merge), plus the `viewer-domain` unit/property tests (the new
  `CounterThread` `from_rows` projection + render).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store
is seeded through the REAL ingest path. RED classification: both acceptance targets COMPILE
green, scenarios FAIL via `todo!()` = MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-06)

Executed **11 roadmap steps** via DES-monitored crafter dispatches, each commit carrying a
`Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton (01-01)**: **01-01 is the THICK walking skeleton** —
  the new read-only `query_counter_claims(target_cid)` two-step seam (Step A ref lookup + Step
  B artifact reason), the `get_claim` `peer_claims` fallback, the `CounterThread` ADT +
  `from_rows`, the render woven into `render_claim_detail_fragment`, and the CT-1 threaded
  fragment (a counter rendered beneath the verbatim claim from the LOCAL store). It shipped
  page = chrome + fragment, so most downstream scenarios fell out of the skeleton.
- **Phase 02 — real work (02-xx)**: **02-02** carried the **anti-merging** entries + the
  **NEW `build_verifiable_peer_counter_record` test builder** (the existing peer-record
  builder hardcoded `references:[]`; this variant emits `references:[{type:counters,
  cid:target}]` + reason through the production pull with correct CID recomputation) — two
  distinct authors countering the same CID render as two attributed entries; **02-03** carried
  the **empty-reason → "no reason provided"** render.
- **Confirmatory + gold**: most other steps were confirmatory once the thick WS landed (parity
  / drill-link / verbatim-claim-region fell out of the skeleton), and the gold invariants
  (read-only / no-write / offline-chrome / offline-data / shown-never-applied) flipped GREEN
  last off the confirmatory render path.

The 11-step shape: a **thorough WS at 01-01** (the two-step read + the `peer_claims` fallback
+ the `CounterThread` ADT + the woven render) flipped most downstream scenarios green for
free; only the **anti-merging entries + the peer-counter builder** (02-02) and the
**empty-reason render** (02-03) carried genuinely new behavior; the gold went last.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-CT-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..10 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-CT-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the `CounterThread` `from_rows` projection + render), matching slice-02..10 DV-2. The killing properties are kept IN-CRATE (the `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. 8/8 in-diff viable caught, 0 missed. |
| DV-CT-3 | **Counter-claims are SHOWN but NEVER APPLIED** (I-CT-2, CARDINAL) — the countered claim renders VERBATIM, at original confidence, in a byte-identical claim region; the counter thread is appended as additive context, never a re-weight / filter / merge. | Shown-never-applied is the J-003b cardinal: a disagreement surfaces *next to* a claim, it does not silently re-rank or hide it. Guaranteed by the ABSENCE of any viewer-side re-weight/filter path — the claim region is rendered identically whether or not counters exist (the gold shown-never-applied proves it byte-for-byte). |
| DV-CT-4 | **`query_counter_claims` Step A is an indexed UNION-ALL ref lookup over `claims ∪ peer_claims` with explicit `author_did` + `cid` and NO cross-store merge** (ADR-046). | A cross-store merge / aggregate is exactly where two distinct authors countering the same CID would collapse into a "disputed by N" consensus row (anti-merging, I-CT-3, is cardinal); the UNION-ALL with an explicit `author_did` preserves every counter as its own attributed entry — anti-merging BY CONSTRUCTION, not by a test. |
| DV-CT-5 | **The reason is read best-effort from the local artifact (Step B), NOT from a DB column; the DB ref lookup is authoritative** (ADR-046). | The `--reason` lives in the signed-claim JSON artifact, not a DB column. The two-step read keeps the DB ref lookup as the authoritative source of *which* counters exist, with the reason a best-effort artifact read on top — so a missing/unreadable artifact degrades the reason, never the existence of the counter. |
| DV-CT-6 | **A missing / unreadable / undeserializable counter artifact degrades to `reason: None` ("no reason provided") — the counter still renders from the authoritative DB ref lookup; never 5xx** (review D2 fix). | The detail route must never 5xx because one peer's artifact is missing or malformed; the counter's existence is authoritative from the DB, so the page degrades gracefully (the counter renders with "no reason provided") instead of failing the whole request. The missing-artifact degradation test pins it. |
| DV-CT-7 | **`get_claim` falls back to `peer_claims`** so a countered claim that has become a peer's (the ADR-015 self-counter rule) still resolves on the detail route. | The self-counter rule (ADR-015) means a countered claim can live in `peer_claims`; without the fallback the detail route would 404 the very claim a reader is trying to see the counters for. The fallback keeps the countered claim resolvable from either store. |
| DV-CT-8 | **Depth-1, no recursion** (ADR-047) — the page threads a claim's DIRECT counters; the counter `cid` is a one-hop `/claims/{counter_cid}` drill-link, NOT an inline recursive thread. | Recursion (threading a counter's own counters inline) would make the page unbounded and the "shown-never-applied" reasoning hard to hold; a one-hop drill-link keeps the page bounded and lets the reader navigate the disagreement graph explicitly, one claim at a time. |

## Cardinal release gates + slice-11 invariants (I-CT-1..5)

The cardinal release gates realized on the detail surface — all release-blocking:

1. **Read-only / no key (I-CT-1)** — `GET /claims/{cid}` (now threaded) is a READ; no
   write/sign/subscribe route; the web process holds no signing key; the counter-read seam has
   NO mutation method (type-level). Three-layer: TYPE (no write method) + STRUCTURAL (`xtask
   check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-write).
2. **Shown-never-applied (I-CT-2, CARDINAL)** — the countered claim renders VERBATIM, at its
   original confidence, in a byte-identical claim region; the counter thread is additive
   context, never a re-weight / filter / merge (DV-CT-3 + the gold shown-never-applied).
3. **Anti-merging (I-CT-3)** — two distinct `(author, cid)` counters render as two attributed
   entries; no "disputed by N" consensus row (Step A UNION-ALL + the no-aggregate
   `CounterThread` ADT, DV-CT-4) (CT anti-merging + the peer-counter builder).
4. **Robust / graceful-degradation (I-CT-4)** — a missing / unreadable / undeserializable
   counter artifact degrades to `reason: None` ("no reason provided"); the counter still
   renders from the authoritative DB ref lookup; never 5xx (DV-CT-6, the missing-artifact
   degradation test).
5. **Offline / local-only (I-CT-5)** — the two-step read hits the LOCAL DB + local artifact
   filesystem with no network (fully offline); the page references only the vendored local
   htmx asset (no CDN); loopback-only bind; nothing persisted (the two offline golds).

The full slice-11 invariant set (I-CT-1..5; structural-guarantee detail in the DESIGN section
of `feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-CT-1 | Read-only / no key (the threaded `GET /claims/{cid}` is a READ; no write/sign/subscribe route; no key in the process; the counter-read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only/no-write). Cardinal. |
| I-CT-2 | Shown-never-applied (the countered claim renders verbatim, original confidence, byte-identical claim region; the counter is additive context, never a re-weight/filter/merge). | STRUCTURAL (no viewer-side re-weight/filter path; the claim region is rendered identically with/without counters, DV-CT-3) + BEHAVIORAL (gold shown-never-applied, byte-for-byte). CARDINAL. |
| I-CT-3 | Anti-merging (two distinct `(author, cid)` counters = two attributed entries; no "disputed by N" consensus row). | STRUCTURAL (Step A UNION-ALL, explicit `author_did`, no cross-store merge / aggregate; the no-aggregate `CounterThread` ADT, DV-CT-4) + BEHAVIORAL (CT anti-merging + the peer-counter builder). Cardinal. |
| I-CT-4 | Robust / graceful-degradation (a missing/unreadable/undeserializable counter artifact → `reason: None` "no reason provided"; the counter still renders from the DB ref lookup; never 5xx). | STRUCTURAL (the DB ref lookup is authoritative; the artifact reason is best-effort, DV-CT-5/6) + BEHAVIORAL (the missing-artifact degradation test). |
| I-CT-5 | Offline / local-only (the two-step read hits the LOCAL DB + local artifact fs, no network; no-CDN chrome; loopback-only; nothing persisted). | STRUCTURAL (the read-only local DB + artifact-fs read; the shared `htmx_script` fn + pinned asset; loopback guard unchanged) + BEHAVIORAL (the two offline golds + read-only row-count delta). Cardinal. |

All slice-11 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets
(read-only / no key / human gate / offline + loopback / progressive enhancement / structural
fragment/page parity); confidence stays shown verbatim on the countered claim.

## Quality gates — final report

- **Acceptance / integration**: 9 `viewer_counter_claim_threads` (CT-1..CT-9, the CT-1
  walking skeleton) + 5 GOLD `viewer_counter_claim_threads_invariants` GREEN + the
  missing-artifact degradation test + the adapter-level two-author `query_counter_claims`
  test + the `viewer-domain` unit/property tests (the new `CounterThread` projection); slices
  06/07 + federated-read corpora GREEN — zero regression. The `ViewerServer` harness drives
  the REAL `openlore ui` over HTTP; the store is seeded through the REAL ingest path.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, **no
  new allowlist edge** (the dependency graph was already in place) + the confirmed viewer
  capability rule (read-only counter reads; no signing/identity/PDS, no store-write).
- **Refactor (L1-L4)**: clippy + check-arch clean; the refactor extracted a **shared
  `peer_origin` helper** (the `claims ∪ peer_claims` origin tagging reused across the read
  paths); `viewer-domain` purity intact (no I/O imports; maud + ports only; the `Shape`
  dispatch + the two-step read live in the effect shell).
- **Adversarial review**: **APPROVED after one revision** — D2 (graceful-degradation: a
  missing/unreadable counter artifact must degrade to "no reason provided", never 5xx) +
  D1/D5 (test strengthening) were fixed in one pass. The shown-never-applied confirmed
  load-bearing (the byte-identical claim region gold, DV-CT-3); the anti-merging confirmed
  structural (Step A UNION-ALL, no cross-store merge, DV-CT-4); the reason-best-effort read
  confirmed (DB authoritative + artifact best-effort, DV-CT-5/6). Zero Testing Theater.
- **DES integrity**: PASS — all 11 steps have complete DES traces (11/11).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the `CounterThread`
`from_rows` projection + the counter-thread render woven into `render_claim_detail_fragment`).
The slice-04/05 cross-package lesson stays applied — the `viewer-domain` unit/property tests
pin the production functions IN/against the crate, so the per-feature mutation measurement
reaches the real killing suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`CounterThread` `from_rows` projection + counter-thread render, in-diff) | 8 | 8 | 0 | **100%** (8/8 in-diff viable) |

Slice-11 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0
missed). `adapter-http-viewer` + `adapter-duckdb` are NOT mutated by design (effect shell —
the two-step `query_counter_claims` read; covered by the GOLD invariants + the adapter
two-author + missing-artifact tests through the real binary). DEVOPS sweep is the ongoing
backstop.

## Lessons learned / issues

- **The DB ref lookup is authoritative; the reason is a best-effort artifact read (DV-CT-5/6)**:
  the `--reason` lives in the signed-claim JSON artifact, not a DB column, so the read is
  two-step — Step A (the indexed UNION-ALL ref lookup) is authoritative for *which* counters
  exist, Step B (`read_artifact_at → unsigned.reason`) is a best-effort read for *why*. A
  missing/unreadable artifact degrades the reason to "no reason provided"; the counter still
  renders. **Lesson: when an authoritative fact (a counter exists) and an enriching detail (its
  reason) live in different stores, make the authoritative store the source of existence and
  read the detail best-effort — so a missing detail degrades gracefully instead of dropping the
  fact or 5xx-ing the request (review D2).**
- **A reference-carrying peer fixture needs its own builder (the peer-counter builder)**: the
  existing `build_verifiable_peer_*` builder hardcoded `references:[]`, so it could not seed a
  peer that *counters* a CID. The new `build_verifiable_peer_counter_record` emits
  `references:[{type:counters, cid:target}]` + reason through the production pull with correct
  CID recomputation. **Lesson: a test builder that hardcodes an empty `references` cannot fixture
  a reference-carrying record — add a variant that emits the reference through the production
  signing/CID path (correct recomputation), reusable for any future reference-carrying peer
  fixture (e.g. supports, cites).**
- **Multi-counter seeds need a single two-peer pull (slice-09/10 carry-forward)**: seeding two
  distinct authors countering the same CID requires both peers reachable at read time; separate
  per-peer pulls drop earlier peers' PDS → 404. The fix seeds both in ONE two-peer pull.
  **Lesson (carry-forward): when a fixture needs multiple peers in the store, seed them in a
  SINGLE pull — incremental per-peer pulls drop the PDS of peers seeded earlier, surfacing as a
  404 at read time, not at seed time.**
- **A thick walking skeleton flipped most scenarios green for free**: the 01-01 WS shipped the
  two-step read + the `peer_claims` fallback + the `CounterThread` ADT + the woven render on day
  one, so parity / drill-link / verbatim-claim-region fell out of the skeleton and the gold
  invariants flipped green last for free; the real work concentrated into two seams — the
  anti-merging entries + the peer-counter builder (02-02) and the empty-reason render (02-03).
  **Lesson: a walking skeleton that gets the two-step read AND the woven render right on day one
  turns the parity/drill-link/verbatim steps into confirmation — invest in WS depth to
  concentrate the remaining effort onto the few seams that carry genuinely new behavior.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-046/047 fixed the contracts; field-level shaping (the `CounterThread` arms, the `CounterEntry` shape, the `CounterClaimRow` DTO, the two-step query) left to DELIVER. | All adopted; the `CounterThread` arms (`None`/`Countered{counters}`), `from_rows`, the `CounterClaimRow` DTO, and the two-step `query_counter_claims` materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-046 fixed the two-step read intent (DB ref lookup authoritative + artifact reason best-effort). | The two-step read landed; the review D2 fix hardened the Step-B degradation (missing artifact → "no reason provided", never 5xx, the degradation test). | Resolved at DELIVER (review D2). |
| 3 | The `/claims` LIST-row "countered by" flag was in scope discussion. | **Deferred to a recommended slice-12** — this slice ships the DETAIL-route thread only. | Deferred (recommended slice-12). |
| 4 | Review expected to pass clean. | Review APPROVED after ONE revision (D2 graceful-degradation + D1/D5 test strengthening), fixed in one pass. | Found + fixed within DELIVER. |
| 5 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-CT-2, 100% in-diff 8/8, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-counter-claim-threads/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, journey), `design/`
  (architecture-design, component-boundaries, data-models, technology-stack), `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer + the `GET /claims/{cid}` detail route
  this slice threads): `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Parent federated-read archive** (the `claims ∪ peer_claims` store + the ADR-015
  self-counter rule): `docs/evolution/openlore-federated-read-evolution.md`
- **Slice-11 ADRs**:
  `docs/adrs/ADR-046-counter-claim-thread-read-indexed-ref-lookup-artifact-reason.md`,
  `docs/adrs/ADR-047-counter-thread-adt-depth1-no-recursion-empty-reason-display.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-counter-claim-threads/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-counter-claim-threads/deliver/execution-log.json`,
  `docs/feature/viewer-counter-claim-threads/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_counter_claim_threads.rs` (9 CT-scenarios, the CT-1 walking
  skeleton), `tests/acceptance/viewer_counter_claim_threads_invariants.rs` (5 gold invariants)
- **Extended viewer crates**: `crates/viewer-domain` (`CounterThread` + `from_rows` + the
  counter-thread render woven into `render_claim_detail_fragment`), `crates/adapter-http-viewer`
  (the existing `GET /claims/{cid}` handler + the `get_claim` `peer_claims` fallback),
  `crates/adapter-duckdb` (the read-only two-step `query_counter_claims` impl + the shared
  `peer_origin` helper), `crates/ports` (the counter-claim read seam)
- **New reusable test builder**: `build_verifiable_peer_counter_record` (a reference-carrying
  peer fixture — emits `references:[{type:counters, cid:target}]` + reason through the
  production pull with correct CID recomputation)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
