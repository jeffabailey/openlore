# Evolution: viewer-graph-traversal (slice-10 graph-traversal `/project` + `/philosophy` views on the read-only viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-graph-traversal/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `deliver/`) and ADR-042..ADR-045 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-06 (`htmx-scraper-viewer` — the read-only viewer), slice-07
> (`viewer-htmx-swaps` — the htmx progressive-enhancement layer), and slice-09
> (`viewer-contributor-scoring` — the `/score` contributor view this slice cross-links
> back to). Read those parent archives (`docs/evolution/htmx-scraper-viewer-evolution.md`,
> `viewer-htmx-swaps-evolution.md`, `viewer-contributor-scoring-evolution.md`) for the
> surfaces this slice composes. slice-10 **completes J-002**: it realizes **J-002b
> (traverse edges)** — the last unshipped J-002 sub-job.

## Summary

`viewer-graph-traversal` adds two **read-only traversal views** to the `openlore ui`
read-only viewer: **`GET /project?subject=<uri>`** and **`GET /philosophy?object=<uri>`**.
Together they turn the contributor↔project↔philosophy graph into a **glanceable survey**
of attributed edges: given a subject URI, `/project` groups every claim touching that
subject; given an object URI, `/philosophy` groups every claim asserting that philosophy.
Each rendered edge names its `author_did` + `cid` + verbatim confidence + claim-domain
bucket, and each survey **cross-links**: a subject cell links to `/project`, an object
cell links to `/philosophy`, and a contributor cell links to the slice-09 `/score`
terminus (REUSED). This is the J-002 "traverse the graph in the browser" job Maria
(P-001) reaches without dropping to the CLI — depth-1 edge survey, the last J-002 sub-job
to ship.

The load-bearing thesis: **a depth-1 edge survey that GROUPS but never MERGES, on a
surface that takes on authority over nothing**. The view reads the LOCAL store read-only,
groups the attributed edges by a single `GroupDimension`, and renders; it never collapses
two authors' identical edges into a consensus row (anti-merging by construction — UNION
ALL with an explicit `author_did`, no merge JOIN / GROUP BY / AVG). The CARDINAL new
concern is **href-injection safety**: subject/object/DID values are **peer-claim-controlled
(attacker-influenced)** and flow into every cross-link href, so a single SSOT encoder
(`encode_query_component`) percent-encodes every byte outside the RFC3986 unreserved set
into all three href builders (ADR-044), proven round-trip-exact and hostile-byte-never-leaks
by proptest. Read-only is enforced at **three layers** (a `StoreReadPort` with no mutation
method [TYPE], the `xtask check-arch` viewer capability rule [STRUCTURAL], and a behavioral
GOLD invariant [BEHAVIORAL]).

The slice ships **ZERO new crates** (workspace stays at **21 members**). It is an
**additive render surface, not a re-architecture**: it extends `viewer-domain` (a pure
`TraversalView` ADT + the shared `group_by` survey engine + the three href builders),
`adapter-http-viewer` (the `/project` + `/philosophy` handlers + the `Shape` fork + nav
links), the `adapter-duckdb` read impl (two read-only survey queries over a shared SQL
engine), the `ports` (the two read seams), the `cli` (`ui` wiring, still no key), and
`xtask` (one new allowlist edge `viewer-domain → claim-domain` + the capability rule). It
REUSES the slice-09 `/score` terminus (the contributor cross-link target) and the
slice-06/07 viewer render pattern (`viewer-domain` maud, the `Shape` fork, page = chrome +
fragment, the vendored offline htmx asset).

### What shipped (one paragraph)

Two `GET` views — `GET /project?subject=<uri>` and `GET /philosophy?object=<uri>` — each a
GET form (enter a URI) → on submit the viewer runs a read-only survey query (a **UNION
ALL** of `claims ∪ local peer_claims` with an **explicit `author_did`** and **no merge
JOIN / GROUP BY / AVG**), maps the rows to a pure `TraversalView` ADT (`Found{entity,
groups, contributors} | NoClaims{entity}`), groups them with the shared `group_by` engine
(`/project` groups by object, `/philosophy` groups by subject — the dimension is the only
parameter), and projects them into HTML, forking by `Shape::from_request` (the slice-07
`HX-Request` selector) — a full page without the header, the survey fragment with it. Every
edge row carries `author_did` + `cid` + verbatim confidence + claim-domain bucket; identical
content from two distinct authors renders as **two attributed rows** (anti-merging);
genuinely-unrelated co-claimants surface via the object dimension (`/philosophy`), keeping
the contributor view author-scoped. Each survey cross-links — subject→`/project`,
object→`/philosophy`, contributor→`/score` (slice-09) — through three href builders
(`href_project` / `href_philosophy` / `href_score`) that are the single SSOT for link
construction; every interpolated value is percent-encoded via `encode_query_component`
(defense-in-depth over maud's auto-escape) so a peer-claim-controlled byte can never break
out of the href. An unknown / no-edges entity renders a guided `NoClaims{entity}` notice in
**both shapes**. The store read is **LOCAL and read-only** (offline, no network); the bind
stays loopback-only; nothing is persisted.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-06 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-06 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-06 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-06 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **16/16 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **19 acceptance scenarios** GREEN: **14 `viewer_graph_traversal`** (GT-1..GT-14,
  including **two walking skeletons** — the `/project` thread and the symmetric
  `/philosophy` thread) + **5 GOLD invariants** (`viewer_graph_traversal_invariants` —
  read-only, no-write, offline-chrome, offline-data, and the CARDINAL
  security-injection). Plus the `viewer-domain` unit/property tests (the new
  `TraversalView` projection + the `group_by` engine + the **encoder round-trip and
  hostile-byte proptests**). The `ViewerServer` harness drives the REAL `openlore ui` over
  HTTP; the store is seeded through the REAL ingest path.
- **Slices 06/07/09 corpora GREEN — zero regression** (the full workspace acceptance suite
  green across all slices).
- **NO new crate**: extends `viewer-domain` (PURE) + `adapter-http-viewer` (EFFECT) +
  `adapter-duckdb` (EFFECT, read impl) + `ports` + `cli` (DRIVER) + `xtask` (tooling) in
  place; REUSES the slice-09 `/score` terminus. Workspace member count stays **21** (19
  production + 1 test-support + 1 xtask); `cargo xtask check-arch` reports "21 workspace
  members".
- **NO new production dependency**: `claim-domain` (the bucket source) is already
  in-workspace; `maud`/`hyper` unchanged; no `deny.toml` change.
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production
  functions (**25/25 viable caught, 0 missed**) on the in-diff scope — exceeds the ≥80%
  per-feature gate.
- **4 ADRs** (ADR-042..ADR-045) all Accepted/shipped.
- DES integrity: 16/16 steps have complete DES traces.
- Adversarial review: **APPROVED**, zero blockers, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members, one new `viewer-domain →
  claim-domain` allowlist edge). L1-L4 refactor done (the shared `query_survey` SQL engine
  extraction, −58 LOC).

## Wave-by-wave changelog

### DISCUSS (2026-06-06)

Luna framed the slice as a **brownfield DELTA on slices 06/07/09** that **completes J-002**:
the browser surface for **J-002b (traverse edges)** — the last unshipped J-002 sub-job
(J-002a was the read views, J-002c the slice-09 scoring transparency). Persona is **P-001
(Maria, the node operator)**, the viewer's operator wearing the graph-traversal hat. The
load-bearing DISCUSS decision: **the traversal is a depth-1 edge SURVEY that GROUPS but
never MERGES** — it shows every attributed edge touching an entity, grouped by the
complementary dimension, with full attribution (`author_did` + `cid` + verbatim confidence
+ bucket), never a consensus row. slice-10 **REALIZES the existing viewer KPI contracts on
the traversal surface** (read-only / offline guardrails, anti-merging attribution, verbatim
confidence) rather than minting new KPI IDs. The walking skeleton is the `/project` thread
(subject URI → read-only survey → pure group → attributed-edge HTML fragment), validating
the riskiest assumption first — that a read-only survey can render the full attributed edge
graph at depth-1 while preserving anti-merging and, the new cardinal, href-injection safety
on peer-controlled URIs.

### DESIGN (2026-06-06)

Morgan locked slice-10 as an **additive render surface, not a re-architecture** — ZERO new
crates, ZERO new binary, ZERO new architectural style, ZERO new persisted type. The open
decisions were resolved adopting the DISCUSS leans, captured in four ADRs:

- **ADR-042** (viewer project/philosophy survey reads — two methods, anti-merging): two
  **NEW read-only seams** `query_project_survey` / `query_philosophy_survey` on the store
  read port — **deliberately two public methods** (the two views' contracts stay
  independent and individually testable) backed, after the L2 refactor, by an **internal
  shared `query_survey` SQL engine**. Each is a read-only **UNION ALL** of `claims ∪ local
  peer_claims` with an **explicit `author_did`** and **no merge JOIN / GROUP BY / AVG** —
  anti-merging by construction.
- **ADR-043** (`TraversalView` ADT + `viewer-domain` survey projection, depth-1): a **NEW
  pure `viewer-domain` projection** — a `TraversalView` ADT (`Found{entity, groups,
  contributors} | NoClaims{entity}`; `EdgeGroup{key, edges}`; `EdgeRow{author_did,
  confidence, cid}`) and a **shared `group_by` engine parameterized by `GroupDimension`**
  (`/project` groups by object, `/philosophy` by subject — one engine, the dimension is the
  only difference). Depth-1 only — the survey shows direct edges, not transitive closure.
- **ADR-044** (traversal routes + cross-link hrefs + bare-DID percent-encoding — security):
  the two **OWN routes** `GET /project?subject=<uri>` / `GET /philosophy?object=<uri>`,
  added to the nav; GET forms → bookmarkable/shareable URLs + plain no-JS navigation, htmx
  fragment fork via `HX-Request` (the slice-07 pattern). The **CARDINAL injection boundary**:
  subject/object/DID are peer-claim-controlled (attacker-influenced), so the three href
  builders (`href_project` / `href_philosophy` / `href_score`) — the single SSOT for link
  construction — percent-encode every interpolated value via `encode_query_component`
  (every byte outside RFC3986-unreserved), defense-in-depth OVER maud's auto-escape.
- **ADR-045** (`viewer-domain → claim-domain` bucket reuse + `check-arch` deltas): the
  claim-domain **bucket is REUSED** from `claim-domain` (one bucket taxonomy
  workspace-wide), adding the new **pure→pure allowlist edge** `viewer-domain →
  claim-domain` (no new reachability — both are pure cores) + the confirmed viewer
  capability rule (read-only traversal reads; no signing/identity/PDS, no store-write).

The read-only contract is enforced at THREE layers (a `StoreReadPort` with no mutation
method, the `xtask check-arch` viewer capability rule, and a behavioral GOLD invariant).
The C4 views, the `/project` + `/philosophy` data-flow, and the I-GT-1..8 structural-guarantee
table are in the DESIGN sections of `feature-delta.md` and `design/`.

### DISTILL (2026-06-06)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_graph_traversal.rs`** (Tier A — `GT-` ids GT-1..GT-14): **two walking
  skeletons** (the `/project` subject-survey attributed-edge fragment and the symmetric
  `/philosophy` object-survey fragment), the no-JS full page + fragment/page parity for
  both routes, the attributed-edge survey naming `author_did` + `cid` + verbatim confidence
  + bucket, the **anti-merging two-author rows** (identical content from two authors = two
  attributed rows, fed by `seed_two_author_same_edge`), the three cross-links
  (subject→`/project`, object→`/philosophy`, contributor→`/score`), and the guided
  `NoClaims{entity}` in both shapes.
- **`viewer_graph_traversal_invariants.rs`** (gold guardrails — 5 ids): read-only (store
  row counts unchanged across rich/empty × page/fragment), no-write (no sign/publish/subscribe
  control on any shape), offline-chrome (only the vendored local htmx asset, no CDN),
  offline-data (the survey reads the LOCAL store with no network), and the CARDINAL
  **security-injection** (a hostile peer-controlled byte in a subject/object/DID never
  escapes the rendered href).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store
is seeded through the REAL ingest path. The encoder is additionally pinned in
`viewer-domain` by **proptests**: round-trip exact (`decode ∘ encode == id`) and
hostile-byte-never-leaks. RED classification: both targets COMPILE green, scenarios FAIL
via `todo!()` = MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-06)

Executed **16 roadmap steps** via DES-monitored crafter dispatches, each commit carrying a
`Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — `/project` (01-xx)**: **01-01 is the THICK walking skeleton** — the
  `/project` route + `Shape::from_request` dispatch + the `TraversalView` ADT + the shared
  `group_by` engine + the read-only `query_project_survey` seam + the `render_*` parity
  split + the `ui` wiring. It shipped page = chrome + fragment, so **01-02..01-06 were
  mostly confirmatory** (the survey render fell out of the skeleton).
- **Phase 02 — `/philosophy` symmetric (02-xx)**: **02-01** mirrored `/project` (the
  `query_philosophy_survey` seam + the symmetric route + the `GroupDimension` swap) and got
  parity for free off the WS structure; **02-02** carried real work — the **anti-merging
  seed** (`seed_two_author_same_edge`) and the first cross-link.
- **Phase 03 — cross-links + injection (03-xx)**: the three href builders (`href_project` /
  `href_philosophy` / `href_score`) as the SSOT; **03-03 is the SECURITY step** — the
  `encode_query_component` percent-encoder + the round-trip and hostile-byte proptests +
  the gold security-injection invariant.
- **Phase 04 — gold (04-xx)**: the read-only / no-write / offline-chrome / offline-data
  guardrails driving the real binary — they **flipped GREEN for free** off the confirmatory
  render path.

The 16-step shape: a **thorough WS at 01-01** flipped ~10 downstream scenarios green for
free (page = chrome + fragment makes parity/offline structural; the symmetric `/philosophy`
mirrored `/project`); only the **anti-merging seed** (02-02), the **`/philosophy` WS**
(02-01), and the **injection encoder** (03-03) carried real new work. The L2 refactor
extracted the shared `query_survey` SQL engine (−58 LOC) while preserving the ADR-042
two-public-method boundary.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-GT-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..09 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-GT-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the `TraversalView` projection + the `group_by` engine + the `encode_query_component` encoder), matching slice-02..09 DV-2. The killing properties are kept IN-CRATE (the `viewer-domain` unit/property tests, incl. the encoder proptests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. 25/25 in-diff viable caught, 0 missed. |
| DV-GT-3 | **Two PUBLIC read methods (`query_project_survey` / `query_philosophy_survey`) over ONE internal shared `query_survey` SQL engine** (ADR-042; the engine extracted at the L2 refactor). | The two views' contracts stay independent and individually testable, while the SQL anti-merging guarantee lives in ONE place — the refactor removed 58 LOC of duplication without widening the public surface or weakening the boundary. |
| DV-GT-4 | **The survey reads are a read-only UNION ALL of `claims ∪ local peer_claims` with an explicit `author_did` and NO merge JOIN / GROUP BY / AVG** (ADR-042). | A merge JOIN / GROUP BY / AVG is exactly where two distinct authors' identical edges would collapse into a consensus row (anti-merging is cardinal); the UNION ALL with an explicit `author_did` preserves every author's edge as its own attributed row — anti-merging BY CONSTRUCTION, not by a test. |
| DV-GT-5 | **The contributor view stays author-scoped; genuinely-unrelated co-claimants surface via the OBJECT dimension (`/philosophy`), not `/project`** (carry-forward from slice-09 DV-CS-6). | `/project` answers "what edges touch THIS subject" and the contributor cells are author-scoped; finding everyone who asserted a philosophy is the OBJECT-dimension question answered by `/philosophy`. Keeping the scope explicit keeps each view's contract honest. |
| DV-GT-6 | **All cross-link hrefs go through three SSOT builders (`href_project` / `href_philosophy` / `href_score`) that percent-encode every interpolated value via `encode_query_component`** (ADR-044), defense-in-depth OVER maud's auto-escape; proven round-trip-exact + hostile-byte-never-leaks by proptest. | subject/object/DID are PEER-CLAIM-CONTROLLED (attacker-influenced); one SSOT encoder means there is no second link-construction path where a hostile byte could break out of a href. Defense-in-depth: maud escapes HTML context; `encode_query_component` escapes URL context — both layers must hold. |
| DV-GT-7 | **Multi-author seeds use the single `seed_own_plus_peer_graph` pull** (slice-09 carry-forward). | Separate `peer pull`s drop earlier peers' PDS → 404 (the slice-09 institutional lesson); seeding all postures in ONE pull keeps every peer's PDS reachable at survey time. |
| DV-GT-8 | **Read-only enforced at three layers** (a `StoreReadPort` with no mutation method [TYPE] + the `xtask check-arch` viewer capability rule [STRUCTURAL] + the gold read-only / no-write invariants [BEHAVIORAL]) plus the new pure→pure `viewer-domain → claim-domain` allowlist edge (no new reachability). | The read-only guarantee cannot be defeated by any single-layer slip; the new allowlist edge is pure→pure (bucket reuse), so it adds NO new I/O reachability to the viewer surface. |

## Cardinal release gates + slice-10 invariants (I-GT-1..8)

The cardinal release gates realized on the traversal surface — all release-blocking:

1. **Read-only / no key (I-GT-1)** — `/project` + `/philosophy` are READs; no write/sign/subscribe
   route; the web process holds no signing key; the survey read seams have NO mutation
   method (type-level). Three-layer: TYPE (no write method) + STRUCTURAL (`xtask check-arch`
   viewer capability rule) + BEHAVIORAL (gold read-only / no-write).
2. **Offline / local-only (I-GT-2)** — the surveys read the LOCAL store with no network
   (fully offline); the page references only the vendored local htmx asset (no CDN);
   loopback-only bind; nothing persisted (the two offline golds).
3. **Anti-merging (I-GT-3)** — identical content from two distinct authors renders as two
   attributed rows; no merged/consensus row (the read seams UNION-ALL with an explicit
   `author_did`, no merge JOIN / GROUP BY / AVG) (GT anti-merging + `seed_two_author_same_edge`).
4. **No-invented-edges (I-GT-4)** — every rendered edge corresponds to a stored claim; the
   survey shows direct depth-1 edges only, no transitive/synthetic edges.
5. **Verbatim confidence (I-GT-5)** — confidence rendered through the EXISTING
   `render_confidence` (`0.90`, never `0.9`/`90%`).
6. **Fragment/page parity (I-GT-6)** — full page without `HX-Request`, the same survey
   fragment with it; page = chrome + fragment by construction.
7. **No-CDN chrome (I-GT-7)** — the traversal pages reference only the vendored local htmx
   asset; zero off-host references.
8. **Href-injection-safe (CARDINAL, I-GT-8)** — every peer-claim-controlled value
   (subject/object/DID) is percent-encoded through the SSOT `encode_query_component` into
   all hrefs; round-trip exact; a hostile byte never escapes the rendered href (the gold
   security-injection + the encoder proptests).

The full slice-10 invariant set (I-GT-1..8; structural-guarantee detail in the DESIGN
section of `feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-GT-1 | Read-only / no key (`/project` + `/philosophy` are READs; no write/sign/subscribe route; no key in the process; the survey seams hold no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only/no-write). Cardinal. |
| I-GT-2 | Offline / local-only (the surveys read the LOCAL store with no network; no-CDN chrome; loopback-only; nothing persisted). | STRUCTURAL (the read-only local survey queries; the shared `htmx_script` fn + pinned asset; loopback guard unchanged) + BEHAVIORAL (the two offline golds + read-only row-count delta). Cardinal. |
| I-GT-3 | Anti-merging (identical-content-different-author = two attributed rows; no merged/consensus row). | STRUCTURAL (read seams UNION ALL, explicit `author_did`, no merge JOIN/GROUP BY/AVG, DV-GT-4) + BEHAVIORAL (anti-merging scenario + `seed_two_author_same_edge`). Cardinal. |
| I-GT-4 | No-invented-edges (every rendered edge maps to a stored claim; depth-1 only, no transitive/synthetic edges). | STRUCTURAL (`group_by` projects only stored rows; depth-1 survey) + BEHAVIORAL (the survey scenarios). |
| I-GT-5 | Confidence verbatim (rendered through the EXISTING `render_confidence` — `0.90`, never `0.9`/`90%`). | STRUCTURAL (one `render_confidence` site, reused) + BEHAVIORAL (verbatim assertion). |
| I-GT-6 | Fragment/page parity (full page without `HX-Request`, the same survey fragment with it; page = chrome + fragment). | STRUCTURAL (the page renderer embeds the survey fragment) + BEHAVIORAL (parity scenarios, both routes). |
| I-GT-7 | Offline / no-CDN chrome (only the vendored local htmx asset; zero off-host references). | STRUCTURAL (the shared `htmx_script` fn + SHA-256-pinned asset) + BEHAVIORAL (gold offline-chrome). |
| I-GT-8 | Href-injection-safe (peer-claim-controlled subject/object/DID percent-encoded via the SSOT `encode_query_component` into all hrefs; round-trip exact; no hostile-byte escape). | TYPE/STRUCTURAL (three SSOT href builders, one encoder, defense-in-depth over maud, DV-GT-6) + BEHAVIORAL (gold security-injection + round-trip and hostile-byte proptests). CARDINAL. |

All slice-10 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets
(read-only / no key / human gate / offline + loopback / progressive enhancement /
structural fragment/page parity); confidence stays shown verbatim in both shapes.

## Quality gates — final report

- **Acceptance / integration**: 14 `viewer_graph_traversal` (GT-1..GT-14, two walking
  skeletons) + 5 GOLD `viewer_graph_traversal_invariants` GREEN + the `viewer-domain`
  unit/property tests (incl. the encoder round-trip and hostile-byte proptests); slices
  06/07/09 corpora GREEN — zero regression. The `ViewerServer` harness drives the REAL
  `openlore ui` over HTTP; the store is seeded through the REAL ingest path.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate; the new delta is
  the `viewer-domain → claim-domain` pure-core dependency allowlist entry (pure → pure
  edge, no new reachability) + the confirmed viewer capability rule (read-only traversal
  reads; no signing/identity/PDS, no store-write).
- **Refactor (L1-L4)**: clippy + check-arch clean; the L2 refactor extracted the shared
  `query_survey` SQL engine (−58 LOC) while preserving the ADR-042 two-public-method
  boundary; `viewer-domain` purity intact (no I/O imports; maud + ports + the new
  `claim-domain` pure dep only; the `Shape` dispatch lives in the effect shell).
- **Adversarial review**: **APPROVED**, zero blockers, zero Testing Theater. The anti-merging
  confirmed structural (UNION ALL, explicit `author_did`, no merge JOIN/GROUP BY/AVG,
  DV-GT-4); the href-injection guarantee confirmed load-bearing (the SSOT encoder +
  round-trip/hostile-byte proptests, DV-GT-6); the two-public-method boundary confirmed
  preserved across the shared-engine refactor (DV-GT-3).
- **DES integrity**: PASS — all 16 steps have complete DES traces (16/16).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the `TraversalView`
projection + the `group_by` survey engine + the `encode_query_component` encoder + the
inherited slice-06/07 render arithmetic). The slice-04/05 cross-package lesson stays applied
— the `viewer-domain` unit/property tests (incl. the encoder proptests) pin the production
functions IN/against the crate, so the per-feature mutation measurement reaches the real
killing suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`TraversalView` projection + `group_by` engine + `encode_query_component` encoder, in-diff) | 25 | 25 | 0 | **100%** (25/25 in-diff viable) |

Slice-10 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0
missed). `adapter-http-viewer` + `adapter-duckdb` are NOT mutated by design (effect shell;
covered by the GOLD invariants through the real binary); `claim-domain` is REUSED (the
bucket is already mutation-covered at its owning slice). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **A thorough walking skeleton flipped ~10 scenarios green for free**: the 01-01 WS shipped
  page = chrome + fragment AND the shared `group_by` engine on day one, so parity/offline
  became structural and most of Phase 01 (01-02..01-06) plus the symmetric `/philosophy`
  (02-01) and the gold invariants (04-xx) were confirmatory. The real work concentrated into
  three seams: the anti-merging seed (02-02), the `/philosophy` WS (02-01), and the injection
  encoder (03-03). **Lesson: a walking skeleton that gets BOTH the page = chrome + fragment
  structure AND the shared core engine right on day one turns a symmetric second route and
  most render steps into confirmation — invest in WS depth to concentrate the remaining
  effort onto the few seams that carry genuinely new behavior.**
- **Peer-claim-controlled values are an injection boundary — encode at the SSOT (DV-GT-6)**:
  subject/object/DID flow from peer claims (attacker-influenced) straight into cross-link
  hrefs. The fix routes all link construction through three SSOT builders that percent-encode
  every byte outside RFC3986-unreserved via `encode_query_component`, defense-in-depth over
  maud's HTML auto-escape, proven round-trip-exact and hostile-byte-never-leaks by proptest.
  **Lesson: when attacker-influenced data flows into a NEW output context (a URL href, not
  just HTML text), the framework's HTML auto-escape is NOT sufficient — add a context-correct
  encoder at a SINGLE SSOT and prove it with a round-trip + hostile-byte property, so there
  is no second link-construction path where a hostile byte escapes.**
- **Two public methods over one shared engine keeps boundaries AND removes duplication
  (DV-GT-3)**: ADR-042 deliberately kept two public read methods (independent, testable view
  contracts) while the L2 refactor extracted the shared `query_survey` SQL engine behind them
  (−58 LOC, the anti-merging guarantee in ONE place). **Lesson: a deliberate "two public
  methods" boundary and "one internal engine" are not in tension — keep the public surface
  shaped by the contracts the callers need, and refactor the shared implementation behind it;
  the boundary is a public-API decision, the dedup is an implementation decision.**
- **Keep the contributor view author-scoped; co-claimants live on the object dimension
  (DV-GT-5, slice-09 carry-forward)**: `/project`'s contributor cells stay author-scoped;
  genuinely-unrelated co-claimants surface via `/philosophy` (the object dimension).
  **Lesson: when two views form complementary dimensions of the same graph, let each answer
  ITS question honestly rather than widening one to cover the other — the object dimension is
  the right home for "everyone who asserted this," not the subject view.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-042..045 fixed the contracts; field-level shaping (`TraversalView` arms, the `EdgeGroup`/`EdgeRow` shapes, the `GroupDimension` parameterization, the survey queries) left to DELIVER. | All adopted; the `TraversalView` arms (`Found{entity, groups, contributors}`/`NoClaims{entity}`), the shared `group_by` engine, and the two survey UNION-ALL queries materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-042 fixed the two-public-method read boundary; the shared internal engine left to DELIVER. | The L2 refactor extracted the shared `query_survey` SQL engine (−58 LOC) behind the two public methods (DV-GT-3). | Resolved at DELIVER (refactor); boundary preserved. |
| 3 | ADR-044 fixed the href-injection intent (SSOT builders + percent-encoding); the encoder + proptests left to DELIVER. | The three SSOT href builders + `encode_query_component` + the round-trip and hostile-byte proptests + the gold security-injection invariant landed at 03-03 (DV-GT-6). | Resolved at DELIVER. |
| 4 | ADR-045 fixed the `viewer-domain → claim-domain` bucket-reuse edge + `check-arch` deltas. | The pure→pure allowlist edge + the read-only capability rule landed; `check-arch` reports 21 members. | Resolved at DELIVER. |
| 5 | Review expected to pass clean. | Review APPROVED, zero blockers, zero Testing Theater. | Confirmed at DELIVER. |
| 6 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-GT-2, 100% in-diff, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-graph-traversal/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, journey), `design/`
  (architecture-design, component-boundaries, data-models, technology-stack), `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer this slice extends):
  `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Parent slice-09 archive** (the `/score` contributor view this slice cross-links to):
  `docs/evolution/viewer-contributor-scoring-evolution.md`
- **Slice-10 ADRs**:
  `docs/adrs/ADR-042-viewer-project-philosophy-survey-reads-two-method-anti-merging.md`,
  `docs/adrs/ADR-043-traversalview-adt-viewer-domain-survey-projection-depth1.md`,
  `docs/adrs/ADR-044-traversal-routes-crosslink-hrefs-bare-did-percent-encoding-security.md`,
  `docs/adrs/ADR-045-viewer-domain-claim-domain-bucket-reuse-edge-check-arch-deltas.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-graph-traversal/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-graph-traversal/deliver/execution-log.json`,
  `docs/feature/viewer-graph-traversal/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_graph_traversal.rs` (14 GT-scenarios, two walking skeletons),
  `tests/acceptance/viewer_graph_traversal_invariants.rs` (5 gold invariants)
- **Reused cross-link terminus**: `crates/viewer-domain` (`render_score_*`, slice-09) via
  the `href_score` builder
- **Reused bucket taxonomy**: `crates/claim-domain` (the claim-domain bucket)
- **Extended viewer crates**: `crates/viewer-domain` (`TraversalView` + `group_by` +
  `href_*` + `encode_query_component`), `crates/adapter-http-viewer` (`GET /project` +
  `GET /philosophy` handlers + `Shape` fork + nav links), `crates/adapter-duckdb` (the
  read-only `query_project_survey` / `query_philosophy_survey` impls over the shared
  `query_survey` engine), `crates/ports` (the two traversal read seams)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
