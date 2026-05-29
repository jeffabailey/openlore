# Evolution: openlore-appview-search (slice-05 appview / network indexer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/openlore-appview-search/`
> (feature-delta.md + the five wave dirs) and ADR-023..ADR-027 under
> `docs/adrs/`; this file is the post-mortem summary.

## Summary

`openlore-appview-search` is the slice-05 AppView / network indexer of the
OpenLore umbrella — the FINAL umbrella slice and its **architecturally headline**
one (job **J-005**: discover well-evidenced signed claims, and the people behind
them, from across the WHOLE network in a single search session WITHOUT first
knowing whom to follow). It closes the last unmet third of the J-001 push:
slices 01/02/04 solved structure + signing, slice-03 solved MANUAL federation
(own + subscribed peers), but a great claim by a never-subscribed author stayed
**undiscoverable**. Slice-05 closes the DISCOVERABILITY gap at NETWORK scale.

It ships the **first network service** in the umbrella: `openlore-indexer`, a
second self-hostable single binary (the ATProto AppView pattern), signing-INCAPABLE
by construction. The indexer:

1. **Ingests** PUBLIC signed claims from across the network via PULL-based bounded
   ingestion (ADR-024; NOT Firehose).
2. **Verifies** each claim's signature against the author's REAL PLC `z6Mk...`
   DID-document key (ADR-026) AND recomputes its CID BEFORE indexing — reusing the
   PURE `claim-domain` verification core (no second path).
3. **Indexes** every verified record into a SEPARATE `index.duckdb` store with a
   non-`Option` `author_did` per row (ADR-025) — NO merged consensus schema.
4. **Serves** network-scale discovery via the `org.openlore.appview.searchClaims`
   XRPC method over HTTP (ADR-027), consumed by the new `openlore search` verb.

The CLI + signed claims REMAIN the source of truth. The indexer NEVER overwrites,
merges, signs, or publishes; discovery is a FRONT-DOOR that feeds the slice-03
federation flow (`openlore peer add`, reused verbatim), not a replacement. It does
all of this while keeping the local-first CLI structurally unaffected (the network
surface is additive and non-load-bearing for authoring; KPI-5).

Five user-visible capabilities prove the thesis:

1. **Search by philosophy** — `openlore search --object <philosophy>` (the headline,
   US-AV-002): grouped per-author, every result `[verified]`, never a faceless
   network-consensus row.
2. **Search by contributor / subject** — `--contributor <did>` / `--subject <project>`
   (US-AV-003): one developer's whole reasoning trail; honest "not a community
   consensus" framing.
3. **Trust + inspect** — `--show <cid>` (US-AV-004): "Signature: VERIFIED against
   <did>" + "CID recomputed, matches published record" — the SAME pure-core
   verification result computed at ingest (no second path).
4. **Discovery → federation funnel** — the `peer add` follow affordance for
   unfollowed authors (US-AV-005): a render-only hint reusing slice-03 verbatim;
   no auto-follow.
5. **Shareable discovery** — `--share` (US-AV-006) emits a stable
   `openlore://search?...` QUERY-encoding link (not a snapshot); `openlore search
   <link>` re-runs the query, re-composing CURRENT per-author-attributed verified
   results (anti-merging across the share boundary).

### Wave timeline

| Wave    | Date       | Owner                              |
|---------|------------|------------------------------------|
| DISCUSS | 2026-05-28 | Luna (nw-product-owner)            |
| DESIGN  | 2026-05-28 | Morgan (nw-solution-architect)     |
| DEVOPS  | 2026-05-28 | Apex (nw-platform-architect)       |
| DISTILL | 2026-05-28 | Quinn (nw-acceptance-designer)     |
| DELIVER | 2026-05-28..29 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **43/43 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **38/38 slice-05 acceptance scenarios** GREEN: 22 `appview_search` (AV-8..29)
  + 9 `appview_core` (AVC-1..8, with AVC-3 split into AVC-3a/3b) + 7
  `indexer_ingest` (AV-1..7; the `indexer_ingest` binary reports 9 incl. 2
  ingest-support self-tests).
- **6 NEW crates** (the indexer subsystem): 1 pure (`appview-domain`) + 4 effect
  (`adapter-atproto-ingest`, `adapter-index-store`, `adapter-xrpc-query-server`,
  `adapter-index-query`) + 1 binary driver (`openlore-indexer`). Workspace member
  count **13 → 19** (17 production + 1 test-support + 1 xtask); `cargo xtask
  check-arch` reports "19 workspace members".
- **The SECOND shipped binary** (`openlore-indexer`: serve / ingest / stats) — the
  first network service in the umbrella.
- **Zero regression** on slice-01/02/03/04 suites (full acceptance suite GREEN
  across all slices).
- **100% mutation kill rate** on the new pure `appview-domain` production
  functions (`ingest_decision` / `compose_results` / `near_match_suggestion`:
  37/37 caught; the 6 proptest-strategy generator survivors are test-infra, out
  of D-D40 scope — see Mutation report).
- **5 ADRs** (ADR-023..ADR-027) all Accepted.
- DES integrity: `des-verify-integrity` reports "All 43 steps have complete DES
  traces."
- Adversarial review: **APPROVED** with zero blockers (Testing Theater clean).

## Wave-by-wave changelog

### DISCUSS (2026-05-28)

Defined the J-005 network-discovery objective: a developer who does NOT already
know whom to follow discovers well-evidenced signed claims — and the people behind
them — from across the whole network in a single search session, trusts each
result because it is signature-verified and per-author-attributed (never a faceless
network-consensus row), and turns a discovery into a followed peer that grows their
trusted LOCAL graph — all without the AppView ever becoming an authority that
overwrites the CLI-first, local-first source of truth. Authored six outcome KPIs
(KPI-AV-1..6) with **KPI-AV-1** (>=60% of dogfood discovery sessions surface >=1
relevant claim by an unfollowed author within 30 days) as the north star and two
cardinal guardrails: **KPI-AV-2** (anti-merging at NETWORK scale — zero attribution
loss / zero faceless consensus rows) and **KPI-AV-3** (signature-verified-before-
index — zero unverified/unsigned/CID-mismatched claims ever indexed or returned).
Inherited KPI-4 (zero silent normalization), KPI-5 (local-first — REINFORCED-WITH-
A-TENSION: the AppView is a network service, yet offline compose/sign/local-query
still succeed and `search` degrades gracefully), and the slice-03/04 anti-merging
KPIs (KPI-FED-1/2, KPI-GRAPH-2) EXTENDED into network aggregates. Flagged the
load-bearing local-first <-> network-service tension and the slice-03 DV-4 pubkey-
seam dependency for DESIGN.

### DESIGN (2026-05-28)

Morgan locked WD-111..WD-124 and authored five ADRs. The headline decision:
slice-05 is an ADDITIVE EXTENSION (WD-111) — same hexagonal modular monolith +
functional paradigm, now TWO single-purpose binaries in one workspace; the new
crates map 1:1 to the new ports the first network service genuinely requires. The
pivotal resolutions: **ADR-023** (the AppView indexer is a SELF-HOSTABLE single
binary, signing-incapable by construction — a hosted/community service is a
documented future option, not slice-05's call; preserves data sovereignty and
mirrors the slice-02 `adapter-github` human-gate I-SCR-1); **ADR-024** (the
ADR-016 Firehose re-evaluation — PULL-based bounded ingestion CHOSEN; Firehose is
a documented additive future option, NOT slice-05's mechanism — pull suffices for
the per-session discovery-rate thesis, is hermetically testable for the cardinal
verify gate, and reuses the slice-03 verification discipline verbatim); **ADR-025**
(network index = a SEPARATE `index.duckdb` + anti-merging at network scale — reuse
DuckDB, NOT a search engine, to reuse the proven `no_cross_table_join_elides_author`
enforcement substrate; NO merged/consensus schema — the load-bearing absence);
**ADR-026** (production PLC `z6Mk...` multibase pubkey decode NOW — resolving the
slice-03 DV-4 test-only seam; the PURE decode helper lives in `claim-domain`, the
EFFECT DID-document resolution in the verify-only identity adapter; the test seam
RETAINED but release-forbidden); **ADR-027** (a NEW top-level `openlore search`
verb + CLI->indexer HTTP/XRPC transport + graceful degradation). Authored the
slice-05 invariants I-AV-1..9 (below) with three-layer enforcement for the two
cardinal ones. DEVOPS (parallel) added `appview-domain` to the nightly mutation
sweep, the KPI-AV-1..6 telemetry events, the index-coverage/freshness dashboard,
and the two consumer-driven contract tests (CLI->indexer XRPC; indexer->PDS/PLC).

### DISTILL (2026-05-28)

Quinn authored the executable acceptance corpus across three targets:
`appview_search` (AV-8..29 — network search by dimension, anti-merging at network
scale, the universal `[verified]` marker, `--show` trust inspection, the public-
data banner, the discovery->federation funnel, `--share` query-encoding round-trip,
counter-shown-not-applied, local-first graceful degradation), `appview_core`
(AVC-1..8 — the pure `appview-domain` properties: the verify-before-index gate, the
author-derived-from-signed-payload rule, `ingest_decision` / `compose_results`
determinism, two-author anti-merging compose, the universal verified marker, the
counter annotation, the near-match suggestion), and `indexer_ingest` (AV-1..7 — the
live-indexer walking skeleton: ingest into a real `index.duckdb`, anti-merging at
ingest, the cardinal verify-before-index gate against adversarial fixtures, the real
z6Mk decode gold path, the capability-boundary refusal, the wire-probe-use startup
refusal, public-data-only ingest). Materialized the adversarial ingest fixtures
(unsigned / tampered-signature / CID-mismatch) + a real-`z6Mk...` DID-document
fixture + multi-author same-(subject,object) network-search fixtures, and the
acceptance harness that stands up a live indexer for the search demos. AVC scenarios
carry `@property` and use proptest.

### DELIVER (2026-05-28..29)

Executed 43 roadmap steps across 5 phases via DES-monitored crafter dispatches,
each commit carrying a `Step-ID: NN-NN` trailer:

- **Phase 01 — Bootstrap (01-01..05):** bootstrap the pure `appview-domain` crate +
  the `claim-domain` decode helper; hoist the appview boundary ADTs to `ports` + add
  the four indexer ports; scaffold the four indexer effect adapters + extend the
  anti-merging SQL rule; scaffold the `openlore-indexer` binary + the CLI `search`
  verb + the capability/seam xtask rules; materialize the ingest fixtures + the
  acceptance harness + register the 3 test targets. Fail-for-right-reason RED gate
  (DD-AV-13) — all 38 ATs compile and classify RED.
- **Phase 02 — appview-domain pure core (02-01..09):** AVC-1..8 — the verify-before-
  index gate `ingest_decision`, the author-derived-from-signed-payload rule, the
  `ingest_decision` / `compose_results` determinism properties, the anti-merging
  `compose_results`, the two-author anti-merging compose, the universal verified
  marker, the counter annotation (shown-not-applied), the `near_match_suggestion`
  edit-distance ranker.
- **Phase 03 — indexer ingest walking skeleton (03-01..07):** AV-1..7 — wire the
  ingest skeleton against a live `index.duckdb`; prove anti-merging at ingest (two-
  author, no merge table); the cardinal verify-before-index gate (reject adversarial,
  never index/search them); the real z6Mk PLC decode gold path (seam unset); the
  indexer capability boundary (no signing, no local store); the wire-probe-use
  startup refusal + real substrate-lie probes; public-data-only ingest.
- **Phase 04 — search walking skeleton + trust surface (04-01..07):** AV-8..14 +
  AV-23 — wire the search walking skeleton (B1 serve+query+render); cardinal
  anti-merging at search (distinct attributed rows, no consensus row); the public-
  data banner; the universal `[verified]` marker; cardinal local-first (offline
  authoring + soft search degradation); the B1 localhost transport + the
  author_did-present / reachable-shape probes; the `--show` trust-inspection surface.
- **Phase 05 — discovery funnel + share (05-01..15):** AV-12, AV-15..22, AV-24..29 —
  empty-result near-match suggestion (exit 0); contributor- and subject-dimension
  search; subscribed-peer labels with no redundant affordance; the discovery->
  federation funnel reusing slice-03 `peer add`; read-only discovery (never auto-
  subscribes); affordance suppressed for subscribed peers; the discovery-followed-
  author zero-residue purge (via slice-03); `--show` absent-cid usage error;
  countered-claim shown with annotation (not applied); `--share` query-encoding link
  for all dimensions + the CLI re-run resolver + the "current results, not a stale
  snapshot" guarantee.

Phase 4-7 (refactor / review / mutation / integrity) outcomes are in the Quality
Gates + Mutation sections below.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-1 | DES `project_id` header added to execution-log right after `des-init-log` (same hook-defect workaround as slice-02/03/04 DV-1). | Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-2 | Mutation = per-feature 100% on the new PURE `appview-domain` production functions (Phase 6), matching slice-02/03/04 DV-2. | Per-feature gate at deliver-time + DEVOPS nightly sweep as backstop. The slice-04 cross-package lesson informed keeping the killing properties IN-CRATE (`appview_core` proptests). |
| DV-3 | **`hyper`, NOT `axum`, for the XRPC query server** (`adapter-xrpc-query-server`). | `axum` is on the `deny.toml` ban list (it pulls a transitive tree the supply-chain policy narrows out); a hand-rolled `hyper` handler serves the single `org.openlore.appview.searchClaims` route with no new banned dependency. Q-DELIVER-AV-2 resolved in `hyper`'s favor at deliver-time. |
| DV-4 | Found + fixed a REAL pre-existing **slice-03 bug**: `adapter-duckdb::hard_purge` failed a DuckDB FK constraint when deleting `peer_claims` + child evidence in ONE transaction (masked by slice-03's childless PS-6 fixture; exposed by the slice-05 AV-22 discovery-funnel pull). Fixed via a two-transaction purge (commit `bf6df62`); slice-03 ATs still 10/10. | The AV-22 discovery->follow->purge funnel pulls a peer WITH child evidence — the first time the purge path met a non-childless peer. A latent slice-03 defect would have shipped silently; the slice-05 funnel earned its keep by surfacing it. |
| DV-5 | Added the `references` field to the `SearchResultDto` wire shape (a `data-models.md` gap, step 05-11). | The OD-AV-7 counter annotation (`annotate_counter_relationship`) needs the references graph across the CLI<->indexer transport; the DTO had to carry it to annotate "countered-by <cid> (by <did>)" on the CLI side. |
| DV-6 | Release-gated the slice-03 `OPENLORE_PEER_PUBKEY_HEX` seam via `cfg(debug_assertions)` + broadened the xtask `no_pubkey_seam_in_release_build` rule (commit `d6c8d9a`, ADR-026). | I-AV-6: production verification MUST use the REAL PLC `z6Mk...` decode; the slice-03 DV-4 test-only seam is release-forbidden. This closed a slice-03 carry-over (the seam was previously only test-only by convention, not enforced in release builds). |

## Cardinal release gates + slice-05 invariants

The three cardinal release gates (load-bearing; unshippable on any violation):

1. **AV-3 verify-before-index** (KPI-AV-3 / I-AV-1): tampered-signature + CID-mismatch
   + unsigned adversarial fixtures are REJECTED at ingest; none enter the index;
   none appear in any search result. The ingest gate reuses `claim_domain::verify`
   (no second path); production uses the REAL PLC z6Mk decode.
2. **AV-9 anti-merging at network scale** (KPI-AV-2 / I-AV-2): every indexed /
   searched / shared result carries a non-`Option` author DID; identical-content-
   different-author = two rows; the index has NO merged/consensus row; the share
   boundary re-composes per-author. Three-layer enforced.
3. **AV-13 local-first** (KPI-5 / I-AV-3): with the indexer down AND network
   disabled, `claim add` / offline `claim publish` / `graph query` all succeed;
   `search` degrades to a clear local-only message without a fatal error.

The generative halves AVC-1 (the pure verify-before-index gate property) + AVC-2
(the pure anti-merging compose property) back the AV-3 / AV-9 release gates with
property-based proof of the pure-core decision functions.

The full slice-05 invariant set (I-AV-1..9; detail + enforcement columns in
`docs/feature/openlore-appview-search/design/component-boundaries.md`):

| # | Invariant | Cardinal? |
|---|---|---|
| I-AV-1 | Verified-before-index (REAL PLC key + CID recompute via the pure core; no second path; every result `[verified]`; `verified_against NOT NULL`). | YES (KPI-AV-3) |
| I-AV-2 | Anti-merging at network scale (non-`Option` author DID; NO merged schema/row; identical-content-different-author = separate rows). Three-layer: TYPE / STRUCTURAL / BEHAVIORAL. Extends I-FED-1 -> I-GRAPH-1/2. | YES (KPI-AV-2) |
| I-AV-3 | Local-first preserved (the CLI links no indexer code; `search` is the only network verb + degrades gracefully; the indexer is not probed at CLI startup; disjoint composition roots). | YES (KPI-5) |
| I-AV-4 | Public-data-only (the indexer ingests ONLY public signed claims; no private read; no surveillance affordance; the public-data banner). | guardrail (KPI-AV-5) |
| I-AV-5 | Indexer signing-incapable + holds no local store (ADR-023; mirrors I-SCR-1); cannot author/sign/mutate/publish; cannot touch `openlore.duckdb`. Three-layer. | structural |
| I-AV-6 | Production pubkey decode is real (ADR-026; the test seam is release-forbidden). | structural |
| I-AV-7 | Discovery feeds federation via `peer add` VERBATIM (WD-110; reuses I-FED-5); render-only hint; no auto-follow. | behavioral (KPI-AV-4) |
| I-AV-8 | Shareable link encodes the QUERY, not a snapshot (WD-110); resolving re-composes current per-author-attributed verified results. | behavioral (KPI-AV-6) |
| I-AV-9 | Counter shown, not applied (OD-AV-7); a countered/retracted public verified claim stays discoverable; the counter relationship is annotated, never silently filtered/down-weighted. | behavioral |

## Quality gates — final report

- **Acceptance / integration**: 38/38 slice-05 scenarios GREEN; slice-01/02/03/04
  suites zero regression. Full workspace acceptance suite GREEN across all slices.
- **`cargo xtask check-arch`**: OK (19 workspace members) — `appview-domain`
  pure-core allowlist + the anti-merging SQL rule extended to `adapter-index-store`
  (the index-store no-author-eliding-aggregate rule) + the new
  `indexer_holds_no_signing_or_local_store` + `no_pubkey_seam_in_release_build`
  rules active; I-3 covers BOTH binaries (disjoint composition roots).
- **`cargo xtask check-probes`**: OK — the four new adapters' non-stub `probe()`
  bodies picked up (`adapter-atproto-did` resolve-probe de-allowlisted in the
  refactor); `appview-domain` correctly requires no `probe()` (pure crate).
- **Refactor (Phase 4)**: cleared the deferred DELIVER debt — AV-4 env
  test-isolation, the `appview-domain` clippy `large_enum_variant` + doc-indent,
  the `AtProtoDidAdapter` resolve-probe de-allowlist, and the
  `OPENLORE_PEER_PUBKEY_HEX_<did>` seam release-gated via `cfg(debug_assertions)`
  (ADR-026) — commits `baebe89` + `d6c8d9a`.
- **Adversarial review (Phase 5)**: APPROVED, zero blockers (Testing Theater clean).
- **DES integrity (Phase 7)**: PASS — all 43 steps have complete DES traces.

## Mutation testing — final report

**Scope**: the new pure `appview-domain` production functions — `ingest_decision`
(the verify-before-index gate), `compose_results` (the anti-merging composition),
and `near_match_suggestion` (the edit-distance ranker) — plus the targeted
survivor-kill pass D-D40 (commit `4a1a357`).

| Mutant category | Tested | Caught | Kill rate |
|---|---:|---:|---|
| `ingest_decision` / `compose_results` / `near_match_suggestion` production logic (incl. the D-D40 targeted kills: confidence carry, levenshtein arithmetic, counter tiebreak) | 37 | 37 | **100%** |

**Generator-survivor caveat.** The 6 surviving mutants live in the proptest
STRATEGY generators (`proptest_strategies`) — test-infrastructure, not production
behavior. A mutation in a generator only narrows the input space the property
explores; it cannot make a wrong production answer pass. These are correctly OUT of
the D-D40 production-kill scope (the per-feature gate is on production functions).

**The slice-04 cross-package lesson reaffirmed.** Slice-04's Phase-6 hardening
learned that a pure crate should carry its OWN behavior properties IN-CRATE so the
per-feature mutation gate is locally verifiable (cargo-mutants scopes a mutant's
test run to the mutated crate's own package). Slice-05 applied this from the start:
the `appview_core` (AVC-1..8) properties pin `ingest_decision` / `compose_results` /
`near_match_suggestion` directly in/against the `appview-domain` crate, so the
per-feature mutation measurement reached the real killing suite without the
cross-package detour slice-04 hit. The targeted D-D40 pass then closed the last
production survivors (confidence carry, levenshtein arithmetic, counter tiebreak)
to 100%. DEVOPS nightly sweep is the ongoing backstop.

## Lessons learned / issues

- **A latent slice-03 bug surfaced by a slice-05 funnel (DV-4 above)**: the
  `adapter-duckdb::hard_purge` FK-constraint failure was real and pre-existing,
  masked by slice-03's childless PS-6 fixture and only exposed when the AV-22
  discovery->follow->purge funnel pulled a peer WITH child evidence. Institutional
  lesson: a new slice that exercises an old path against richer data is a free
  regression test for the old slice — slice-05's funnel earned its keep.
- **The wire DTO is part of the anti-merging surface (DV-5 above)**: the OD-AV-7
  counter annotation needed the `references` graph across the CLI<->indexer
  transport; the `data-models.md` `SearchResultDto` shape did not carry it. Lesson:
  when an invariant (counter shown-not-applied) spans a cross-process boundary, the
  wire shape is part of the invariant's enforcement surface and must be checked at
  design-of-the-DTO time, not discovered at render time.
- **Supply-chain policy can pick the framework (DV-3 above)**: `axum`'s `deny.toml`
  ban steered the query server to a hand-rolled `hyper` handler. For a single-route
  XRPC surface this was a net simplification, not a cost — the framework choice
  Q-DELIVER-AV-2 left open resolved cleanly to the lighter option the policy
  already favored.
- **In-crate properties make the mutation gate locally verifiable (slice-04 lesson
  applied)**: keeping the `appview_core` killing properties in/against the
  `appview-domain` crate avoided the slice-04 cross-package cargo-mutants scope
  problem. The remaining generator survivors are test-infra and correctly out of
  scope.
- **Known follow-up (NOT in the gate)**: a pre-existing `clippy::manual_is_multiple_of`
  nit in `adapter-atproto-did` `decode_hex` (toolchain drift, not introduced by
  slice-05, not part of the CI clippy gate at this toolchain) — documented for a
  future housekeeping sweep.

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | Q-DELIVER-AV-2 left the HTTP server framework open (`axum` tokio-native OR a hand-rolled `hyper` handler). | Hand-rolled `hyper` handler (`axum` is `deny.toml`-banned). | Decided at deliver-time; recorded as DV-3. |
| 2 | ADR-026 deferred the production PLC pubkey decode resolution + the seam release-gating to DELIVER (Q-DELIVER-AV-6/8). | Real `z6Mk...` PLC decode shipped in `claim-domain::decode_ed25519_multibase` (the gold test runs the real path, seam unset); the slice-03 seam release-gated via `cfg(debug_assertions)` + the broadened xtask rule (`d6c8d9a`). | Resolved; the slice-03 DV-4 carry-over closed. |
| 3 | `data-models.md` `SearchResultDto` did not carry a `references` field. | Added the `references` field to the wire shape (step 05-11) for the OD-AV-7 counter annotation. | Cosmetic-vs-gap deviation recorded as DV-5; functionality + I-AV-9 invariant intact. |
| 4 | DESIGN assumed the slice-03 `hard_purge` path was correct (reused verbatim for the discovery funnel). | Found + fixed a real pre-existing slice-03 FK-constraint bug (two-transaction purge, `bf6df62`); slice-03 ATs still 10/10. | Resolved within DELIVER; recorded as DV-4. |
| 5 | DEVOPS scheduled mutation nightly + per-feature at deliver-time. | DELIVER ran mutation per-feature at deliver-time (DV-2, 100% on production functions) with the targeted D-D40 survivor-kill pass; the 6 generator survivors are out of scope. | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/openlore-appview-search/` (feature-delta.md + discuss/ design/
  distill/ devops/ deliver/)
- **Slice-05 ADRs**: `docs/adrs/ADR-023-appview-indexer-self-hostable-single-binary.md`,
  `docs/adrs/ADR-024-pull-based-bounded-network-ingestion.md`,
  `docs/adrs/ADR-025-network-index-duckdb-schema-anti-merging.md`,
  `docs/adrs/ADR-026-production-plc-pubkey-decode.md`,
  `docs/adrs/ADR-027-search-verb-cli-indexer-transport-graceful-degradation.md`
- **DELIVER wave decisions**: `docs/feature/openlore-appview-search/deliver/wave-decisions.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/openlore-appview-search/deliver/execution-log.json`,
  `docs/feature/openlore-appview-search/deliver/roadmap.json`
- **Outcome KPIs (slice-05 rationale)**:
  `docs/feature/openlore-appview-search/discuss/outcome-kpis.md`
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`
- **CI / nightly mutation**: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
</content>
</invoke>
