# Evolution: viewer-search-full-follow-state (slice-20 — completing the four-arm `AuthorRelationship` on the read-only `GET /search` view)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-search-full-follow-state/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL/DELIVER
> [REF] sections, plus `discuss/`, `design/`, `distill/`, `deliver/`) and ADR-057 under
> `docs/adrs/`; this file is the post-mortem summary. This slice is a **DELTA on shipped
> work** — it is the **completion of slice-16** (`viewer-search-follow-state`, ADR-053):
> slice-16 shipped the BINARY `SubscribedPeer` / `NetworkUnfollowed` resolution and left
> the `You` and `UnsubscribedCache` match arms EMPTY (deferred). slice-20 fills those two
> arms, turning the slice-16 binary resolution into the **TOTAL four-arm
> `AuthorRelationship` resolution**. Read the slice-16 archive
> (`docs/evolution/viewer-search-follow-state-evolution.md`) for the surface this slice
> completes — the `GET /search` enrichment, `render_following_indicator`, the
> `read_local_active_set` read, the `to_indexed_claim` classification, the `bare_did` SSOT,
> and the `#[cfg(debug_assertions)]` fault-seam pattern this slice reuses per-read.
> Together **slice-16 + slice-20 COMPLETE the four-arm search follow-state** — every
> `AuthorRelationship` variant now renders honestly on the read-only `GET /search` view.
> slice-20 continues to realize **J-005c** ("turn a discovery into a follow") — now with
> the operator's OWN claims and SOFT-REMOVED (cached) peers correctly distinguished from
> genuinely-new authors.

## Summary

`viewer-search-full-follow-state` **completes** the slice-16 read-only **`GET /search`**
follow-state enrichment by filling the two arms slice-16 left empty. slice-16 resolved each
result author against the operator's LOCAL active peer subscriptions as a BINARY decision
(`SubscribedPeer` else `NetworkUnfollowed`), leaving `You | UnsubscribedCache => {}` as a
deferred empty render arm. slice-20 adds **two new read-only LOCAL presence reads** —
`distinct_own_author_dids` (over `claims` → the operator's OWN claims) and
`distinct_cached_peer_author_dids` (over `peer_claims`, **NO `removed_at` filter** so it
includes soft-removed peers whose cached claims remain on disk → `UnsubscribedCache`) — and
turns the binary classifier into a **TOTAL four-arm precedence resolution**:
**`You > SubscribedPeer > UnsubscribedCache > NetworkUnfollowed`**. The two newly-filled arms
each render a neutral render-only `<p>` indicator — `SEARCH_SELF_INDICATOR = "Your own claim"`
(for `You`) and `SEARCH_REMOVED_CACHED_INDICATOR = "A peer you removed (cached)"` (for
`UnsubscribedCache`) — siblings of slice-16's `render_following_indicator`, and like
`SubscribedPeer` NEITHER renders an `openlore peer add` affordance. The slice-16
`SubscribedPeer` / `NetworkUnfollowed` arms are REUSED VERBATIM and stay **byte-stable**
(additive, no regression). This is J-005c rendered FULLY: a discovery the operator already
follows says "Following"; the operator's OWN claim says "Your own claim"; a peer the operator
soft-removed (whose cache is still on disk) says "A peer you removed (cached)"; only a
genuinely-new author carries the `openlore peer add <did>` follow affordance.

The load-bearing thesis: **a per-row relationship enrichment completed to TOTALITY that takes
on authority over nothing — all four arms stay render-only TEXT, the network index query is
unchanged, each of the three LOCAL reads degrades independently, and the resolution is a pure
total function over three `HashSet`s with a fixed precedence.** The viewer signs/writes/
persists nothing and holds no signing key; the relationship is resolved from three LOCAL sets
(active REUSED + own/cached NEW; all read-only, offline), the network index query is untouched
and per-user-neutral (the same results for every operator; only the per-row affordance differs
by who the operator is and follows), and attribution/ranking are unchanged (the relationship
is per-row enrichment, NOT a merge or re-rank). Each failed read maps to an empty set
INDEPENDENTLY (slice-17 per-count `.ok()` independence): failed own → no `You`; failed cached
→ no `UnsubscribedCache`; failed active → slice-16 all-`NetworkUnfollowed`; worst case (all
three fail) = the slice-08 "always-unfollowed" status quo, no crash/5xx/leak.

The slice ships **ZERO new crates** (workspace stays at **21 members**). It is **near-all-EXTEND
— an additive completion, not a re-architecture**: NO new route, NO new crate, NO new
`AuthorRelationship` variant (it FILLS the two existing empty arms of the existing enum). The
work is two single-table read-only adapter reads + an effect-shell extension (read the two new
LOCAL sets ONCE each into `HashSet`s alongside the slice-16 active set; `to_indexed_claim`
becomes a total four-arm precedence resolver) + a pure render addition (the two empty arms
filled with two SSOT constants and two render-only sibling fns, making the render a TOTAL
`match`). The one genuinely-new bit of machinery is the **per-read fault seam**: OQ-1's
conditional escalation FIRED — the real-binary subprocess harness cannot inject a per-read
`Err` via a fake `StoreReadPort`, so two distinct `#[cfg(debug_assertions)]` per-read fault
tokens were materialized (`OPENLORE_VIEWER_FAIL_OWN_DIDS_READ`,
`OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ`) mirroring the slice-16
`OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` seam, and the `xtask` `VIEWER_FAIL_SEAM_TOKENS` guard
was extended **4 → 6** tokens.

### What shipped (one paragraph)

The slice-16 `GET /search` handler now, on render, reads THREE LOCAL sets ONCE each into bare-DID
`HashSet`s (the effect shell): the slice-16 active set (REUSED via `read_local_active_set`), the
NEW own-claim author DIDs (`distinct_own_author_dids` over `claims`), and the NEW cached-peer
author DIDs (`distinct_cached_peer_author_dids` over `peer_claims`, NO `removed_at` filter so
soft-removed peers' cached claims still classify). `to_indexed_claim` computes
`bare_did(author_did)` and resolves each result author by **fixed precedence**:
`∈ own → You`, else `∈ active → SubscribedPeer`, else `∈ cached → UnsubscribedCache`, else
`NetworkUnfollowed` — a **batch-once** total resolution (no N+1; the three reads happen once per
render, not once per result). The bare-DID strip on the RESULT side reconciles the indexed author
DID with the bare DIDs in all three sets (the slice-16 `bare_did` SSOT). The pure `viewer-domain`
render fills the two empty arms: `You` renders the neutral `SEARCH_SELF_INDICATOR` ("Your own
claim"); `UnsubscribedCache` renders the neutral `SEARCH_REMOVED_CACHED_INDICATOR` ("A peer you
removed (cached)"); both via render-only sibling fns of `render_following_indicator`, NEITHER
emitting an `openlore peer add` command. The slice-16 `SubscribedPeer` "Following" indicator and
`NetworkUnfollowed` `openlore peer add <did>` guidance arms are unchanged (byte-stable). The
render is now a **TOTAL `match`** over all four `AuthorRelationship` variants. The three reads are
**LOCAL and read-only** (offline); the network index query is UNCHANGED (still per-user-neutral);
attribution/ranking are UNCHANGED (per-row enrichment, no merge/re-rank). Each read degrades
INDEPENDENTLY via `unwrap_or_default()` → empty set for that read only; worst case (all three fail)
= the slice-08 status quo, no crash/5xx/leak. The bind stays loopback-only; nothing is persisted;
the viewer holds no key.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-11 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-11 | Morgan (nw-solution-architect)                          |
| DISTILL | 2026-06-11 | Quinn (nw-acceptance-designer)                          |
| DELIVER | 2026-06-11 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **9 roadmap steps** done across **3 phases** (all COMMIT/PASS — or APPROVED_SKIP with
  rationale — in `deliver/execution-log.json`).
- **Acceptance scenarios GREEN**: the `viewer_search_full_follow_state` corpus (FF-1..FF-11 —
  including the **thick four-arm walking skeleton** at FF-1, and the FF-11 own-read
  independent-degrade AT added at 02-05) + the GOLD invariants
  (`viewer_search_full_follow_state_invariants` — the **6 GOLD invariants** FF-INV-*). Plus the
  `viewer-domain` unit/property tests (the two new render arms + the two indicator fns) and the
  dedicated `adapter-http-viewer` resolution + per-read-seam unit tests (the effect-shell
  four-arm classification + the two new fault seams). The `ViewerServer` harness drives the REAL
  `openlore ui` over HTTP; the indexer is the only mocked boundary (a REAL slice-08
  `openlore-indexer serve`); the LOCAL store is seeded through the REAL `claim add` (own → `You`)
  / `peer add` (active → `SubscribedPeer`) / `peer add`+`peer pull`+`peer remove` no-`--purge`
  (cached → `UnsubscribedCache`) verbs.
- **Slices 08/15/16 corpora GREEN — zero regression** (the slice-16 `SubscribedPeer` /
  `NetworkUnfollowed` arms byte-stable; the full workspace acceptance suite green across all
  slices).
- **NO new crate (near-all-EXTEND)**: extends `crates/ports` (+2 read-only `StoreReadPort`
  methods, no mutation method — I-VIEW-1 preserved), `crates/adapter-duckdb` (+2 single-table
  `SELECT DISTINCT author_did` impls over the shared connection), `crates/adapter-http-viewer`
  (EFFECT — the two new `read_local_*` reads + the four-arm `to_indexed_claim` precedence
  resolution + the two new per-read fault seams), `crates/viewer-domain` (PURE — the two filled
  arms + `SEARCH_SELF_INDICATOR` / `SEARCH_REMOVED_CACHED_INDICATOR` + `render_self_indicator` /
  `render_cached_unsubscribed_indicator`), `crates/xtask` (`VIEWER_FAIL_SEAM_TOKENS` extended
  4→6). REUSES the slice-08 `/search` route, the slice-16 active-set read + `bare_did` SSOT +
  fault-seam pattern, and the existing four-variant `AuthorRelationship` enum. Workspace member
  count stays **21**; `cargo xtask check-arch` reports "21 workspace members".
- **NO new `AuthorRelationship` variant, NO new route, NO new crate**: the resolution FILLS the
  two already-present-but-empty enum arms; the enum is unchanged.
- **Anti-merging green BY CONSTRUCTION**: each of the two new reads is **single-table**
  (`SELECT DISTINCT author_did FROM claims` / `FROM peer_claims`), so the `xtask`
  `no_cross_table_join_elides_author` rule's cross-store precondition is structurally unreachable
  — no JOIN, no author elision.
- **Mutation 100% of viable in-diff**: `viewer-domain` (the two render arms) + the two
  distinct-DID presence reads + the four-arm resolver covered; the **2 remaining "missed" are the
  cfg-dead release-identity-sibling artifact** (the `#[cfg(not(debug_assertions))]` siblings of
  the two new fault seams, not compiled under the debug test profile) — pinned structurally by the
  xtask guard + the release-build seam-free check (slice-16 DV-SF-9 precedent). The ≥80%-of-viable
  gate is MET (100% of viable).
- **1 ADR** (ADR-057) Accepted/shipped.
- DES integrity: **9/9** steps have complete DES traces.
- Adversarial review: **APPROVED**, **0 defects, 0 Testing Theater** (the four-arm precedence +
  the two per-read fault seams' release-safety verified).
- Gates: **DoR 9/9**, DESIGN **APPROVED**, DISTILL consolidated review (DISCUSS APPROVED, DESIGN
  APPROVED, DISTILL **CONDITIONALLY APPROVED** — the 1 high folded to DELIVER, RESOLVED there),
  Phase-3 refactor **none-needed**, review **APPROVED**, mutation **100% of viable**, integrity
  **9/9**, `check-arch` **OK (21)**, release build verified **seam-free** (all 6 fault tokens
  absent from the release rlib).

## Wave-by-wave changelog

### DISCUSS (2026-06-11)

Luna framed the slice as a **brownfield DELTA that COMPLETES slice-16** — it RESOLVES the two
arms slice-16 deferred (`You`, `UnsubscribedCache`), turning the binary `/search` follow-state
resolution into its full four-arm form. Persona is **P-001 (the node operator)**, the viewer's
operator wearing the network-discovery hat. The load-bearing DISCUSS decision: slice-16 left
`You | UnsubscribedCache => {}` EMPTY, so two relationship states the operator can genuinely be
in were collapsed into `NetworkUnfollowed` — the operator's OWN claim was shown as a follow-me
target, and a peer the operator had SOFT-REMOVED (cache still on disk) was shown as a new author
to follow. slice-20 RESOLVES each result author against THREE LOCAL sets so an own claim renders
a neutral self indicator, a soft-removed peer's cached claim renders a neutral residue indicator,
an active peer keeps slice-16's "Following", and only a genuinely-new author keeps the
`openlore peer add` affordance. The CARDINAL framing carried forward from slice-08/15/16:
read-only/no-key (all four affordances render-only TEXT, no executable control), LOCAL/offline
(the relationship resolved from LOCAL sets, the index query unchanged), per-read independent
graceful degrade (slice-17 `.ok()` independence per read), additive/no-regression (the slice-16
arms byte-stable), and neutral framing (non-pejorative copy for the two new indicators). 2 stories
(1 infra + 1 user-visible). Scope PASS (~0.5–1 day). DoR PASS (9/9).

### DESIGN (2026-06-11)

Morgan formalized the three DISCUSS-flagged questions into **ADR-057**, locking the slice as a
**near-all-EXTEND completion** — ZERO new crate, ZERO new route, ZERO new `AuthorRelationship`
variant. The open decisions were resolved adopting the DISCUSS leans, captured in one ADR:

- **ADR-057** (two single-table read-only presence reads + total four-arm precedence resolution +
  two neutral render-only indicators): the resolution is **effect-shell** — the
  `adapter-http-viewer` `/search` handler reads THREE LOCAL sets ONCE each into bare-DID
  `HashSet`s (active REUSED via `read_local_active_set`; own/cached NEW via two `read_local_*`
  sibling helpers); `to_indexed_claim` becomes a **TOTAL four-arm precedence resolution**
  (`You > SubscribedPeer > UnsubscribedCache > NetworkUnfollowed`) — a pure fn over the three
  sets, **batch-once** (no N+1). The **two new reads** are read-only `StoreReadPort` methods —
  `distinct_own_author_dids` (`SELECT DISTINCT author_did FROM claims` → `You`) and
  `distinct_cached_peer_author_dids` (`SELECT DISTINCT author_did FROM peer_claims`, **NO
  `removed_at` filter** so soft-removed peers' cached claims classify → `UnsubscribedCache`),
  each **single-table** so the `xtask` `no_cross_table_join_elides_author` anti-merging rule
  passes **BY CONSTRUCTION**. The **pure render** fills the two empty arms with two SSOT consts
  (`SEARCH_SELF_INDICATOR`, `SEARCH_REMOVED_CACHED_INDICATOR`) + two render-only sibling fns
  (`render_self_indicator`, `render_cached_unsubscribed_indicator`), making the render a TOTAL
  `match`; the slice-16 `SubscribedPeer` / `NetworkUnfollowed` arms are REUSED VERBATIM
  (byte-stable). It REUSES the existing four-variant `AuthorRelationship` enum (NO new variant)
  and adds NO new route/crate (workspace stays 21). The **fault seam** was the one open question
  (OQ-1, D-4): the NO-new-seam default was to inject a per-read `Err` via a fake `StoreReadPort`,
  with a **conditional escalation** — IF the harness cannot inject a per-read fault, DELIVER MUST
  add a distinct `#[cfg(debug_assertions)]` token per read + extend `VIEWER_FAIL_SEAM_TOKENS`
  (mirroring the slice-16 ADR-026-derived release-gate pattern). Alternatives rejected:
  combined-read (would lose per-read independent degrade), held-identity-surface-for-`You`
  (keyless-viewer-breaking), N+1 (rejected for the batch-once read).

The C4 L1+L2 views, the `/search` four-arm data-flow, and the FF-invariant structural-guarantee
table are in the DESIGN sections of `feature-delta.md` and `design/`. DESIGN closed **APPROVED**.

### DISTILL (2026-06-11)

Quinn authored the executable acceptance corpus across two `[[test]]` targets, mirroring the
slice-16 proven shape:

- **`viewer_search_full_follow_state.rs`** (Tier A — `FF-` ids FF-1..FF-10, plus FF-11 added at
  DELIVER): the **thick four-arm walking skeleton** (FF-1 — the `/search` view resolving the four
  arms: an own claim → "Your own claim", an active peer → "Following", a soft-removed cached peer
  → "A peer you removed (cached)", a genuinely-new author → `openlore peer add`), the
  **follow-state isolation** matrix (FF-2 / FF-3), the **precedence** assertions (FF-4 / FF-5 —
  `You` beats `SubscribedPeer` beats `UnsubscribedCache`), the **no-regression** assertion (FF-6 —
  the slice-16 arms byte-stable), the **bare-DID strip on the result side** (FF-7), the
  **read-only / neutral** assertion (FF-8 — all four affordances render-only TEXT, neutral copy),
  the **degrade** (FF-9), and the **htmx / no-JS parity** (FF-10 — the enrichment renders
  identically across full page and the results fragment).
- **`viewer_search_full_follow_state_invariants.rs`** (gold guardrails — the **6 GOLD invariants**
  FF-INV-*): read-only / no-write (all four arms render-only TEXT, no executable control on any
  shape), LOCAL / offline (the relationship resolved from the three LOCAL sets; vendored htmx
  only; loopback-only), four-arm completeness (the render is a TOTAL `match`), attribution /
  ranking unchanged (per-row enrichment, no merge/re-rank), additive / no-regression (the slice-16
  arms byte-stable), and graceful-degrade (each failed read → empty set independently → no
  crash/leak).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the indexer is
the only mocked boundary (a REAL slice-08 `openlore-indexer serve`); the LOCAL store is seeded
through the REAL `claim add` / `peer add` / `peer pull` / `peer remove` (no `--purge`) verbs. The
Reconciliation HARD GATE passed (0 contradictions). The DoR closed **9/9**. RED classification: 9
RED (8 assertion + 1 `todo!()` OQ-1 scaffold), 5 GREEN-today guardrails, 0 BROKEN. **The
consolidated DISTILL review was CONDITIONALLY APPROVED** — the 1 high was the OQ-1 fault-seam
question, FOLDED to DELIVER. Two seeding-seam bugs found by the fail-for-the-right-reason gate
were fixed (multi-peer pull collision; active-and-cached reseed).

### DELIVER (2026-06-11)

Executed **9 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick four-arm walking skeleton + isolation + precedence (01-xx)**: **01-01 is the
  THICK four-arm walking skeleton** (FF-1, 1b3eff7) — the two new presence reads + the four-arm
  `to_indexed_claim` precedence resolution + the two filled render arms + the two SSOT consts + the
  two render-only sibling fns. **01-02 (follow-state isolation, FF-2/3, 8362cc2)** and **01-03
  (precedence, FF-4/5, 511413d)** were confirmatory off the WS structure (RED_ACCEPTANCE/RED_UNIT
  APPROVED_SKIP — greened by the WS). **The thick WS drove most of the thread green.**
- **Phase 02 — bare-DID strip + no-regression/neutral + parity + the fault seams + own-degrade
  (02-xx)**: **02-01** the **fragment-strip cached-peer match** (FF-7, 5fb893f); **02-02** the
  **no-regression + read-only/neutral** edges (FF-6/8, 08494bf); **02-03** the **htmx / no-JS
  parity** (FF-10, 7fc7d34); **02-04** the **genuinely-new per-read fault seams** (531b635 — the
  OQ-1 escalation FIRED: `OPENLORE_VIEWER_FAIL_OWN_DIDS_READ` +
  `OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ`, each `#[cfg(debug_assertions)]` with a release
  identity sibling, + `xtask` `VIEWER_FAIL_SEAM_TOKENS` 4→6 + the in-crate seam unit tests) — **the
  real implementation work of the slice**; **02-05** the **FF-11 own-read independent-degrade AT**
  (09b111d — added to bring error coverage ≥40%, consuming the 02-04 own-DIDs seam;
  RED_UNIT APPROVED_SKIP since the seam is already pinned by the 02-04 in-crate tests).
- **Phase 03 — gold (03-xx)**: **03-01** the **6 GOLD invariants** (1b16ef8). They flipped GREEN
  off the confirmatory four-arm render path.
- **Mutation gate close**: c59d7d9 added the dedicated unit tests closing the mutation gap on the
  four-arm render arms + the two distinct-DID presence reads (the package-scoped harness gap, the
  slice-16/17 precedent).

The 9-step shape: a **thorough four-arm WS at 01-01** drove most scenarios green for free (the
total precedence resolution + the two filled arms make the isolation/precedence/no-regression/parity
scenarios confirmatory); the real implementation work was **the two per-read fault seams (OQ-1
escalation, 02-04)** and the **FF-11 own-degrade AT (02-05)**. The Phase-3 refactor was
**none-needed** (the two new render fns are already siblings of `render_following_indicator`; the
two reads are already single-table; the two fault seams must each stay a distinct literal token at
its cfg-gated site — a unification would defeat the xtask classifier's per-token guard, the
slice-18/19 precedent).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-FF-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..19 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-FF-2 | Mutation = per-feature 100% of the genuinely-viable in-diff (the two render arms + the two distinct-DID reads + the four-arm resolver caught; the 2 "missed" = the cfg-dead release-identity-sibling artifact of the two new fault seams), matching slice-16..19 DV-2/9. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. The 2 "missed" land on the `#[cfg(not(debug_assertions))]` siblings (not compiled under the debug test profile), pinned structurally; ≥80%-of-viable gate MET. |
| DV-FF-3 | **FILL the two empty `You | UnsubscribedCache` arms — NO new `AuthorRelationship` variant** (ADR-057). | slice-16 left the two arms EMPTY in the existing four-variant enum; filling them needs NO new variant — the enum was already total, slice-16 just hadn't rendered two of its arms. The render becomes a TOTAL `match`. |
| DV-FF-4 | **TWO new single-table read-only `StoreReadPort` reads — `distinct_own_author_dids` (claims) + `distinct_cached_peer_author_dids` (peer_claims, NO `removed_at` filter)**, read ONCE each per render into `HashSet`s (ADR-057). | The `You` arm needs the operator's OWN author DIDs; the `UnsubscribedCache` arm needs cached-peer author DIDs INCLUDING soft-removed peers — hence NO `removed_at` filter (the cache survives soft-remove). Single-table reads keep the anti-merging rule green by construction and avoid the N+1. |
| DV-FF-5 | **A pure TOTAL four-arm precedence resolver `You > SubscribedPeer > UnsubscribedCache > NetworkUnfollowed`** over the three `HashSet`s (ADR-057). | An author can be in more than one set (the operator's own DID could also appear as an active/cached peer DID in a degenerate seed); a FIXED precedence makes the resolution deterministic and total — own-identity wins, then active-follow, then cached-residue, then genuinely-new. |
| DV-FF-6 | **The bare-DID strip runs on the RESULT side against all three sets, via the slice-16 `bare_did` SSOT** (ADR-057). | The indexed author DID and the stored DIDs (own/active/cached) must compare on the SAME normalization across all three sets, or a known author silently classifies as `NetworkUnfollowed`; reusing the slice-16 `bare_did` SSOT keeps DID normalization in ONE place across all four arms. |
| DV-FF-7 | **All four arms stay render-only TEXT** — `You` → `SEARCH_SELF_INDICATOR` ("Your own claim"), `UnsubscribedCache` → `SEARCH_REMOVED_CACHED_INDICATOR` ("A peer you removed (cached)"), `SubscribedPeer` → slice-16 "Following", `NetworkUnfollowed` → slice-16 `openlore peer add` — `render_self_indicator` / `render_cached_unsubscribed_indicator` siblings of slice-16's `render_following_indicator`; NEITHER new arm renders a `peer add` affordance (ADR-057, OQ-2 neutral-copy gate). | The viewer is read-only and holds no key; rendering all four states as TEXT keeps the discovery→follow loop honest WITHOUT giving the web surface a mutation control. The neutral, non-pejorative copy ("removed (cached)", not "blocked"/"banned") keeps the residue framing factual. |
| DV-FF-8 | **OQ-1 escalation FIRED: two distinct `#[cfg(debug_assertions)]` per-read fault tokens (`OPENLORE_VIEWER_FAIL_OWN_DIDS_READ`, `OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ`); `xtask` `VIEWER_FAIL_SEAM_TOKENS` extended 4→6** (ADR-057 D-4 conditional escalation, materialized at 02-04). | The NO-new-seam default assumed a fake `StoreReadPort` could inject a per-read `Err`; the real-binary subprocess harness CANNOT (it drives the REAL `openlore ui` over HTTP, no fake-port seam). So the conditional escalation fired — each read gets its own `#[cfg(debug_assertions)]` token (release sibling = identity, no env read compiled in), mirroring the slice-16 `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` seam, with the xtask guard extended to cover all 6 tokens. |
| DV-FF-9 | **FF-11 own-read independent-degrade AT added at 02-05** to bring error coverage ≥40%, consuming the 02-04 own-DIDs seam (the DISTILL CONDITIONALLY-APPROVED high, RESOLVED). | The consolidated DISTILL review folded a high to DELIVER (error coverage). FF-11 exercises the own-read degrade end-to-end (failed own → no `You` arm, the other three arms intact — per-read independence), bringing error coverage ≥40%; the RED_UNIT was APPROVED_SKIP since the own-DIDs seam is already pinned by the 02-04 in-crate unit tests (a new unit test would be Test Duplication). |
| DV-FF-10 | **Each of the three reads degrades INDEPENDENTLY via `unwrap_or_default()` → empty set for the failed read only** (slice-17 per-count `.ok()` independence, ADR-057 D-4). | A single shared degrade would collapse all three arms on any one read failure; per-read independence means failed own → no `You` (others intact), failed cached → no `UnsubscribedCache`, failed active → slice-16 all-`NetworkUnfollowed`; worst case (all three fail) = slice-08 status quo. Never 5xx. |
| DV-FF-11 | **Phase-3 refactor: none-needed** (ADR-057). | The two new render fns are already siblings of `render_following_indicator` (no duplication to extract); the two reads are already single-table (no SQL-helper extraction — and a shared helper would defeat the xtask literal-SQL word-scan, the slice-19 precedent); the two fault tokens must each stay a distinct literal at its cfg-gated site (a unification would defeat the per-token xtask guard, the slice-18 precedent). Refactor correctly DECLINED. |

## Cardinal release gates + slice-20 invariants (I-FF-1..n)

The cardinal release gates realized on the four-arm search-follow-state surface — all
release-blocking:

1. **Read-only / no key (CARDINAL, I-FF-1)** — all four affordances render-only TEXT (`You` "Your
   own claim", `SubscribedPeer` "Following", `UnsubscribedCache` "A peer you removed (cached)",
   `NetworkUnfollowed` `openlore peer add <did>`), no executable control; the viewer holds no key.
   Three-layer: TYPE (no write method; +2 reads only) + STRUCTURAL (`xtask check-arch` viewer
   capability rule) + BEHAVIORAL (FF-8 + the read-only / no-write gold).
2. **Four-arm completeness (CARDINAL, I-FF-2)** — the render is a TOTAL `match` over all four
   `AuthorRelationship` variants; an own claim → "Your own claim", a soft-removed cached peer → "A
   peer you removed (cached)", an active peer → "Following", a genuinely-new author → keeps `peer
   add`. This COMPLETES slice-16's binary resolution (FF-1/2/3, the four-arm gold).
3. **Precedence (CARDINAL, I-FF-3)** — the resolution is a deterministic total fn with fixed
   precedence `You > SubscribedPeer > UnsubscribedCache > NetworkUnfollowed` (FF-4/5).
4. **LOCAL / offline (CARDINAL, I-FF-4)** — the relationship is resolved from THREE LOCAL sets
   (read-only, offline); the network index query is UNCHANGED and per-user-neutral (the same
   results for every operator; only the per-row affordance differs by who the operator is and
   follows) (FF-10 + the LOCAL/offline gold).
5. **Attribution / ranking unchanged (CARDINAL, I-FF-5)** — the relationship is per-row enrichment,
   NOT a merge or re-rank; attribution/ranking carry forward unchanged from slice-08/16.
6. **Additive / no-regression (CARDINAL, I-FF-6)** — the slice-16 `SubscribedPeer` /
   `NetworkUnfollowed` arms are REUSED VERBATIM (byte-stable); slice-08/15/16 corpora green (FF-6 +
   the no-regression gold).
7. **Graceful degrade — per-read independent (CARDINAL, I-FF-7)** — each failed read → empty set
   INDEPENDENTLY (failed own → no `You`; failed cached → no `UnsubscribedCache`; failed active →
   slice-16 all-`NetworkUnfollowed`); worst case (all three fail) = the slice-08 status quo, no
   crash/5xx/leak (FF-9/FF-11, via the two new test-only per-read fault seams).
8. **No-N+1 (I-FF-8)** — the three sets are read ONCE each per render into `HashSet`s, not once per
   result (the four-arm resolution is batch-once over the three sets).
9. **Anti-merging by construction (I-FF-9)** — each new read is single-table, so the `xtask`
   `no_cross_table_join_elides_author` rule's cross-store precondition is structurally unreachable.

| # | Invariant | Enforcement |
|---|---|---|
| I-FF-1 | Read-only / no key (all four arms render-only TEXT; no executable control; no key in the process). | TYPE (no write method; +2 reads only) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (FF-8 + the read-only / no-write gold, DV-FF-7). Cardinal. |
| I-FF-2 | Four-arm completeness (the render is a TOTAL `match`; own → self indicator, soft-removed cached → residue indicator, active → "Following", new → `peer add`). | STRUCTURAL (the total four-arm `to_indexed_claim` precedence resolution, DV-FF-3/5) + BEHAVIORAL (FF-1/2/3 + the four-arm gold). Cardinal. |
| I-FF-3 | Precedence (`You > SubscribedPeer > UnsubscribedCache > NetworkUnfollowed`, deterministic total fn). | STRUCTURAL (the fixed-precedence resolver, DV-FF-5) + BEHAVIORAL (FF-4/5). Cardinal. |
| I-FF-4 | LOCAL / offline (the relationship resolved from the three LOCAL sets, read-only/offline; the network index query unchanged + per-user-neutral). | STRUCTURAL (the three LOCAL reads; the index query untouched, DV-FF-4) + BEHAVIORAL (FF-10 + the LOCAL/offline gold). Cardinal. |
| I-FF-5 | Attribution / ranking unchanged (the relationship is per-row enrichment, not a merge/re-rank). | STRUCTURAL (per-row enrichment, no grouping/re-rank path) + BEHAVIORAL (the attribution gold). Cardinal. |
| I-FF-6 | Additive / no-regression (the slice-16 `SubscribedPeer` / `NetworkUnfollowed` arms byte-stable). | STRUCTURAL (the slice-16 arms reused verbatim) + BEHAVIORAL (FF-6 + the no-regression gold). Cardinal. |
| I-FF-7 | Graceful degrade — per-read independent (each failed read → empty set independently; no crash/5xx/leak). | STRUCTURAL (the per-read `unwrap_or_default` independence; the two test-only `#[cfg(debug_assertions)]` per-read fault seams + release identity siblings, DV-FF-8/10) + BEHAVIORAL (FF-9/FF-11 + the graceful-degrade gold). Cardinal. |
| I-FF-8 | No-N+1 (the three sets are read ONCE each per render into `HashSet`s, not once per result). | STRUCTURAL (the batch-once reads into sets, DV-FF-4) + BEHAVIORAL (covered by the four-arm WS + parity). |
| I-FF-9 | Anti-merging by construction (each new read is single-table; the cross-store author-elision rule is structurally unreachable). | STRUCTURAL (the single-table reads; `xtask no_cross_table_join_elides_author` green by construction, DV-FF-4). |

All slice-20 invariants INHERIT the slice-08 I-NS-1..9 + slice-16 I-SF-1..7 sets (read-only / no
key / offline + loopback / progressive enhancement / structural fragment/page parity /
anti-merging / verified-by-construction / public-data framing); the four-arm completion is
per-row enrichment that touches none of them.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_search_full_follow_state` corpus (FF-1..FF-11, the
  thick four-arm walking skeleton at FF-1, the own-read degrade at FF-11) + the GOLD
  `viewer_search_full_follow_state_invariants` (the 6 GOLD invariants) GREEN + the `viewer-domain`
  unit/property tests (the two new render arms + the two indicator fns) + the dedicated
  `adapter-http-viewer` resolution + per-read-seam unit tests; slices 08/15/16 corpora GREEN —
  zero regression. The `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the indexer
  is the only mocked boundary (a REAL slice-08 `openlore-indexer serve`); the LOCAL store is seeded
  through the REAL `claim add` / `peer add` / `peer pull` / `peer remove` (no `--purge`) verbs.
- **`cargo xtask check-arch`**: **OK (21 workspace members)** — no new crate, no new route, no new
  `AuthorRelationship` variant. The viewer capability rule is unchanged (read-only; no
  signing/identity/PDS, no store-write). The anti-merging rule `no_cross_table_join_elides_author`
  is green BY CONSTRUCTION (each new read single-table). `VIEWER_FAIL_SEAM_TOKENS` extended 4→6
  (the two new per-read fault tokens), and the seam guard fails if any of the 6 tokens is read
  outside a cfg gate.
- **Refactor (L1-L4)**: clippy + check-arch clean; **Phase-3 refactor none-needed** (the two new
  render fns are already `render_following_indicator` siblings; the two reads are single-table; the
  two fault tokens must each stay a distinct literal at its cfg-gated site — unification declined,
  slice-18/19 precedent). `viewer-domain` purity intact (no I/O imports; maud + ports only; the
  three reads + the four-arm classification live in the effect shell, not the pure core).
- **Release-build seam check**: the `#[cfg(not(debug_assertions))]` release build was verified
  **seam-free** — all 6 viewer fault tokens (the slice-16 active token + the slice-17 peer-claims
  token + the slice-18 own-countered token + the slice-19 peer-countered token + the two NEW
  own-DIDs / cached-peer-DIDs tokens) are absent from the release rlib (each release sibling is the
  identity function).
- **Adversarial review**: **APPROVED**, **0 defects, 0 Testing Theater**. The four-arm completion
  confirmed load-bearing (own → self indicator, soft-removed cached → residue indicator, active →
  Following, new → keeps-add, DV-FF-3/4/5); the read-only / render-only-affordance confirmed (all
  four arms TEXT, no executable control, DV-FF-7); the **two per-read fault seams' release-safety +
  the per-read independent degrade verified** (each seam test-only under `debug_assertions`, each
  release sibling the identity function, the xtask guard structurally forbids ungated reads of all
  6 tokens, DV-FF-8/9/10); the LOCAL/offline + attribution-unchanged + no-regression confirmed
  structural.
- **DES integrity**: PASS — all 9 steps have complete DES traces (**9/9**).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the two filled `You` /
`UnsubscribedCache` render arms + `render_self_indicator` / `render_cached_unsubscribed_indicator`
+ `SEARCH_SELF_INDICATOR` / `SEARCH_REMOVED_CACHED_INDICATOR`) + the `adapter-http-viewer`
effect-shell four-arm resolution (the four-arm `to_indexed_claim` precedence + the two new
`read_local_*` reads). The slice-16/17 cross-package lesson stays applied — the `viewer-domain`
tests pin the pure functions IN-crate, and the resolution + the two distinct-DID reads are covered
by the dedicated `adapter-http-viewer` unit tests (added at c59d7d9) + the acceptance suite.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (the two filled render arms + the two indicator fns, in-diff) | viable | all | 0 | **100%** (in-diff viable) |
| `adapter-http-viewer` four-arm resolution + the two distinct-DID reads | viable | all | 0 | covered by dedicated unit tests + the acceptance suite |
| cfg-dead release-identity siblings (the two new fault seams) | — | — | 2 | cfg-dead-branch artifact (not viable) |

**Mutation note (precise)**: mutation is **100% on the genuinely-viable in-diff**. The **2
remaining "missed" mutants** are the **cfg-dead-branch artifact**: they land on the
`#[cfg(not(debug_assertions))]` release **identity siblings** of the two NEW per-read fault seams,
which are NOT compiled under the debug test profile — so they are **neither reachable nor genuinely
viable**. Their debug twins' equivalent mutations are **killed by the four-arm WS + FF-11**, and the
release siblings are **independently pinned by the xtask guard (`VIEWER_FAIL_SEAM_TOKENS` 4→6) + the
release-build seam-free check** (DV-FF-8/9, the slice-16 DV-SF-9 precedent). The slice-20 per-feature
gate (≥80% of VIABLE) is **MET** (100% of viable, 0 genuinely-viable missed). `adapter-http-viewer` is
otherwise not mutation-swept by design (effect shell; covered by the GOLD invariants + the dedicated
unit tests through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **Filling a deferred-empty match arm completes a type to totality with NO new variant — the enum
  was already total, slice-16 just hadn't rendered two of its arms (DV-FF-3)**: slice-16 left
  `You | UnsubscribedCache => {}` empty in the existing four-variant `AuthorRelationship` enum.
  slice-20 needed NO new variant — only the two new LOCAL reads to populate the sets and the two
  render arms to project them. **Lesson: when a prior slice defers arms of an already-total ADT by
  leaving them empty, the completion is purely additive (new reads + filled arms), not a type
  change — the deferral cost is two empty arms, not a binary-vs-quaternary type migration.**
- **A thorough four-arm walking skeleton drove most of the thread green for free (DV-FF-3/5)**: the
  01-01 WS shipped the two reads + the four-arm precedence resolution + the two filled arms on day
  one, so isolation (FF-2/3), precedence (FF-4/5), no-regression (FF-6), and parity (FF-10) became
  confirmatory off the structure. **Lesson: when a slice's cardinals are a classification-shape
  decision (total precedence over a batch of set-membership reads), get the total resolution right
  inside the walking skeleton and most downstream scenarios become confirmation of the structure —
  the real work was the ONE genuinely-new bit (the per-read fault seams, OQ-1).**
- **A conditional fault-seam escalation must FIRE the moment the harness can't inject the fault —
  don't ship the NO-seam default on hope (DV-FF-8)**: ADR-057's D-4 default was to inject a per-read
  `Err` via a fake `StoreReadPort`, with a conditional escalation IF the harness couldn't. The
  real-binary subprocess harness drives the REAL `openlore ui` over HTTP — there IS no fake-port
  seam, so the escalation FIRED at DISTILL (OQ-1 RESOLVED) and materialized at 02-04. **Lesson: when
  a degrade scenario's NO-seam default depends on a fake-port injection the real-binary harness
  cannot provide, fire the escalation early (at DISTILL, when the harness shape is known) — gate each
  per-read trigger behind `debug_assertions`, make each release sibling the identity function, and
  extend the CI guard token set; the slice-16 seam is the proven pattern to mirror.**
- **Per-read independent degrade needs per-read seams (DV-FF-8/10)**: the three reads degrade
  INDEPENDENTLY (slice-17 `.ok()` independence), so a SINGLE shared fault token could not exercise
  "failed own → no `You`, others intact". Each read got its OWN `#[cfg(debug_assertions)]` token,
  and FF-11 (added at 02-05) exercises the own-read degrade end-to-end. **Lesson: when each read
  degrades independently, the fault seams must be per-read too — a shared token can only test
  all-fail, not the per-read isolation the independence promises.**
- **`distinct_cached_peer_author_dids` deliberately OMITS the `removed_at` filter — the cache
  survives soft-remove, and that residue IS the `UnsubscribedCache` arm (DV-FF-4)**: a soft-removed
  peer (`peer remove` no `--purge`) keeps its cached `peer_claims` on disk; the `UnsubscribedCache`
  arm exists precisely to render that residue honestly. Filtering on `removed_at` would have
  collapsed soft-removed peers into `NetworkUnfollowed`. **Lesson: when an arm's whole purpose is to
  surface residue, its read must NOT filter the residue out — the absence of the `removed_at` filter
  is load-bearing, mirroring slice-15's residue-made-visible cardinal from the opposite direction
  (slice-15 HIDES soft-removed peers from `/peers`; slice-20 SHOWS their cached claims' residue on
  `/search`).**
- **The bare-DID strip must run against all three sets on the RESULT side (DV-FF-6)**: extending
  slice-16's two-set comparison to three sets means the indexed author DID must normalize through the
  same `bare_did` SSOT against own/active/cached alike, or a known author silently classifies as
  `NetworkUnfollowed`. **Lesson: when a resolution widens from two sets to N, the normalization SSOT
  must cover every set at the comparison site — a strip that covers two of three sets is exactly
  where the third arm silently mis-resolves.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-057 fixed the contracts; field-level shaping (the two filled arms, the four-arm `to_indexed_claim` precedence, the two `read_local_*` reads, OQ-3 3-params-vs-struct) left to DELIVER. | All adopted; the two arms + the two SSOT consts + the two render fns + the batch-once four-arm precedence over three `HashSet`s materialized at DELIVER against the render + resolution tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-057 fixed "FILL the two empty arms (no new variant) + two single-table reads (no anti-merging risk) + workspace stays 21." | Shipped exactly — the two arms filled; `distinct_own_author_dids` / `distinct_cached_peer_author_dids` single-table; enum unchanged; workspace stays 21. | Resolved at DELIVER. |
| 3 | ADR-057 D-4: NO new fault seam DEFAULT; conditional escalation IF the harness can't inject a per-read fault. | The escalation FIRED (OQ-1) — the real-binary harness cannot inject a per-read `Err`; two `#[cfg(debug_assertions)]` per-read tokens (`OPENLORE_VIEWER_FAIL_OWN_DIDS_READ` / `OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ`) landed at 02-04; `VIEWER_FAIL_SEAM_TOKENS` 4→6; release verified seam-free. | Escalated as designed (the conditional branch fired); resolved at DELIVER (DV-FF-8). |
| 4 | DISTILL expected to close APPROVED. | The consolidated DISTILL review was CONDITIONALLY APPROVED — the 1 high (error coverage) folded to DELIVER. | Clarified at DISTILL; RESOLVED at DELIVER (FF-11, DV-FF-9). |
| 5 | OQ-1 (per-read fault injection) flagged for DISTILL/DELIVER. | RESOLVED — escalation fired (item 3); the seams materialized at 02-04, FF-11 own-degrade added at 02-05 (error coverage ≥40%), FF-1 docstring reworded. | Resolved at DELIVER. |
| 6 | OQ-2 (exact neutral copy) flagged for DELIVER within the blocklist gate. | Shipped the DESIGN names verbatim within the neutral gate: `SEARCH_SELF_INDICATOR = "Your own claim"`, `SEARCH_REMOVED_CACHED_INDICATOR = "A peer you removed (cached)"` (non-pejorative, blocklist-safe). | Resolved at DELIVER (OQ-2). |
| 7 | OQ-3 (`to_indexed_claim` 3 `&HashSet` params vs a resolution-context struct) flagged for the crafter. | Shipped as the total fn over the three sets (the PRODUCT/DESIGN contract); field-level shaping resolved at DELIVER. | Resolved at DELIVER (OQ-3). |
| 8 | Mutation expected 100% on the in-diff. | The two render arms + the two reads + the four-arm resolver caught; the 2 "missed" are the cfg-dead release-identity-sibling artifact of the two new fault seams (not viable), pinned structurally (DV-FF-2). ≥80%-of-viable gate MET. | Recorded; the artifact explained. |
| 9 | Phase-3 refactor expected to be evaluated. | None-needed — the two render fns are already `render_following_indicator` siblings; the two reads single-table; the two fault tokens must stay distinct literals at their cfg-gated sites (unification declined, slice-18/19 precedent, DV-FF-11). | Confirmed at DELIVER (refactor declined). |
| 10 | Review expected to pass clean. | Review APPROVED, 0 defects, 0 Testing Theater (the four-arm precedence + the two per-read fault seams' release-safety + the per-read independent degrade verified). | Confirmed at DELIVER. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-search-full-follow-state/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL/DELIVER [REF] sections), `discuss/`, `design/`, `distill/`, `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-16 archive** (the binary `/search` follow-state resolution this slice COMPLETES —
  the source of `render_following_indicator`, `read_local_active_set`, the `to_indexed_claim`
  classification, the `bare_did` SSOT, and the `#[cfg(debug_assertions)]` fault-seam pattern):
  `docs/evolution/viewer-search-follow-state-evolution.md`
- **Grandparent slice-08 archive** (the read-only `GET /search` network-discovery view + the
  `render_follow_guidance` / `openlore peer add` render-only affordance):
  `docs/evolution/viewer-network-search-evolution.md`
- **Slice-15 archive** (the soft-remove / residue-made-visible semantics the `UnsubscribedCache`
  arm mirrors): `docs/evolution/viewer-peer-subscriptions-evolution.md`
- **Slice-20 ADR**:
  `docs/adrs/ADR-057-search-four-arm-follow-state-two-local-presence-reads-own-claim-author-dids-and-cached-peer-author-dids-and-precedence-resolution-completing-the-author-relationship-adt.md`
- **Release-gate pattern reused**: `docs/adrs/ADR-026` (the pubkey-seam release-gate the per-read
  fault seams mirror; the xtask guard shares `classify_cfg_gated_token`) + the slice-16 ADR-053
  active-set fault seam (the direct precedent)
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-search-full-follow-state/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-search-full-follow-state/deliver/execution-log.json`,
  `docs/feature/viewer-search-full-follow-state/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_search_full_follow_state.rs` (FF-1..FF-11, the thick four-arm walking
  skeleton at FF-1, the own-read degrade at FF-11),
  `tests/acceptance/viewer_search_full_follow_state_invariants.rs` (the 6 GOLD invariants)
- **Reused render-only-affordance pattern**: `crates/viewer-domain`
  (`render_following_indicator`, slice-16) — `render_self_indicator` /
  `render_cached_unsubscribed_indicator` are its siblings; all share the `bare_did` SSOT
- **Reused active-set read**: `crates/adapter-http-viewer::read_local_active_set` (slice-16) — read
  ONCE per render into a `HashSet`, alongside the two NEW `read_local_*` reads
- **New reads + render arms (this slice)**: `crates/ports` (the two read-only `StoreReadPort`
  methods `distinct_own_author_dids` / `distinct_cached_peer_author_dids`), `crates/adapter-duckdb`
  (the two single-table `SELECT DISTINCT author_did` impls), `crates/adapter-http-viewer` (the two
  `read_local_*` reads + the four-arm `to_indexed_claim` precedence + the two
  `#[cfg(debug_assertions)]` per-read fault seams), `crates/viewer-domain` (the two filled arms +
  `SEARCH_SELF_INDICATOR` / `SEARCH_REMOVED_CACHED_INDICATOR` + `render_self_indicator` /
  `render_cached_unsubscribed_indicator`), `crates/xtask` (`VIEWER_FAIL_SEAM_TOKENS` 4→6)
- **Federation mutation surface (NOT replaced)**: the slice-03 CLI (`openlore peer add`) — the keyed
  follow path the read-only view points to
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml` — J-005c (turn a
  discovery into a follow — now COMPLETE across all four `AuthorRelationship` arms)
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-claim-list-flags-evolution.md`, `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`, `viewer-peer-subscriptions-evolution.md`,
  `viewer-search-follow-state-evolution.md`, `viewer-landing-dashboard-evolution.md`,
  `viewer-counter-aware-counts-evolution.md`, `viewer-peer-counter-aware-counts-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS cb9183c → DESIGN 00027bc → DISTILL 87406f7 → roadmap 3ea0b42 → 01-01 1b3eff7 (FF-1 WS) →
01-02 8362cc2 → 01-03 511413d → 02-01 5fb893f → 02-02 08494bf → 02-03 7fc7d34 →
02-04 531b635 (per-read fault seams, OQ-1 escalation) → 02-05 09b111d (FF-11 own-degrade) →
03-01 1b16ef8 (gold) → mutation-gate unit tests c59d7d9.
