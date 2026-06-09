# Evolution: viewer-search-follow-state (slice-16 per-result follow-state resolution on the read-only `GET /search` view)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-search-follow-state/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `distill/`, `deliver/`) and ADR-053 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-08 (`viewer-network-search` — the read-only `GET /search` network-discovery
> view this slice ENRICHES, and the source of the `render_follow_guidance` /
> `openlore peer add <did>` render-only affordance) and slice-15
> (`viewer-peer-subscriptions` — the source of the `list_active_peer_subscriptions`
> read this slice REUSES, and the `bare_did` SSOT this slice reconciles against).
> Read those parent archives
> (`docs/evolution/viewer-network-search-evolution.md`,
> `viewer-peer-subscriptions-evolution.md`) for the surfaces this slice composes.
> slice-16 realizes **J-005c** ("turn a discovery into a follow") — closing the
> discovery→federation loop on the browser surface.

## Summary

`viewer-search-follow-state` enriches the read-only **`GET /search`** network-discovery
view (slice-08) so each result resolves its author's **relationship against the
operator's LOCAL active peer subscriptions**. On render the viewer reads the slice-15
`list_active_peer_subscriptions` ONCE into a `HashSet` of bare peer DIDs; for each result
row the author DID is classified — an **already-followed author (`SubscribedPeer`)**
renders a neutral render-only **"Following" indicator** (`SEARCH_FOLLOWING_INDICATOR`, NO
peer-add command), while a **genuinely-unfollowed author (`NetworkUnfollowed`)** keeps the
slice-08 render-only `openlore peer add <did>` affordance. This **replaces the slice-08
hardcoded `NetworkUnfollowed`** — the gap where every result was always shown as
unfollowed, so an operator who had already subscribed to a discovered author was still
told to "follow" them. This is the J-005c "turn a discovery into a follow" job rendered
honestly: discoveries the operator already follows say so; only genuinely-new authors
carry the follow affordance. (You/own-DID resolution is deferred.)

The load-bearing thesis: **a per-row relationship enrichment that takes on authority over
nothing — both affordances stay render-only TEXT, the network index query is unchanged,
and a failed local read degrades to the slice-08 status quo.** The viewer signs/writes/
persists nothing and holds no signing key; the relationship is resolved from the LOCAL
active set (read-only, offline), the network index query is untouched and per-user-neutral
(the same results for every operator; only the per-row affordance differs by who the
operator follows), and attribution/ranking are unchanged (the relationship is per-row
enrichment, NOT a merge or re-rank). A failed active-set read maps to an empty set → all
authors classify as `NetworkUnfollowed` → the slice-08 "always-unfollowed" status quo, no
crash/5xx/leak.

The slice ships **ZERO new crates** (workspace stays at **21 members**). It is an
**additive enrichment, not a re-architecture**: NO new route, NO new crate, NO new
`AuthorRelationship` variant (it REUSES the existing enum), NO new read method (it REUSES
the slice-15 `list_active_peer_subscriptions`). The work is an effect-shell resolution
(read the local active set into a `HashSet`; `to_indexed_claim` classifies each author by
membership) + a pure render addition (a `SubscribedPeer` arm + the
`SEARCH_FOLLOWING_INDICATOR` constant + `render_following_indicator`, a sibling of
slice-08's `render_follow_guidance`). The one genuinely-new bit of machinery is the
**fault-injection seam** for the degrade scenario (SF-8): a TEST-ONLY env seam honored
only under `#[cfg(debug_assertions)]`, plus a NEW `xtask check-arch` guard that fails if
the seam token is read outside a cfg gate.

### What shipped (one paragraph)

The slice-08 `GET /search` handler now, on render, reads the slice-15
`list_active_peer_subscriptions` ONCE (the effect shell) into a `HashSet` of **bare**
peer DIDs; `to_indexed_claim` computes `bare_did(author_did)` and classifies each result
author by set membership — `∈ active → SubscribedPeer`, else `NetworkUnfollowed` — a
**batch-once** resolution (no N+1). The bare-DID strip on the RESULT side reconciles the
indexed author DID with the bare `peer_did` already stored in the active set (the slice-15
`bare_did` SSOT). The pure `viewer-domain` render gains a `SubscribedPeer` arm plus the
`SEARCH_FOLLOWING_INDICATOR` constant and `render_following_indicator` (sibling of
`render_follow_guidance`): a `SubscribedPeer` row renders the neutral render-only
"Following" indicator (NO `openlore peer add` command); a `NetworkUnfollowed` row keeps
the slice-08 render-only `openlore peer add <did>` guidance TEXT. The resolution is
**binary** (`SubscribedPeer` vs `NetworkUnfollowed`; `You`/`UnsubscribedCache` deferred).
The active-set read is **LOCAL and read-only** (offline); the network index query is
UNCHANGED (still per-user-neutral); attribution/ranking are UNCHANGED (per-row
enrichment, no merge/re-rank). A failed active-set read → empty set → all-`NetworkUnfollowed`
(the slice-08 status quo), no crash/5xx/leak. The bind stays loopback-only; nothing is
persisted; the viewer holds no key.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-09 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-09 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-09 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-09 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **8/8 roadmap steps** done across **3 phases** (all COMMIT/PASS in
  `deliver/execution-log.json`).
- **Acceptance scenarios GREEN**: the `viewer_search_follow_state` corpus
  (SF-1..SF-10 — including the **thick walking skeleton** at 01-01) + the GOLD invariants
  (`viewer_search_follow_state_invariants` — the 4 GOLD invariants). Plus the
  `viewer-domain` unit/property tests (the new `SubscribedPeer` arm +
  `render_following_indicator`) and the **3 dedicated `adapter-http-viewer` resolution
  unit tests** (the effect-shell `HashSet` classification). The `ViewerServer` harness
  drives the REAL `openlore ui` over HTTP; the indexer is the only mocked boundary (a REAL
  slice-08 `openlore-indexer serve`), and the active set is seeded through the REAL `peer
  add` / `peer pull` verbs.
- **Slices 08/15 corpora GREEN — zero regression** (the full workspace acceptance suite
  green across all slices).
- **NO new crate**: extends `viewer-domain` (PURE — the `SubscribedPeer` arm +
  `SEARCH_FOLLOWING_INDICATOR` + `render_following_indicator`) + `adapter-http-viewer`
  (EFFECT — the active-set read + the `to_indexed_claim` classification + the fault seam) +
  `xtask` (the new guard) in place; REUSES the slice-08 `/search` route, the slice-15
  `list_active_peer_subscriptions` read, the existing `AuthorRelationship` enum, and the
  `bare_did` SSOT. Workspace member count stays **21**; `cargo xtask check-arch` reports
  "21 workspace members".
- **NO new `AuthorRelationship` variant, NO new read method, NO new route, NO new crate**:
  the resolution reuses the existing enum (binary `SubscribedPeer` / `NetworkUnfollowed`;
  `You`/`UnsubscribedCache` deferred) and the slice-15 read.
- **Mutation 100% on the genuinely-viable in-diff**: `viewer-domain` **2/2 caught**;
  `adapter-http-viewer` resolution covered by the 3 dedicated unit tests + the acceptance
  suite. The single cargo-mutants "missed" is a **cfg-dead-branch artifact** (see the
  Mutation report) — not a genuinely-viable survivor; the ≥80%-of-viable gate is MET.
- **1 ADR** (ADR-053) Accepted/shipped.
- DES integrity: **8/8** steps have complete DES traces.
- Adversarial review: **APPROVED**, **0 defects, 0 Testing Theater** (the fault-seam
  release-safety + the xtask-guard robustness verified).
- Gates: **DoR 9/9**, DESIGN sound (the DESIGN reviewer's "blocked" was a mislabeled
  pre-DELIVER implementation-TODO list, not design defects), DISTILL **APPROVED**, review
  **APPROVED**, mutation **100% viable**, integrity **8/8**, `check-arch` **OK (21)**.

## Wave-by-wave changelog

### DISCUSS (2026-06-09)

Luna framed the slice as a **brownfield DELTA on slices 08/15** that realizes **J-005c**
("turn a discovery into a follow") — closing the discovery→federation loop on the browser
surface. Persona is **P-001 (the node operator)**, the viewer's operator wearing the
network-discovery hat. The load-bearing DISCUSS decision: the slice-08 `/search` view
**hardcoded every result author as `NetworkUnfollowed`**, so an operator who had ALREADY
subscribed to a discovered author was still told to "follow" them — the discovery→follow
loop showed the wrong state. slice-16 RESOLVES each result author's relationship against
the operator's LOCAL active peer subscriptions so an already-followed author renders a
neutral "Following" indicator (no follow affordance) and only a genuinely-unfollowed
author keeps the `openlore peer add` affordance. The CARDINAL framing carried forward from
slice-08/15: read-only/no-key (both affordances render-only TEXT), LOCAL/offline (the
relationship resolved from the LOCAL active set, the index query unchanged), and
attribution/ranking unchanged (per-row enrichment, no merge/re-rank). The walking skeleton
is the thick thread (the active-set read + the `to_indexed_claim` classification + the
`SubscribedPeer` render arm + the indicator), validating the riskiest assumption first —
that the read-only `/search` view can resolve per-result follow state against the local
active set while preserving every cardinal and adding no executable control.

### DESIGN (2026-06-09)

Morgan locked slice-16 as an **additive enrichment, not a re-architecture** — ZERO new
crates, ZERO new binary, ZERO new architectural style, ZERO new persisted type, ZERO new
route, ZERO new `AuthorRelationship` variant, ZERO new read method. The open decisions were
resolved adopting the DISCUSS leans, captured in one ADR:

- **ADR-053** (viewer-side search relationship resolution against the local active set +
  render-only Following indicator): the resolution is **effect-shell** — the
  `adapter-http-viewer` `/search` handler reads `list_active_peer_subscriptions` (slice-15,
  REUSED) ONCE into a `HashSet` of bare peer DIDs; `to_indexed_claim` computes
  `bare_did(author_did)` and classifies `∈ active → SubscribedPeer` else
  `NetworkUnfollowed` — a **batch-once** resolution (no N+1). The **bare-DID strip on the
  RESULT side** reconciles the indexed author DID with the bare `peer_did` stored in the
  active set (the slice-15 `bare_did` SSOT). The **pure render** adds a `SubscribedPeer`
  arm + the `SEARCH_FOLLOWING_INDICATOR` constant + `render_following_indicator` (a sibling
  of slice-08's `render_follow_guidance` — TEXT, not an executable control). The resolution
  is **binary** (`SubscribedPeer` vs `NetworkUnfollowed`; `You`/`UnsubscribedCache`
  deferred). It REUSES the existing `AuthorRelationship` enum (NO new variant), the slice-15
  read (NO new read method), and adds NO new route/crate (workspace stays 21). The **fault
  seam** (SF-8) — the mid-request active-set read-failure degrade — is exercised via a
  TEST-ONLY `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` env seam honored ONLY by a
  `#[cfg(debug_assertions)]` function (the `#[cfg(not(debug_assertions))]` release sibling
  is the identity function — NO env read compiled in), with a NEW `xtask check-arch` guard
  (`scan_viewer_fail_seam_guard`) that fails if the token is read outside a cfg gate
  (mirroring the ADR-026 pubkey-seam release-gate pattern + sharing the hardened
  `classify_cfg_gated_token`).

The C4 views, the `/search` enrichment data-flow, and the SF-invariant structural-guarantee
table are in the DESIGN sections of `feature-delta.md` and `design/`. The DESIGN reviewer's
initial "blocked" was a **mislabeled pre-DELIVER implementation-TODO list, not design
defects** — the design itself was sound; DISTILL closed at APPROVED.

### DISTILL (2026-06-09)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_search_follow_state.rs`** (Tier A — `SF-` ids SF-1..SF-10): the **thick
  walking skeleton** (SF-1 — the `/search` view resolving a result author against the local
  active set, rendering the `SubscribedPeer` "Following" indicator), the
  **all-followed / none-followed** matrix (SF-2 / SF-3 — every result author followed →
  all "Following"; none followed → all keep `openlore peer add`), the **read-only** assertion
  (SF-4 — both affordances render-only TEXT, no executable control), the **LOCAL + fragment**
  parity (SF-5 / SF-6 — the relationship resolved from the LOCAL active set, parity across
  page/fragment), the **no-N+1** assertion (SF-7 — the active set read ONCE per render, not
  per-result), the **fault-injection degrade** (SF-8 — a mid-request active-set read failure
  → empty set → all-`NetworkUnfollowed`, the slice-08 status quo, no crash/5xx/leak, via the
  test-only fault seam), and the **parity + attribution** assertions (SF-9 / SF-10 — the
  network index query unchanged + per-user-neutral; attribution/ranking unchanged).
- **`viewer_search_follow_state_invariants.rs`** (gold guardrails — the 4 GOLD invariants):
  read-only / no-write (both affordances render-only TEXT, no executable control on any
  shape), LOCAL / offline (the relationship resolved from the LOCAL active set; the page
  references only the vendored local htmx asset, no CDN; loopback-only), attribution / ranking
  unchanged (per-row enrichment, no merge/re-rank), and graceful-degrade (a failed active-set
  read → empty set → the slice-08 status quo, no crash/leak).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the indexer
is the only mocked boundary (a REAL slice-08 `openlore-indexer serve`), and the active set is
seeded through the REAL `peer add` / `peer pull` verbs. The DoR closed **9/9**. RED
classification: both targets COMPILE green, scenarios FAIL via `todo!()` =
MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-09)

Executed **8 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each
commit carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton + all/none + read-only (01-xx)**: **01-01 is the THICK
  walking skeleton** (SF-1) — the active-set read into a `HashSet` + the `to_indexed_claim`
  classification + the `SubscribedPeer` render arm + the `SEARCH_FOLLOWING_INDICATOR` constant
  + `render_following_indicator`. **01-02 (all-followed / none-followed, SF-2/3)** and **01-03
  (read-only, SF-4)** were confirmatory off the WS structure. **The thick WS drove most of the
  thread green.**
- **Phase 02 — LOCAL + fragment + no-N+1 + the fault seam + parity/attribution (02-xx)**:
  **02-01** the **LOCAL + fragment** parity (SF-5/6); **02-02** the **no-N+1** assertion (SF-7
  — the active set read ONCE per render); **02-03** the **genuinely-new fault-injection seam**
  (SF-8 — the `#[cfg(debug_assertions)]` env seam + the release identity sibling + the new
  `xtask` guard `scan_viewer_fail_seam_guard`) — **the real implementation work of the slice**;
  **02-04** the **parity + attribution** assertions (SF-9/10 — index query unchanged +
  per-user-neutral, attribution/ranking unchanged).
- **Phase 03 — gold (03-xx)**: **03-01** the **4 GOLD invariants** (read-only / no-write,
  LOCAL / offline, attribution / ranking unchanged, graceful-degrade). They flipped GREEN off
  the confirmatory render path.

The 8-step shape: a **thorough WS at 01-01** drove most scenarios green for free (the
classification + the render arm make the all/none/read-only/parity scenarios confirmatory); the
real implementation work was **SF-8 (the fault seam + the xtask guard)**. The Phase-3 refactor
(d20a177) **renamed `index_query_active_set` → `read_local_active_set`** (an L1 clarity rename:
the read is LOCAL, not the remote network index).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-SF-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..15 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-SF-2 | Mutation = per-feature 100% on the genuinely-viable in-diff (`viewer-domain` 2/2 caught; `adapter-http-viewer` resolution covered by 3 dedicated unit tests + the acceptance suite), matching slice-02..15 DV-2. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. The single cargo-mutants "missed" is a cfg-dead-branch artifact (see Mutation note), not a viable survivor; ≥80%-of-viable gate MET. |
| DV-SF-3 | **REUSE the existing `AuthorRelationship` enum — NO new variant** (binary `SubscribedPeer` vs `NetworkUnfollowed`; `You`/`UnsubscribedCache` deferred) (ADR-053). | A new enum variant would ripple through every match site in the render; the binary resolution covers J-005c (followed vs not) with the existing enum, deferring `You`/`UnsubscribedCache` to a later slice rather than widening the type now. |
| DV-SF-4 | **REUSE the slice-15 `list_active_peer_subscriptions` read — NO new read method**, read ONCE per render into a `HashSet` (ADR-053). | A new read seam (or a per-result read) is the classic N+1 and the place the active-set semantics could drift from slice-15; reading the slice-15 aggregate ONCE into a set keeps ONE active-set definition workspace-wide and the resolution batch-once (no N+1, SF-7). |
| DV-SF-5 | **The bare-DID strip on the RESULT side reconciles the indexed author DID with the bare `peer_did` in the active set, via the slice-15 `bare_did` SSOT** (ADR-053). | The indexed author DID and the stored `peer_did` must compare on the SAME normalization, or a followed author silently classifies as unfollowed; reusing the slice-15 `bare_did` SSOT on the result side keeps DID normalization in ONE place across both surfaces. |
| DV-SF-6 | **Both affordances stay render-only TEXT** — the `SubscribedPeer` neutral "Following" indicator (`SEARCH_FOLLOWING_INDICATOR`, no peer-add command) + the `NetworkUnfollowed` `openlore peer add <did>` guidance — `render_following_indicator` a sibling of slice-08's `render_follow_guidance` (ADR-053). | The viewer is read-only and holds no key; rendering both states as TEXT keeps the discovery→follow loop honest WITHOUT giving the web surface a mutation control. The read-only / no-write gold proves no executable control exists on any shape. |
| DV-SF-7 | **The fault seam is TEST-ONLY: the `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` token is honored ONLY by a `#[cfg(debug_assertions)]` function; the `#[cfg(not(debug_assertions))]` release sibling is the identity function (NO env read compiled in)** — release build verified seam-free (ADR-053, mirroring the ADR-026 pubkey-seam release-gate pattern). | SF-8 (the mid-request degrade) needs a deterministic fault trigger, but a fault hook compiled into release is a production liability; gating the env read behind `debug_assertions` keeps the release binary seam-free while the debug profile drives the degrade scenario. |
| DV-SF-8 | **A NEW `xtask check-arch` guard `scan_viewer_fail_seam_guard` fails if the seam token is ever read outside a cfg gate**, sharing the hardened `classify_cfg_gated_token` with the ADR-026 pubkey-seam guard (ADR-053). | A `#[cfg]`-gated seam is only release-safe if NOTHING reads the token outside the gate; the xtask guard makes that a STRUCTURAL check (CI-enforced), not a code-review hope — and sharing `classify_cfg_gated_token` keeps the cfg-gate classification logic in ONE hardened place. |
| DV-SF-9 | **The fault-seam degrade is independently pinned at three layers**: the debug seam's classification mutation is killed by SF-1; the release identity sibling is pinned by the xtask guard + the release-build seam-free check (ADR-053). | The single cargo-mutants "missed" lands on the release identity sibling (not compiled under the debug test profile, so neither reachable nor genuinely viable); its debug twin is killed by SF-1, and the release sibling is structurally pinned — so the cfg-dead-branch artifact is covered without theatre. |
| DV-SF-10 | **Phase-3 refactor renamed `index_query_active_set` → `read_local_active_set`** (L1 clarity, d20a177). | The original name implied a remote network-index query; the read is LOCAL (the slice-15 active set on disk). Renaming it keeps the LOCAL/offline cardinal legible at the call site — the name now matches the behavior. |

## Cardinal release gates + slice-16 invariants (I-SF-1..n)

The cardinal release gates realized on the search-follow-state surface — all release-blocking:

1. **Read-only / no key (CARDINAL, I-SF-1)** — both affordances render-only TEXT (the
   `SubscribedPeer` "Following" indicator + the `NetworkUnfollowed` `openlore peer add <did>`
   guidance), no executable control; the viewer holds no key. Three-layer: TYPE (no write
   method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (SF-4 + the
   read-only / no-write gold).
2. **Accuracy (CARDINAL, I-SF-2)** — a followed author → "Following" + no `peer add`; an
   unfollowed author → keeps the `peer add` affordance. This is the FIX to slice-08's
   "always-unfollowed" gap (SF-1/2/3).
3. **LOCAL / offline (CARDINAL, I-SF-3)** — the relationship is resolved from the LOCAL active
   set (read-only, offline); the network index query is UNCHANGED and per-user-neutral (the same
   results for every operator; only the per-row affordance differs by who the operator follows)
   (SF-5/6/9).
4. **Attribution / ranking unchanged (CARDINAL, I-SF-4)** — the relationship is per-row
   enrichment, NOT a merge or re-rank; attribution/ranking carry forward unchanged from slice-08
   (SF-10).
5. **Graceful degrade (CARDINAL, I-SF-5)** — a failed active-set read → empty set → all authors
   classify as `NetworkUnfollowed` → the slice-08 "always-unfollowed" status quo, no crash/5xx/leak
   (SF-8, via the test-only fault seam).
6. **No-N+1 (I-SF-6)** — the active set is read ONCE per render into a `HashSet`, not once per
   result (SF-7).
7. **Fragment/page parity (I-SF-7)** — the enrichment renders identically across full page (without
   `HX-Request`) and the results fragment (with it) (SF-5/6).

| # | Invariant | Enforcement |
|---|---|---|
| I-SF-1 | Read-only / no key (both affordances render-only TEXT — the `SubscribedPeer` "Following" indicator and the `NetworkUnfollowed` `openlore peer add` guidance; no executable control; no key in the process). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (SF-4 + the read-only / no-write gold, DV-SF-6). Cardinal. |
| I-SF-2 | Accuracy (followed → "Following" + no `peer add`; unfollowed → keeps `peer add`; the fix to slice-08's always-unfollowed gap). | STRUCTURAL (the `to_indexed_claim` set-membership classification, DV-SF-4) + BEHAVIORAL (SF-1/2/3). Cardinal. |
| I-SF-3 | LOCAL / offline (the relationship resolved from the LOCAL active set, read-only/offline; the network index query unchanged + per-user-neutral). | STRUCTURAL (the LOCAL active-set read; the index query untouched, DV-SF-4) + BEHAVIORAL (SF-5/6/9 + the LOCAL / offline gold). Cardinal. |
| I-SF-4 | Attribution / ranking unchanged (the relationship is per-row enrichment, not a merge/re-rank). | STRUCTURAL (per-row enrichment, no grouping/re-rank path) + BEHAVIORAL (SF-10 + the attribution gold). Cardinal. |
| I-SF-5 | Graceful degrade (a failed active-set read → empty set → all-`NetworkUnfollowed`, the slice-08 status quo; no crash/5xx/leak). | STRUCTURAL (the empty-set-on-error mapping; the test-only `#[cfg(debug_assertions)]` fault seam + the release identity sibling, DV-SF-7) + BEHAVIORAL (SF-8 + the graceful-degrade gold). Cardinal. |
| I-SF-6 | No-N+1 (the active set is read ONCE per render into a `HashSet`, not once per result). | STRUCTURAL (the batch-once read into a set, DV-SF-4) + BEHAVIORAL (SF-7). |
| I-SF-7 | Fragment/page parity (the enrichment renders identically across full page without `HX-Request` and the results fragment with it). | STRUCTURAL (the page renderer embeds the results fragment, slice-08 pattern) + BEHAVIORAL (SF-5/6). |

All slice-16 invariants INHERIT the slice-08 I-NS-1..9 + slice-15 I-PS-1..n sets (read-only /
no key / offline + loopback / progressive enhancement / structural fragment/page parity /
anti-merging / verified-by-construction / public-data framing); the relationship is per-row
enrichment that touches none of them.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_search_follow_state` corpus (SF-1..SF-10, the thick
  walking skeleton at SF-1) + the GOLD `viewer_search_follow_state_invariants` (the 4 GOLD
  invariants) GREEN + the `viewer-domain` unit/property tests (the `SubscribedPeer` arm +
  `render_following_indicator`) + the **3 dedicated `adapter-http-viewer` resolution unit tests**
  (the effect-shell `HashSet` classification); slices 08/15 corpora GREEN — zero regression. The
  `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the indexer is the only mocked
  boundary (a REAL slice-08 `openlore-indexer serve`); the active set is seeded through the REAL
  `peer add` / `peer pull` verbs.
- **`cargo xtask check-arch`**: **OK (21 workspace members)** — no new crate, no new route, no new
  `AuthorRelationship` variant, no new read method. The viewer capability rule is unchanged
  (read-only; no signing/identity/PDS, no store-write). The **NEW guard** is
  `scan_viewer_fail_seam_guard` (fails if the `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` token is read
  outside a cfg gate), sharing the hardened `classify_cfg_gated_token` with the ADR-026 pubkey-seam
  guard.
- **Refactor (L1-L4)**: clippy + check-arch clean; the Phase-3 refactor (d20a177) **renamed
  `index_query_active_set` → `read_local_active_set`** (L1 clarity — the read is LOCAL, not the
  remote network index); `viewer-domain` purity intact (no I/O imports; maud + ports only; the
  classification + the active-set read live in the effect shell, not the pure core).
- **Release-build seam check**: the `#[cfg(not(debug_assertions))]` release build was verified
  **seam-free** — the env token is not compiled into the release binary (the release sibling is the
  identity function).
- **Adversarial review**: **APPROVED**, **0 defects, 0 Testing Theater**. The accuracy fix
  confirmed load-bearing (followed → "Following" + no-add, unfollowed → keeps-add, DV-SF-3/4); the
  read-only / render-only-affordance confirmed (both states are TEXT, no executable control,
  DV-SF-6); the **fault-seam release-safety + the xtask-guard robustness verified** (the seam is
  test-only under `debug_assertions`, the release sibling is the identity function, the xtask guard
  structurally forbids ungated reads, DV-SF-7/8/9); the LOCAL/offline + attribution-unchanged
  confirmed structural.
- **DES integrity**: PASS — all 8 steps have complete DES traces (**8/8**).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the `SubscribedPeer` render arm +
`render_following_indicator` + `SEARCH_FOLLOWING_INDICATOR`) + the `adapter-http-viewer`
effect-shell resolution (`to_indexed_claim` classification + the active-set read). The slice-04/05
cross-package lesson stays applied — the `viewer-domain` tests pin the pure functions IN-crate, and
the resolution is covered by the 3 dedicated `adapter-http-viewer` unit tests + the acceptance
suite.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`SubscribedPeer` arm + `render_following_indicator`, in-diff) | 2 | 2 | 0 | **100%** (2/2 in-diff viable) |
| `adapter-http-viewer` resolution (`to_indexed_claim` classification + active-set read) | — | — | — | covered by 3 dedicated unit tests + the acceptance suite |

**Mutation note (precise)**: mutation is **100% on the genuinely-viable in-diff**. The single
cargo-mutants **"missed"** is a **cfg-dead-branch artifact**: the mutant (`Ok(Default::default())`)
lands on the `#[cfg(not(debug_assertions))]` release **identity sibling**, which is NOT compiled
under the debug test profile — so it is **neither reachable nor genuinely viable**. Its debug-twin's
equivalent mutation is **killed by SF-1**, and the release sibling is **independently pinned by the
xtask guard + the release-build seam-free check** (DV-SF-9). The slice-16 per-feature gate
(≥80% of VIABLE) is **MET** (100% of viable, 0 genuinely-viable missed). `adapter-http-viewer` is
otherwise not mutation-swept by design (effect shell; covered by the GOLD invariants + the 3
dedicated unit tests through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **A thorough walking skeleton drove most of the thread green for free**: the 01-01 WS shipped the
  active-set read into a `HashSet` + the `to_indexed_claim` classification + the `SubscribedPeer`
  render arm on day one, so all-followed / none-followed (SF-2/3), read-only (SF-4), LOCAL + parity
  (SF-5/6), and no-N+1 (SF-7) became confirmatory off the structure. **Lesson: when a slice's
  cardinals are a classification-shape decision (set-membership over a batch-once read), get the
  classification right inside the walking skeleton and most downstream scenarios become confirmation
  of the structure rather than new work — the real implementation work was the ONE genuinely-new bit
  (the fault seam, SF-8).**
- **The fault-injection seam is the one genuinely-new bit of machinery — gate it behind
  `debug_assertions` and STRUCTURALLY forbid ungated reads (DV-SF-7/8)**: SF-8 (the mid-request
  active-set read-failure degrade) needs a deterministic fault trigger, but a fault hook compiled
  into release is a production liability. The seam is honored ONLY by a `#[cfg(debug_assertions)]`
  function (the release sibling is the identity function, no env read compiled in), and a NEW xtask
  guard fails if the token is read outside a cfg gate. **Lesson: when a degrade scenario needs a
  deterministic fault, mirror the established release-gate pattern (ADR-026 pubkey seam) — gate the
  trigger behind `debug_assertions`, make the release sibling the identity function, and add a CI
  guard that structurally forbids ungated reads of the token; share the cfg-gate classifier so the
  gate logic lives in ONE hardened place.**
- **A cargo-mutants "missed" on a cfg-dead branch is an artifact, not a survivor — verify before
  treating it as a gate failure (DV-SF-9)**: the single "missed" mutant landed on the
  `#[cfg(not(debug_assertions))]` release identity sibling, which the debug test profile does not
  compile, so it is neither reachable nor genuinely viable; its debug twin is killed by SF-1 and the
  release sibling is pinned by the xtask guard + the release-build check. **Lesson: when a mutant
  survives on a cfg-gated branch, check which profile compiles it — a mutant on a branch that is not
  compiled under the test profile is a cfg-dead-branch artifact (neither reachable nor viable), and
  the gate is the ratio of VIABLE mutants killed; pin the dead branch structurally (the xtask guard
  + the release-build check) rather than chasing an unkillable artifact.**
- **REUSE the enum and the read; defer the variants (DV-SF-3/4)**: the binary resolution
  (`SubscribedPeer` vs `NetworkUnfollowed`) covers J-005c with the EXISTING `AuthorRelationship`
  enum and the EXISTING slice-15 read — no new variant, no new read method. `You`/`UnsubscribedCache`
  are deferred. **Lesson: realize the job with the existing type and the existing read where the
  binary distinction suffices; deferring `You`/`UnsubscribedCache` keeps the enum (and every match
  site) from widening before there's a scenario that needs it.**
- **The bare-DID strip must run on the RESULT side too, against the slice-15 `bare_did` SSOT
  (DV-SF-5)**: the indexed author DID and the stored `peer_did` must compare on the SAME
  normalization, or a followed author silently classifies as unfollowed. **Lesson: when resolving a
  relationship by comparing two DIDs from two surfaces, normalize BOTH through the same SSOT (here
  `bare_did`) at the comparison site — a one-sided strip is exactly where a followed author reads as
  unfollowed.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-053 fixed the contracts; field-level shaping (the `SubscribedPeer` render arm, the `to_indexed_claim` classification, the `HashSet` read) left to DELIVER. | All adopted; the `SubscribedPeer` arm + `SEARCH_FOLLOWING_INDICATOR` + `render_following_indicator` + the batch-once classification materialized at DELIVER against the render + resolution tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-053 fixed "REUSE the existing `AuthorRelationship` enum (no new variant) + the slice-15 read (no new read method)." | Shipped exactly — binary `SubscribedPeer` / `NetworkUnfollowed`; `You`/`UnsubscribedCache` deferred; slice-15 read reused; workspace stays 21. | Resolved at DELIVER. |
| 3 | ADR-053 fixed the test-only fault seam (`#[cfg(debug_assertions)]` env seam + release identity sibling + the new xtask guard, mirroring ADR-026). | `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` honored only under `debug_assertions`; release sibling the identity function (seam-free verified); `scan_viewer_fail_seam_guard` landed, sharing `classify_cfg_gated_token`. | Resolved at DELIVER (SF-8, DV-SF-7/8). |
| 4 | The DESIGN reviewer flagged "blocked." | The "blocked" was a **mislabeled pre-DELIVER implementation-TODO list, not design defects**; the design was sound and DISTILL closed APPROVED. | Clarified at DELIVER; not a design defect. |
| 5 | Mutation expected 100% on the in-diff. | `viewer-domain` 2/2 caught; the single cargo-mutants "missed" is a cfg-dead-branch artifact (not viable), pinned structurally (DV-SF-9). ≥80%-of-viable gate MET. | Recorded; the artifact explained. |
| 6 | The bare-DID strip was expected on the active-set (peer_did) side. | Shipped on the RESULT side too, reconciling against the slice-15 `bare_did` SSOT (DV-SF-5). | Resolved at DELIVER. |
| 7 | The seam read fn was named `index_query_active_set`. | Phase-3 refactor (d20a177) renamed it `read_local_active_set` (L1 clarity — the read is LOCAL, not the remote index, DV-SF-10). | Improved at DELIVER (refactor). |
| 8 | Review expected to pass clean. | Review APPROVED, 0 defects, 0 Testing Theater (fault-seam release-safety + xtask-guard robustness verified). | Confirmed at DELIVER. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-search-follow-state/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/`, `design/`, `distill/`, `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-08 archive** (the read-only `GET /search` view this slice enriches + the source
  of the `render_follow_guidance` / `openlore peer add` render-only affordance):
  `docs/evolution/viewer-network-search-evolution.md`
- **Parent slice-15 archive** (the source of the `list_active_peer_subscriptions` read this slice
  reuses + the `bare_did` SSOT this slice reconciles against):
  `docs/evolution/viewer-peer-subscriptions-evolution.md`
- **Slice-16 ADR**:
  `docs/adrs/ADR-053-viewer-side-search-relationship-resolution-against-local-active-set-and-render-only-following-indicator.md`
- **Release-gate pattern reused**: `docs/adrs/ADR-026` (the pubkey-seam release-gate the fault seam
  mirrors; the xtask guard shares `classify_cfg_gated_token`)
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-search-follow-state/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-search-follow-state/deliver/execution-log.json`,
  `docs/feature/viewer-search-follow-state/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_search_follow_state.rs` (SF-1..SF-10, the thick walking skeleton at SF-1),
  `tests/acceptance/viewer_search_follow_state_invariants.rs` (the 4 GOLD invariants)
- **Reused render-only-affordance pattern**: `crates/viewer-domain` (`render_follow_guidance`,
  slice-08) — `render_following_indicator` is its sibling; both share the `bare_did` SSOT
- **Reused active-set read**: `crates/adapter-duckdb` / `crates/ports`
  (`list_active_peer_subscriptions`, slice-15) — read ONCE per render into a `HashSet`
- **Extended crates**: `crates/viewer-domain` (the `SubscribedPeer` render arm +
  `SEARCH_FOLLOWING_INDICATOR` + `render_following_indicator`), `crates/adapter-http-viewer` (the
  `read_local_active_set` read + the `to_indexed_claim` classification + the test-only
  `#[cfg(debug_assertions)]` fault seam), `crates/xtask` (the new `scan_viewer_fail_seam_guard`
  guard sharing `classify_cfg_gated_token`)
- **Federation mutation surface (NOT replaced)**: the slice-03 CLI (`openlore peer add`) — the keyed
  follow path the read-only view points to
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-claim-list-flags-evolution.md`, `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`, `viewer-peer-subscriptions-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS 55181e6 → DESIGN 8115c6d → DISTILL 78b006a → roadmap → 01-01 7032395 →
01-02 0d04ec6 → 01-03 bda380c → 02-01 b0c3180 → 02-02 1d8ee15 → 02-03 b69c78e →
02-04 b4bff8c → 03-01 72a077f → refactor d20a177.
